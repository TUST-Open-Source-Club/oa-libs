//! 总线消息模型与 Redis 原始回复解析。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 从事件类型推导流名：`task.assigned` → `events.task`。
pub fn stream_name(event_type: &str) -> String {
    let module = event_type
        .split('.')
        .next()
        .filter(|module| !module.is_empty())
        .unwrap_or("system");
    format!("events.{module}")
}

/// 消费到的一条事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusMessage {
    /// Redis 消息 ID（ACK 时使用）。
    pub id: String,
    /// 来源流名。
    pub stream: String,
    /// 事件类型（信封中的 type）。
    pub event_type: String,
    /// 完整事件信封 JSON。
    pub payload: Value,
}

/// 解析 `XREADGROUP` 原始回复为消息列表。
///
/// Redis 回复结构：
/// `[[stream_name, [[id, [field, value, ...]], ...]], ...]`
/// 空回复（BLOCK 超时）为 `Nil`。
pub fn parse_stream_reply(value: &redis::Value) -> Vec<BusMessage> {
    let mut messages = Vec::new();
    let redis::Value::Array(streams) = value else {
        return messages;
    };
    for stream in streams {
        let redis::Value::Array(stream_parts) = stream else {
            continue;
        };
        let (Some(redis::Value::BulkString(stream_name)), Some(redis::Value::Array(entries))) =
            (stream_parts.first(), stream_parts.get(1))
        else {
            continue;
        };
        let stream_name = String::from_utf8_lossy(stream_name).to_string();
        for entry in entries {
            let redis::Value::Array(entry_parts) = entry else {
                continue;
            };
            let (Some(redis::Value::BulkString(id)), Some(redis::Value::Array(fields))) =
                (entry_parts.first(), entry_parts.get(1))
            else {
                continue;
            };
            let mut event_type = None;
            let mut payload = None;
            let mut index = 0;
            while index + 1 < fields.len() {
                if let (redis::Value::BulkString(field), redis::Value::BulkString(raw)) =
                    (&fields[index], &fields[index + 1])
                {
                    match field.as_slice() {
                        b"eventType" => {
                            event_type = Some(String::from_utf8_lossy(raw).to_string());
                        }
                        b"payload" => {
                            payload = serde_json::from_slice::<Value>(raw).ok();
                        }
                        _ => {}
                    }
                }
                index += 2;
            }
            if let Some(payload) = payload {
                let event_type = event_type
                    .or_else(|| {
                        payload
                            .get("type")
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
                messages.push(BusMessage {
                    id: String::from_utf8_lossy(id).to_string(),
                    stream: stream_name.clone(),
                    event_type,
                    payload,
                });
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stream_name_uses_module_prefix() {
        assert_eq!(stream_name("task.assigned"), "events.task");
        assert_eq!(stream_name("single"), "events.single");
        assert_eq!(stream_name(""), "events.system");
    }

    #[test]
    fn parse_message_from_redis_value() {
        let payload = json!({ "id": "e1", "type": "task.assigned", "title": "t" });
        let value = redis::Value::Array(vec![redis::Value::Array(vec![
            redis::Value::BulkString(b"events.task".to_vec()),
            redis::Value::Array(vec![redis::Value::Array(vec![
                redis::Value::BulkString(b"1-0".to_vec()),
                redis::Value::Array(vec![
                    redis::Value::BulkString(b"eventType".to_vec()),
                    redis::Value::BulkString(b"task.assigned".to_vec()),
                    redis::Value::BulkString(b"payload".to_vec()),
                    redis::Value::BulkString(serde_json::to_vec(&payload).expect("serialize")),
                ]),
            ])]),
        ])]);
        let messages = parse_stream_reply(&value);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "1-0");
        assert_eq!(messages[0].stream, "events.task");
        assert_eq!(messages[0].event_type, "task.assigned");
        assert_eq!(messages[0].payload, payload);
    }

    #[test]
    fn parse_tolerates_empty_and_broken_replies() {
        assert!(parse_stream_reply(&redis::Value::Nil).is_empty());
        assert!(parse_stream_reply(&redis::Value::Array(vec![])).is_empty());
        assert!(parse_stream_reply(&redis::Value::Array(vec![redis::Value::Int(1)])).is_empty());
        // payload 缺失 → 忽略该条
        let value = redis::Value::Array(vec![redis::Value::Array(vec![
            redis::Value::BulkString(b"events.x".to_vec()),
            redis::Value::Array(vec![redis::Value::Array(vec![
                redis::Value::BulkString(b"1-0".to_vec()),
                redis::Value::Array(vec![redis::Value::BulkString(b"eventType".to_vec())]),
            ])]),
        ])]);
        assert!(parse_stream_reply(&value).is_empty());
    }

    #[test]
    fn event_type_falls_back_to_payload_type() {
        let payload = json!({ "type": "im.message.created" });
        let value = redis::Value::Array(vec![redis::Value::Array(vec![
            redis::Value::BulkString(b"events.im".to_vec()),
            redis::Value::Array(vec![redis::Value::Array(vec![
                redis::Value::BulkString(b"2-0".to_vec()),
                redis::Value::Array(vec![
                    redis::Value::BulkString(b"payload".to_vec()),
                    redis::Value::BulkString(serde_json::to_vec(&payload).expect("serialize")),
                ]),
            ])]),
        ])]);
        let messages = parse_stream_reply(&value);
        assert_eq!(messages[0].event_type, "im.message.created");
    }
}
