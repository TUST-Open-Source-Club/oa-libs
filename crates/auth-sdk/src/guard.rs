//! Bot 模块级读写守卫（axum 中间件）。
//!
//! 用法（各服务在路由末尾挂载）：
//! ```ignore
//! .layer(axum::middleware::from_fn(|req, next| {
//!     club_auth_sdk::guard::guard_bot_request(req, next, "task")
//! }))
//! ```
//! 说明：守卫只做「收紧」——从 Token 载荷（不验签）读取账号类型与权限矩阵，
//! 人类账号与无效令牌一律放行给业务层认证；因此伪造载荷无法提权。

use axum::extract::Request;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;

use club_common::AppError;

use crate::claims::Claims;

/// 不验签解析 Token 载荷（仅用于收紧 Bot 权限，越权判定不依赖签名）。
fn decode_claims_unverified(token: &str) -> Option<Claims> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 方法是否视为写操作。
pub fn is_write_method(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// 从 Authorization 头提取 Bearer Token。
fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

/// Bot 守卫：Bot 账号缺少模块读写权限时返回 403。
pub async fn guard_bot_request(req: Request, next: Next, module: &'static str) -> Response {
    if let Some(token) = bearer_token(&req) {
        if let Some(claims) = decode_claims_unverified(token) {
            let write = is_write_method(req.method());
            if !claims.allow_module(module, write) {
                return AppError::forbidden(
                    "AUTH_FORBIDDEN_MODULE",
                    format!("Bot 无权{}模块 {module}", if write { "写入" } else { "读取" }),
                )
                .into_response();
            }
        }
    }
    next.run(req).await
}
