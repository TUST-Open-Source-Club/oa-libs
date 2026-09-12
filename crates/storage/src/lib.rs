//! 社团 OA 存储抽象。
//!
//! 统一 S3（连接已有对象存储）与本地磁盘两种后端；业务只依赖 [`StorageBackend`]。
//! 本地后端下载走 HMAC 签名 URL，由业务服务的下载路由校验后回源文件。

#![warn(missing_docs)]

/// 本地磁盘后端。
pub mod local;
/// 后端接口与错误。
pub mod traits;
// S3 兼容后端（object_store）将在 drive 接入时以 `s3` feature 落地，
// 并通过 MinIO 容器做集成测试（见 docs/architecture.md 存储章节）。

pub use local::LocalBackend;
pub use traits::{StorageBackend, StorageError};
