//! 本地磁盘后端 + HMAC 下载签名。

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use tokio::io::AsyncWriteExt;

use crate::traits::{StorageBackend, StorageError};

type HmacSha256 = Hmac<Sha256>;

/// 本地磁盘后端。
#[derive(Debug, Clone)]
pub struct LocalBackend {
    root: PathBuf,
    sign_secret: Vec<u8>,
}

impl LocalBackend {
    /// 创建本地后端；`root` 为存储根目录，`sign_secret` 用于签名下载 URL。
    pub fn new(root: impl Into<PathBuf>, sign_secret: impl Into<Vec<u8>>) -> Self {
        Self {
            root: root.into(),
            sign_secret: sign_secret.into(),
        }
    }

    /// 校验并映射 Key 到磁盘路径。
    ///
    /// 规则：非空、仅允许 `[A-Za-z0-9._/-]`、禁止以 `/` 开头、禁止 `..` 段。
    pub fn resolve_path(&self, key: &str) -> Result<PathBuf, StorageError> {
        validate_key(key)?;
        Ok(self.root.join(key))
    }

    /// 生成下载签名：`HMAC(key\nexpiresAt)`（base64url 无填充）。
    pub fn sign_download(&self, key: &str, expires_at: DateTime<Utc>) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.sign_secret).expect("hmac key");
        mac.update(format!("{key}\n{}", expires_at.timestamp()).as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    /// 校验下载签名与过期时间。
    pub fn verify_download(
        &self,
        key: &str,
        expires_at: DateTime<Utc>,
        signature: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        if expires_at < now {
            return Err(StorageError::InvalidSignature);
        }
        let raw = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| StorageError::InvalidSignature)?;
        let mut mac = HmacSha256::new_from_slice(&self.sign_secret).expect("hmac key");
        mac.update(format!("{key}\n{}", expires_at.timestamp()).as_bytes());
        mac.verify_slice(&raw)
            .map_err(|_| StorageError::InvalidSignature)
    }

    /// 生成带签名的下载路径（业务服务路由自行拼接前缀）。
    pub fn download_url(&self, key: &str, expires_at: DateTime<Utc>) -> String {
        format!(
            "{key}?expires={}&signature={}",
            expires_at.timestamp(),
            self.sign_download(key, expires_at)
        )
    }
}

/// Key 合法性校验（防路径穿越）。
pub fn validate_key(key: &str) -> Result<(), StorageError> {
    if key.is_empty() || key.starts_with('/') || key.len() > 512 {
        return Err(StorageError::InvalidKey(key.to_string()));
    }
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        return Err(StorageError::InvalidKey(key.to_string()));
    }
    if key
        .split('/')
        .any(|segment| segment == ".." || segment.is_empty())
    {
        return Err(StorageError::InvalidKey(key.to_string()));
    }
    Ok(())
}

#[async_trait]
impl StorageBackend for LocalBackend {
    async fn put(&self, key: &str, body: Bytes, _content_type: &str) -> Result<(), StorageError> {
        let path = self.resolve_path(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| StorageError::Backend(err.to_string()))?;
        }
        let mut file = tokio::fs::File::create(&path)
            .await
            .map_err(|err| StorageError::Backend(err.to_string()))?;
        file.write_all(&body)
            .await
            .map_err(|err| StorageError::Backend(err.to_string()))?;
        file.flush()
            .await
            .map_err(|err| StorageError::Backend(err.to_string()))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        let path = self.resolve_path(key)?;
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(Bytes::from(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(key.to_string()))
            }
            Err(err) => Err(StorageError::Backend(err.to_string())),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.resolve_path(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(StorageError::Backend(err.to_string())),
        }
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let path = self.resolve_path(key)?;
        Ok(path.exists())
    }

    fn driver(&self) -> &'static str {
        "local"
    }
}

