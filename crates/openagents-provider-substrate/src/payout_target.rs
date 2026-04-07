use bitcoin::secp256k1::{
    Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature,
};
use sha2::{Digest, Sha256};

const PROVIDER_PAYOUT_TARGET_DOMAIN: &str = "openagents:nexus-treasury-payout-target:v1";

pub fn sign_provider_payout_target_registration(
    private_key_hex: &str,
    nostr_pubkey_hex: &str,
    session_id: &str,
    challenge: &str,
    spark_address: &str,
) -> Result<String, String> {
    require_non_empty(private_key_hex, "private_key_hex")?;
    require_non_empty(nostr_pubkey_hex, "nostr_pubkey_hex")?;
    require_non_empty(session_id, "session_id")?;
    require_non_empty(challenge, "challenge")?;
    require_non_empty(spark_address, "spark_address")?;

    let private_key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("failed to decode payout-target private key: {error}"))?;
    let secret_key = SecretKey::from_slice(private_key_bytes.as_slice())
        .map_err(|error| format!("invalid payout-target private key: {error}"))?;
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, &secret_key);
    let message = Message::from_digest(payout_target_digest(
        nostr_pubkey_hex,
        session_id,
        challenge,
        spark_address,
    ));
    let signature = secp.sign_schnorr_no_aux_rand(&message, &keypair);
    Ok(hex::encode(signature.as_ref()))
}

pub fn verify_provider_payout_target_registration_signature(
    nostr_pubkey_hex: &str,
    session_id: &str,
    challenge: &str,
    spark_address: &str,
    signature_hex: &str,
) -> Result<(), String> {
    require_non_empty(nostr_pubkey_hex, "nostr_pubkey_hex")?;
    require_non_empty(session_id, "session_id")?;
    require_non_empty(challenge, "challenge")?;
    require_non_empty(spark_address, "spark_address")?;
    require_non_empty(signature_hex, "challenge_signature_hex")?;

    let pubkey_bytes = hex::decode(nostr_pubkey_hex.trim())
        .map_err(|error| format!("failed to decode payout-target nostr pubkey: {error}"))?;
    let pubkey = XOnlyPublicKey::from_slice(pubkey_bytes.as_slice())
        .map_err(|error| format!("invalid payout-target nostr pubkey: {error}"))?;
    let signature_bytes = hex::decode(signature_hex.trim())
        .map_err(|error| format!("failed to decode payout-target signature: {error}"))?;
    let signature = Signature::from_slice(signature_bytes.as_slice())
        .map_err(|error| format!("invalid payout-target signature: {error}"))?;
    let message = Message::from_digest(payout_target_digest(
        nostr_pubkey_hex,
        session_id,
        challenge,
        spark_address,
    ));
    let secp = Secp256k1::verification_only();
    secp.verify_schnorr(&signature, &message, &pubkey)
        .map_err(|error| format!("payout-target signature verification failed: {error}"))
}

fn payout_target_digest(
    nostr_pubkey_hex: &str,
    session_id: &str,
    challenge: &str,
    spark_address: &str,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROVIDER_PAYOUT_TARGET_DOMAIN.as_bytes());
    hasher.update(b":");
    hasher.update(nostr_pubkey_hex.as_bytes());
    hasher.update(b":");
    hasher.update(session_id.as_bytes());
    hasher.update(b":");
    hasher.update(challenge.as_bytes());
    hasher.update(b":");
    hasher.update(spark_address.as_bytes());
    hasher.finalize().into()
}

fn require_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} is required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        sign_provider_payout_target_registration,
        verify_provider_payout_target_registration_signature,
    };

    #[test]
    fn payout_target_signature_round_trip_is_valid() {
        let private_key_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let nostr_pubkey_hex = "4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";
        let signature = sign_provider_payout_target_registration(
            private_key_hex,
            nostr_pubkey_hex,
            "session-a",
            "challenge-a",
            "spark:alice",
        )
        .expect("signature should build");
        verify_provider_payout_target_registration_signature(
            nostr_pubkey_hex,
            "session-a",
            "challenge-a",
            "spark:alice",
            signature.as_str(),
        )
        .expect("signature should verify");
    }
}
