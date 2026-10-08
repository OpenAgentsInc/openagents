//! Bounded admission for the original coast pack. The character remains
//! in the existing shared pack; coast models carry no executable content.

pub use verse_zone_everglade::zones::everglade_pack::format::{Limits, ZonePack};

/// Immutable original pack, retained by digest. Decode on coast entry only.
pub const SHA256: &str = "105ec8dc07ec7fbd5efa0acc3a57232b95435086e81680b1a3dad58619873d7c";
pub const BYTES: u64 = 487425;
#[rustfmt::skip]
const BUNDLED: &[u8] = include_bytes!("../../../assets/verse/coast/105ec8dc07ec7fbd5efa0acc3a57232b95435086e81680b1a3dad58619873d7c.vtp");

/// This small public pack ships with native and browser clients. Decoded
/// geometry belongs to the coast world and is released when it is replaced.
pub fn bundled() -> Result<ZonePack, String> {
    decode(BUNDLED, SHA256, BYTES)
}

pub const DIRECTORY: &str = "assets/verse/coast";
pub const SETS: &[&str] = &[
    "rocks",
    "beach",
    "harbor",
    "lighthouse",
    "boats",
    "reef",
    "beasts",
    "lod",
];
pub const LIMITS: Limits = Limits {
    pack_bytes: 48 * 1024 * 1024,
    decoded_texture_bytes: 32 * 1024 * 1024,
    texture_edge: 512,
    triangles: 120_000,
    model_triangles: 12_000,
    committed_bytes: 64 * 1024 * 1024,
    character_triangles: 800,
    body_bytes: 64 * 1024 * 1024,
};

/// Verify content identity before the bounded shared decoder allocates.
pub fn decode(bytes: &[u8], digest: &str, length: u64) -> Result<ZonePack, String> {
    use sha2::{Digest, Sha256};
    if length > LIMITS.pack_bytes || bytes.len() as u64 != length {
        return Err("The coast pack length differs from its pin".into());
    }
    if format!("{:x}", Sha256::digest(bytes)) != digest {
        return Err("The coast pack digest differs from its pin".into());
    }
    verse_zone_everglade::zones::everglade_pack::format::decode(bytes, &LIMITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pin_and_wildlife_clips_are_complete() {
        let pack = bundled().unwrap();
        assert_eq!(pack.models.len(), 108);
        assert_eq!(pack.forms.len(), 4);
        assert!(pack.decoded_texture_bytes() < 1024 * 1024);
        for (name, clips) in [
            ("gull", &["idle", "flap", "glide"][..]),
            ("crab", &["idle", "walk"][..]),
            ("seal", &["idle", "swim"][..]),
            ("fish", &["idle", "swim"][..]),
        ] {
            let form = pack.form(&format!("beasts/{name}")).unwrap();
            assert!(form.triangles() < 800);
            for clip in clips {
                assert!(form.clip(clip).is_some(), "{name}/{clip}");
            }
        }
    }

    #[test]
    fn a_wrong_content_identity_is_rejected_before_decode() {
        assert!(
            decode(b"wrong", &"0".repeat(64), 5)
                .unwrap_err()
                .contains("digest")
        );
        assert!(
            decode(b"wrong", &"0".repeat(64), 4)
                .unwrap_err()
                .contains("length")
        );
    }

    #[test]
    fn admitted_sources_reproduce_the_retained_pin() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/verse/coast/source");
        let compiled = verse_zone_everglade::zones::everglade_pack::compile::compile(
            &root, SETS, None, &LIMITS,
        )
        .unwrap();
        assert_eq!(compiled.sha256, SHA256);
        assert_eq!(compiled.bytes, BUNDLED);
    }
}
