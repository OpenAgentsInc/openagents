//! Threshold ECDH for encryption
//!
//! This module implements threshold Elliptic Curve Diffie-Hellman for
//! decrypting messages encrypted to the threshold group public key.

use crate::Result;

/// Perform threshold ECDH
///
/// Computes a shared secret with a peer using threshold shares.
pub fn threshold_ecdh(_peer_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
    // TODO: Implement threshold ECDH
    todo!("Implement threshold ECDH")
}
