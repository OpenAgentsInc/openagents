//! Terminal shares (NIP-TERM's shares feature): a host-signed grant that
//! lets one more device watch or drive exactly one terminal, from a first
//! readable sequence number, until an expiry.
//!
//! A share is narrower than a NIP-HOST grant. It names one terminal and one
//! grantee key; `watch` reads and `drive` also types and resizes under the
//! typist rule; and neither opens, closes, or signals a terminal or reaches
//! any other resource. The host records every share it issues and checks
//! the record on every operation, so a share is never a bearer credential:
//! the signed copy only lets the grantee read its own terms.
//!
//! This module is the wire contract and its validation. The host half keeps
//! the records and enforces them (`host::Host::share`), and the resident
//! host signs each grant through `host::Authorize`.

use serde::{Deserialize, Serialize};

use crate::ext::{Features, ext_header};
use crate::wire::{Refusal, TerminalRef, common_id, version};

/// The shares feature: share and unshare requests, and admission under a
/// share.
pub const SHARES: &str = "openagents.terminal-shares.v1";
/// The presence capability a host advertises for [`SHARES`].
pub const CAPABILITY_SHARES: &str = "term-shares";
/// `v` of a share request.
pub const SHARE: &str = "openagents.terminal-share.v1";
/// `v` of an unshare request.
pub const UNSHARE: &str = "openagents.terminal-unshare.v1";
/// `v` of a share grant, and the schema of its signed envelope.
pub const GRANT: &str = "openagents.terminal-share-grant.v1";
/// `v` of a pause or resume of every share of a terminal.
pub const PAUSE: &str = "openagents.terminal-share-pause.v1";
/// `v` of a read of a terminal's attachments and shares.
pub const VIEWERS: &str = "openagents.terminal-viewers.v1";

/// The longest a share lasts, in seconds: seven days.
pub const LIFETIME_MAX: u64 = 7 * 24 * 60 * 60;
/// The most shares one terminal holds at once.
pub const SHARES_MAX: usize = 32;
/// The longest delegation chain, counting the root share.
pub const DEPTH_MAX: usize = 8;

/// What a share lets its grantee do. `Drive` covers `Watch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareMode {
    /// Attach in `observe` mode and read what the share discloses.
    Watch,
    /// Also attach in `interact` mode, and type and resize while holding
    /// the typist role.
    Drive,
}

/// Ask the host to share one terminal with one device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShareRequest {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    /// The grantee's device key.
    pub grantee: String,
    pub mode: ShareMode,
    /// The first sequence number the grantee may read, at least 1 and at
    /// most the terminal's head plus one. Null asks for the head plus one,
    /// so nothing written before the share is disclosed.
    pub from: Option<u64>,
    /// Unix seconds; at most [`LIFETIME_MAX`] from now.
    pub expires_at: u64,
    /// The sender's own share this one narrows, or null for a share issued
    /// under the `terminal` right.
    pub parent: Option<String>,
}

impl ShareRequest {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        grantee: impl Into<String>,
        mode: ShareMode,
        expires_at: u64,
    ) -> Self {
        ShareRequest {
            v: SHARE.into(),
            requires: vec![SHARES.into()],
            request: request.into(),
            terminal,
            grantee: grantee.into(),
            mode,
            from: None,
            expires_at,
            parent: None,
        }
    }

    /// The same request with an explicit first readable sequence number.
    #[must_use]
    pub fn from_sequence(mut self, from: u64) -> Self {
        self.from = Some(from);
        self
    }

    /// The same request, delegated under the sender's own share `parent`.
    #[must_use]
    pub fn under(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            SHARE,
            &self.requires,
            &self.request,
            SHARES,
            features,
        )?;
        self.terminal.check()?;
        common_id(&self.grantee, "grantee")?;
        if self.from == Some(0) {
            return Err(Refusal::malformed("sequence numbers start at 1"));
        }
        if let Some(parent) = &self.parent {
            common_id(parent, "parent")?;
        }
        Ok(())
    }
}

/// Ask the host to end one share and every share delegated from it, or,
/// with `share` null, every share of the terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unshare {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub share: Option<String>,
}

impl Unshare {
    /// End one share.
    #[must_use]
    pub fn one(
        request: impl Into<String>,
        terminal: TerminalRef,
        share: impl Into<String>,
    ) -> Self {
        Unshare {
            v: UNSHARE.into(),
            requires: vec![SHARES.into()],
            request: request.into(),
            terminal,
            share: Some(share.into()),
        }
    }

    /// End every share of the terminal.
    #[must_use]
    pub fn all(request: impl Into<String>, terminal: TerminalRef) -> Self {
        Unshare {
            v: UNSHARE.into(),
            requires: vec![SHARES.into()],
            request: request.into(),
            terminal,
            share: None,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            UNSHARE,
            &self.requires,
            &self.request,
            SHARES,
            features,
        )?;
        self.terminal.check()?;
        if let Some(share) = &self.share {
            common_id(share, "share")?;
        }
        Ok(())
    }
}

/// Pause or resume what every share of a terminal reads. While paused,
/// no output reaches an attachment under a share; on resume each receives
/// a gap for the paused output, and nothing from before the resume is
/// readable under a share again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharePause {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub paused: bool,
}

impl SharePause {
    #[must_use]
    pub fn new(request: impl Into<String>, terminal: TerminalRef, paused: bool) -> Self {
        SharePause {
            v: PAUSE.into(),
            requires: vec![SHARES.into()],
            request: request.into(),
            terminal,
            paused,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            PAUSE,
            &self.requires,
            &self.request,
            SHARES,
            features,
        )?;
        self.terminal.check()
    }
}

/// Read who is attached to a terminal and which shares it has.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewersRead {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
}

