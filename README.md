# club-oa-libs

社团 OA 共享 Rust 库（独立仓库，作为主仓库 `libs/` 子模块引入，供各服务仓库使用）。

| crate | 说明 |
| --- | --- |
| `crates/common` | 统一错误（RFC 7807）、分页、UUIDv7、校验 |
| `crates/auth-sdk` | JWT claims/签发/验签、JWKS、axum 提取器、服务间令牌 |
| `crates/storage` | 存储抽象：S3（已有对象存储）/ 本地磁盘（规划中） |
| `crates/bus` | Redis Streams 事件总线与 outbox（规划中） |

## 使用方式

服务仓库在开发期通过相对路径依赖（主仓库子模块布局）：

```toml
club-common = { path = "../../libs/crates/common" }
```

发布远程仓库后改为 git 依赖并打 tag：

```toml
club-common = { git = "ssh://git@your-host/club-oa-libs.git", tag = "v0.1.0" }
```

## 测试与覆盖率

```bash
cargo test --workspace
cargo llvm-cov --workspace --fail-under-lines 80
```

## 许可证

本项目采用 **AGPL-3.0-or-later** 许可证，详见 [LICENSE](LICENSE)。
