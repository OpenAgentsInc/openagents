//! The GitHub people list: who a deployment lets in, and who is a site admin.
//!
//! The account service (`accounts.invite_only` in the gateway config) and
//! the web server (`invite_only` in its Cloud config) both read this list.
//! With it set and closed, a GitHub sign-in from anyone not on it is refused
//! before any account is made or session issued (`403 invite_only`), and
//! the web server shows "Login is invite-only for now" without setting a
//! cookie. Absent means anyone with a GitHub account may sign in.
//!
//! With `"open": true` anyone with a GitHub account may sign in, and the
//! list only marks site admins (`admin`). Since 2026-10-15 openagents.com
//! runs open.
//!
//! ```json
//! "invite_only": {"open": true, "github": [{"id": 14167547, "login": "AtlantisPleb", "admin": true}]}
//! ```
//!
//! An entry with an `id` matches that GitHub user id only (ids never
//! change; logins can be renamed and then taken by someone else). An entry
//! with only a `login` matches it case-insensitively. `admin` marks the
//! person as a site admin: `GET /v1/account` answers `"admin": true`.
//!
//! `admins` names site admins by account instead of by GitHub (#12003): an
//! account id (`acct_…`), or a Nostr key (`npub1…` or 64 hex digits) that
//! signs the account in. An account named there is admin with or without a
//! linked GitHub identity; the GitHub entries stay as a fallback.
//!
//! ```json
//! "invite_only": {"open": true, "admins": ["acct_0123456789abcdef"], "github": [...]}
//! ```

use serde::{Deserialize, Serialize};

/// The people a deployment lets sign in.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InviteOnly {
    /// Anyone with a GitHub account may sign in; the list only marks
    /// site admins.
    #[serde(default)]
    pub open: bool,
    /// GitHub accounts that may sign in.
    #[serde(default)]
    pub github: Vec<Invited>,
    /// Site admins by account id or Nostr key, matched without GitHub.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admins: Vec<String>,
}

/// The Nostr key an `admins` entry names, as lowercase hex, when it names
/// one (`npub1…` or 64 hex digits).
fn nostr_hex(entry: &str) -> Option<String> {
    let entry = entry.trim();
    if entry.starts_with("npub1") {
        let key = nostr::nip19::decode_npub(entry).ok()?;
        return Some(key.iter().map(|byte| format!("{byte:02x}")).collect());
    }
    (entry.len() == 64 && entry.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| entry.to_ascii_lowercase())
}

/// Whether `entry` is an account id: `acct_` and hex digits.
fn account_id(entry: &str) -> bool {
    entry.strip_prefix("acct_").is_some_and(|rest| {
        !rest.is_empty() && rest.len() <= 64 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
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
        for entry in &self.admins {
            if !account_id(entry.trim()) && nostr_hex(entry).is_none() {
                return Err(
                    "every invite_only.admins entry is an account id (`acct_…`) or a Nostr key (`npub1…`)"
                        .into(),
                );
            }
        }
        Ok(())
    }

    /// The entry for the GitHub user `id` / `login`, if invited.
    #[must_use]
    pub fn find(&self, id: u64, login: &str) -> Option<&Invited> {
        self.github.iter().find(|entry| entry.matches(id, login))
    }

    /// Whether the GitHub user `id` / `login` may sign in: everyone when
    /// the list is open.
    #[must_use]
    pub fn allows(&self, id: u64, login: &str) -> bool {
        self.open || self.find(id, login).is_some()
    }

    /// Whether sign-in is actually limited to the list (not open).
    #[must_use]
    pub fn closed(&self) -> bool {
        !self.open
    }

    /// Whether the GitHub user `id` / `login` is a site admin.
    #[must_use]
    pub fn admin(&self, id: u64, login: &str) -> bool {
        self.find(id, login).is_some_and(|entry| entry.admin)
    }

    /// Whether `admins` names the account `account`, by its id or by a
    /// Nostr key among its `principals` (`nostr:<hex>`).
    #[must_use]
    pub fn admin_account(&self, account: &str, principals: &[String]) -> bool {
        self.admins.iter().any(|entry| {
            let entry = entry.trim();
            if account_id(entry) {
                return entry == account;
            }
            nostr_hex(entry).is_some_and(|hex| {
                principals.iter().any(|principal| {
                    principal
                        .strip_prefix("nostr:")
                        .is_some_and(|key| key.eq_ignore_ascii_case(&hex))
                })
            })
        })
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
    fn open_lets_a_stranger_in_and_an_admin_stays_admin() {
        let open: InviteOnly = serde_json::from_value(serde_json::json!({"open": true, "github": [
            {"id": 14167547, "login": "AtlantisPleb", "admin": true}
        ]}))
        .unwrap();
        assert!(!open.closed());
        assert!(open.allows(1, "stranger"));
        assert!(allowed(Some(&open), 1, "stranger"));
        assert!(!open.admin(1, "stranger"), "a stranger is not admin");
        assert!(open.admin(14167547, "AtlantisPleb"));
        assert!(open.find(14167547, "renamed").is_some());
        assert!(list().closed(), "absent `open` is closed");
    }

    #[test]
    fn admins_name_an_account_by_id_or_nostr_key_without_github() {
        let hex = "ab".repeat(32);
        let mut key = [0u8; 32];
        key.fill(0xab);
        let npub = nostr::nip19::encode_npub(&key);
        let list: InviteOnly = serde_json::from_value(serde_json::json!({
            "open": true,
            "admins": ["acct_0123456789abcdef", npub],
        }))
        .unwrap();
        assert!(list.validate().is_ok());
        assert!(list.admin_account("acct_0123456789abcdef", &[]));
        assert!(!list.admin_account("acct_ffffffffffffffff", &[]));
        assert!(list.admin_account("acct_ffffffffffffffff", &[format!("nostr:{hex}")]));
        assert!(!list.admin_account(
            "acct_ffffffffffffffff",
            &[format!("nostr:{}", "cd".repeat(32))]
        ));
        let by_hex: InviteOnly =
            serde_json::from_value(serde_json::json!({"admins": [hex.to_uppercase()]})).unwrap();
        assert!(by_hex.validate().is_ok());
        assert!(by_hex.admin_account("acct_1", &[format!("nostr:{hex}")]));
        let bad: InviteOnly =
            serde_json::from_value(serde_json::json!({"admins": ["AtlantisPleb"]})).unwrap();
        assert!(bad.validate().is_err());
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
