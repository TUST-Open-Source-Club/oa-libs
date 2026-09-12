use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

/// 统一错误类型，序列化为 RFC 7807 Problem Details。
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{detail}")]
    BadRequest { code: String, detail: String },
    #[error("{detail}")]
    Unauthorized { code: String, detail: String },
    #[error("{detail}")]
    Forbidden { code: String, detail: String },
    #[error("{detail}")]
    NotFound { code: String, detail: String },
    #[error("{detail}")]
    Conflict { code: String, detail: String },
    #[error("{detail}")]
    TooManyRequests { code: String, detail: String },
    #[error("{detail}")]
    Unprocessable {
        code: String,
        detail: String,
        errors: Vec<FieldError>,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn bad_request(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::BadRequest {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn unauthorized(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Unauthorized {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn forbidden(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Forbidden {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn not_found(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::NotFound {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn conflict(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Conflict {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn too_many_requests(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::TooManyRequests {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn unprocessable(
        code: impl Into<String>,
        detail: impl Into<String>,
        errors: Vec<FieldError>,
    ) -> Self {
        Self::Unprocessable {
            code: code.into(),
            detail: detail.into(),
            errors,
        }
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        Self::Internal(err.to_string())
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest { .. } => StatusCode::BAD_REQUEST,
            AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            AppError::Forbidden { .. } => StatusCode::FORBIDDEN,
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::Unprocessable { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &str {
        match self {
            AppError::BadRequest { code, .. }
            | AppError::Unauthorized { code, .. }
            | AppError::Forbidden { code, .. }
            | AppError::NotFound { code, .. }
            | AppError::Conflict { code, .. }
            | AppError::TooManyRequests { code, .. }
            | AppError::Unprocessable { code, .. } => code,
            AppError::Internal(_) => "INTERNAL",
        }
    }

    pub fn problem(&self) -> ProblemDetails {
        let (title, detail, errors) = match self {
            AppError::BadRequest { detail, .. } => ("Bad Request", detail.clone(), None),
            AppError::Unauthorized { detail, .. } => ("Unauthorized", detail.clone(), None),
            AppError::Forbidden { detail, .. } => ("Forbidden", detail.clone(), None),
            AppError::NotFound { detail, .. } => ("Not Found", detail.clone(), None),
            AppError::Conflict { detail, .. } => ("Conflict", detail.clone(), None),
            AppError::TooManyRequests { detail, .. } => {
                ("Too Many Requests", detail.clone(), None)
            }
            AppError::Unprocessable { detail, errors, .. } => {
                ("Unprocessable Entity", detail.clone(), Some(errors.clone()))
            }
            AppError::Internal(_) => (
                "Internal Server Error",
                "服务器内部错误".to_string(),
                None,
            ),
        };

        ProblemDetails {
            type_uri: format!("https://club-oa.local/errors/{}", self.code()),
            title: title.to_string(),
            status: self.status().as_u16(),
            detail,
            instance: None,
            code: self.code().to_string(),
            errors,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        if let AppError::Internal(err) = &self {
            tracing::error!(error = %err, "internal server error");
        }
        (status, Json(self.problem())).into_response()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FieldError>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_problem(err: AppError, status: StatusCode, code: &str) {
        assert_eq!(err.status(), status);
        assert_eq!(err.code(), code);
        let problem = err.problem();
        assert_eq!(problem.status, status.as_u16());
        assert_eq!(problem.code, code);
        assert_eq!(problem.type_uri, format!("https://club-oa.local/errors/{code}"));
    }

    #[test]
    fn maps_each_variant_to_status_and_code() {
        assert_problem(
            AppError::bad_request("A_BAD", "x"),
            StatusCode::BAD_REQUEST,
            "A_BAD",
        );
        assert_problem(
            AppError::unauthorized("A_UNAUTH", "x"),
            StatusCode::UNAUTHORIZED,
            "A_UNAUTH",
        );
        assert_problem(
            AppError::forbidden("A_FORBIDDEN", "x"),
            StatusCode::FORBIDDEN,
            "A_FORBIDDEN",
        );
        assert_problem(
            AppError::not_found("A_NOT_FOUND", "x"),
            StatusCode::NOT_FOUND,
            "A_NOT_FOUND",
        );
        assert_problem(
            AppError::conflict("A_CONFLICT", "x"),
            StatusCode::CONFLICT,
            "A_CONFLICT",
        );
        assert_problem(
            AppError::too_many_requests("A_LIMIT", "x"),
            StatusCode::TOO_MANY_REQUESTS,
            "A_LIMIT",
        );
        assert_problem(
            AppError::unprocessable("A_VALIDATION", "x", vec![]),
            StatusCode::UNPROCESSABLE_ENTITY,
            "A_VALIDATION",
        );
        assert_problem(
            AppError::Internal("db down".into()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
        );
    }

    #[test]
    fn internal_error_hides_details() {
        let problem = AppError::Internal("connection refused to 10.0.0.1".into()).problem();
        assert_eq!(problem.detail, "服务器内部错误");
        assert!(!problem.detail.contains("10.0.0.1"));
    }

    #[test]
    fn unprocessable_carries_field_errors() {
        let err = AppError::unprocessable(
            "AUTH_VALIDATION",
            "validation failed",
            vec![
                FieldError::new("email", "格式错误"),
                FieldError::new("password", "过短"),
            ],
        );
        let problem = err.problem();
        let errors = problem.errors.expect("errors present");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].field, "email");
    }

    #[test]
    fn serializes_to_problem_details_json() {
        let err = AppError::not_found("AUTH_USER_NOT_FOUND", "用户不存在");
        let value = serde_json::to_value(err.problem()).expect("serialize");
        assert_eq!(value["type"], "https://club-oa.local/errors/AUTH_USER_NOT_FOUND");
        assert_eq!(value["status"], 404);
        assert_eq!(value["code"], "AUTH_USER_NOT_FOUND");
        assert_eq!(value["detail"], "用户不存在");
        assert!(value.get("errors").is_none());
    }
}
