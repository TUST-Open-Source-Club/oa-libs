//! S3 兼容后端（object_store）：连接已有对象存储（AWS S3 / 阿里云 OSS / 腾讯 COS / MinIO 等）。

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use http::Method;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::signer::Signer;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};

use crate::traits::{StorageBackend, StorageError};

/// S3 后端。
pub struct S3Backend {
    store: object_store::aws::AmazonS3,
}

impl std::fmt::Debug for S3Backend {
    /// 手动实现，避免打印凭据相关内部状态。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Backend").finish_non_exhaustive()
    }
}

impl S3Backend {
    /// 创建后端。
    ///
    /// - `endpoint`：S3 兼容端点（如 `https://oss-cn-hangzhou.aliyuncs.com` 或 `http://minio:9000`）
    /// - `region`：区域（MinIO 可填 `us-east-1`）
    /// - 使用 Path-style 请求以兼容 MinIO/OSS 自定义端点
    pub fn new(
        endpoint: &str,
        region: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Result<Self, StorageError> {
        let store = AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_region(region)
            .with_bucket_name(bucket)
            .with_access_key_id(access_key)
            .with_secret_access_key(secret_key)
            .with_allow_http(endpoint.starts_with("http://"))
            .with_virtual_hosted_style_request(false)
            .build()
            .map_err(|err| StorageError::Backend(err.to_string()))?;
        Ok(Self { store })
    }

    /// Key → object_store 路径。
    fn path(key: &str) -> ObjectPath {
        ObjectPath::from(key)
    }
}

/// object_store 错误 → 统一错误（NotFound 单独识别）。
fn map_error(err: object_store::Error) -> StorageError {
    if matches!(err, object_store::Error::NotFound { .. }) {
        StorageError::NotFound(err.to_string())
    } else {
        StorageError::Backend(err.to_string())
    }
}

#[async_trait]
impl StorageBackend for S3Backend {
    async fn put(&self, key: &str, body: Bytes, _content_type: &str) -> Result<(), StorageError> {
        self.store
            .put(&Self::path(key), PutPayload::from_bytes(body))
            .await
            .map(|_| ())
            .map_err(map_error)
    }

    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        let result = self.store.get(&Self::path(key)).await.map_err(map_error)?;
        result.bytes().await.map_err(map_error)
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match self.store.delete(&Self::path(key)).await {
            Ok(()) => Ok(()),
            // 幂等：不存在视为成功
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(err) => Err(map_error(err)),
        }
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        match self.store.head(&Self::path(key)).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(err) => Err(map_error(err)),
        }
    }

    async fn presign_get(
        &self,
        key: &str,
        expires_seconds: u64,
    ) -> Result<Option<String>, StorageError> {
        let url = self
            .store
            .signed_url(
                Method::GET,
                &Self::path(key),
                Duration::from_secs(expires_seconds.clamp(1, 7 * 24 * 3600)),
            )
            .await
            .map_err(map_error)?;
        Ok(Some(url.to_string()))
    }

    fn driver(&self) -> &'static str {
        "s3"
    }
}
