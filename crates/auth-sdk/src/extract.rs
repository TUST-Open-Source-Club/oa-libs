use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use club_common::AppError;

use crate::claims::Claims;

/// 服务实现该 trait 后即可使用 `AuthUser` / `OptionalAuthUser` 提取器。
pub trait TokenVerifier: Send + Sync {
    fn verify_token(&self, token: &str) -> Result<Claims, AppError>;
}

/// 必须登录的请求提取器。
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

/// 可选登录的请求提取器（公开接口中识别登录态）。
#[derive(Debug, Clone)]
pub struct OptionalAuthUser(pub Option<Claims>);

fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|t| !t.is_empty())
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: TokenVerifier,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts)
            .ok_or_else(|| AppError::unauthorized("AUTH_MISSING_TOKEN", "缺少访问令牌"))?;
        state.verify_token(token).map(AuthUser)
    }
}

impl<S> FromRequestParts<S> for OptionalAuthUser
where
    S: TokenVerifier,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuthUser(
            bearer_token(parts).and_then(|token| state.verify_token(token).ok()),
        ))
    }
}

impl AuthUser {
    pub fn claims(&self) -> &Claims {
        &self.0
    }

    pub fn require_role(&self, role: &str) -> Result<(), AppError> {
        if self.0.has_role(role) {
            Ok(())
        } else {
            Err(AppError::forbidden("AUTH_FORBIDDEN", "权限不足"))
        }
    }

    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.0.is_admin() {
            Ok(())
        } else {
            Err(AppError::forbidden("AUTH_FORBIDDEN", "需要管理员权限"))
        }
    }

    pub fn require_scope(&self, scope: &str) -> Result<(), AppError> {
        if self.0.has_scope(scope) {
            Ok(())
        } else {
            Err(AppError::forbidden("AUTH_SCOPE_DENIED", "缺少模块访问权限"))
        }
    }

    pub fn require_resource_scope(&self, kind: &str, id: &str) -> Result<(), AppError> {
        if self.0.has_resource_scope(kind, id) {
            Ok(())
        } else {
            Err(AppError::forbidden("AUTH_SCOPE_DENIED", "缺少资源访问权限"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    struct FakeVerifier;

    impl TokenVerifier for FakeVerifier {
        fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
            if token == "valid" {
                Ok(Claims {
                    sub: "u1".into(),
                    name: "张三".into(),
                    avatar: None,
                    roles: vec!["member".into()],
                    scopes: vec!["im".into()],
                    guest: false,
                    iss: "https://oa.test".into(),
                    iat: 0,
                    exp: i64::MAX,
                    jti: "j".into(),
                })
            } else {
                Err(AppError::unauthorized("AUTH_INVALID_TOKEN", "令牌无效"))
            }
        }
    }

    fn parts_with_auth(value: Option<&str>) -> Parts {
        let mut builder = Request::builder().uri("/");
        if let Some(value) = value {
            builder = builder.header(AUTHORIZATION, value);
        }
        let (parts, _) = builder.body(()).expect("request").into_parts();
        parts
    }

    #[tokio::test]
    async fn auth_user_accepts_valid_bearer() {
        let mut parts = parts_with_auth(Some("Bearer valid"));
        let user = AuthUser::from_request_parts(&mut parts, &FakeVerifier)
            .await
            .expect("extract");
        assert_eq!(user.claims().sub, "u1");
        assert!(user.require_scope("im").is_ok());
        assert!(user.require_scope("task").is_err());
        assert!(user.require_role("member").is_ok());
        assert!(user.require_admin().is_err());
    }

    #[tokio::test]
    async fn auth_user_rejects_missing_or_invalid() {
        let mut missing = parts_with_auth(None);
        let err = AuthUser::from_request_parts(&mut missing, &FakeVerifier)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "AUTH_MISSING_TOKEN");

        let mut malformed = parts_with_auth(Some("Token abc"));
        assert!(AuthUser::from_request_parts(&mut malformed, &FakeVerifier)
            .await
            .is_err());

        let mut invalid = parts_with_auth(Some("Bearer nope"));
        assert!(AuthUser::from_request_parts(&mut invalid, &FakeVerifier)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn optional_auth_user_never_rejects() {
        let mut none = parts_with_auth(None);
        let user = OptionalAuthUser::from_request_parts(&mut none, &FakeVerifier)
            .await
            .expect("extract");
        assert!(user.0.is_none());

        let mut invalid = parts_with_auth(Some("Bearer nope"));
        let user = OptionalAuthUser::from_request_parts(&mut invalid, &FakeVerifier)
            .await
            .expect("extract");
        assert!(user.0.is_none());

        let mut valid = parts_with_auth(Some("Bearer valid"));
        let user = OptionalAuthUser::from_request_parts(&mut valid, &FakeVerifier)
            .await
            .expect("extract");
        assert_eq!(user.0.expect("claims").sub, "u1");
    }
}
