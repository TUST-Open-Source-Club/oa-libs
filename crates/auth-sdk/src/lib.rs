//! 统一认证 SDK：JWT claims、签发/验签、JWKS 解析、axum 提取器、服务令牌。
//!
//! 所有自研服务通过本 crate 本地验签，不逐请求回调 auth 服务。

#![warn(missing_docs)]

/// Access Token 载荷结构。
pub mod claims;
/// axum 请求提取器（AuthUser / OptionalAuthUser）。
pub mod extract;
/// JWKS 类型与解码 key 解析。
pub mod jwks;
/// JWT 签发与验签。
pub mod jwt;
/// 服务间调用令牌（HMAC）。
pub mod service_token;

pub use claims::Claims;
pub use extract::{AuthUser, OptionalAuthUser, TokenVerifier};
pub use jwks::{decoding_key_from_rsa_pem, jwk_from_rsa_public_components, Jwk, Jwks};
pub use jwt::{decode_access_token, encode_access_token, JwtError};
pub use service_token::{sign_service_token, verify_service_token, ServiceTokenError};
