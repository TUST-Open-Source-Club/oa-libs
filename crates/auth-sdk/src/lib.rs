//! 统一认证 SDK：JWT claims、签发/验签、JWKS 解析、axum 提取器、服务令牌。
//!
//! 所有自研服务通过本 crate 本地验签，不逐请求回调 auth 服务。

pub mod claims;
pub mod extract;
pub mod jwks;
pub mod jwt;
pub mod service_token;

pub use claims::Claims;
pub use extract::{AuthUser, OptionalAuthUser, TokenVerifier};
pub use jwks::{Jwk, Jwks};
pub use jwt::{decode_access_token, encode_access_token, JwtError};
pub use service_token::{sign_service_token, verify_service_token, ServiceTokenError};
