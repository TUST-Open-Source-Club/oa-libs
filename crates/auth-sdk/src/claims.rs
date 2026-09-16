use serde::{Deserialize, Serialize};

/// Access Token 载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// 用户 ID（游客为 `guest:{grantId}`）。
    pub sub: String,
    /// 显示名。
    pub name: String,
    /// 头像存储 key（可选）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// 角色列表（member/admin/superadmin/...）。
    #[serde(default)]
    pub roles: Vec<String>,
    /// 模块级 scope 与资源级 scope（meeting:{id} 等）。
    #[serde(default)]
    pub scopes: Vec<String>,
    /// 是否游客令牌。
    #[serde(default)]
    pub guest: bool,
    /// 账号类型：human / bot（缺省视为 human）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    /// Bot 权限矩阵（模块 → {read, write}）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_permissions: Option<serde_json::Value>,
    /// 签发者（issuer URL）。
    pub iss: String,
    /// 签发时间（Unix 秒）。
    pub iat: i64,
    /// 过期时间（Unix 秒）。
    pub exp: i64,
    /// 令牌唯一 ID（用于审计/追踪）。
    pub jti: String,
}

impl Claims {
    /// 是否拥有指定角色。
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// 是否为管理员（admin 或 superadmin）。
    pub fn is_admin(&self) -> bool {
        self.has_role("admin") || self.has_role("superadmin")
    }

    /// 是否拥有模块级访问 scope（如 `im`、`drive`）。
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// 资源级 scope，形如 `meeting:{id}`、`drive:{id}`、`doc:{id}`。
    pub fn has_resource_scope(&self, kind: &str, id: &str) -> bool {
        let expected = format!("{kind}:{id}");
        self.scopes.iter().any(|s| s == &expected)
    }

    /// 是否为游客身份（临时令牌，绑定单一资源）。
    pub fn is_guest(&self) -> bool {
        self.guest
    }

    /// 是否为 Bot 账号。
    pub fn is_bot(&self) -> bool {
        self.account_type.as_deref() == Some("bot")
    }

    /// 是否允许访问指定模块操作（人类账号恒允许；Bot 按权限矩阵）。
    pub fn allow_module(&self, module: &str, write: bool) -> bool {
        if !self.is_bot() {
            return true;
        }
        let Some(matrix) = self.bot_permissions.as_ref().and_then(|value| value.get(module)) else {
            return false;
        };
        let key = if write { "write" } else { "read" };
        matrix.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false)
    }

    /// 是否已过期（按 Unix 秒比较，调用方可注入 clock 便于测试）。
    pub fn is_expired(&self, now: i64) -> bool {
        self.exp <= now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member() -> Claims {
        Claims {
            sub: "u1".into(),
            name: "张三".into(),
            avatar: None,
            roles: vec!["member".into()],
            scopes: vec!["im".into(), "doc".into()],
            guest: false,
            account_type: None,
            bot_permissions: None,
            iss: "https://oa.test".into(),
            iat: 100,
            exp: 200,
            jti: "j1".into(),
        }
    }

    #[test]
    fn checks_roles_and_admin() {
        let c = member();
        assert!(c.has_role("member"));
        assert!(!c.has_role("admin"));
        assert!(!c.is_admin());

        let mut admin = c.clone();
        admin.roles.push("admin".into());
        assert!(admin.is_admin());

        let mut superadmin = member();
        superadmin.roles = vec!["superadmin".into()];
        assert!(superadmin.is_admin());
    }

    #[test]
    fn checks_scopes() {
        let c = member();
        assert!(c.has_scope("im"));
        assert!(!c.has_scope("task"));

        let guest = Claims {
            guest: true,
            account_type: None,
            bot_permissions: None,
            scopes: vec!["meeting:abc".into()],
            roles: vec![],
            ..member()
        };
        assert!(guest.is_guest());
        assert!(guest.has_resource_scope("meeting", "abc"));
        assert!(!guest.has_resource_scope("meeting", "other"));
        // 普通 scope 不应被资源 scope 匹配
        assert!(!guest.has_scope("meeting"));
    }

    #[test]
    fn bot_permissions() {
        let mut bot = member();
        bot.account_type = Some("bot".into());
        bot.bot_permissions = Some(serde_json::json!({
            "task": { "read": true, "write": false },
            "im": { "read": true, "write": true }
        }));
        assert!(bot.is_bot());
        assert!(bot.allow_module("task", false));
        assert!(!bot.allow_module("task", true));
        assert!(bot.allow_module("im", true));
        assert!(!bot.allow_module("doc", false));

        let human = member();
        assert!(!human.is_bot());
        assert!(human.allow_module("doc", true));
    }

    #[test]
    fn checks_expiry() {
        let c = member();
        assert!(!c.is_expired(199));
        assert!(c.is_expired(200));
        assert!(c.is_expired(201));
    }

    #[test]
    fn serde_roundtrip_and_optional_fields() {
        let c = member();
        let json = serde_json::to_value(&c).expect("serialize");
        assert!(json.get("avatar").is_none(), "None avatar must be skipped");
        assert_eq!(json["guest"], false);
        let back: Claims = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, c);

        let minimal: Claims =
            serde_json::from_str(r#"{"sub":"u","name":"n","iss":"i","iat":1,"exp":2,"jti":"j"}"#)
                .expect("deserialize minimal");
        assert!(!minimal.guest);
        assert!(minimal.roles.is_empty());
        assert!(minimal.scopes.is_empty());
    }
}
