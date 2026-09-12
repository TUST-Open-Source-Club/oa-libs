use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// 服务间调用令牌的默认时间窗口（秒）。
pub const DEFAULT_LEEWAY_SECONDS: i64 = 60;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ServiceTokenError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("timestamp out of allowed window")]
    Expired,
    #[error("malformed token")]
    Malformed,
}

fn body_digest_hex(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn candidate_message(service: &str, timestamp: i64, body: &[u8]) -> String {
    format!("{service}\n{timestamp}\n{}", body_digest_hex(body))
}

fn mac_for(secret: &[u8], message: &str) -> HmacSha256 {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts keys of any size");
    mac.update(message.as_bytes());
    mac
}

/// 生成服务令牌签名（base64 标准编码）。
pub fn sign_service_token(service: &str, secret: &[u8], timestamp: i64, body: &[u8]) -> String {
    let mac = mac_for(secret, &candidate_message(service, timestamp, body));
    STANDARD.encode(mac.finalize().into_bytes())
}

/// 校验服务令牌：时间窗口 + 恒定时间签名比较。
pub fn verify_service_token(
    service: &str,
    secret: &[u8],
    timestamp: i64,
    body: &[u8],
    signature: &str,
    now: i64,
    leeway_seconds: i64,
) -> Result<(), ServiceTokenError> {
    if (now - timestamp).abs() > leeway_seconds {
        return Err(ServiceTokenError::Expired);
    }
    let raw = STANDARD
        .decode(signature)
        .map_err(|_| ServiceTokenError::Malformed)?;
    let mac = mac_for(secret, &candidate_message(service, timestamp, body));
    mac.verify_slice(&raw)
        .map_err(|_| ServiceTokenError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"super-secret";
    const NOW: i64 = 1_700_000_000;

    #[test]
    fn sign_and_verify_roundtrip() {
        let sig = sign_service_token("notify", SECRET, NOW, b"{}");
        assert!(verify_service_token("notify", SECRET, NOW, b"{}", &sig, NOW, 60).is_ok());
        // 时间窗口内允许偏移
        assert!(verify_service_token("notify", SECRET, NOW, b"{}", &sig, NOW + 30, 60).is_ok());
        assert!(verify_service_token("notify", SECRET, NOW, b"{}", &sig, NOW - 30, 60).is_ok());
    }

    #[test]
    fn rejects_tampered_body_or_service() {
        let sig = sign_service_token("notify", SECRET, NOW, b"{}");
        assert_eq!(
            verify_service_token("notify", SECRET, NOW, b"{\"x\":1}", &sig, NOW, 60),
            Err(ServiceTokenError::InvalidSignature)
        );
        assert_eq!(
            verify_service_token("im", SECRET, NOW, b"{}", &sig, NOW, 60),
            Err(ServiceTokenError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_wrong_secret() {
        let sig = sign_service_token("notify", SECRET, NOW, b"{}");
        assert_eq!(
            verify_service_token("notify", b"other", NOW, b"{}", &sig, NOW, 60),
            Err(ServiceTokenError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_expired_timestamp() {
        let sig = sign_service_token("notify", SECRET, NOW, b"{}");
        assert_eq!(
            verify_service_token("notify", SECRET, NOW, b"{}", &sig, NOW + 61, 60),
            Err(ServiceTokenError::Expired)
        );
    }

    #[test]
    fn rejects_malformed_signature() {
        assert_eq!(
            verify_service_token("notify", SECRET, NOW, b"{}", "not-base64!!", NOW, 60),
            Err(ServiceTokenError::Malformed)
        );
    }
}
