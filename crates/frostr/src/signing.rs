//! FROST signing protocol
//!
//! This module implements the FROST threshold signing protocol for Schnorr signatures.

use crate::{Error, Result, keygen::FrostShare};
use frost_secp256k1::{Identifier, SigningPackage, VerifyingKey, aggregate, round1, round2};
use std::collections::BTreeMap;

/// Round 1: Nonce commitment generation
///
/// Each signer generates nonces and commitments for a signing round.
/// The nonces must be kept secret, and commitments are shared with all participants.
///
/// # Examples
///
/// ```
/// use frostr::keygen::generate_key_shares;
/// use frostr::signing::round1_commit;
///
/// // Generate 2-of-3 shares
/// let shares = generate_key_shares(2, 3).unwrap();
///
/// // Each signer generates commitments
/// let (nonces1, commitments1) = round1_commit(&shares[0]);
/// let (nonces2, commitments2) = round1_commit(&shares[1]);
///
/// // Nonces are kept secret, commitments are shared
/// // Note: nonces and commitments types are from frost_secp256k1 crate
/// ```
pub fn round1_commit(
    frost_share: &FrostShare,
) -> (round1::SigningNonces, round1::SigningCommitments) {
    let mut rng = rand::thread_rng();
    let signing_share = frost_share.key_package.signing_share();
    round1::commit(signing_share, &mut rng)
}

/// Round 2: Partial signature generation
///
/// Each signer creates a partial signature using their share, nonces,
/// and the signing package from the coordinator.
pub fn round2_sign(
    frost_share: &FrostShare,
    signing_nonces: &round1::SigningNonces,
    signing_package: &SigningPackage,
) -> Result<round2::SignatureShare> {
    round2::sign(signing_package, signing_nonces, &frost_share.key_package)
        .map_err(|e| Error::FrostError(format!("Round 2 signing failed: {:?}", e)))
}

/// Aggregate partial signatures into a final Schnorr signature
///
/// The coordinator collects all signature shares and aggregates them
/// into a complete signature that can be verified with the group public key.
///
/// # Examples
///
/// ```no_run
/// use frostr::keygen::generate_key_shares;
/// use frostr::signing::{round1_commit, round2_sign, aggregate_signatures};
/// use frost_secp256k1::SigningPackage;
/// use std::collections::BTreeMap;
///
/// # fn example() -> Result<(), frostr::Error> {
/// let shares = generate_key_shares(2, 3)?;
/// let message = b"Sign this message";
///
/// // Round 1: Generate commitments
/// let (nonces1, commitments1) = round1_commit(&shares[0]);
/// let (nonces2, commitments2) = round1_commit(&shares[1]);
///
/// let mut signing_commitments = BTreeMap::new();
/// signing_commitments.insert(*shares[0].key_package.identifier(), commitments1);
/// signing_commitments.insert(*shares[1].key_package.identifier(), commitments2);
///
/// let signing_package = SigningPackage::new(signing_commitments, message);
///
/// // Round 2: Generate signature shares
/// let sig_share1 = round2_sign(&shares[0], &nonces1, &signing_package)?;
/// let sig_share2 = round2_sign(&shares[1], &nonces2, &signing_package)?;
///
/// let mut signature_shares = BTreeMap::new();
/// signature_shares.insert(*shares[0].key_package.identifier(), sig_share1);
/// signature_shares.insert(*shares[1].key_package.identifier(), sig_share2);
///
/// // Aggregate into final signature
/// let signature = aggregate_signatures(&signing_package, &signature_shares, &shares[0])?;
/// # Ok(())
/// # }
/// ```
pub fn aggregate_signatures(
    signing_package: &SigningPackage,
    signature_shares: &BTreeMap<Identifier, round2::SignatureShare>,
    frost_share: &FrostShare,
) -> Result<frost_secp256k1::Signature> {
    aggregate(
        signing_package,
        signature_shares,
        &frost_share.public_key_package,
    )
    .map_err(|e| Error::FrostError(format!("Signature aggregation failed: {:?}", e)))
}

