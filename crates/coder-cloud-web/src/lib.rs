//! Retire private browser content when its session or visible lifetime ends.
//! The server owns session admission; this client can only remove access.

use serde::Deserialize;

pub mod resource;

/// The largest session response or initial standing document.
pub const MAX_STANDING_BYTES: usize = 16 * 1024;

/// Public session identity and workspace standing, without credentials.
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Standing {
    pub active: bool,
    pub session_id: String,
    pub account: String,
    pub workspace: Option<String>,
    pub members_epoch: Option<u64>,
    pub projection_digest: String,
    pub expires_at: u64,
}

impl Standing {
    fn current(&self, now: u64) -> bool {
        let id = |value: &str| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| byte.is_ascii_graphic())
        };
        self.active
            && self.expires_at > now
            && id(&self.session_id)
            && id(&self.account)
            && self.workspace.as_deref().is_none_or(id)
            && self.workspace.is_some() == self.members_epoch.is_some()
            && digest(&self.projection_digest)
    }

    fn same_authority(&self, other: &Self) -> bool {
        self.session_id == other.session_id
            && self.account == other.account
            && self.workspace == other.workspace
            && self.members_epoch == other.members_epoch
            && self.projection_digest == other.projection_digest
    }
}

fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// A visible mount's authority can retire but cannot resume in place.
#[derive(Default)]
pub struct Privacy {
    standing: Option<Standing>,
}

impl Privacy {
    pub fn admit(bytes: &[u8], now: u64) -> Option<Self> {
        let standing = parse(bytes)?;
        standing.current(now).then_some(Self {
            standing: Some(standing),
        })
    }

    pub fn active(&self, now: u64) -> bool {
        self.standing
            .as_ref()
            .is_some_and(|standing| standing.current(now))
    }

    pub fn refresh(&mut self, bytes: &[u8], now: u64) -> bool {
        let Some(next) = parse(bytes) else {
            self.retire();
            return false;
        };
        if !self.active(now)
            || !next.current(now)
            || !self
                .standing
                .as_ref()
                .is_some_and(|old| old.same_authority(&next))
        {
            self.retire();
            return false;
        }
        self.standing = Some(next);
        true
    }

    pub fn expires_at(&self) -> Option<u64> {
        self.standing.as_ref().map(|standing| standing.expires_at)
    }

    pub fn retire(&mut self) {
        self.standing = None;
    }
}

fn parse(bytes: &[u8]) -> Option<Standing> {
    if bytes.len() > MAX_STANDING_BYTES {
        return None;
    }
    serde_json::from_slice(bytes).ok()
}

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::start;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn standing() -> Value {
        json!({"active":true,"session_id":"session-fixture","account":"account-fixture","workspace":"workspace-fixture","members_epoch":7,"projection_digest":format!("sha256:{}", "a".repeat(64)),"expires_at":100})
    }

    fn bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }

    #[test]
    fn changed_authority_retires_and_late_responses_cannot_resume_it() {
        for field in [
            "session_id",
            "account",
            "workspace",
            "members_epoch",
            "projection_digest",
        ] {
            let initial = bytes(&standing());
            let mut privacy = Privacy::admit(&initial, 1).unwrap();
            let mut changed = standing();
            changed[field] = if field == "members_epoch" {
                json!(8)
            } else if field == "projection_digest" {
                json!(format!("sha256:{}", "b".repeat(64)))
            } else {
                json!("another-identity")
            };
            assert!(!privacy.refresh(&bytes(&changed), 2));
            assert!(!privacy.active(2));
            assert!(!privacy.refresh(&initial, 2));
            assert_eq!(privacy.expires_at(), None);
        }
    }

    #[test]
    fn expiry_revocation_and_invalid_responses_retire_the_mount() {
        let initial = bytes(&standing());
        let mut privacy = Privacy::admit(&initial, 1).unwrap();
        assert!(privacy.active(99));
        assert!(!privacy.active(100));
        assert!(!privacy.refresh(&initial, 100));
        let mut invalid = Vec::new();
        let mut inactive = standing();
        inactive["active"] = json!(false);
        invalid.push(bytes(&inactive));
        let mut extra = standing();
        extra["credential"] = json!("unknown field");
        invalid.push(bytes(&extra));
        let mut missing_epoch = standing();
        missing_epoch["members_epoch"] = Value::Null;
        invalid.push(bytes(&missing_epoch));
        let mut missing_projection = standing();
        missing_projection
            .as_object_mut()
            .unwrap()
            .remove("projection_digest");
        invalid.push(bytes(&missing_projection));
        for value in [
            "".into(),
            "sha256:short".into(),
            "a".repeat(64),
            format!("sha256:{}", "A".repeat(64)),
        ] {
            let mut bad_digest = standing();
            bad_digest["projection_digest"] = json!(value);
            invalid.push(bytes(&bad_digest));
        }
        invalid.push(vec![b' '; MAX_STANDING_BYTES + 1]);
        invalid.push(b"not JSON".to_vec());
        for response in invalid {
            let mut privacy = Privacy::admit(&initial, 1).unwrap();
            assert!(!privacy.refresh(&response, 2));
            assert!(!privacy.active(2));
        }
    }

    #[test]
    fn changed_unselected_membership_cannot_reveal_an_old_projection() {
        let mut personal = standing();
        personal["workspace"] = Value::Null;
        personal["members_epoch"] = Value::Null;
        let initial = bytes(&personal);
        let mut privacy = Privacy::admit(&initial, 1).unwrap();
        personal["projection_digest"] = json!(format!("sha256:{}", "b".repeat(64)));
        assert!(!privacy.refresh(&bytes(&personal), 2));
        assert!(!privacy.active(2));
        assert!(!privacy.refresh(&initial, 2));
    }

    #[test]
    fn current_standing_and_server_renewal_preserve_the_visible_mount() {
        let initial = bytes(&standing());
        let mut privacy = Privacy::admit(&initial, 1).unwrap();
        assert!(privacy.refresh(&initial, 2));
        let mut renewed = standing();
        renewed["expires_at"] = json!(200);
        assert!(privacy.refresh(&bytes(&renewed), 99));
        assert!(privacy.active(100));
        privacy.retire();
        assert!(!privacy.refresh(&initial, 2));
        let mut personal = standing();
        personal["workspace"] = Value::Null;
        personal["members_epoch"] = Value::Null;
        assert!(Privacy::admit(&bytes(&personal), 1).is_some());
    }
}
