//! Redis Streams 客户端封装。

use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::{cmd, AsyncCommands};
use serde_json::Value;

use club_common::AppError;

use crate::message::{parse_stream_reply, BusMessage};

/// 事件总线客户端（内部持有可克隆的连接管理器，连接自动重连）。
#[derive(Clone)]
pub struct Bus {
    connection: ConnectionManager,
}

impl Bus {
    /// 连接 Redis。
    ///
    /// 显式关闭响应超时：redis 1.7 的 [`ConnectionManager`] 默认 500ms，
    /// 会让 `XREADGROUP ... BLOCK` 等阻塞命令在客户端侧提前超时（服务端其实已投递）。
    pub async fn connect(redis_url: &str) -> Result<Self, AppError> {
        let client = redis::Client::open(redis_url).map_err(AppError::internal)?;
        let config = ConnectionManagerConfig::new()
            .set_response_timeout(None)
            .set_connection_timeout(Some(Duration::from_secs(5)));
        let connection = ConnectionManager::new_with_config(client, config)
            .await
            .map_err(AppError::internal)?;
        Ok(Self { connection })
    }

    /// 发布事件到对应模块的流，返回消息 ID。
    pub async fn publish(&self, event_type: &str, payload: &Value) -> Result<String, AppError> {
        let stream = crate::message::stream_name(event_type);
        let mut connection = self.connection.clone();
        let id: String = cmd("XADD")
            .arg(&stream)
            .arg("*")
            .arg("eventType")
            .arg(event_type)
            .arg("payload")
            .arg(serde_json::to_string(payload).map_err(AppError::internal)?)
            .query_async(&mut connection)
            .await
            .map_err(AppError::internal)?;
        Ok(id)
    }

    /// 确保消费组存在（幂等：组已存在时忽略 BUSYGROUP）。
    pub async fn ensure_group(&self, stream: &str, group: &str) -> Result<(), AppError> {
        let mut connection = self.connection.clone();
        let result: Result<(), _> = cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut connection)
            .await;
        match result {
            Ok(()) => Ok(()),
            Err(err) if err.to_string().contains("BUSYGROUP") => Ok(()),
            Err(err) => Err(AppError::internal(err)),
        }
    }

    /// 以消费组身份读取新消息（`>`），阻塞 `block_ms` 毫秒。
    pub async fn read_group(
        &self,
        streams: &[&str],
        group: &str,
        consumer: &str,
        count: usize,
        block_ms: usize,
    ) -> Result<Vec<BusMessage>, AppError> {
        if streams.is_empty() {
            return Ok(Vec::new());
        }
        let mut connection = self.connection.clone();
        let mut command = cmd("XREADGROUP");
        command
            .arg("GROUP")
            .arg(group)
            .arg(consumer)
            .arg("COUNT")
            .arg(count)
            .arg("BLOCK")
            .arg(block_ms)
            .arg("STREAMS");
        for stream in streams {
            command.arg(*stream);
        }
        for _ in streams {
            command.arg(">");
        }
        let value: redis::Value = command
            .query_async(&mut connection)
            .await
            .map_err(AppError::internal)?;
        Ok(parse_stream_reply(&value))
    }

    /// 确认消息已处理（`XACK`）。
    pub async fn ack(&self, stream: &str, group: &str, ids: &[String]) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut connection = self.connection.clone();
        let mut command = cmd("XACK");
        command.arg(stream).arg(group);
        for id in ids {
            command.arg(id);
        }
        let _: i64 = command
            .query_async(&mut connection)
            .await
            .map_err(AppError::internal)?;
        Ok(())
    }

    /// 简单探活（PING）。
    pub async fn ping(&self) -> Result<(), AppError> {
        let mut connection = self.connection.clone();
        let _: String = connection.ping().await.map_err(AppError::internal)?;
        Ok(())
    }
}
