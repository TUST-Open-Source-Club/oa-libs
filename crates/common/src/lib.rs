//! 服务共享基础库：错误模型、分页、ID、校验。
//!
//! 该 crate 不依赖数据库与网络，保证可在纯单元测试中运行。

#![warn(missing_docs)]

/// 统一错误模型（AppError / ProblemDetails / FieldError）。
pub mod error;
/// UUIDv7 主键生成。
pub mod id;
/// 游标分页与页码分页。
pub mod pagination;
/// 通用输入校验（邮箱 / 密码 / 用户名）。
pub mod validate;

pub use error::{AppError, FieldError, ProblemDetails, Result};
pub use id::new_id;
pub use pagination::{CursorPage, CursorParams, Page, PageParams};
