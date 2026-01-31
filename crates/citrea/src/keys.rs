use bitcoin::secp256k1::{self, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr};

use crate::{Bytes32, Bytes64, CitreaError};

pub type SchnorrKeypair = nostr::Keypair;

pub fn derive_keypair_full(
    mnemonic: &str,
    passphrase: &str,
    account: u32,
) -> Result<SchnorrKeypair, CitreaError> {
    nostr::derive_keypair_full(mnemonic, passphrase, account)
        .map_err(|e| CitreaError::KeyDerivation(e.to_string()))
}

pub fn derive_agent_keypair(
    mnemonic: &str,
    passphrase: &str,
    agent_id: u32,
) -> Result<(SchnorrKeypair, u32), CitreaError> {
    let account = agent_id
        .checked_add(1)
        .ok_or_else(|| CitreaError::KeyDerivation("agent index overflow".to_string()))?;
    let keypair = derive_keypair_full(mnemonic, passphrase, account)?;
    Ok((keypair, account))
}

pub fn xonly_pubkey_from_secret(secret: &Bytes32) -> Result<Bytes32, CitreaError> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(secret)?;
    let (xonly, _) = sk.x_only_public_key(&secp);
    Ok(xonly.serialize())
}

pub fn sign_schnorr(secret: &Bytes32, message: &Bytes32) -> Result<Bytes64, CitreaError> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(secret)?;
    let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
    let msg = Message::from_digest_slice(message)?;
    let sig = secp.sign_schnorr_no_aux_rand(&msg, &keypair);
    Ok(sig.serialize())
}

pub fn verify_schnorr(
    pubkey: &Bytes32,
    message: &Bytes32,
    signature: &Bytes64,
) -> Result<bool, CitreaError> {
    let secp = Secp256k1::verification_only();
    let sig = schnorr::Signature::from_slice(signature)?;
    let pubkey = XOnlyPublicKey::from_slice(pubkey)?;
    let msg = Message::from_digest_slice(message)?;
    match secp.verify_schnorr(&sig, &msg, &pubkey) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}
