use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 服务内部常用的 Result 别名，默认错误为 [`AppError`]。
pub type Result<T, E = AppError> = std::result::Result<T, E>;

/// 统一错误类型，序列化为 RFC 7807 Problem Details。
#[derive(Debug, Error)]
pub enum AppError {
    /// 400：请求参数错误。
    #[error("{detail}")]
    BadRequest {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
    },
    /// 401：未认证或令牌无效。
    #[error("{detail}")]
    Unauthorized {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
    },
    /// 403：已认证但权限不足。
    #[error("{detail}")]
    Forbidden {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
    },
    /// 404：资源不存在。
    #[error("{detail}")]
    NotFound {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
    },
    /// 409：唯一约束或状态冲突。
    #[error("{detail}")]
    Conflict {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
    },
    /// 429：触发限流。
    #[error("{detail}")]
    TooManyRequests {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
    },
    /// 422：表单校验失败。
    #[error("{detail}")]
    Unprocessable {
        /// 业务错误码。
        code: String,
        /// 面向用户的错误描述。
        detail: String,
        /// 字段级错误列表。
        errors: Vec<FieldError>,
    },
    /// 500：内部错误（详情不对外暴露，写入日志）。
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// 构造 400 请求参数错误。
    pub fn bad_request(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::BadRequest {
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// 构造 401 未认证错误。
    pub fn unauthorized(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Unauthorized {
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// 构造 403 无权限错误。
    pub fn forbidden(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Forbidden {
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// 构造 404 资源不存在错误。
    pub fn not_found(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::NotFound {
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// 构造 409 冲突错误（唯一约束、状态冲突等）。
    pub fn conflict(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Conflict {
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// 构造 429 限流错误。
    pub fn too_many_requests(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::TooManyRequests {
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// 构造 422 表单校验错误（携带字段级错误列表）。
    pub fn unprocessable(
        code: impl Into<String>,
        detail: impl Into<String>,
        errors: Vec<FieldError>,
    ) -> Self {
        Self::Unprocessable {
            code: code.into(),
            detail: detail.into(),
            errors,
        }
    }

    /// 包装内部错误（对外只返回"服务器内部错误"，详情写入日志）。
    pub fn internal(err: impl std::fmt::Display) -> Self {
        Self::Internal(err.to_string())
    }

    /// 映射到 HTTP 状态码。
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest { .. } => StatusCode::BAD_REQUEST,
            AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            AppError::Forbidden { .. } => StatusCode::FORBIDDEN,
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::Unprocessable { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// 稳定的业务错误码（前端据此做文案映射）。
    pub fn code(&self) -> &str {
        match self {
            AppError::BadRequest { code, .. }
            | AppError::Unauthorized { code, .. }
            | AppError::Forbidden { code, .. }
            | AppError::NotFound { code, .. }
            | AppError::Conflict { code, .. }
            | AppError::TooManyRequests { code, .. }
            | AppError::Unprocessable { code, .. } => code,
            AppError::Internal(_) => "INTERNAL",
        }
    }

    /// 转换为 RFC 7807 Problem Details。
    pub fn problem(&self) -> ProblemDetails {
        let (title, detail, errors) = match self {
            AppError::BadRequest { detail, .. } => ("Bad Request", detail.clone(), None),
            AppError::Unauthorized { detail, .. } => ("Unauthorized", detail.clone(), None),
            AppError::Forbidden { detail, .. } => ("Forbidden", detail.clone(), None),
            AppError::NotFound { detail, .. } => ("Not Found", detail.clone(), None),
            AppError::Conflict { detail, .. } => ("Conflict", detail.clone(), None),
            AppError::TooManyRequests { detail, .. } => ("Too Many Requests", detail.clone(), None),
            AppError::Unprocessable { detail, errors, .. } => {
                ("Unprocessable Entity", detail.clone(), Some(errors.clone()))
            }
            AppError::Internal(_) => ("Internal Server Error", "服务器内部错误".to_string(), None),
        };

        ProblemDetails {
            type_uri: format!("https://club-oa.local/errors/{}", self.code()),
            title: title.to_string(),
            status: self.status().as_u16(),
            detail,
            instance: None,
            code: self.code().to_string(),
            errors,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        if let AppError::Internal(err) = &self {
            tracing::error!(error = %err, "internal server error");
        }
        (status, Json(self.problem())).into_response()
    }
}

/// 字段级校验错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    /// 字段名（表单 key）。
    pub field: String,
    /// 错误文案或 i18n key。
    pub message: String,
}

impl FieldError {
    /// 构造字段错误。
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// RFC 7807 Problem Details 响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemDetails {
    /// 错误类型 URI（`https://club-oa.local/errors/{code}`）。
    #[serde(rename = "type")]
    pub type_uri: String,
    /// 状态码短语。
    pub title: String,
    /// HTTP 状态码。
    pub status: u16,
    /// 错误描述。
    pub detail: String,
    /// 请求路径（可选）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// 业务错误码。
    pub code: String,
    /// 字段级错误（仅 422）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FieldError>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_problem(err: AppError, status: StatusCode, code: &str) {
        assert_eq!(err.status(), status);
        assert_eq!(err.code(), code);
        let problem = err.problem();
        assert_eq!(problem.status, status.as_u16());
        assert_eq!(problem.code, code);
        assert_eq!(
            problem.type_uri,
            format!("https://club-oa.local/errors/{code}")
        );
    }

    #[test]
    fn maps_each_variant_to_status_and_code() {
        assert_problem(
            AppError::bad_request("A_BAD", "x"),
            StatusCode::BAD_REQUEST,
            "A_BAD",
        );
        assert_problem(
            AppError::unauthorized("A_UNAUTH", "x"),
            StatusCode::UNAUTHORIZED,
            "A_UNAUTH",
        );
        assert_problem(
            AppError::forbidden("A_FORBIDDEN", "x"),
            StatusCode::FORBIDDEN,
            "A_FORBIDDEN",
        );
        assert_problem(
            AppError::not_found("A_NOT_FOUND", "x"),
            StatusCode::NOT_FOUND,
            "A_NOT_FOUND",
        );
        assert_problem(
            AppError::conflict("A_CONFLICT", "x"),
            StatusCode::CONFLICT,
            "A_CONFLICT",
        );
        assert_problem(
            AppError::too_many_requests("A_LIMIT", "x"),
            StatusCode::TOO_MANY_REQUESTS,
            "A_LIMIT",
        );
        assert_problem(
            AppError::unprocessable("A_VALIDATION", "x", vec![]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "A_VALIDATION",
        );
        assert_problem(
            AppError::Internal("db down".into()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
        );
    }

    #[test]
    fn internal_error_hides_details() {
        let problem = AppError::Internal("connection refused to 10.0.0.1".into()).problem();
        assert_eq!(problem.detail, "服务器内部错误");
        assert!(!problem.detail.contains("10.0.0.1"));
    }

    #[test]
    fn unprocessable_carries_field_errors() {
        let err = AppError::unprocessable(
            "AUTH_VALIDATION",
            "validation failed",
            vec![
                FieldError::new("email", "格式错误"),
                FieldError::new("password", "过短"),
            ],
        );
        let problem = err.problem();
        let errors = problem.errors.expect("errors present");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].field, "email");
    }

    #[test]
    fn serializes_to_problem_details_json() {
        let err = AppError::not_found("AUTH_USER_NOT_FOUND", "用户不存在");
        let value = serde_json::to_value(err.problem()).expect("serialize");
        assert_eq!(
            value["type"],
            "https://club-oa.local/errors/AUTH_USER_NOT_FOUND"
        );
        assert_eq!(value["status"], 404);
        assert_eq!(value["code"], "AUTH_USER_NOT_FOUND");
        assert_eq!(value["detail"], "用户不存在");
        assert!(value.get("errors").is_none());
    }
}
