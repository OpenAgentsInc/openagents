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
        build: "23",
        title: "Ready for launch",
        what_to_test: "Open Account, then Playtest, and check there is no card of zeros: the playtest card shows only once playtest awards are counted. From the menu, test a tool and check it runs on our computers; if a test set is too big for them, the card says how big it can be. Look through Chat, Wallet, Account, and the Verse and tell us if anything shows a number or a name that isn't yours or the Gym's.",
        items: &[
            Item {
                title: "No sample screens",
                detail: "The offline sample chats, Gym results, wallet, computers, and conversation we used for screenshots are gone from this build. Every screen shows your own records or the Gym's.",
            },
            Item {
                title: "No empty playtest card",
                detail: "Account, then Playtest, no longer shows a card of zeros before playtest awards are counted. The card comes back when they are.",
            },
            Item {
                title: "Plain reasons when we can't run your tests",
                detail: "A card no longer says our test computers aren't open. It says the real reason: a test set bigger than our computers run, with the limit, or a draft this phone no longer has.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "22",
        title: "Clearer credit",
        what_to_test: "From the menu, tap Check a result and run the check; when your XP arrives, tap Check a result again and check the card no longer promises XP for the same test set. Open Profile and check Your results lists only your full runs, and the XP bar fills as you earn. Tap What's new and check the first words show in about a second.",
        items: &[
            Item {
                title: "A second check says what it earns",
                detail: "You earn XP for checking a test set once. A second check of the same test set no longer promises XP, and Profile no longer says XP is on its way for it.",
            },
            Item {
                title: "Results you can check",
                detail: "Check a result offers only results our test computers can run again and that still earn XP when you confirm them.",
            },
            Item {
                title: "Your results",
                detail: "Profile's Your results lists only your full runs. Tries and checks show under What you made.",
            },
            Item {
                title: "Faster Gym news",
                detail: "What's new in the Gym shows its first words and the news in about a second.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "21",
        title: "Test tools in chat",
        what_to_test: "On a fresh install, tap Choose Coder, then Let's go, then Start the test, and check the test starts in three taps; when the result comes, tap Add to the Gym. From the menu, tap What's new and Check a result, and run a check. In a chat, say \"Help me make a tool that writes changelog entries\" and answer each step with Looks good or a change. Open Profile to see what you made and the XP it earned, and tap The Gym in the Verse to see the board.",
        items: &[
            Item {
                title: "A menu with chat first",
                detail: "The Chat tab opens on a menu whose big button is Chat with OpenAgents, with your trainer name, level, and a line that says what to do next. Test a tool, What's new, and Check a result each start a chat with that question.",
            },
            Item {
                title: "Your first test in three taps",
                detail: "A new install walks you through choosing Coder and testing Project map in chat. If you leave, the app reopens where you were.",
            },
            Item {
                title: "Test a tool from chat",
                detail: "Ask which tool to try and chat shows the tool with Start the test. We run the tests on our computers, with the tool and without it, and the card shows the result: how many tests Coder passed each way.",
            },
            Item {
                title: "Make your own tool by chatting",
                detail: "Chat drafts a tool and its tests with you, one step at a time. Tap Looks good to go on or Change it to say what to change, try it once, then run the full test set. The draft stays on your phone.",
            },
            Item {
                title: "Add to the Gym",
                detail: "Add to the Gym shows exactly what becomes public before anything does. Your result then waits for other trainers to check it.",
            },
            Item {
                title: "Check others' results and earn XP",
                detail: "Check a result runs another trainer's tests again. When a check confirms a result, both trainers earn XP, and Coder can adopt a tool that holds up. Ask what you've earned, or open Profile. XP can't be spent.",
            },
            Item {
                title: "Gym news",
                detail: "Ask what's new in the Gym for the latest results, checks, and builds, each with where it came from.",
            },
            Item {
                title: "The Gym board in the Verse",
                detail: "The Gym in the Verse shows published results by test set and tool, with how many trainers confirmed each. See the board under a result opens it.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "20",
        title: "Smarter chat and a simpler Wallet",
        what_to_test: "In a new chat, ask \"Who are you?\", \"Connect to my GitHub\", \"What's the Grid?\", and \"Fix the failing test in my repo\", and tap the offer under a reply (Run Coder, Connect a computer, or a link to a screen). Open the Wallet tab and try Receive and Send. On a prepared reply, tap Wrong answer.",
        items: &[
            Item {
                title: "Instant answers",
                detail: "Common questions get a prepared answer at once, marked \"Prepared answer\". OpenAgents also answers questions about the app, and about our code with citations to the files it read.",
            },
            Item {
                title: "Work goes to Coder",
                detail: "Ask for work on your code and the reply says \"We'll dispatch Coder to…\" with a Run Coder button, or Connect a computer if none is connected yet.",
            },
            Item {
                title: "Links to the right screen",
                detail: "Replies about your wallet, computers, and the rest of the app carry a button that opens that screen.",
            },
            Item {
                title: "Commands on your computer",
                detail: "Some replies offer a read-only command card that runs on your connected computer (one with terminal access) and shows its output in the chat.",
            },
            Item {
                title: "Follow-ups and feedback",
                detail: "Chips under a reply suggest what to ask next. Wrong answer on a reply reports it, and Report a problem can share this chat.",
            },
            Item {
                title: "One voice",
                detail: "Chat speaks as \"we\" everywhere, including the lines about waiting and daily limits.",
            },
            Item {
                title: "A simpler Wallet",
                detail: "The Wallet shows your balance, two big buttons (Receive and Send), and your last five payments. Receive shows a QR code right away; Send takes one pasted or scanned code and figures out what it is. A card asks you to back up your recovery words until you do. Everything else (other ways to receive, buying, deposits, people, agent payments, the amount unit, recovery) is under Advanced.",
            },
            Item {
                title: "Keyboard",
                detail: "Tap outside a text field on any screen to put the keyboard away.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "19",
        title: "New chats go to OpenAgents",
        what_to_test: "With a computer connected, ask \"Who are you?\" in a new chat and check the answer appears within a second.",
        items: &[Item {
            title: "New chats go to OpenAgents",
            detail: "A new chat answers right away even with a computer connected; Coder runs on a computer only when you pick it in the selector or tap a workspace.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "18",
        title: "Chat with OpenAgents",
        what_to_test: "Tap the Chat tab and start typing right away; ask who you're talking to or what model it is, and check the answer is instant. Pick your computer in the selector, send a task, and watch Coder's reply stream in. Open the menu at the top left and check that earlier chats open quickly, and that an OpenCode or Devin session a Coder task started shows inside its chat.",
        items: &[
            Item {
                title: "Ready to type",
                detail: "The Chat tab (the message icon) opens on a new chat with the cursor in the composer.",
            },
            Item {
                title: "OpenAgents and Coder",
                detail: "You chat with OpenAgents, which speaks as \"we\"; work for your computer is dispatched to Coder there.",
            },
            Item {
                title: "Where it goes",
                detail: "A selector beside the title picks one of your computers or Cloud, and suggestion chips above the composer continue recent chats or pick a workspace.",
            },
            Item {
                title: "Previous chats",
                detail: "Earlier chats are behind the menu button at the top left. Only OpenAgents and Coder chats are listed; the Claude Code, Codex, OpenCode, and Devin lists are gone.",
            },
            Item {
                title: "Instant answers",
                detail: "Common questions, like who you are talking to or which model answers, get a prepared answer right away.",
            },
            Item {
                title: "Faster chats",
                detail: "Chat lists and transcripts load and open much faster: the phone keeps what it last saw and gets updates pushed instead of polling, and Coder on a computer streams its reply.",
            },
            Item {
                title: "Delegated sessions",
                detail: "An OpenCode or Devin session a Coder task delegated to shows inside that Coder chat.",
            },
            Item {
                title: "Wallet notice removed",
                detail: "The wallet no longer shows the \"whole numbers\" notice.",
            },
            Item {
                title: "Lagrange 1 portal hidden",
                detail: "The Grid's portal to Lagrange 1 is hidden for now.",
            },
            Item {
                title: "Playtest logging",
                detail: "On for everyone during the playtest, with no switch: the phone notes which tab and screen you're on, kept on this phone and attached only to a report you preview. The Playtest session toggle is gone.",
            },
        ],
    },
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
