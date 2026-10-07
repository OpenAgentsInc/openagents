//! The `openagents.verse.private-asset.v1` manifest: what an asset is, where
//! it came from, under what license, who may load it, and how it was built.
//!
//! The manifest lives at `manifests/<name>.json` in the private bucket. The
//! broker reads it on every grant, so a change to `readers` takes effect on
//! the next request.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{MAX_MANIFEST_BYTES, MAX_PACK_BYTES, valid_digest, valid_name};

/// The manifest schema.
pub const SCHEMA: &str = "openagents.verse.private-asset.v1";
/// Most readers one asset may name.
pub const MAX_READERS: usize = 32;

/// One private asset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    /// The registry name, as in `manifests/<name>.json`.
    pub name: String,
    /// The listing's title.
    pub title: String,
    /// `character` today.
    pub kind: String,
    pub source: Source,
    pub license: License,
    /// Nostr public keys (lowercase hex) that may load the pack.
    pub readers: Vec<String>,
    pub pack: Pack,
    pub vendor: Vendor,
    pub provenance: Provenance,
}

/// Where the asset came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The marketplace, such as `fab`.
    pub marketplace: String,
    /// The listing's ID there.
    pub listing: String,
    /// The listing's page.
    pub url: String,
    /// The seller's name.
    pub seller: String,
    /// Whether the listing says it was AI-generated.
    pub ai_generated: bool,
}

/// The license the asset was acquired under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct License {
    /// Such as `fab-standard`.
    pub id: String,
    /// What it allows and forbids, in a sentence or two.
    pub summary: String,
}

/// The compiled pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    /// Lowercase hexadecimal SHA-256 of the pack.
    pub sha256: String,
    /// The pack's length.
    pub bytes: u64,
    /// `VTP3`.
    pub format: String,
    /// The forms the pack holds, such as `private/guest`.
    pub forms: Vec<String>,
    /// Triangles in each form, in `forms` order.
    pub triangles: Vec<u64>,
    /// The near level's texture edge, pixels.
    pub texture_edge: u32,
    /// The character's height, meters.
    pub height_m: f32,
    /// How it is rigged.
    pub rig: String,
    /// Its clips.
    pub clips: Vec<String>,
}

/// The raw vendor files, archived under `vendor/<name>/`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vendor {
    /// The archive's prefix in the bucket.
    pub prefix: String,
    /// SHA-256 of each archived file by its path under the prefix.
    pub files: BTreeMap<String, String>,
}

/// How the pack was made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The conversion script, relative to the repository.
    pub script: String,
    /// The script's SHA-256.
    pub script_sha256: String,
    /// The Blender version that ran it.
    pub blender: String,
    /// The repository commit the compiler was built from.
    pub commit: String,
    /// The conversion's parameters, as given.
    pub parameters: BTreeMap<String, String>,
    /// When it was compiled, as an RFC 3339 UTC time.
    pub compiled_at: String,
}

