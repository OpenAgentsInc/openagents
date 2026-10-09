//! Invite-only sign-in: the GitHub people a deployment lets in.
//!
//! The account service (`accounts.invite_only` in the gateway config) and
//! the web server (`invite_only` in its Cloud config) both read this list.
//! With it set, a GitHub sign-in from anyone not on it is refused before
//! any account is made or session issued (`403 invite_only`), and the web
//! server shows "Sign-in is invite-only for now" without setting a cookie.
//! Absent means anyone with a GitHub account may sign in.
//!
//! ```json
//! "invite_only": {"github": [{"id": 14167547, "login": "AtlantisPleb", "admin": true}]}
//! ```
//!
//! An entry with an `id` matches that GitHub user id only (ids never
//! change; logins can be renamed and then taken by someone else). An entry
//! with only a `login` matches it case-insensitively. `admin` marks the
//! person as a site admin: `GET /v1/account` answers `"admin": true`.

use serde::{Deserialize, Serialize};

/// The people a deployment lets sign in.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InviteOnly {
    /// GitHub accounts that may sign in.
    #[serde(default)]
    pub github: Vec<Invited>,
}

/// One invited GitHub account.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Invited {
    /// GitHub's numeric user id. When set, only this id matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// The GitHub login, matched case-insensitively when there is no id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    /// A site admin.
    #[serde(default)]
    pub admin: bool,
}

impl Invited {
    fn matches(&self, id: u64, login: &str) -> bool {
        match (self.id, self.login.as_deref()) {
            (Some(invited), _) => invited == id,
            (None, Some(invited)) => invited.eq_ignore_ascii_case(login),
            (None, None) => false,
        }
    }
}

impl InviteOnly {
    /// Every entry names an id or a login.
    pub fn validate(&self) -> Result<(), String> {
        for entry in &self.github {
            if entry.id.is_none() && entry.login.as_deref().is_none_or(str::is_empty) {
                return Err("every invite_only.github entry needs an `id` or a `login`".into());
            }
        }
        Ok(())
    }

    /// The entry for the GitHub user `id` / `login`, if invited.
    #[must_use]
    pub fn find(&self, id: u64, login: &str) -> Option<&Invited> {
        self.github.iter().find(|entry| entry.matches(id, login))
    }

    /// Whether the GitHub user `id` / `login` may sign in.
    #[must_use]
    pub fn allows(&self, id: u64, login: &str) -> bool {
        self.find(id, login).is_some()
    }

    /// Whether the GitHub user `id` / `login` is a site admin.
    #[must_use]
    pub fn admin(&self, id: u64, login: &str) -> bool {
        self.find(id, login).is_some_and(|entry| entry.admin)
    }
}

/// Whether `invite` (absent: everyone) lets the GitHub user in.
#[must_use]
pub fn allowed(invite: Option<&InviteOnly>, id: u64, login: &str) -> bool {
    invite.is_none_or(|list| list.allows(id, login))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> InviteOnly {
        serde_json::from_value(serde_json::json!({"github": [
            {"id": 14167547, "login": "AtlantisPleb", "admin": true},
            {"login": "Invited-Login"}
        ]}))
        .unwrap()
    }

    #[test]
    fn an_id_entry_matches_only_that_id() {
        let list = list();
        assert!(list.allows(14167547, "AtlantisPleb"));
        assert!(list.allows(14167547, "renamed"));
        assert!(
            !list.allows(1, "AtlantisPleb"),
            "a login taken over is not the owner"
        );
        assert!(list.admin(14167547, "renamed"));
    }

    #[test]
    fn a_login_entry_matches_case_insensitively_and_is_not_admin() {
        let list = list();
        assert!(list.allows(99, "invited-login"));
        assert!(!list.admin(99, "invited-login"));
        assert!(!list.allows(99, "someone-else"));
    }

    #[test]
    fn absent_lets_everyone_in_and_empty_lets_no_one_in() {
        assert!(allowed(None, 1, "anyone"));
        assert!(!allowed(Some(&InviteOnly::default()), 1, "anyone"));
    }

    #[test]
    fn an_entry_needs_an_id_or_a_login() {
        let bad: InviteOnly =
            serde_json::from_value(serde_json::json!({"github": [{"admin": true}]})).unwrap();
        assert!(bad.validate().is_err());
        assert!(list().validate().is_ok());
        assert!(
            serde_json::from_value::<InviteOnly>(serde_json::json!({"github": [{"name": "x"}]}))
                .is_err()
        );
    }
}
