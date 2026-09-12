//! 社团 OA 存储抽象。
//!
//! 统一 S3（连接已有对象存储）与本地磁盘两种后端；业务只依赖 [`StorageBackend`]。
//! 本地后端下载走 HMAC 签名 URL，由业务服务的下载路由校验后回源文件。

#![warn(missing_docs)]

/// 本地磁盘后端。
pub mod local;
/// S3 兼容后端（`s3` feature）。
#[cfg(feature = "s3")]
pub mod s3;
/// 后端接口与错误。
pub mod traits;

pub use local::LocalBackend;
#[cfg(feature = "s3")]
pub use s3::S3Backend;
pub use traits::{StorageBackend, StorageError};
