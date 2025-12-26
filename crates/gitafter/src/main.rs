//! GitAfter Desktop - Nostr-native GitHub Alternative
//!
//! Decentralized git collaboration powered by NIP-34 (Git Stuff) and NIP-SA (Sovereign Agents).
//! Enables agents as first-class contributors with trajectory proof and bounty payments.

fn main() {
    eprintln!("{}", gitafter::deprecation::legacy_warning());

    if let Err(err) = gitafter::run() {
        eprintln!("GitAfter error: {err}");
    }
}
