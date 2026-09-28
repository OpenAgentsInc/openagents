//! The Account tab's own screens: this device's identity keys and the
//! changelog.
//!
//! The device key is a secp256k1 key the platform host creates from its
//! secure random source and keeps in its protected store (Keychain on iOS).
//! It is not derived from a NIP-06 seed phrase, so no mnemonic can restore
//! it. Keys are shown in NIP-19 form first (`npub`, `nsec`), with hex beside
//! the public key. The secret key leaves this crate only in the answer to an
//! explicit reveal; the app packet never carries it.

use secp256k1::{Secp256k1, SecretKey};
use serde::Serialize;

/// One released version and what it brought.
#[derive(Serialize)]
pub struct Release {
    pub version: &'static str,
    pub title: &'static str,
    pub items: &'static [Item],
}

/// One change: a short name and a sentence about it.
#[derive(Serialize)]
pub struct Item {
    pub title: &'static str,
    pub detail: &'static str,
}

/// The app's releases, newest first.
pub const CHANGELOG: &[Release] = &[Release {
    version: "1.0.0",
    title: "First release",
    items: &[
        Item {
            title: "Coder launch",
            detail: "Chat with Coder on your own computers from the Coder tab.",
        },
        Item {
            title: "Verse launch",
            detail: "Walk Verse's world with other players in the Verse tab.",
        },
        Item {
            title: "Computers",
            detail: "Add a computer by invitation or over your tailnet, then order work and open its terminal.",
        },
        Item {
            title: "Identity keys",
            detail: "See this device's npub, and reveal and copy its nsec.",
        },
    ],
}];

/// The answer to an `account` request.
#[derive(Serialize)]
pub struct AccountPacket {
    pub schema: &'static str,
    /// The device's public key in NIP-19 form.
    pub npub: String,
    /// The device's public key in hex (x-only, as NIP-01 uses it).
    pub public_hex: String,
    /// How the key was made, for the Identity Keys screen.
    pub origin: &'static str,
    pub changelog: &'static [Release],
    /// The device's secret key in NIP-19 form. Present only when the
    /// request asked to reveal it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsec: Option<String>,
}

/// What the key is. Both hosts create it from the platform's secure random
/// source and keep it in a this-device-only store.
pub const ORIGIN: &str = "Made at random on this device and kept only in its secure storage. It isn't derived from a seed phrase (NIP-06), so the nsec is its only backup.";

/// The device's public key as `(hex, npub)`.
pub fn public(secret: &SecretKey) -> (String, String) {
    let (key, _) = secret.x_only_public_key(&Secp256k1::new());
    (key.to_string(), nostr::nip19::encode_npub(&key.serialize()))
}

pub fn packet(secret: &SecretKey, reveal: bool) -> AccountPacket {
    let (public_hex, npub) = public(secret);
    AccountPacket {
        schema: "openagents.account.v1",
        npub,
        public_hex,
        origin: ORIGIN,
        changelog: CHANGELOG,
        nsec: reveal.then(|| nostr::nip19::encode_nsec(&secret.secret_bytes())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Config, Request};
    use std::str::FromStr;

    // NIP-06's first test vector: the NIP-19 forms of one key pair.
    const SECRET: &str = "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
    const PUBLIC: &str = "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
    const NPUB: &str = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
    const NSEC: &str = "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp";

    fn app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let app = App::new(Config {
            state_dir: dir.path().to_path_buf(),
            secret_hex: SECRET.into(),
        })
        .expect("app");
        (app, dir)
    }

    fn account(app: &mut App, reveal: bool) -> serde_json::Value {
        serde_json::from_slice(&app.respond(Request::Account { reveal })).expect("account packet")
    }

    #[test]
    fn keys_show_as_npub_first_with_hex_beside_it() {
        let secret = SecretKey::from_str(SECRET).expect("secret");
        assert_eq!(public(&secret), (PUBLIC.to_owned(), NPUB.to_owned()));
        let (mut app, _dir) = app();
        let packet = account(&mut app, false);
        assert_eq!(packet["schema"], "openagents.account.v1");
        assert_eq!(packet["npub"], NPUB);
        assert_eq!(packet["public_hex"], PUBLIC);
        assert!(
            packet["origin"]
                .as_str()
                .is_some_and(|o| o.contains("NIP-06"))
        );
        let snapshot: serde_json::Value =
            serde_json::from_slice(&app.respond(Request::Snapshot)).expect("app packet");
        assert_eq!(snapshot["device"], PUBLIC);
        assert_eq!(snapshot["device_npub"], NPUB);
    }

    #[test]
    fn only_an_explicit_reveal_returns_the_nsec() {
        let (mut app, _dir) = app();
        let hidden = account(&mut app, false);
        assert!(hidden.get("nsec").is_none());
        let shown = account(&mut app, true);
        assert_eq!(shown["nsec"], NSEC);
        let decoded = nostr::nip19::decode_nsec(NSEC).expect("nsec");
        assert_eq!(
            decoded,
            SecretKey::from_str(SECRET).expect("secret").secret_bytes()
        );
    }

    #[test]
    fn the_app_packet_never_carries_the_secret_key() {
        let (mut app, _dir) = app();
        let _ = account(&mut app, true);
        let mut packets = vec![
            app.respond(Request::Snapshot),
            app.respond(Request::ComputersRefresh),
        ];
        // Through `call`, which the host never uses for it, a reveal still
        // yields only the app packet.
        packets
            .push(serde_json::to_vec(&app.call(Request::Account { reveal: true })).expect("json"));
        for packet in packets {
            let text = String::from_utf8(packet).expect("utf-8");
            assert!(text.contains(NPUB));
            assert!(!text.contains(SECRET));
            assert!(!text.contains("nsec1"));
        }
    }

    #[test]
    fn the_changelog_starts_with_the_first_release() {
        let first = &CHANGELOG[0];
        assert_eq!(first.version, "1.0.0");
        let titles: Vec<&str> = first.items.iter().map(|item| item.title).collect();
        assert!(titles.contains(&"Coder launch") && titles.contains(&"Verse launch"));
        let (mut app, _dir) = app();
        assert_eq!(account(&mut app, false)["changelog"][0]["version"], "1.0.0");
    }
}
