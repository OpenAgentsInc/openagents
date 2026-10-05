//! The closed right set. No right implies another.
use crate::{Code, Error, Result, fail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Right {
    /// Read sessions, tasks, and files under the host's disclosure policy.
    Observe,
    /// Create, steer, and cancel tasks and sessions.
    Operate,
    /// Open and drive terminals.
    Terminal,
    /// Write reviews and diffs.
    Review,
    /// List enrolled devices and pending invitations.
    AccessRead,
    /// Invite, approve, and revoke devices, only within held rights.
    AccessAdmin,
    /// Join the host's world instances over a direct channel.
    World,
}
impl Right {
    pub const ALL: [Right; 7] = [
        Right::Observe,
        Right::Operate,
        Right::Terminal,
        Right::Review,
        Right::AccessRead,
        Right::AccessAdmin,
        Right::World,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Operate => "operate",
            Self::Terminal => "terminal",
            Self::Review => "review",
            Self::AccessRead => "access_read",
            Self::AccessAdmin => "access_admin",
            Self::World => "world",
        }
    }
    pub fn parse(text: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|right| right.as_str() == text)
            .ok_or_else(|| Error::new(Code::Malformed, "unknown access right"))
    }
}

/// A nonempty, duplicate-free set in the canonical order of [`Right::ALL`].
/// The wire form must already be canonical; anything else is malformed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Right>", into = "Vec<Right>")]
pub struct Rights(Vec<Right>);
impl TryFrom<Vec<Right>> for Rights {
    type Error = Error;
    fn try_from(rights: Vec<Right>) -> Result<Self> {
        if rights.is_empty() || !rights.windows(2).all(|pair| pair[0] < pair[1]) {
            return fail(
                Code::Malformed,
                "rights must be a nonempty duplicate-free list in canonical order",
            );
        }
        Ok(Self(rights))
    }
}
impl From<Rights> for Vec<Right> {
    fn from(rights: Rights) -> Self {
        rights.0
    }
}
impl Rights {
    /// Build a set from any order. Duplicates collapse; an empty set refuses.
    pub fn new(rights: impl IntoIterator<Item = Right>) -> Result<Self> {
        let mut rights: Vec<_> = rights.into_iter().collect();
        rights.sort();
        rights.dedup();
        Self::try_from(rights)
    }
    /// The standard device grant. It excludes both access rights.
    pub fn standard() -> Self {
        Self(vec![
            Right::Observe,
            Right::Operate,
            Right::Terminal,
            Right::Review,
        ])
    }
    /// Every right. Only the locally established owner holds this implicitly.
    pub fn all() -> Self {
        Self(Right::ALL.to_vec())
    }
    /// What every pairing grants the owner's phone: a connect code (QR or
    /// copied), a nearby approval, and `openagents connect invite`. It is
    /// every host right, because the phone is the owner's and does
    /// everything the owner does on the computer: watch, run, open
    /// terminals, review, and see and manage who has access. Nothing on
    /// either screen narrows it; the owner narrows a phone by revoking it.
    /// It leaves out `world`, which clients built before that right existed
    /// cannot parse; the owner grants `world` explicitly.
    pub fn pairing() -> Self {
        Self(Right::ALL[..6].to_vec())
    }
    /// Parse `standard`, `admin`, `all`, or a comma-separated list of rights.
    pub fn parse_list(text: &str) -> Result<Self> {
        match text {
            "standard" => Ok(Self::standard()),
            "admin" => Ok(Self(vec![Right::AccessRead, Right::AccessAdmin])),
            "all" => Ok(Self::all()),
            _ => Self::new(
                text.split(',')
                    .map(|part| Right::parse(part.trim()))
                    .collect::<Result<Vec<_>>>()?,
            ),
        }
    }
    pub fn contains(&self, right: Right) -> bool {
        self.0.contains(&right)
    }
    pub fn iter(&self) -> impl Iterator<Item = Right> + '_ {
        self.0.iter().copied()
    }
    /// The first right in `self` that `held` lacks, if any.
    pub fn first_missing(&self, held: &Rights) -> Option<Right> {
        self.iter().find(|right| !held.contains(*right))
    }
    /// Refuse with the missing right named.
    pub fn require(&self, right: Right) -> Result<()> {
        if self.contains(right) {
            Ok(())
        } else {
            Err(Error::missing(right))
        }
    }
    pub fn to_list(&self) -> String {
        self.iter().map(Right::as_str).collect::<Vec<_>>().join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rights_are_closed_canonical_and_independent() {
        let standard = Rights::standard();
        assert!(!standard.contains(Right::AccessRead));
        assert!(!standard.contains(Right::AccessAdmin));
        let admin = Rights::parse_list("admin").unwrap();
        assert!(!admin.contains(Right::Observe));
        assert_eq!(
            admin.require(Right::Operate).unwrap_err().missing,
            Some(Right::Operate)
        );
        assert!(!Rights::pairing().contains(Right::World));
        assert!(Rights::all().contains(Right::World));
        assert_eq!(
            Rights::parse_list("world,observe").unwrap().to_list(),
            "observe,world"
        );
        assert_eq!(
            Rights::parse_list("operate,observe").unwrap().to_list(),
            "observe,operate"
        );
        for bad in [
            "[]",
            "[\"operate\",\"observe\"]",
            "[\"observe\",\"observe\"]",
            "[\"root\"]",
        ] {
            assert!(serde_json::from_str::<Rights>(bad).is_err(), "{bad}");
        }
        assert_eq!(
            Rights::parse_list("observe").unwrap().first_missing(&admin),
            Some(Right::Observe)
        );
    }
}
