//! 社团 OA 事件总线（Redis Streams）。
//!
//! 设计：
//! - 生产者将事件写入 `events.{module}` 流（字段：`eventType` + `payload` 完整信封 JSON）；
//! - 消费者以消费组（每组一个服务）读取，处理成功后 `XACK`；
//! - 未确认消息可通过 `XPENDING`/`XAUTOCLAIM` 重投（后续补充）；
//! - 服务业务事务内写 outbox 表、后台任务调用本库投递，保证不丢事件（outbox 由各服务实现）。

#![warn(missing_docs)]

/// Redis 客户端封装。
pub mod client;
/// 消息模型与解析。
pub mod message;
/// Outbox 可靠投递。
pub mod audit;
pub mod outbox;

pub use client::Bus;
pub use message::{parse_stream_reply, stream_name, BusMessage};
