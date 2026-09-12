use serde::{Deserialize, Serialize};

/// 列表默认条数。
pub const DEFAULT_LIMIT: u32 = 20;
/// 单页最大条数（防止恶意大分页）。
pub const MAX_LIMIT: u32 = 100;

/// 游标分页参数：`?cursor=&limit=`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorParams {
    /// 上一页返回的游标（首次请求为空）。
    pub cursor: Option<String>,
    /// 每页条数（空则默认 20）。
    pub limit: Option<u32>,
}

impl CursorParams {
    /// 归一化分页条数：默认 20，范围 1 ~ 100。
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

/// 游标分页响应：`{ items, nextCursor }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPage<T> {
    /// 当前页数据。
    pub items: Vec<T>,
    /// 下一页游标（无更多数据时为 null）。
    pub next_cursor: Option<String>,
}

impl<T> CursorPage<T> {
    /// 构造游标分页结果。
    pub fn new(items: Vec<T>, next_cursor: Option<String>) -> Self {
        Self { items, next_cursor }
    }

    /// 构造无下一页的单页结果。
    pub fn single(items: Vec<T>) -> Self {
        Self {
            items,
            next_cursor: None,
        }
    }
}

/// 页码分页参数：`?page=&pageSize=`（管理台列表）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParams {
    /// 页码（从 1 开始）。
    pub page: Option<u32>,
    /// 每页条数（空则默认 20）。
    pub page_size: Option<u32>,
}

impl PageParams {
    /// 归一化页码：最小为 1。
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    /// 归一化每页条数：默认 20，范围 1 ~ 100。
    pub fn page_size(&self) -> u32 {
        self.page_size.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }

    /// SQL OFFSET（i64，避免大页码溢出）。
    pub fn offset(&self) -> i64 {
        i64::from(self.page() - 1) * i64::from(self.page_size())
    }
}

/// 页码分页响应：`{ items, total, page, pageSize }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    /// 当前页数据。
    pub items: Vec<T>,
    /// 总条数。
    pub total: i64,
    /// 当前页码。
    pub page: u32,
    /// 每页条数。
    pub page_size: u32,
}

impl<T> Page<T> {
    /// 构造页码分页结果。
    pub fn new(items: Vec<T>, total: i64, params: &PageParams) -> Self {
        Self {
            items,
            total,
            page: params.page(),
            page_size: params.page_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_limit_has_default_and_bounds() {
        let params = CursorParams::default();
        assert_eq!(params.limit(), DEFAULT_LIMIT);

        let params = CursorParams {
            cursor: None,
            limit: Some(500),
        };
        assert_eq!(params.limit(), MAX_LIMIT);

        let params = CursorParams {
            cursor: None,
            limit: Some(0),
        };
        assert_eq!(params.limit(), 1);

        let params = CursorParams {
            cursor: None,
            limit: Some(50),
        };
        assert_eq!(params.limit(), 50);
    }

    #[test]
    fn cursor_page_serializes_camel_case() {
        let page = CursorPage::new(vec![1, 2, 3], Some("next".to_string()));
        let value = serde_json::to_value(&page).expect("serialize");
        assert_eq!(value["items"], serde_json::json!([1, 2, 3]));
        assert_eq!(value["nextCursor"], "next");
    }

    #[test]
    fn page_params_offsets() {
        let params = PageParams {
            page: Some(1),
            page_size: Some(20),
        };
        assert_eq!(params.offset(), 0);

        let params = PageParams {
            page: Some(3),
            page_size: Some(15),
        };
        assert_eq!(params.offset(), 30);

        let params = PageParams {
            page: Some(0),
            page_size: Some(0),
        };
        assert_eq!(params.page(), 1);
        assert_eq!(params.page_size(), 1);
        assert_eq!(params.offset(), 0);
    }

    #[test]
    fn page_response_serializes_camel_case() {
        let params = PageParams {
            page: Some(2),
            page_size: Some(10),
        };
        let page = Page::new(vec!["a"], 11, &params);
        let value = serde_json::to_value(&page).expect("serialize");
        assert_eq!(value["total"], 11);
        assert_eq!(value["page"], 2);
        assert_eq!(value["pageSize"], 10);
    }
}
