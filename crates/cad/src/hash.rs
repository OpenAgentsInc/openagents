/// Deterministic FNV-1a 64-bit hash used for CAD receipts, IDs, and rebuild artifacts.
pub fn stable_fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Deterministic fixed-width lowercase hex digest for CAD hash surfaces.
pub fn stable_hex_digest(bytes: &[u8]) -> String {
    format!("{:016x}", stable_fnv1a64(bytes))
}

#[cfg(test)]
mod tests {
    use super::{stable_fnv1a64, stable_hex_digest};

    #[test]
    fn fnv_is_deterministic_for_identical_inputs() {
        let a = stable_fnv1a64(b"cad-deterministic");
        let b = stable_fnv1a64(b"cad-deterministic");
        assert_eq!(a, b);
    }

    #[test]
    fn hex_digest_is_deterministic_and_fixed_width() {
        let a = stable_hex_digest(b"cad-deterministic");
        let b = stable_hex_digest(b"cad-deterministic");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }
}
