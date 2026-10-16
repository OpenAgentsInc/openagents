//! NIP-LAB delivery roles after execution
//! ([NIP-LAB](../../nips/openagents/NIP-LAB.md)): independently operated
//! worker, reviewer, buyer and resolver participants, each with its own
//! [`host::Host`] and journal, exchanging signed private `3188` records.
//!
//! - [`terms`]: admitted terms, declared operator independence, rights.
//! - [`records`]: the closed submission, delivery, verification, review,
//!   dispute, cancellation, resolution and acceptance bodies.
//! - [`book`]: one participant's checked view: bounded rework, resolver
//!   adjudication under `labor-evidence-v1`, cancellation and unavailable
//!   buyers, final outcome, and the obligation's collection state.
//! - [`host`]: sealing records to the other roles, receiving them, a
//!   crash-safe journal, and funding through `pay-ledger`.
//!
//! Execution of the worker's code stays with `coder-labor` and the task
//! owner. A passing check never pays; acceptance records an obligation;
//! only a central receive adapter's funding receipt reaches the ledger.
pub mod book;
pub mod host;
pub mod records;
pub mod terms;

#[cfg(test)]
mod tests;

pub type Result<T> = std::result::Result<T, String>;

/// `sha256:<hex>` of a value's canonical (JCS) bytes.
///
/// # Errors
///
/// When the value does not encode.
pub fn digest_of(value: &impl serde::Serialize) -> Result<String> {
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    let bytes = nostr::contracts::jcs(&value).map_err(|e| e.to_string())?;
    Ok(nostr::contracts::digest_bytes(&bytes))
}

pub(crate) fn hex64(value: &str, what: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(format!("{what} is not 64 lowercase hex characters"))
    }
}
