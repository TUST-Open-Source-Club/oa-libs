use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jsonwebtoken::DecodingKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::jwt::JwtError;

/// 单个 RSA 公钥 JWK。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jwk {
    pub kty: String,
    pub kid: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub alg: String,
    pub n: String,
    pub e: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// 由公钥 PEM 内容派生稳定 kid（SHA-256 前 16 字节 base64url）。
pub fn key_id_from_pem(public_pem: &[u8]) -> String {
    let digest = Sha256::digest(public_pem);
    URL_SAFE_NO_PAD.encode(&digest[..16])
}

/// 用模数与指数（大端字节）构造 JWK。
pub fn jwk_from_rsa_public_components(kid: &str, modulus: &[u8], exponent: &[u8]) -> Jwk {
    Jwk {
        kty: "RSA".into(),
        kid: kid.into(),
        use_: "sig".into(),
        alg: "RS256".into(),
        n: URL_SAFE_NO_PAD.encode(modulus),
        e: URL_SAFE_NO_PAD.encode(exponent),
    }
}

pub fn decoding_key_from_jwk(jwk: &Jwk) -> Result<DecodingKey, JwtError> {
    if jwk.kty != "RSA" {
        return Err(JwtError::Invalid(format!("unsupported kty: {}", jwk.kty)));
    }
    DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| JwtError::Invalid(format!("invalid rsa components: {e}")))
}

pub fn decoding_key_from_jwks(jwks: &Jwks, kid: Option<&str>) -> Result<DecodingKey, JwtError> {
    let jwk = match kid {
        Some(kid) => jwks
            .keys
            .iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| JwtError::Invalid(format!("kid not found: {kid}")))?,
        None => jwks
            .keys
            .first()
            .ok_or_else(|| JwtError::Invalid("empty jwks".into()))?,
    };
    decoding_key_from_jwk(jwk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_id_is_stable_and_sha256_based() {
        let a = key_id_from_pem(b"pem-content");
        let b = key_id_from_pem(b"pem-content");
        let c = key_id_from_pem(b"other-content");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(!a.is_empty());
        assert!(!a.contains('='), "base64url no padding expected");
    }

    #[test]
    fn jwk_serializes_with_standard_field_names() {
        let jwk = jwk_from_rsa_public_components("kid1", &[1, 2, 3], &[1, 0, 1]);
        let value = serde_json::to_value(&jwk).expect("serialize");
        assert_eq!(value["kty"], "RSA");
        assert_eq!(value["use"], "sig");
        assert_eq!(value["alg"], "RS256");
        assert_eq!(value["n"], URL_SAFE_NO_PAD.encode([1u8, 2, 3]));
        assert_eq!(value["e"], URL_SAFE_NO_PAD.encode([1u8, 0, 1]));
    }

    #[test]
    fn rejects_non_rsa_and_missing_kid() {
        let mut jwk = jwk_from_rsa_public_components("kid1", &[1], &[1]);
        jwk.kty = "EC".into();
        assert!(matches!(
            decoding_key_from_jwk(&jwk),
            Err(JwtError::Invalid(_))
        ));

        let jwks = Jwks {
            keys: vec![jwk_from_rsa_public_components("kid1", &[1], &[1])],
        };
        assert!(decoding_key_from_jwks(&jwks, Some("missing")).is_err());
        assert!(decoding_key_from_jwks(&Jwks { keys: vec![] }, None).is_err());
    }
}
