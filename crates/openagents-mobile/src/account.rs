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

/// One TestFlight build and what it brought.
#[derive(Serialize)]
pub struct Release {
    pub version: &'static str,
    /// The build number, or the range of builds the entry covers.
    pub build: &'static str,
    pub title: &'static str,
    /// Where testers should look in this build: every build has one.
    pub what_to_test: &'static str,
    pub items: &'static [Item],
}

/// One change: a short name and a sentence about it.
#[derive(Serialize)]
pub struct Item {
    pub title: &'static str,
    pub detail: &'static str,
}

/// The app's builds, newest first. The first entry's build is the one
/// `bins/openagents-ios/host/project.yml` builds, so every TestFlight build
/// ships with its own entry and its What to test line.
pub const CHANGELOG: &[Release] = &[
    Release {
        version: "1.0.0",
        build: "17",
        title: "Chat",
        what_to_test: "Open the Chat tab without a computer connected, start a new chat, and watch the basic Coder's reply stream in; then tap Run Coder to hand the conversation to a computer. Check that Devin and OpenCode sessions from your computer show up in the chat list. In Account, Trainer, turn Show my level on and off, and export your trainer card.",
        items: &[
            Item {
                title: "Chat tab",
                detail: "The first tab is Chat. A new chat talks to the basic Coder in the OpenAgents cloud, with no computer needed, and its reply streams in as it's written.",
            },
            Item {
                title: "Run Coder",
                detail: "From a conversation, Run Coder carries it to one of your computers, or Connect a computer leads to Account, Computers.",
            },
            Item {
                title: "Devin and OpenCode chats",
                detail: "The chat list shows Devin CLI and OpenCode sessions from your computers alongside Coder chats.",
            },
            Item {
                title: "Cloud fallback",
                detail: "A computer with no coding agent signed in can still finish a Coder turn through the OpenAgents cloud.",
            },
            Item {
                title: "Trainer level and card",
                detail: "Choose whether your level shows over your head in the Grid, link your keys, and export your trainer card as a signed link.",
            },
            Item {
                title: "Automatic payments",
                detail: "A standing spend grant lets the phone pay trusted payees small amounts without a tap, within the grant's limits.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "16",
        title: "Playtesting",
        what_to_test: "Report something from Account, Report a problem, or long-press the tab bar on any screen, and find it in My reports. Turn on Playtest session in Account, Playtest, move around, and check that the log shows only tabs, screens, and times.",
        items: &[
            Item {
                title: "Report a problem",
                detail: "Send a private report with the build and screen filled in, and a screenshot only if you choose one. Never from the Wallet or a key screen.",
            },
            Item {
                title: "Playtest session",
                detail: "An opt-in log of which tab and screen you're on, kept on this phone and attached only to a report you preview.",
            },
            Item {
                title: "My reports",
                detail: "Every report you filed, with the code to quote.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "15",
        title: "Scanning, paying, and long chats",
        what_to_test: "Scan a Lightning invoice or address with the Wallet's scanner and pay a tiny amount. Open a long Coder chat and scroll it. Walk the Grid with the look stick.",
        items: &[
            Item {
                title: "Wallet scanner",
                detail: "The scanner reads payment codes, not only Coder invitations.",
            },
            Item {
                title: "Lightning addresses",
                detail: "Pay Lightning addresses and LNURL codes within the recipient's limits.",
            },
            Item {
                title: "Long chats",
                detail: "A chat's rows are laid out from Rust, so long chats open and scroll faster.",
            },
            Item {
                title: "Look stick",
                detail: "The Grid's look stick turns at a gentler speed.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "1–14",
        title: "First release",
        what_to_test: "Open the app with no help and say what it's for. Walk the Grid and push the ball, open the Gym's RESULTS board, chat with Coder on your own computer, and send a tiny Wallet payment.",
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
    },
];

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
    fn every_build_has_a_changelog_entry_with_what_to_test() {
        // The newest entry is the build the iOS project makes.
        let project = include_str!("../../../bins/openagents-ios/host/project.yml");
        let newest = &CHANGELOG[0];
        assert!(
            project.contains(&format!("CURRENT_PROJECT_VERSION: {}\n", newest.build)),
            "add a Changelog entry for the build in project.yml"
        );
        for release in CHANGELOG {
            assert!(!release.what_to_test.trim().is_empty(), "{}", release.build);
            assert!(!release.items.is_empty());
        }
        let first = CHANGELOG.last().expect("first release");
        let titles: Vec<&str> = first.items.iter().map(|item| item.title).collect();
        assert!(titles.contains(&"Coder launch") && titles.contains(&"Verse launch"));
        let (mut app, _dir) = app();
        let packet = account(&mut app, false);
        assert_eq!(packet["changelog"][0]["build"], newest.build);
        assert!(packet["changelog"][0]["what_to_test"].is_string());
    }
}
