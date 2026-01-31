use sha3::{Digest, Keccak256};

use crate::{Address, Bytes32, CitreaError};

pub fn keccak256(data: &[u8]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

pub fn format_address(address: &Address) -> String {
    format!("0x{}", hex::encode(address))
}

pub fn address_from_uncompressed_pubkey(pubkey: &[u8]) -> Result<Address, CitreaError> {
    let (payload, expected_len) = match pubkey.len() {
        65 if pubkey[0] == 0x04 => (&pubkey[1..], 64),
        64 => (pubkey, 64),
        other => {
            return Err(CitreaError::InvalidLength {
                expected: 64,
                actual: other,
            });
        }
    };

    if payload.len() != expected_len {
        return Err(CitreaError::InvalidLength {
            expected: expected_len,
            actual: payload.len(),
        });
    }

    let hash = keccak256(payload);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    Ok(address)
}

pub fn eoa_address_from_secret(secret: &Bytes32) -> Result<Address, CitreaError> {
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let sk = bitcoin::secp256k1::SecretKey::from_slice(secret)?;
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let uncompressed = pk.serialize_uncompressed();
    address_from_uncompressed_pubkey(&uncompressed)
}

pub fn create2_address(factory: &Address, salt: &Bytes32, init_code_hash: &Bytes32) -> Address {
    let mut payload = Vec::with_capacity(1 + 20 + 32 + 32);
    payload.push(0xff);
    payload.extend_from_slice(factory);
    payload.extend_from_slice(salt);
    payload.extend_from_slice(init_code_hash);
    let hash = keccak256(&payload);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}
