//! NIP-HOST's single-use redemption rules as a pure, in-memory state
//! machine.
//!
//! The host's durable store (`coder_access`) is the authority for real
//! redemptions. This ledger states the same rules without storage, for the
//! desktop app's fake host, the phone's tests, and anything else that needs
//! to reason about a code before touching the host:
//!
//! - an unknown invitation or a wrong capability earns no answer
//!   ([`Redeem::Silent`]);
//! - a cancelled invitation is `revoked`, an expired one `expired`;
//! - the first valid redemption binds the invitation to that device; the
//!   same device may retry and gets the same result, and any other device
//!   is `forbidden`.
//!
//! Capabilities are held only as SHA-256 digests.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::code::ConnectCode;
use crate::{CLOCK_SKEW, Code, Error, Result, fail, unhex32};

/// Most invitations a host retains (NIP-HOST).
pub const MAX_INVITATIONS: usize = 64;

/// The outcome of one redemption attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Redeem {
    /// The first valid redemption: bind this device.
    Admitted,
    /// The same device again: return the result already given.
    Retry,
    /// Unknown invitation or wrong capability: send nothing.
    Silent,
    /// A signed refusal: `revoked`, `expired`, or `forbidden`.
    Refused(Code),
}

#[derive(Clone, Debug)]
struct Entry {
    capability: [u8; 32],
    issued_at: u64,
    expires_at: u64,
    cancelled: bool,
    device: Option<String>,
}

/// Outstanding invitations by ID.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    entries: BTreeMap<String, Entry>,
}

impl Ledger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a code the host is about to show.
    ///
    /// # Errors
    /// `limit_exceeded` past [`MAX_INVITATIONS`] after dropping expired
    /// entries; `forbidden` for an invitation ID already recorded.
    pub fn issue(&mut self, code: &ConnectCode, now: u64) -> Result<()> {
        self.entries
            .retain(|_, entry| entry.device.is_some() || now < entry.expires_at);
        if self.entries.contains_key(&code.invitation()) {
            return fail(Code::Forbidden, "invitation ID already recorded");
        }
        if self.entries.len() >= MAX_INVITATIONS {
            return fail(Code::LimitExceeded, "too many invitations");
        }
        self.entries.insert(
            code.invitation(),
            Entry {
                capability: digest(&code.capability())?,
                issued_at: code.issued_at(),
                expires_at: code.expires_at(),
                cancelled: false,
                device: None,
            },
        );
        Ok(())
    }

    /// Try to redeem `invitation` with `capability` for `device` at `now`.
    /// The request's own `issued_at` must lie inside the invitation's window.
    #[must_use]
    pub fn redeem(
        &mut self,
        invitation: &str,
        capability: &str,
        device: &str,
        request_issued_at: u64,
        now: u64,
    ) -> Redeem {
        let Some(entry) = self.entries.get_mut(invitation) else {
            return Redeem::Silent;
        };
        match digest(capability) {
            Ok(presented) if presented == entry.capability => {}
            _ => return Redeem::Silent,
        }
        if let Some(bound) = &entry.device {
            // A redeemed invitation answers only its device, even after
            // expiry, so a retry after a lost reply still lands.
            return if bound == device {
                Redeem::Retry
            } else {
                Redeem::Refused(Code::Forbidden)
            };
        }
        if entry.cancelled {
            return Redeem::Refused(Code::Revoked);
        }
        if now >= entry.expires_at {
            return Redeem::Refused(Code::Expired);
        }
        if request_issued_at < entry.issued_at.saturating_sub(CLOCK_SKEW)
            || request_issued_at >= entry.expires_at
        {
            return Redeem::Refused(Code::Forbidden);
        }
        entry.device = Some(device.to_owned());
        Redeem::Admitted
    }

    /// Cancel an unredeemed invitation. Returns whether one was cancelled.
    pub fn cancel(&mut self, invitation: &str) -> bool {
        match self.entries.get_mut(invitation) {
            Some(entry) if entry.device.is_none() && !entry.cancelled => {
                entry.cancelled = true;
                true
            }
            _ => false,
        }
    }

    /// Cancel every unredeemed invitation, as after a pairing or when the
    /// code window hides. Returns how many were cancelled.
    pub fn cancel_all(&mut self) -> usize {
        let mut count = 0;
        for entry in self.entries.values_mut() {
            if entry.device.is_none() && !entry.cancelled {
                entry.cancelled = true;
                count += 1;
            }
        }
        count
    }

    /// Invitations that can still be redeemed at `now`.
    #[must_use]
    pub fn outstanding(&self, now: u64) -> usize {
        self.entries
            .values()
            .filter(|e| e.device.is_none() && !e.cancelled && now < e.expires_at)
            .count()
    }
}

fn digest(capability: &str) -> Result<[u8; 32]> {
    let bytes = unhex32(capability).map_err(|_| Error::new(Code::Malformed, "capability"))?;
    Ok(Sha256::digest(bytes).into())
}
