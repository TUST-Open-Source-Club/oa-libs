//! 存储后端接口。

use async_trait::async_trait;
use bytes::Bytes;

/// 存储错误。
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// 对象不存在。
    #[error("object not found: {0}")]
    NotFound(String),
    /// 非法 Key（路径穿越等）。
    #[error("invalid key: {0}")]
    InvalidKey(String),
    /// 签名无效或已过期。
    #[error("invalid or expired signature")]
    InvalidSignature,
    /// 底层 IO/网络错误。
    #[error("storage backend error: {0}")]
    Backend(String),
}

/// 存储后端统一接口。
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 写入对象（覆盖）。
    async fn put(&self, key: &str, body: Bytes, content_type: &str) -> Result<(), StorageError>;

    /// 读取对象。
    async fn get(&self, key: &str) -> Result<Bytes, StorageError>;

    /// 删除对象（不存在时视为成功）。
    async fn delete(&self, key: &str) -> Result<(), StorageError>;

    /// 对象是否存在。
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;

    /// 后端类型标识：`local` / `s3`。
    fn driver(&self) -> &'static str;
}
