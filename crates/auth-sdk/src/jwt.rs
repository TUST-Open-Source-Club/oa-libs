use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::claims::Claims;

/// JWT 处理错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JwtError {
    /// 令牌已过期（超出 leeway 容差）。
    #[error("token expired")]
    Expired,
    /// 令牌无效：签名、issuer、格式等校验失败。
    #[error("invalid token: {0}")]
    Invalid(String),
    /// 签发失败（私钥格式错误等）。
    #[error("token encoding failed: {0}")]
    Encoding(String),
}

/// 使用 RSA 私钥 PEM 签发 Access Token（RS256），`kid` 应与 JWKS 中的 kid 一致
/// （由公钥 PEM 经 [`key_id_from_pem`] 派生）。
pub fn encode_access_token(
    claims: &Claims,
    private_pem: &[u8],
    kid: &str,
) -> Result<String, JwtError> {
    let key =
        EncodingKey::from_rsa_pem(private_pem).map_err(|e| JwtError::Encoding(e.to_string()))?;
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());
    jsonwebtoken::encode(&header, claims, &key).map_err(|e| JwtError::Encoding(e.to_string()))
}

/// 校验 Access Token：算法、签名、issuer、过期时间。
pub fn decode_access_token(
    token: &str,
    key: &DecodingKey,
    issuer: &str,
) -> Result<Claims, JwtError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.validate_exp = true;

    jsonwebtoken::decode::<Claims>(token, key, &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            _ => {
                let _ = e;
                JwtError::Invalid("token verification failed".into())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jwks::key_id_from_pem;
    use jsonwebtoken::DecodingKey;
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};
    use std::sync::OnceLock;

    struct TestKeys {
        private_pem: Vec<u8>,
        public_pem: Vec<u8>,
    }

    fn test_keys() -> &'static TestKeys {
        static KEYS: OnceLock<TestKeys> = OnceLock::new();
        KEYS.get_or_init(|| {
            let mut rng = rand_core::OsRng;
            let private = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("keygen");
            let private_pem = private
                .to_pkcs8_pem(LineEnding::LF)
                .expect("encode private")
                .as_bytes()
                .to_vec();
            use rsa::pkcs8::EncodePublicKey;
            let public_pem = rsa::RsaPublicKey::from(&private)
                .to_public_key_pem(LineEnding::LF)
                .expect("encode public")
                .into_bytes();
            TestKeys {
                private_pem,
                public_pem,
            }
        })
    }

    fn now_ts() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_secs() as i64
    }

    fn sample_claims(exp_offset: i64) -> Claims {
        let now = now_ts();
        Claims {
            sub: "u1".into(),
            name: "张三".into(),
            avatar: None,
            roles: vec!["member".into()],
            scopes: vec!["im".into()],
            guest: false,
            account_type: None,
            bot_permissions: None,
            iss: "https://oa.test".into(),
            iat: now,
            exp: now + exp_offset,
            jti: "j1".into(),
        }
    }

    fn decoding_key() -> DecodingKey {
        DecodingKey::from_rsa_pem(&test_keys().public_pem).expect("public key")
    }

    /// JWKS 中使用的 kid（由公钥派生，与 token 头部一致）。
    fn test_kid() -> String {
        key_id_from_pem(&test_keys().public_pem)
    }

    fn sign(claims: &Claims) -> String {
        encode_access_token(claims, &test_keys().private_pem, &test_kid()).expect("sign")
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let claims = sample_claims(3600);
        let token = sign(&claims);
        let decoded =
            decode_access_token(&token, &decoding_key(), "https://oa.test").expect("verify");
        assert_eq!(decoded, claims);

        let header = jsonwebtoken::decode_header(&token).expect("header");
        assert_eq!(header.alg, Algorithm::RS256);
        assert_eq!(header.kid, Some(test_kid()));
    }

    #[test]
    fn rejects_expired_token() {
        // jsonwebtoken 默认 60s leeway（时钟偏移容差），过期需超过该窗口
        let claims = sample_claims(-120);
        let token = sign(&claims);
        let err = decode_access_token(&token, &decoding_key(), "https://oa.test").unwrap_err();
        assert_eq!(err, JwtError::Expired);
    }

    #[test]
    fn rejects_wrong_issuer() {
        let token = sign(&sample_claims(3600));
        let err = decode_access_token(&token, &decoding_key(), "https://evil.test").unwrap_err();
        assert!(matches!(err, JwtError::Invalid(_)));
    }

    #[test]
    fn rejects_tampered_token() {
        let token = sign(&sample_claims(3600));
        let mut tampered = token.clone();
        tampered.push('x');
        let err = decode_access_token(&tampered, &decoding_key(), "https://oa.test").unwrap_err();
        assert!(matches!(err, JwtError::Invalid(_)));

        // 修改载荷（签名不再匹配）
        let parts: Vec<&str> = token.split('.').collect();
        let forged_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"sub":"attacker","name":"x","iss":"https://oa.test","iat":1,"exp":9999999999,"jti":"j"}"#);
        let forged = format!("{}.{}.{}", parts[0], forged_payload, parts[2]);
        let err = decode_access_token(&forged, &decoding_key(), "https://oa.test").unwrap_err();
        assert!(matches!(err, JwtError::Invalid(_)));
    }

    #[test]
    fn rejects_invalid_encoding_key() {
        let err = encode_access_token(&sample_claims(3600), b"not-a-pem", "kid").unwrap_err();
        assert!(matches!(err, JwtError::Encoding(_)));
    }

    use base64::Engine;
}
