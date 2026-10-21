//! The deploy pointer: a signed Nostr event that names a tenant, a
//! channel, and the manifest that channel serves.
//!
//! Kind 30440 is parameterized replaceable, so a relay keeps the newest
//! pointer per signer and `d` tag. The `d` tag is `TENANT/CHANNEL`; the
//! tenant's own key signs it.
//!
//! The content is empty, or the manifest itself when it is small enough
//! ([`MAX_INLINE_MANIFEST`]), so a follower skips one store round trip. A
//! follower uses inline content only when it hashes to `m`.
//!
//! ```json
//! {"kind": 30440, "content": "",
//!  "tags": [["d", "site/staging"], ["tenant", "site"], ["channel", "staging"],
//!           ["m", "<manifest sha-256>"], ["prev", "<previous manifest>"]]}
//! ```

use anyhow::{Result, bail};
use nostr::domain::{Event, RelaySigner, Tag};

/// The prototype's event kind for a deploy pointer.
pub const KIND: u16 = 30440;

/// The largest manifest a pointer carries inline.
pub const MAX_INLINE_MANIFEST: usize = 48 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    pub tenant: String,
    pub channel: String,
    pub manifest: String,
    pub prev: Option<String>,
    pub created_at: u64,
    pub id: String,
    pub pubkey: String,
    /// The manifest's bytes, when the pointer carried them and they hash
    /// to `manifest`.
    pub body: Option<Vec<u8>>,
}

pub fn d_tag(tenant: &str, channel: &str) -> String {
    format!("{tenant}/{channel}")
}

/// Tenant and channel names: lowercase letters, digits, and hyphens.
pub fn check_name(kind: &str, name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !ok {
        bail!("{kind} name {name:?} must be 1-64 lowercase letters, digits, or hyphens");
    }
    Ok(())
}

pub fn sign(
    signer: &RelaySigner,
    tenant: &str,
    channel: &str,
    manifest: &str,
    prev: Option<&str>,
    created_at: u64,
) -> Result<Event> {
    sign_with_body(signer, tenant, channel, manifest, prev, created_at, None)
}

/// Like [`sign`], carrying the manifest's bytes when they are UTF-8 and at
/// most [`MAX_INLINE_MANIFEST`] long.
pub fn sign_with_body(
    signer: &RelaySigner,
    tenant: &str,
    channel: &str,
    manifest: &str,
    prev: Option<&str>,
    created_at: u64,
    body: Option<&[u8]>,
) -> Result<Event> {
    check_name("tenant", tenant)?;
    check_name("channel", channel)?;
    let mut tags = vec![
        tag(&["d", &d_tag(tenant, channel)]),
        tag(&["tenant", tenant]),
        tag(&["channel", channel]),
        tag(&["m", manifest]),
    ];
    if let Some(prev) = prev {
        tags.push(tag(&["prev", prev]));
    }
    let content = body
        .filter(|body| body.len() <= MAX_INLINE_MANIFEST)
        .and_then(|body| std::str::from_utf8(body).ok())
        .unwrap_or_default()
        .to_owned();
    Ok(signer.sign(created_at, KIND, tags, content))
}

fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|value| (*value).to_owned()).collect())
}

/// Checks a pointer event: its shape, id, and signature, and that the key
/// `authorized` signed it for `tenant` on `channel`. Anything else is
/// refused.
pub fn verify(event: &Event, authorized: &str, tenant: &str, channel: &str) -> Result<Pointer> {
    if event.kind != KIND {
        bail!("kind {} is not a deploy pointer", event.kind);
    }
    event
        .validate_nip01_structure()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    event
        .validate_crypto()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    if event.pubkey != authorized {
        bail!(
            "pointer signed by {}, not the tenant's deploy key",
            event.pubkey
        );
    }
    let value = |name: &str| {
        event
            .tags
            .iter()
            .find(|tag| tag.name() == Some(name))
            .and_then(|tag| tag.value())
            .map(str::to_owned)
    };
    if value("d").as_deref() != Some(&d_tag(tenant, channel))
        || value("tenant").as_deref() != Some(tenant)
        || value("channel").as_deref() != Some(channel)
    {
        bail!("pointer is not for {tenant}/{channel}");
    }
    let Some(manifest) = value("m") else {
        bail!("pointer names no manifest");
    };
    if manifest.len() != 64 || !manifest.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("pointer's manifest is not a SHA-256");
    }
    let body = (!event.content.is_empty()
        && crate::sha256_hex(event.content.as_bytes()) == manifest)
        .then(|| event.content.clone().into_bytes());
    Ok(Pointer {
        tenant: tenant.to_owned(),
        channel: channel.to_owned(),
        manifest,
        prev: value("prev"),
        created_at: event.created_at,
        id: event.id.clone(),
        pubkey: event.pubkey.clone(),
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn accepts_only_the_tenants_key_for_its_channel() {
        let site = signer("11");
        let other = signer("22");
        let manifest = "ab".repeat(32);
        let event = sign(&site, "site", "staging", &manifest, None, 100).unwrap();
        let pointer = verify(&event, site.pubkey(), "site", "staging").unwrap();
        assert_eq!(pointer.manifest, manifest);

        let forged = sign(&other, "site", "staging", &manifest, None, 101).unwrap();
        assert!(verify(&forged, site.pubkey(), "site", "staging").is_err());
        assert!(verify(&event, site.pubkey(), "site", "production").is_err());
        assert!(verify(&event, site.pubkey(), "game", "staging").is_err());

        let mut tampered = event.clone();
        tampered.tags[3] = tag(&["m", &"cd".repeat(32)]);
        assert!(verify(&tampered, site.pubkey(), "site", "staging").is_err());
    }

    #[test]
    fn a_pointer_carries_its_manifest_only_when_it_matches() {
        let site = signer("11");
        let body = br#"{"schema": "x"}"#;
        let hash = crate::sha256_hex(body);
        let event = sign_with_body(&site, "site", "staging", &hash, None, 1, Some(body)).unwrap();
        let pointer = verify(&event, site.pubkey(), "site", "staging").unwrap();
        assert_eq!(pointer.body.as_deref(), Some(&body[..]));
        let other = "ab".repeat(32);
        let event = sign_with_body(&site, "site", "staging", &other, None, 1, Some(body)).unwrap();
        assert!(
            verify(&event, site.pubkey(), "site", "staging")
                .unwrap()
                .body
                .is_none()
        );
    }

    #[test]
    fn names_are_checked() {
        assert!(check_name("tenant", "toy-game").is_ok());
        assert!(check_name("tenant", "Site").is_err());
        assert!(check_name("tenant", "a/b").is_err());
    }
}
