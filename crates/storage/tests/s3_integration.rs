//! S3 后端集成测试（需要 MinIO/本地 S3；未配置 `S3_TEST_ENDPOINT` 时跳过）。
//!
//! 本地运行：
//! ```bash
//! ./scripts/s3-test-env.sh          # 启动 MinIO 并建桶
//! cargo test --features s3 --test s3_integration
//! ./scripts/s3-test-env.sh stop
//! ```

#![cfg(feature = "s3")]
#![allow(missing_docs)]

use bytes::Bytes;
use club_storage::{S3Backend, StorageBackend, StorageError};

fn endpoint() -> Option<String> {
    std::env::var("S3_TEST_ENDPOINT").ok()
}

fn backend() -> S3Backend {
    S3Backend::new(
        &endpoint().expect("S3_TEST_ENDPOINT"),
        &std::env::var("S3_TEST_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        &std::env::var("S3_TEST_BUCKET").unwrap_or_else(|_| "club-oa-test".to_string()),
        &std::env::var("S3_TEST_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
        &std::env::var("S3_TEST_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
    )
    .expect("构建 S3 后端")
}

#[tokio::test]
async fn put_get_exists_delete_roundtrip() {
    if endpoint().is_none() {
        eprintln!("跳过：未设置 S3_TEST_ENDPOINT");
        return;
    }
    let storage = backend();
    let key = format!("it/{}.txt", uuid::Uuid::now_v7().simple());
    storage
        .put(&key, Bytes::from_static(b"s3-hello"), "text/plain")
        .await
        .expect("put");
    assert!(storage.exists(&key).await.expect("exists"));
    assert_eq!(
        storage.get(&key).await.expect("get"),
        Bytes::from_static(b"s3-hello")
    );
    storage.delete(&key).await.expect("delete");
    assert!(!storage.exists(&key).await.expect("exists2"));
    assert!(matches!(
        storage.get(&key).await,
        Err(StorageError::NotFound(_))
    ));
    // 重复删除幂等
    storage.delete(&key).await.expect("delete again");
    assert_eq!(storage.driver(), "s3");
}

#[tokio::test]
async fn presign_get_returns_usable_url() {
    if endpoint().is_none() {
        eprintln!("跳过：未设置 S3_TEST_ENDPOINT");
        return;
    }
    let storage = backend();
    let key = format!("it/{}.txt", uuid::Uuid::now_v7().simple());
    storage
        .put(&key, Bytes::from_static(b"presign"), "text/plain")
        .await
        .expect("put");
    let url = storage
        .presign_get(&key, 300)
        .await
        .expect("presign")
        .expect("s3 必须返回预签名 URL");
    assert!(
        url.contains("X-Amz-Signature") || url.contains("Signature"),
        "url={url}"
    );

    // 预签名 URL 可直接下载（不携带额外鉴权头）
    let response = reqwest::get(&url).await.expect("presigned get");
    assert!(
        response.status().is_success(),
        "status={}",
        response.status()
    );
    assert_eq!(
        response.bytes().await.expect("body"),
        Bytes::from_static(b"presign")
    );
}

#[tokio::test]
async fn missing_key_returns_not_found() {
    if endpoint().is_none() {
        eprintln!("跳过：未设置 S3_TEST_ENDPOINT");
        return;
    }
    let storage = backend();
    let key = format!("it/not-exists-{}.txt", uuid::Uuid::now_v7().simple());
    assert!(!storage.exists(&key).await.expect("exists"));
    assert!(matches!(
        storage.get(&key).await,
        Err(StorageError::NotFound(_))
    ));
}