impl ViewersRead {
    #[must_use]
    pub fn new(request: impl Into<String>, terminal: TerminalRef) -> Self {
        ViewersRead {
            v: VIEWERS.into(),
            requires: vec![SHARES.into()],
            request: request.into(),
            terminal,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            VIEWERS,
            &self.requires,
            &self.request,
            SHARES,
            features,
        )?;
        self.terminal.check()
    }
}

/// One attachment as the viewer list shows it: who, how, and under which
/// share, never what it read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewer {
    pub attachment: String,
    pub device: String,
    pub mode: crate::wire::Mode,
    /// The share that admitted it, or null for a device right.
    pub share: Option<String>,
    /// Whether it holds the typist role.
    pub typist: bool,
}

/// A terminal's attachments, its current shares, and whether sharing is
/// paused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewers {
    pub viewers: Vec<Viewer>,
    pub shares: Vec<ShareGrant>,
    pub paused: bool,
    /// The agent that holds the typist role, when one does.
    pub agent: Option<crate::ext::AgentTypist>,
}

/// The terms of one share, as the host records and signs them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShareGrant {
    pub v: String,
    /// The host's common ID for the share.
    pub share: String,
    pub terminal: TerminalRef,
    /// The device that asked for the share.
    pub issuer: String,
    pub grantee: String,
    pub mode: ShareMode,
    /// The first sequence number the grantee may read.
    pub from: u64,
    /// The terminal's share epoch when the share was issued. Ending every
    /// share of a terminal advances the epoch, and a share from an earlier
    /// epoch admits nothing.
    pub epoch: u64,
    /// The share this one narrows, or null.
    pub parent: Option<String>,
    pub issued_at: u64,
    pub expires_at: u64,
}

impl ShareGrant {
    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, GRANT)?;
        common_id(&self.share, "share")?;
        self.terminal.check()?;
        common_id(&self.issuer, "issuer")?;
        common_id(&self.grantee, "grantee")?;
        if self.issuer == self.grantee {
            return Err(Refusal::malformed("a share names another device"));
        }
        if self.from == 0 || self.epoch == 0 {
            return Err(Refusal::malformed("sequence numbers and epochs start at 1"));
        }
        if let Some(parent) = &self.parent {
            common_id(parent, "parent")?;
        }
        if self.expires_at <= self.issued_at || self.expires_at - self.issued_at > LIFETIME_MAX {
            return Err(Refusal::malformed(
                "a share expires after it is issued and within seven days",
            ));
        }
        Ok(())
    }

    /// Whether this share is no wider than `parent`: the same terminal, a
    /// mode `parent` covers, nothing readable before `parent`'s first
    /// sequence number, and no later expiry. Delegation can only narrow.
    #[must_use]
    pub fn narrows(&self, parent: &ShareGrant) -> bool {
        self.terminal == parent.terminal
            && self.mode <= parent.mode
            && self.from >= parent.from
            && self.expires_at <= parent.expires_at
            && self.issuer == parent.grantee
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> TerminalRef {
        TerminalRef {
            generation: "1".repeat(64),
            terminal: "2".repeat(64),
        }
    }

    fn grant() -> ShareGrant {
        ShareGrant {
            v: GRANT.into(),
            share: "3".repeat(64),
            terminal: reference(),
            issuer: "a".repeat(64),
            grantee: "b".repeat(64),
            mode: ShareMode::Drive,
            from: 10,
            epoch: 1,
            parent: None,
            issued_at: 1_000,
            expires_at: 2_000,
        }
    }

    #[test]
    fn requests_round_trip_and_name_the_feature() {
        let request = ShareRequest::new(
            "4".repeat(64),
            reference(),
            "b".repeat(64),
            ShareMode::Watch,
            9,
        )
        .from_sequence(3)
        .under("5".repeat(64));
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<ShareRequest>(&text).unwrap(),
            request
        );
        request.check().unwrap();
        assert_eq!(
            request.check_with(Features::NONE).unwrap_err().reason,
            crate::wire::Reason::UnsupportedFeature
        );
        assert!(request.clone().from_sequence(0).check().is_err());
        let all = Unshare::all("6".repeat(64), reference());
        all.check().unwrap();
        assert!(
            serde_json::to_string(&all)
                .unwrap()
                .contains("\"share\":null")
        );
    }

    #[test]
    fn a_delegated_grant_only_narrows() {
        let parent = grant();
        parent.check().unwrap();
        let narrow = ShareGrant {
            share: "4".repeat(64),
            issuer: parent.grantee.clone(),
            grantee: "c".repeat(64),
            mode: ShareMode::Watch,
            from: 12,
            parent: Some(parent.share.clone()),
            expires_at: 1_500,
            ..parent.clone()
        };
        assert!(narrow.narrows(&parent));
        let watching = ShareGrant {
            mode: ShareMode::Watch,
            ..parent.clone()
        };
        let driving = ShareGrant {
            mode: ShareMode::Drive,
            ..narrow.clone()
        };
        assert!(!driving.narrows(&watching));
        for wider in [
            ShareGrant {
                from: 9,
                ..narrow.clone()
            },
            ShareGrant {
                expires_at: 2_001,
                ..narrow.clone()
            },
            ShareGrant {
                issuer: "d".repeat(64),
                ..narrow.clone()
            },
        ] {
            assert!(!wider.narrows(&parent));
        }
        let long = ShareGrant {
            expires_at: 1_000 + LIFETIME_MAX + 1,
            ..grant()
        };
        assert!(long.check().is_err());
        let itself = ShareGrant {
            grantee: "a".repeat(64),
            ..grant()
        };
        assert!(itself.check().is_err());
    }
}
