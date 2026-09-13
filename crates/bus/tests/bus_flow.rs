//! bus 集成测试：需要本地 Redis（默认 127.0.0.1:56379，可用 REDIS_TEST_URL 覆盖）。

use club_bus::{stream_name, Bus};
use serde_json::json;

fn redis_url() -> String {
    std::env::var("REDIS_TEST_URL").unwrap_or_else(|_| "redis://127.0.0.1:56379".to_string())
}

fn unique_group(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

#[tokio::test]
async fn publish_read_and_ack_roundtrip() {
    let bus = Bus::connect(&redis_url()).await.expect("连接 Redis");
    bus.ping().await.expect("ping");

    let stream = stream_name("task.assigned");
    let group = unique_group("g");
    bus.ensure_group(&stream, &group).await.expect("建组");

    let payload = json!({ "id": "e1", "type": "task.assigned", "title": "t" });
    let id = bus.publish("task.assigned", &payload).await.expect("发布");
    assert!(!id.is_empty());

    let messages = bus
        .read_group(&[&stream], &group, "c1", 10, 1000)
        .await
        .expect("读取");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].event_type, "task.assigned");
    assert_eq!(messages[0].payload, payload);

    bus.ack(&stream, &group, &[messages[0].id.clone()])
        .await
        .expect("确认");

    // 已 ACK 的消息不会再次投递
    let again = bus
        .read_group(&[&stream], &group, "c1", 10, 200)
        .await
        .expect("再次读取");
    assert!(again.is_empty());
}

/// 回归：redis 1.7 ConnectionManager 默认 500ms 响应超时，
/// 空流上的阻塞读必须超过该时长也不能报错。
#[tokio::test]
async fn blocking_read_longer_than_default_response_timeout() {
    let bus = Bus::connect(&redis_url()).await.expect("连接 Redis");
    let stream = stream_name("doc.node.created");
    let group = unique_group("g");
    bus.ensure_group(&stream, &group).await.expect("建组");

    let messages = bus
        .read_group(&[&stream], &group, "c1", 10, 1200)
        .await
        .expect("阻塞读不应超时");
    assert!(messages.is_empty());
}

#[tokio::test]
async fn ensure_group_is_idempotent() {
    let bus = Bus::connect(&redis_url()).await.expect("连接 Redis");
    let stream = stream_name("im.message.created");
    let group = unique_group("g");
    bus.ensure_group(&stream, &group).await.expect("首次建组");
    // 第二次应吞掉 BUSYGROUP 错误
    bus.ensure_group(&stream, &group)
        .await
        .expect("重复建组应幂等");
}
