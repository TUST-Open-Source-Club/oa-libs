//! 服务共享基础库：错误模型、分页、ID、校验。
//!
//! 该 crate 不依赖数据库与网络，保证可在纯单元测试中运行。

pub mod error;
pub mod id;
pub mod pagination;
pub mod validate;

pub use error::{AppError, FieldError, ProblemDetails, Result};
pub use id::new_id;
pub use pagination::{CursorPage, CursorParams, Page, PageParams};
