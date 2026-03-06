use bitcoin::hashes::{Hash, sha256};

pub fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let digest = sha256::Hash::hash(bytes);
    format!("sha256:{digest}")
}

pub fn sha256_prefixed_text(value: &str) -> String {
    sha256_prefixed_bytes(value.as_bytes())
}
