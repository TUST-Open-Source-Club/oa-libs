//! Outbox 投递器：业务事务内写 outbox 表，后台任务可靠投递到 Redis Streams。
//!
//! 各服务在执行迁移时执行 [`DDL`] 创建表（表位于各自 schema 的 search_path 下），
//! 然后由后台任务周期性调用 [`drain_once`]。

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::Value;
use uuid::Uuid;

use club_common::AppError;

use crate::client::Bus;

/// 建表 SQL（服务迁移中直接 `execute_unprepared(DDL)` 执行）。
pub const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS outbox (
    id uuid PRIMARY KEY,
    event_type varchar(64) NOT NULL,
    payload jsonb NOT NULL,
    status varchar(16) NOT NULL DEFAULT 'pending',
    attempts integer NOT NULL DEFAULT 0,
    available_at timestamptz NOT NULL,
    last_error text,
    created_at timestamptz NOT NULL,
    sent_at timestamptz
);
CREATE INDEX IF NOT EXISTS ix_outbox_pending ON outbox (status, available_at);
"#;

/// outbox 表实体。
pub mod entity {
    use sea_orm::entity::prelude::*;

    /// 待投递事件。
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "outbox")]
    pub struct Model {
        /// 事件记录 ID。
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        /// 事件类型（决定流名）。
        pub event_type: String,
        /// 事件信封 JSON。
        #[sea_orm(column_type = "JsonBinary")]
        pub payload: Json,
        /// 状态：pending / sent。
        pub status: String,
        /// 已尝试次数。
        pub attempts: i32,
        /// 下次可投递时间（指数退避）。
        pub available_at: DateTimeWithTimeZone,
        /// 最近一次失败原因。
        #[sea_orm(nullable, column_type = "Text")]
        pub last_error: Option<String>,
        /// 创建时间。
        pub created_at: DateTimeWithTimeZone,
        /// 投递成功时间。
        #[sea_orm(nullable)]
        pub sent_at: Option<DateTimeWithTimeZone>,
    }

    /// 关系定义。
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub use entity::{ActiveModel, Column, Entity, Model};

/// 计算退避秒数：5s * 2^(attempts-1)，上限 600s。
pub fn backoff_seconds(attempts: i32) -> i64 {
    let exponent = (attempts - 1).clamp(0, 7) as u32;
    (5 * 2_i64.pow(exponent)).min(600)
}

/// 将业务事件写入 outbox（与业务写操作同一事务时由调用方传入事务连接）。
pub async fn enqueue(
    db: &DatabaseConnection,
    event_type: &str,
    payload: &Value,
    now: DateTime<Utc>,
) -> Result<Uuid, AppError> {
    let id = Uuid::now_v7();
    ActiveModel {
        id: Set(id),
        event_type: Set(event_type.to_string()),
        payload: Set(payload.clone()),
        status: Set("pending".to_string()),
        attempts: Set(0),
        available_at: Set(now.fixed_offset()),
        last_error: Set(None),
        created_at: Set(now.fixed_offset()),
        sent_at: Set(None),
    }
    .insert(db)
    .await
    .map_err(|err: DbErr| AppError::internal(err))?;
    Ok(id)
}

/// 取出到期未发送的事件（按创建顺序）。
pub async fn fetch_due(
    db: &DatabaseConnection,
    limit: u64,
    now: DateTime<Utc>,
) -> Result<Vec<Model>, AppError> {
    Entity::find()
        .filter(Column::Status.eq("pending"))
        .filter(Column::AvailableAt.lte(now.fixed_offset()))
        .order_by_asc(Column::Id)
        .limit(limit)
        .all(db)
        .await
        .map_err(AppError::internal)
}

/// 标记已投递。
pub async fn mark_sent(
    db: &DatabaseConnection,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let mut active = ActiveModel {
        id: Set(id),
        ..Default::default()
    };
    active.status = Set("sent".to_string());
    active.sent_at = Set(Some(now.fixed_offset()));
    active.last_error = Set(None);
    active
        .update(db)
        .await
        .map(|_| ())
        .map_err(AppError::internal)
}

/// 标记失败并按指数退避安排重试。
pub async fn mark_retry(
    db: &DatabaseConnection,
    id: Uuid,
    attempts: i32,
    next_at: DateTime<Utc>,
    error: &str,
) -> Result<(), AppError> {
    let mut active = ActiveModel {
        id: Set(id),
        ..Default::default()
    };
    active.attempts = Set(attempts);
    active.available_at = Set(next_at.fixed_offset());
    active.last_error = Set(Some(error.to_string()));
    active
        .update(db)
        .await
        .map(|_| ())
        .map_err(AppError::internal)
}

/// 执行一轮投递：取一批到期事件发布到总线；成功标记 sent，失败退避重试。
///
/// 返回成功投递的数量。
pub async fn drain_once(
    db: &DatabaseConnection,
    bus: &Bus,
    limit: u64,
    now: DateTime<Utc>,
) -> Result<u32, AppError> {
    let rows = fetch_due(db, limit, now).await?;
    let mut sent = 0;
    for row in rows {
        match bus.publish(&row.event_type, &row.payload).await {
            Ok(_) => {
                mark_sent(db, row.id, now).await?;
                sent += 1;
            }
            Err(err) => {
                let attempts = row.attempts + 1;
                let next_at = now + Duration::seconds(backoff_seconds(attempts));
                tracing::warn!(
                    id = %row.id,
                    attempts,
                    error = %err,
                    "outbox 投递失败，稍后重试"
                );
                mark_retry(db, row.id, attempts, next_at, &err.to_string()).await?;
            }
        }
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        assert_eq!(backoff_seconds(1), 5);
        assert_eq!(backoff_seconds(2), 10);
        assert_eq!(backoff_seconds(3), 20);
        assert_eq!(backoff_seconds(0), 5, "非法次数按首次处理");
        assert_eq!(backoff_seconds(8), 600);
        assert_eq!(backoff_seconds(99), 600);
    }
}
