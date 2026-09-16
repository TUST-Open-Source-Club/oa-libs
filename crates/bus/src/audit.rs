//! 变更日志：记录修改前后快照，供「审计 + 撤销」使用。
//!
//! 各服务在自身 schema 迁移中执行 [`DDL`]，写操作时调用 [`record`]，
//! 撤销接口读取 [`find`]/[`list_for`] 的 `before` 回写即可。

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use uuid::Uuid;

/// 变更日志建表语句（在每个服务的 schema 内执行）。
pub const DDL: &str = "CREATE TABLE IF NOT EXISTS change_log (\
    id uuid PRIMARY KEY, entity text NOT NULL, entity_id text NOT NULL, action text NOT NULL, \
    before jsonb, after jsonb, actor_id uuid, created_at timestamptz NOT NULL)";

/// 一条变更记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEntry {
    /// ID。
    pub id: Uuid,
    /// 实体类型（如 task/project/space/event）。
    pub entity: String,
    /// 实体 ID（字符串，兼容业务主键）。
    pub entity_id: String,
    /// 动作（update/delete/move/status/...）。
    pub action: String,
    /// 修改前快照（撤销时回写）。
    pub before: Option<Json>,
    /// 修改后快照。
    pub after: Option<Json>,
    /// 操作者。
    pub actor_id: Option<Uuid>,
    /// 时间。
    pub created_at: DateTime<Utc>,
}

/// 写入一条变更记录，返回记录 ID。
#[allow(clippy::too_many_arguments)]
pub async fn record(
    db: &DatabaseConnection,
    entity: &str,
    entity_id: &str,
    action: &str,
    before: Option<Json>,
    after: Option<Json>,
    actor_id: Option<Uuid>,
    now: DateTime<Utc>,
) -> Result<Uuid, DbErr> {
    let id = Uuid::now_v7();
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO change_log (id, entity, entity_id, action, before, after, actor_id, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        vec![
            Value::Uuid(Some(id)),
            Value::String(Some(entity.to_string())),
            Value::String(Some(entity_id.to_string())),
            Value::String(Some(action.to_string())),
            Value::Json(before.map(Box::new)),
            Value::Json(after.map(Box::new)),
            Value::Uuid(actor_id),
            Value::ChronoDateTimeUtc(Some(now)),
        ],
    ))
    .await?;
    Ok(id)
}

/// 按 ID 查询变更记录。
pub async fn find(db: &DatabaseConnection, id: Uuid) -> Result<Option<ChangeEntry>, DbErr> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT id, entity, entity_id, action, before, after, actor_id, created_at \
             FROM change_log WHERE id = $1",
            vec![Value::Uuid(Some(id))],
        ))
        .await?;
    rows.first().map(entry_from_row).transpose()
}

/// 查询某实体的变更记录（时间倒序）。
pub async fn list_for(
    db: &DatabaseConnection,
    entity: &str,
    entity_id: &str,
    limit: u64,
) -> Result<Vec<ChangeEntry>, DbErr> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT id, entity, entity_id, action, before, after, actor_id, created_at \
             FROM change_log WHERE entity = $1 AND entity_id = $2 \
             ORDER BY created_at DESC LIMIT $3",
            vec![
                Value::String(Some(entity.to_string())),
                Value::String(Some(entity_id.to_string())),
                Value::BigInt(Some(limit.clamp(1, 200) as i64)),
            ],
        ))
        .await?;
    rows.iter().map(entry_from_row).collect()
}

/// 行 → 记录。
fn entry_from_row(row: &sea_orm::QueryResult) -> Result<ChangeEntry, DbErr> {
    Ok(ChangeEntry {
        id: row.try_get("", "id")?,
        entity: row.try_get("", "entity")?,
        entity_id: row.try_get("", "entity_id")?,
        action: row.try_get("", "action")?,
        before: row.try_get::<Option<Json>>("", "before")?,
        after: row.try_get::<Option<Json>>("", "after")?,
        actor_id: row.try_get("", "actor_id")?,
        created_at: row.try_get("", "created_at")?,
    })
}
