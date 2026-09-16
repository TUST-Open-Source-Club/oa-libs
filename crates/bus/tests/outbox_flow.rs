//! outbox 集成测试：真实 PostgreSQL（建表 + 落库）+ Redis（投递消费）。

use chrono::Utc;
use club_bus::{outbox, Bus};
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::json;
use uuid::Uuid;

fn redis_url() -> String {
    std::env::var("REDIS_TEST_URL").unwrap_or_else(|_| "redis://127.0.0.1:56379".to_string())
}

fn database_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/club_oa".to_string())
}

/// 连接测试数据库（独立 schema + outbox 表）。
async fn test_db() -> sea_orm::DatabaseConnection {
    use sea_orm::{ConnectOptions, ConnectionTrait, Database};
    let schema = format!("test_{}", Uuid::now_v7().simple());
    let bootstrap = Database::connect(database_url()).await.expect("connect");
    bootstrap
        .execute_unprepared(&format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\""))
        .await
        .expect("create schema");
    bootstrap.close().await.expect("close");
    let mut options = ConnectOptions::new(database_url());
    options.set_schema_search_path(schema);
    let db = Database::connect(options).await.expect("connect schema");
    db.execute_unprepared(outbox::DDL).await.expect("ddl");
    db
}

#[tokio::test]
async fn drain_publishes_and_marks_sent() {
    let db = test_db().await;
    let bus = Bus::connect(&redis_url()).await.expect("redis");
    let payload = json!({ "id": "e1", "type": "task.assigned", "title": "t" });
    let id = outbox::enqueue(&db, "task.assigned", &payload, Utc::now())
        .await
        .expect("enqueue");

    // 消费组先建好再 drain，确保能读到
    let stream = club_bus::stream_name("task.assigned");
    let group = format!("g-{}", Uuid::now_v7().simple());
    bus.ensure_group(&stream, &group).await.expect("group");

    let sent = outbox::drain_once(&db, &bus, 10, Utc::now())
        .await
        .expect("drain");
    assert_eq!(sent, 1);

    let messages = bus
        .read_group(&[&stream], &group, "c1", 10, 1000)
        .await
        .expect("read");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].payload, payload);

    // 已发送的事件不重复投递
    let sent_again = outbox::drain_once(&db, &bus, 10, Utc::now())
        .await
        .expect("drain2");
    assert_eq!(sent_again, 0);

    let row = outbox::Entity::find_by_id(id)
        .one(&db)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(row.status, "sent");
    assert!(row.sent_at.is_some());
}

#[tokio::test]
async fn retry_marks_row_and_schedules_backoff() {
    // 说明：drain 的失败分支依赖真实 Redis 连接错误，不便稳定构造；
    // 这里直接验证 mark_retry 的落库结果与退避时间（drain 内部调用同一函数）。
    let db = test_db().await;
    let id = outbox::enqueue(&db, "task.assigned", &json!({ "id": "e2" }), Utc::now())
        .await
        .expect("enqueue");

    let next_at = Utc::now() + chrono::Duration::seconds(outbox::backoff_seconds(1));
    outbox::mark_retry(&db, id, 1, next_at, "connection refused")
        .await
        .expect("mark retry");

    let row = outbox::Entity::find_by_id(id)
        .one(&db)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(row.status, "pending");
    assert_eq!(row.attempts, 1);
    assert_eq!(row.last_error.as_deref(), Some("connection refused"));
    assert!(
        row.available_at > Utc::now().fixed_offset(),
        "应安排到未来重试"
    );
}

#[tokio::test]
async fn audit_record_and_list() {
    let db = test_db().await;
    db.execute_unprepared(club_bus::audit::DDL).await.expect("建表");
    let now = chrono::Utc::now();
    let entity_id = uuid::Uuid::now_v7().to_string();
    let id = club_bus::audit::record(
        &db,
        "task",
        &entity_id,
        "update",
        Some(serde_json::json!({ "title": "旧标题" })),
        Some(serde_json::json!({ "title": "新标题" })),
        None,
        now,
    )
    .await
    .expect("写入");
    let found = club_bus::audit::find(&db, id).await.expect("查询").expect("存在");
    assert_eq!(found.action, "update");
    assert_eq!(found.before.unwrap()["title"], "旧标题");
    let list = club_bus::audit::list_for(&db, "task", &entity_id, 10).await.expect("列表");
    assert_eq!(list.len(), 1);
}