impl Manifest {
    /// Parses and checks a manifest.
    ///
    /// # Errors
    ///
    /// Returns a message when the bytes are too long, not the schema, or
    /// fail [`Self::check`].
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err("manifest is too long".into());
        }
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|error| format!("manifest: {error}"))?;
        manifest.check()?;
        Ok(manifest)
    }

    /// Checks the fields the broker and loader rely on.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first field that fails.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("manifest schema must be {SCHEMA}"));
        }
        if !valid_name(&self.name) {
            return Err("manifest name must be lowercase letters, digits, and hyphens".into());
        }
        if self.kind != "character" {
            return Err("manifest kind must be character".into());
        }
        if self.license.id.trim().is_empty() || self.license.summary.trim().is_empty() {
            return Err("manifest must name its license and summarize it".into());
        }
        if self.readers.len() > MAX_READERS || self.readers.iter().any(|r| !valid_digest(r)) {
            return Err(format!(
                "manifest readers must be at most {MAX_READERS} lowercase hex public keys"
            ));
        }
        if !valid_digest(&self.pack.sha256) {
            return Err("manifest pack digest must be 64 lowercase hex digits".into());
        }
        if self.pack.bytes == 0 || self.pack.bytes > MAX_PACK_BYTES {
            return Err(format!(
                "manifest pack length must be 1 to {MAX_PACK_BYTES} bytes"
            ));
        }
        if self.pack.format != "VTP3"
            || self.pack.forms.is_empty()
            || self.pack.forms.len() != self.pack.triangles.len()
        {
            return Err("manifest pack must be VTP3 with a triangle count per form".into());
        }
        if self.vendor.prefix != format!("{}{}/", crate::VENDOR_PREFIX, self.name)
            || self.vendor.files.values().any(|d| !valid_digest(d))
        {
            return Err("manifest vendor archive must sit under vendor/<name>/".into());
        }
        Ok(())
    }

    /// Whether `pubkey` may load the pack.
    #[must_use]
    pub fn admits(&self, pubkey: &str) -> bool {
        self.readers.iter().any(|r| r == pubkey)
    }

    /// The manifest as stored: pretty JSON with a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns a message when it fails [`Self::check`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.check()?;
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample(readers: &[&str]) -> Manifest {
        Manifest {
            schema: SCHEMA.into(),
            name: "sample-guest".into(),
            title: "Sample Guest".into(),
            kind: "character".into(),
            source: Source {
                marketplace: "fab".into(),
                listing: "00000000-0000-0000-0000-000000000000".into(),
                url: "https://www.fab.com/listings/00000000-0000-0000-0000-000000000000".into(),
                seller: "A Seller".into(),
                ai_generated: true,
            },
            license: License {
                id: "fab-standard".into(),
                summary: "Use in products; no redistribution of the raw asset.".into(),
            },
            readers: readers.iter().map(|r| (*r).to_owned()).collect(),
            pack: Pack {
                sha256: "cd".repeat(32),
                bytes: 1234,
                format: "VTP3".into(),
                forms: vec!["private/guest".into(), "private/guest_far".into()],
                triangles: vec![20_000, 5_000],
                texture_edge: 1024,
                height_m: 1.62,
                rig: "generated spine".into(),
                clips: vec!["idle".into()],
            },
            vendor: Vendor {
                prefix: "vendor/sample-guest/".into(),
                files: BTreeMap::from([("source/model.glb".into(), "ef".repeat(32))]),
            },
            provenance: Provenance {
                script: "scripts/blender/private_character.py".into(),
                script_sha256: "01".repeat(32),
                blender: "5.2.2 LTS".into(),
                commit: "0".repeat(40),
                parameters: BTreeMap::new(),
                compiled_at: "2026-10-06T00:00:00Z".into(),
            },
        }
    }

    #[test]
    fn a_manifest_round_trips_and_admits_only_its_readers() {
        let reader = "aa".repeat(32);
        let manifest = sample(&[&reader]);
        let parsed = Manifest::parse(&manifest.to_bytes().unwrap()).unwrap();
        assert_eq!(parsed, manifest);
        assert!(parsed.admits(&reader));
        assert!(!parsed.admits(&"bb".repeat(32)));
    }

    #[test]
    fn a_manifest_refuses_malformed_fields_and_unknown_ones() {
        let mut bad = sample(&[]);
        bad.readers = vec!["npub1notahexkey".into()];
        assert!(bad.check().is_err());
        let mut bad = sample(&[]);
        bad.pack.sha256 = "../../vendor".into();
        assert!(bad.check().is_err());
        let mut bad = sample(&[]);
        bad.vendor.prefix = "packs/".into();
        assert!(bad.check().is_err());
        let mut bad = sample(&[]);
        bad.pack.bytes = crate::MAX_PACK_BYTES + 1;
        assert!(bad.check().is_err());
        let mut value = serde_json::to_value(sample(&[])).unwrap();
        value["public"] = true.into();
        assert!(Manifest::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
