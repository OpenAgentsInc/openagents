//! Owner grants: the `screen` lease is admitted only under one.
//!
//! A grant is a file under `grants/` in the lease root. The command that
//! writes it asks the owner on an interactive terminal and refuses in an
//! agent environment ([`crate::grant_refusal`]). On one Unix account this
//! stops accidents, not a process set on forging a grant: such a process
//! can write the file itself. A stronger guarantee needs a separate account
//! or a virtual machine.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The grant file's schema.
pub const GRANT_SCHEMA: &str = "openagents.lease.grant.v1";
/// How long a grant lasts when the owner names no duration.
pub const DEFAULT_GRANT: Duration = Duration::from_secs(60 * 60);

/// An owner's grant of a resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The schema, [`GRANT_SCHEMA`].
    pub schema: String,
    /// The resource granted, such as `screen`.
    pub resource: String,
    /// When the owner granted it, in Unix milliseconds.
    pub granted_at_ms: u64,
    /// When it ends, in Unix milliseconds.
    pub expires_at_ms: u64,
    /// The one session it admits, or every session when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

impl Grant {
    /// A grant of `resource` from now for `duration`, to `to` or to every
    /// session.
    #[must_use]
    pub fn new(resource: &crate::Resource, duration: Duration, to: Option<String>) -> Grant {
        let now = crate::now_ms();
        Grant {
            schema: GRANT_SCHEMA.to_owned(),
            resource: resource.to_string(),
            granted_at_ms: now,
            expires_at_ms: now
                .saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)),
            to,
        }
    }

    /// Whether it admits `session` at `now_ms`.
    #[must_use]
    pub fn admits(&self, session: &str, now_ms: u64) -> bool {
        now_ms < self.expires_at_ms && self.to.as_deref().is_none_or(|to| to == session)
    }
}

/// Parses a grant duration such as `90s`, `30m`, `1h`, or `2d`.
///
/// # Errors
/// A sentence when the text is not a positive number with a unit.
pub fn parse_duration(text: &str) -> Result<Duration, String> {
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: u64 = number
        .parse()
        .map_err(|_| format!("`{text}` is not a duration such as 30m, 1h, or 2d"))?;
    let seconds = match unit {
        "s" => 1,
        "m" | "" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => return Err(format!("`{text}` is not a duration such as 30m, 1h, or 2d")),
    };
    if number == 0 {
        return Err("a grant must last longer than zero".to_owned());
    }
    Ok(Duration::from_secs(number.saturating_mul(seconds)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse_with_units() {
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172_800));
        for bad in ["", "h", "0h", "1w", "-1h"] {
            assert!(parse_duration(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_grant_admits_its_session_until_it_expires() {
        let grant = Grant::new(
            &crate::Resource::Screen,
            Duration::from_secs(60),
            Some("seat-1".into()),
        );
        let now = grant.granted_at_ms;
        assert!(grant.admits("seat-1", now));
        assert!(!grant.admits("seat-2", now));
        assert!(!grant.admits("seat-1", grant.expires_at_ms));
        let open = Grant::new(&crate::Resource::Screen, Duration::from_secs(60), None);
        assert!(open.admits("anyone", now));
    }
}
