//! 通用校验：邮箱、密码强度、用户名。返回稳定的错误键，供前端 i18n 映射。

pub const EMAIL_MAX_LEN: usize = 254;
pub const PASSWORD_MIN_LEN: usize = 8;
pub const PASSWORD_MAX_LEN: usize = 128;
pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;

/// 简单邮箱校验：单 `@`、本地与域名均非空、域名含点且无空白字符。
pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if email.is_empty() || email.len() > EMAIL_MAX_LEN || email.chars().any(char::is_whitespace)
    {
        return false;
    }
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    if local.is_empty() || local.len() > 64 || domain.is_empty() {
        return false;
    }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    domain
        .split('.')
        .all(|label| !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// 邮箱域白名单校验：白名单为空视为不限制。
pub fn email_domain_allowed(email: &str, allowed_domains: &[String]) -> bool {
    if allowed_domains.is_empty() {
        return true;
    }
    let normalized = email.trim().to_lowercase();
    let Some((_, domain)) = normalized.split_once('@') else {
        return false;
    };
    allowed_domains
        .iter()
        .any(|allowed| allowed.trim().trim_start_matches('@').eq_ignore_ascii_case(domain))
}

/// 密码策略：长度 8 ~ 128，至少包含一个字母与一个数字。
pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.chars().count() < PASSWORD_MIN_LEN {
        return Err("password.too_short");
    }
    if password.chars().count() > PASSWORD_MAX_LEN {
        return Err("password.too_long");
    }
    if !password.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err("password.missing_letter");
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("password.missing_digit");
    }
    Ok(())
}

/// 用户名：3 ~ 32 位，字母数字开头，允许 `a-z0-9._-`。
pub fn is_valid_username(username: &str) -> bool {
    let len = username.chars().count();
    if !(USERNAME_MIN_LEN..=USERNAME_MAX_LEN).contains(&len) {
        return false;
    }
    let mut chars = username.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_emails() {
        for email in [
            "a@b.cn",
            "zhang.san@club.example.com",
            "user+tag@sub.domain.org",
        ] {
            assert!(is_valid_email(email), "{email} should be valid");
        }
    }

    #[test]
    fn rejects_invalid_emails() {
        for email in [
            "",
            "plain",
            "a@b",
            "@domain.com",
            "a@.com",
            "a@com.",
            "a@@b.com",
            "a b@c.com",
            "a@b c.com",
        ] {
            assert!(!is_valid_email(email), "{email} should be invalid");
        }
    }

    #[test]
    fn normalizes_email() {
        assert_eq!(normalize_email("  Alice@Example.COM "), "alice@example.com");
    }

    #[test]
    fn domain_whitelist() {
        let allowed = vec!["club.example.com".to_string(), "@school.edu.cn".to_string()];
        assert!(email_domain_allowed("a@club.example.com", &allowed));
        assert!(email_domain_allowed("A@School.edu.cn", &allowed));
        assert!(!email_domain_allowed("a@evil.com", &allowed));
        assert!(email_domain_allowed("a@evil.com", &[]));
    }

    #[test]
    fn password_policy() {
        assert!(validate_password("abcd1234").is_ok());
        assert_eq!(validate_password("short1").unwrap_err(), "password.too_short");
        assert_eq!(
            validate_password("abcdefgh").unwrap_err(),
            "password.missing_digit"
        );
        assert_eq!(validate_password("12345678").unwrap_err(), "password.missing_letter");
        let long = "a1".repeat(100);
        assert_eq!(validate_password(&long).unwrap_err(), "password.too_long");
    }

    #[test]
    fn username_policy() {
        assert!(is_valid_username("alice"));
        assert!(is_valid_username("a.b_c-d1"));
        assert!(is_valid_username("abc"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username(".alice"));
        assert!(!is_valid_username("Alice"));
        assert!(!is_valid_username("alice!"));
        assert!(!is_valid_username(&"a".repeat(33)));
    }
}
