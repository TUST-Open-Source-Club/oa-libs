use serde::{Deserialize, Serialize};

/// Access Token 载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub guest: bool,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

impl Claims {
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn is_admin(&self) -> bool {
        self.has_role("admin") || self.has_role("superadmin")
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// 资源级 scope，形如 `meeting:{id}`、`drive:{id}`、`doc:{id}`。
    pub fn has_resource_scope(&self, kind: &str, id: &str) -> bool {
        let expected = format!("{kind}:{id}");
        self.scopes.iter().any(|s| s == &expected)
    }

    pub fn is_guest(&self) -> bool {
        self.guest
    }

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