/// Verify a signature against the group public key
pub fn verify_signature(
    message: &[u8],
    signature: &frost_secp256k1::Signature,
    verifying_key: &VerifyingKey,
) -> Result<()> {
    verifying_key
        .verify(message, signature)
        .map_err(|e| Error::FrostError(format!("Signature verification failed: {:?}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keygen::generate_key_shares;

    #[test]
    fn test_frost_signing_2_of_3() {
        // Generate 2-of-3 shares
        let shares = generate_key_shares(2, 3).unwrap();
        assert_eq!(shares.len(), 3);

        let message = b"Hello, FROST!";

        // Round 1: Generate commitments from first 2 signers
        let (nonces1, commitments1) = round1_commit(&shares[0]);
        let (nonces2, commitments2) = round1_commit(&shares[1]);

        // Coordinator creates signing package
        let mut signing_commitments = BTreeMap::new();
        signing_commitments.insert(*shares[0].key_package.identifier(), commitments1);
        signing_commitments.insert(*shares[1].key_package.identifier(), commitments2);

        let signing_package = SigningPackage::new(signing_commitments, message);

        // Round 2: Generate partial signatures
        let sig_share1 = round2_sign(&shares[0], &nonces1, &signing_package).unwrap();
        let sig_share2 = round2_sign(&shares[1], &nonces2, &signing_package).unwrap();

        // Aggregate partial signatures
        let mut signature_shares = BTreeMap::new();
        signature_shares.insert(*shares[0].key_package.identifier(), sig_share1);
        signature_shares.insert(*shares[1].key_package.identifier(), sig_share2);

        let signature =
            aggregate_signatures(&signing_package, &signature_shares, &shares[0]).unwrap();

        // Verify signature
        let group_pubkey = shares[0].public_key_package.verifying_key();
        verify_signature(message, &signature, group_pubkey).unwrap();
    }

    #[test]
    fn test_frost_signing_3_of_5() {
        // Generate 3-of-5 shares
        let shares = generate_key_shares(3, 5).unwrap();
        assert_eq!(shares.len(), 5);

        let message = b"FROST threshold signatures";

        // Round 1: Use first 3 signers
        let (nonces0, commitments0) = round1_commit(&shares[0]);
        let (nonces1, commitments1) = round1_commit(&shares[1]);
        let (nonces2, commitments2) = round1_commit(&shares[2]);

        let mut signing_commitments = BTreeMap::new();
        signing_commitments.insert(*shares[0].key_package.identifier(), commitments0);
        signing_commitments.insert(*shares[1].key_package.identifier(), commitments1);
        signing_commitments.insert(*shares[2].key_package.identifier(), commitments2);

        let signing_package = SigningPackage::new(signing_commitments, message);

        // Round 2: Partial signatures
        let sig_share0 = round2_sign(&shares[0], &nonces0, &signing_package).unwrap();
        let sig_share1 = round2_sign(&shares[1], &nonces1, &signing_package).unwrap();
        let sig_share2 = round2_sign(&shares[2], &nonces2, &signing_package).unwrap();

        let mut signature_shares = BTreeMap::new();
        signature_shares.insert(*shares[0].key_package.identifier(), sig_share0);
        signature_shares.insert(*shares[1].key_package.identifier(), sig_share1);
        signature_shares.insert(*shares[2].key_package.identifier(), sig_share2);

        let signature =
            aggregate_signatures(&signing_package, &signature_shares, &shares[0]).unwrap();

        // Verify
        let group_pubkey = shares[0].public_key_package.verifying_key();
        verify_signature(message, &signature, group_pubkey).unwrap();
    }

    #[test]
    fn test_frost_signing_different_signers() {
        // Generate 2-of-3 shares
        let shares = generate_key_shares(2, 3).unwrap();

        let message = b"Different signers";

        // First signature: signers 0 and 1
        let (nonces0, commitments0) = round1_commit(&shares[0]);
        let (nonces1, commitments1) = round1_commit(&shares[1]);

        let mut signing_commitments1 = BTreeMap::new();
        signing_commitments1.insert(*shares[0].key_package.identifier(), commitments0);
        signing_commitments1.insert(*shares[1].key_package.identifier(), commitments1);

        let signing_package1 = SigningPackage::new(signing_commitments1, message);

        let sig_share0 = round2_sign(&shares[0], &nonces0, &signing_package1).unwrap();
        let sig_share1 = round2_sign(&shares[1], &nonces1, &signing_package1).unwrap();

        let mut signature_shares1 = BTreeMap::new();
        signature_shares1.insert(*shares[0].key_package.identifier(), sig_share0);
        signature_shares1.insert(*shares[1].key_package.identifier(), sig_share1);

        let signature1 =
            aggregate_signatures(&signing_package1, &signature_shares1, &shares[0]).unwrap();

        // Second signature: signers 1 and 2 (different combination)
        let (nonces1_2, commitments1_2) = round1_commit(&shares[1]);
        let (nonces2, commitments2) = round1_commit(&shares[2]);

        let mut signing_commitments2 = BTreeMap::new();
        signing_commitments2.insert(*shares[1].key_package.identifier(), commitments1_2);
        signing_commitments2.insert(*shares[2].key_package.identifier(), commitments2);

        let signing_package2 = SigningPackage::new(signing_commitments2, message);

        let sig_share1_2 = round2_sign(&shares[1], &nonces1_2, &signing_package2).unwrap();
        let sig_share2 = round2_sign(&shares[2], &nonces2, &signing_package2).unwrap();

        let mut signature_shares2 = BTreeMap::new();
        signature_shares2.insert(*shares[1].key_package.identifier(), sig_share1_2);
        signature_shares2.insert(*shares[2].key_package.identifier(), sig_share2);

        let signature2 =
            aggregate_signatures(&signing_package2, &signature_shares2, &shares[1]).unwrap();

        // Both signatures should verify with the same group public key
        let group_pubkey = shares[0].public_key_package.verifying_key();
        verify_signature(message, &signature1, group_pubkey).unwrap();
        verify_signature(message, &signature2, group_pubkey).unwrap();
    }

    #[test]
    fn test_frost_invalid_signature() {
        let shares = generate_key_shares(2, 3).unwrap();
        let message = b"Valid message";
        let wrong_message = b"Wrong message";

        // Create valid signature for one message
        let (nonces0, commitments0) = round1_commit(&shares[0]);
        let (nonces1, commitments1) = round1_commit(&shares[1]);

        let mut signing_commitments = BTreeMap::new();
        signing_commitments.insert(*shares[0].key_package.identifier(), commitments0);
        signing_commitments.insert(*shares[1].key_package.identifier(), commitments1);

        let signing_package = SigningPackage::new(signing_commitments, message);

        let sig_share0 = round2_sign(&shares[0], &nonces0, &signing_package).unwrap();
        let sig_share1 = round2_sign(&shares[1], &nonces1, &signing_package).unwrap();

        let mut signature_shares = BTreeMap::new();
        signature_shares.insert(*shares[0].key_package.identifier(), sig_share0);
        signature_shares.insert(*shares[1].key_package.identifier(), sig_share1);

        let signature =
            aggregate_signatures(&signing_package, &signature_shares, &shares[0]).unwrap();

        // Verify with correct message succeeds
        let group_pubkey = shares[0].public_key_package.verifying_key();
        verify_signature(message, &signature, group_pubkey).unwrap();

        // Verify with wrong message fails
        assert!(verify_signature(wrong_message, &signature, group_pubkey).is_err());
    }
}