/// 业务侧回源时使用的安全路径解析（与 [`LocalBackend::resolve_path`] 一致）。
pub fn resolve_within(root: &Path, key: &str) -> Result<PathBuf, StorageError> {
    validate_key(key)?;
    Ok(root.join(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn backend() -> (LocalBackend, tempdir::Path) {
        let dir = tempdir::Path::new();
        (LocalBackend::new(dir.path(), b"secret"), dir)
    }

    /// 简易临时目录（避免引入 tempfile 依赖）。
    mod tempdir {
        use std::path::{Path as StdPath, PathBuf};

        /// 临时目录句柄（Drop 删除）。
        pub struct Path(PathBuf);

        impl Path {
            /// 创建唯一目录。
            pub fn new() -> Self {
                let path = std::env::temp_dir().join(format!("club-storage-{}", Self::uuid_like()));
                std::fs::create_dir_all(&path).expect("create temp dir");
                Self(path)
            }

            /// 路径。
            pub fn path(&self) -> &StdPath {
                &self.0
            }

            fn uuid_like() -> String {
                use std::time::{SystemTime, UNIX_EPOCH};
                format!(
                    "{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("time")
                        .as_nanos()
                )
            }
        }

        impl Drop for Path {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[tokio::test]
    async fn put_get_delete_roundtrip() {
        let (storage, _dir) = backend();
        let key = "drive/space1/file.txt";
        storage
            .put(key, Bytes::from_static(b"hello"), "text/plain")
            .await
            .expect("put");
        assert!(storage.exists(key).await.expect("exists"));
        assert_eq!(
            storage.get(key).await.expect("get"),
            Bytes::from_static(b"hello")
        );
        storage.delete(key).await.expect("delete");
        assert!(!storage.exists(key).await.expect("exists after delete"));
        assert!(matches!(storage.delete(key).await, Ok(())), "重复删除幂等");
        assert!(matches!(
            storage.get(key).await,
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn rejects_path_traversal_and_bad_keys() {
        for key in [
            "",
            "/etc/passwd",
            "../secret",
            "a/../b",
            "a//b",
            "中文.txt",
            "a b",
        ] {
            assert!(validate_key(key).is_err(), "应拒绝: {key}");
        }
        for key in ["a.txt", "drive/x/y.png", ".hidden", "a-b_c/d.1"] {
            assert!(validate_key(key).is_ok(), "应接受: {key}");
        }
    }

    #[tokio::test]
    async fn traversal_key_is_rejected_at_runtime() {
        let (storage, _dir) = backend();
        assert!(matches!(
            storage
                .put("../escape.txt", Bytes::from_static(b"x"), "text/plain")
                .await,
            Err(StorageError::InvalidKey(_))
        ));
    }

    #[test]
    fn download_signature_roundtrip_and_tamper() {
        let (storage, _dir) = backend();
        let now = Utc::now();
        let expires = now + Duration::minutes(5);
        let signature = storage.sign_download("a/b.txt", expires);
        assert!(storage
            .verify_download("a/b.txt", expires, &signature, now)
            .is_ok());
        // 篡改 key / 过期 / 伪造签名均失败
        assert!(storage
            .verify_download("a/c.txt", expires, &signature, now)
            .is_err());
        assert!(storage
            .verify_download("a/b.txt", expires, &signature, now + Duration::minutes(6))
            .is_err());
        assert!(storage
            .verify_download("a/b.txt", expires, "forged", now)
            .is_err());
    }

    #[test]
    fn download_url_contains_expiry_and_signature() {
        let (storage, _dir) = backend();
        let expires = Utc::now() + Duration::minutes(5);
        let url = storage.download_url("a/b.txt", expires);
        assert!(url.starts_with("a/b.txt?expires="));
        assert!(url.contains("&signature="));
    }

    #[test]
    fn driver_name_is_local() {
        let (storage, _dir) = backend();
        assert_eq!(storage.driver(), "local");
    }
}
