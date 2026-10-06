//! Terminal shares (NIP-TERM's shares feature) as the resident host signs
//! them and a grantee reads them.
//!
//! `coder-pty` keeps each share with its terminal and enforces it. The host
//! seals every grant it issues to the grantee's key under the host key, so
//! the grantee can check who issued its terms and what they are. The sealed
//! envelope is not a credential: the host admits the grantee by its own
//! record of the share, on every operation, and a revoked or expired share
//! admits nothing whoever still holds the envelope.

use coder_pty::share::{GRANT, ShareGrant};
use coder_pty::wire::{Reason, Refusal};
use nostr::domain::Event;
use secp256k1::SecretKey;

/// Seals share grants to their grantees under the host key.
pub struct Signer(pub SecretKey);

impl coder_pty::host::Authorize for Signer {
    fn authorize(&self, grant: &ShareGrant) -> Result<serde_json::Value, Refusal> {
        let event = coder_reach::artifact::seal(
            grant,
            GRANT,
            &self.0,
            &grant.grantee,
            &grant.share,
            grant.issued_at,
            grant.expires_at,
        )
        .map_err(|_| {
            Refusal::new(
                Reason::Malformed,
                "the grantee is not a device key the share can be sealed to",
            )
        })?;
        serde_json::to_value(event)
            .map_err(|_| Refusal::new(Reason::Unavailable, "the share cannot be encoded"))
    }
}

/// Opens a share envelope as its grantee: the grant `host` signed and
/// sealed to `secret`'s key, checked. The terms say which terminal the
/// grantee may attach to and how; the host still decides every operation.
///
/// # Errors
/// Refuses an envelope another key signed or sealed, or a malformed grant.
pub fn open(
    authorization: &serde_json::Value,
    secret: &SecretKey,
    host: &str,
) -> crate::Result<ShareGrant> {
    let event: Event = serde_json::from_value(authorization.clone())
        .map_err(|_| crate::Error::Terminal(Refusal::new(Reason::Malformed, "not an envelope")))?;
    let me = coder_reach::pubkey(secret);
    let (grant, _): (ShareGrant, _) =
        coder_reach::artifact::open(&event, secret, host, &me, GRANT)?;
    grant.check().map_err(crate::Error::Terminal)?;
    if grant.grantee != me {
        return Err(crate::Error::Terminal(Refusal::new(
            Reason::IdentityMismatch,
            "the share names another grantee",
        )));
    }
    Ok(grant)
}
