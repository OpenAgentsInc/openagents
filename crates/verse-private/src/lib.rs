//! Verse's private asset registry: licensed content that this public
//! repository must never hold (`docs/verse/private-assets.md`).
//!
//! - [`manifest`]: the `openagents.verse.private-asset.v1` manifest each
//!   asset carries in the private bucket.
//! - [`placements`]: the owner-local file that places private assets in a
//!   zone. Committed code never names a private asset.
//! - [`phones`]: the owner-local record of the paired phones that asked for
//!   the placements, and the Verse keys to grant them.
//! - [`auth`]: the NIP-98 grant request a reader signs, and the grant.
//! - [`signed_url`]: Cloud Storage V4 signed URLs over an injected signer.
//! - `broker` (feature `broker`): the `verse-assets` service, which grants a
//!   short-lived URL to a manifest's readers only.
//! - `client` (feature `client`): asks a broker for a grant.
//!
//! Nothing here holds a key. The broker signs through the IAM Credentials
//! API with its service account's Google-managed keys; a reader signs with
//! its own Verse key.

pub mod auth;
pub mod manifest;
pub mod phones;
pub mod placements;
pub mod signed_url;

#[cfg(feature = "broker")]
pub mod broker;
#[cfg(feature = "client")]
pub mod client;

/// The bucket that holds private packs, manifests, and vendor archives.
pub const BUCKET: &str = "openagentsgemini-verse-private-assets";
/// Where packs live in the bucket, each named `<sha256>.vtp`.
pub const PACK_PREFIX: &str = "packs/";
/// Where manifests live in the bucket, each named `<name>.json`.
pub const MANIFEST_PREFIX: &str = "manifests/";
/// Where raw vendor files are archived. Never served.
pub const VENDOR_PREFIX: &str = "vendor/";
/// The largest private pack, bytes; `Limits::PRIVATE` in the Everglade pack
/// format matches it.
pub const MAX_PACK_BYTES: u64 = 8 * 1024 * 1024;
/// The largest manifest, bytes.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// Whether `name` is a registry name: 1 to 64 lowercase letters, digits,
/// and single hyphens, starting with a letter.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `value` is 64 lowercase hexadecimal digits.
#[must_use]
pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The pack object for `sha256`. Built only from a checked digest, so a
/// signed URL can never name another object.
#[must_use]
pub fn pack_object(sha256: &str) -> Option<String> {
    valid_digest(sha256).then(|| format!("{PACK_PREFIX}{sha256}.vtp"))
}

/// The manifest object for `name`.
#[must_use]
pub fn manifest_object(name: &str) -> Option<String> {
    valid_name(name).then(|| format!("{MANIFEST_PREFIX}{name}.json"))
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_objects_admit_only_their_shapes() {
        for good in ["cute-asian-girl", "robot2", "a"] {
            assert!(valid_name(good), "{good}");
        }
        for bad in [
            "",
            "-a",
            "a-",
            "a--b",
            "A",
            "a/b",
            "a.b",
            "1a",
            &"a".repeat(65),
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
        let digest = "ab".repeat(32);
        assert_eq!(
            pack_object(&digest).as_deref(),
            Some(format!("packs/{digest}.vtp").as_str())
        );
        assert!(pack_object("../manifests/x").is_none());
        assert!(pack_object(&"AB".repeat(32)).is_none());
        assert_eq!(
            manifest_object("robot").as_deref(),
            Some("manifests/robot.json")
        );
        assert!(manifest_object("../vendor").is_none());
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
