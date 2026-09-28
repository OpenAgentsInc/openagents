//! The Account tab's trainer card: the level over your head in the Grid,
//! and the counted awards behind it.
//!
//! Your trainer key is the Verse world key, the one whose prefix the Grid
//! shows over your head. The card reads NIP-XP quests, awards, and
//! revocations from the public relay with Verse's reader
//! ([`verse::xp::Board`]), trusts the OpenAgents referee alone, re-checks
//! every award against the signed evidence it names, and names the curve
//! its level uses (`trainer-curve-v1`). It only reads: nothing here
//! publishes, spends, or pays.
//!
//! The world key's secret reaches the interface only in the direct reply
//! to a `trainer` request with `reveal: true`, which the Trainer Key screen
//! sends after the person taps **Reveal nsec** and confirms a warning, so
//! they can sign a reproduction with it on a computer.

use ::verse::xp::{Board, Snapshot};
use secp256k1::{Secp256k1, SecretKey};
use serde::Serialize;

/// Why XP exists and what it isn't, shown under the card.
pub const NOTE: &str = "XP records work the OpenAgents referee accepted, recomputed on this phone from signed Nostr events. It can't be spent, traded, or converted, and nothing here pays.";

/// One counted award on the card.
#[derive(Serialize)]
pub struct AwardRow {
    pub title: String,
    /// `<id>@<version>`.
    pub quest: String,
    pub season: String,
    /// `kb-transfer` or `reproduce`.
    pub rule: String,
    pub role: String,
    pub xp: u64,
    /// The `3193` event ID.
    pub award: String,
    /// The award on a public Nostr gateway.
    pub link: String,
}

/// The direct reply to a `trainer` request. It never reaches the app
/// packet.
#[derive(Serialize)]
pub struct TrainerPacket {
    pub schema: &'static str,
    /// The trainer key in NIP-19 form.
    pub npub: String,
    pub public_hex: String,
    /// The first eight hex characters: the name tag over your head.
    pub tag: String,
    /// `connecting`, `reading`, `ready`, or `preview` (the labeled
    /// fixture).
    pub state: &'static str,
    pub relay: String,
    pub referee_npub: String,
    pub curve: &'static str,
    pub xp: u64,
    pub level: u32,
    /// Cumulative XP at which the next level starts.
    pub next_level_at: u64,
    pub to_next: u64,
    pub titles: Vec<String>,
    pub awards: Vec<AwardRow>,
    /// Open quests from the trusted referee this key can still earn.
    pub open_quests: usize,
    pub note: &'static str,
    /// Playtest XP and titles from the separate playtest referee, shown
    /// beside the trainer XP above and never summed into its level.
    pub playtest: PlaytestSection,
    /// The trainer key's secret in NIP-19 form, only on an explicit reveal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsec: Option<String>,
}

/// Why playtest XP is separate, shown under the playtest card.
pub const PLAYTEST_NOTE: &str = "Playtest XP records accepted playtest contributions: feedback, reproducible bugs, verified fixes, and sessions with a report. The OpenAgents playtest referee signs it, separately from the trainer referee, so it never counts toward your trainer level. Joining earns nothing.";

/// The Account playtest card.
#[derive(Serialize)]
pub struct PlaytestSection {
    /// `unpublished` (no playtest referee key yet), `connecting`,
    /// `reading`, `ready`, or `preview`.
    pub state: &'static str,
    pub referee_npub: Option<String>,
    pub xp: u64,
    pub titles: Vec<String>,
    pub accepted_reports: usize,
    pub fixes_verified: usize,
    pub sessions: usize,
    pub diaries: usize,
    pub awards: Vec<AwardRow>,
    pub note: &'static str,
}

/// The card's reader. It starts on the first `trainer` request and stops
/// when the app leaves the foreground; the last ledger stays for the card.
#[derive(Default)]
pub struct Trainer {
    board: Option<Board>,
    snapshot: Option<Snapshot>,
    preview: bool,
    /// The playtest referee's reader, beside the trainer reader.
    playtest_board: Option<Board>,
    playtest_snapshot: Option<Snapshot>,
}

fn gateway_link(event_id: &str) -> String {
    let bytes: Vec<u8> = (0..event_id.len() / 2)
        .filter_map(|i| u8::from_str_radix(event_id.get(2 * i..2 * i + 2)?, 16).ok())
        .collect();
    match nostr::nip19::encode("note", &bytes) {
        Ok(note) => format!("https://njump.me/{note}"),
        Err(_) => format!("https://njump.me/{event_id}"),
    }
}

fn row(a: ::verse::xp::CardAward) -> AwardRow {
    AwardRow {
        link: gateway_link(&a.award),
        title: a.title,
        quest: a.quest,
        season: a.season,
        rule: a.rule,
        role: a.role,
        xp: a.xp,
        award: a.award,
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Trainer {
    /// The app left the foreground: close the relay connection.
    pub fn pause(&mut self) {
        self.board = None;
        self.playtest_board = None;
    }

    /// The playtest card for `public_hex`: read from the playtest referee
    /// when its key is published, or the labeled fixture in a preview.
    fn playtest(
        &mut self,
        public_hex: &str,
        signer: &nostr::domain::RelaySigner,
    ) -> PlaytestSection {
        let trust = ::verse::xp::playtest_trust();
        if self.preview && self.playtest_snapshot.is_none() {
            let referee = ::verse::xp::fixture::signer(0x91a7_7e57);
            let events = ::verse::xp::fixture::playtest_events(&referee, signer, unix_now());
            let trust = ::verse::xp::XpTrust {
                referees: std::collections::BTreeSet::from([referee.pubkey().to_owned()]),
                runners: std::collections::BTreeSet::new(),
            };
            self.playtest_snapshot = Some(::verse::xp::snapshot(&events, &trust));
        }
        let state = if self.preview {
            "preview"
        } else if let Some(trust) = trust.clone() {
            let board = self.playtest_board.get_or_insert_with(|| {
                Board::start_with(
                    ::verse::session::PUBLIC_RELAY,
                    trust,
                    None,
                    Some(signer.clone()),
                )
            });
            board.tick();
            if let Some(snapshot) = board.snapshot.take() {
                self.playtest_snapshot = Some(snapshot);
            }
            match (&self.playtest_snapshot, board.connected) {
                (Some(_), _) => "ready",
                (None, true) => "reading",
                (None, false) => "connecting",
            }
        } else {
            "unpublished"
        };
        let empty = Snapshot::default();
        let snapshot = self.playtest_snapshot.as_ref().unwrap_or(&empty);
        let card = ::verse::xp::playtest_card(snapshot, &[public_hex.to_owned()]);
        PlaytestSection {
            state,
            referee_npub: ::verse::xp::PLAYTEST_REFEREE
                .and_then(|k| k.parse::<secp256k1::XOnlyPublicKey>().ok())
                .map(|k| nostr::nip19::encode_npub(&k.serialize())),
            xp: card.xp,
            titles: card.titles,
            accepted_reports: card.accepted_reports,
            fixes_verified: card.fixes_verified,
            sessions: card.sessions,
            diaries: card.diaries,
            awards: card.awards.into_iter().map(row).collect(),
            note: PLAYTEST_NOTE,
        }
    }

    /// Answers a `trainer` request for the world key `secret_hex`.
    ///
    /// # Errors
    ///
    /// When the key isn't 64 hexadecimal characters of a valid secret.
    pub fn packet(
        &mut self,
        secret_hex: &str,
        reveal: bool,
        preview: bool,
    ) -> Result<TrainerPacket, String> {
        if secret_hex.len() != 64 || !secret_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid trainer key".into());
        }
        let secret: SecretKey = secret_hex
            .parse()
            .map_err(|_| "Invalid trainer key".to_owned())?;
        let (key, _) = secret.x_only_public_key(&Secp256k1::new());
        let public_hex = key.to_string();
        let signer = nostr::domain::RelaySigner::from_secret_hex(secret_hex)
            .map_err(|_| "Invalid trainer key".to_owned())?;
        if preview && !self.preview {
            // The labeled fixture: six tutorial reproductions credited to
            // this key by a throwaway referee the preview alone trusts.
            let referee = ::verse::xp::fixture::signer(0x0a_de_fe_ee);
            let events = ::verse::xp::fixture::tutorial_events(&referee, &signer, 6, unix_now());
            let mut trust = ::verse::xp::openagents_trust();
            trust.referees = std::collections::BTreeSet::from([referee.pubkey().to_owned()]);
            self.snapshot = Some(::verse::xp::snapshot(&events, &trust));
            self.board = None;
            self.preview = true;
        }
        if !self.preview {
            let board = self.board.get_or_insert_with(|| {
                Board::start_with(
                    ::verse::session::PUBLIC_RELAY,
                    ::verse::xp::openagents_trust(),
                    None,
                    Some(signer.clone()),
                )
            });
            board.tick();
            if let Some(snapshot) = board.snapshot.take() {
                self.snapshot = Some(snapshot);
            }
        }
        let state = match (&self.snapshot, &self.board) {
            _ if self.preview => "preview",
            (Some(_), _) => "ready",
            (None, Some(board)) if board.connected => "reading",
            _ => "connecting",
        };
        let empty = Snapshot::default();
        let snapshot = self.snapshot.as_ref().unwrap_or(&empty);
        let card = ::verse::xp::card(snapshot, std::slice::from_ref(&public_hex));
        let open_quests =
            ::verse::xp::open_quests(snapshot, std::slice::from_ref(&public_hex), unix_now());
        let playtest = self.playtest(&public_hex, &signer);
        Ok(TrainerPacket {
            schema: "openagents.trainer.v1",
            playtest,
            npub: nostr::nip19::encode_npub(&key.serialize()),
            tag: public_hex[..8].to_owned(),
            public_hex,
            state,
            relay: ::verse::session::PUBLIC_RELAY.to_owned(),
            referee_npub: ::verse::xp::OPENAGENTS_REFEREE
                .parse::<secp256k1::XOnlyPublicKey>()
                .map(|k| nostr::nip19::encode_npub(&k.serialize()))
                .unwrap_or_default(),
            curve: card.curve,
            xp: card.xp,
            level: card.level,
            next_level_at: card.next_level_at,
            to_next: card.to_next,
            titles: card.titles,
            awards: card.awards.into_iter().map(row).collect(),
            open_quests,
            note: NOTE,
            nsec: reveal.then(|| nostr::nip19::encode_nsec(&secret.secret_bytes())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: &str = "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
    const NPUB: &str = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";

    #[test]
    fn the_preview_card_shows_level_three_under_the_named_curve() {
        let mut trainer = Trainer::default();
        let packet = trainer.packet(WORLD, false, true).unwrap();
        assert_eq!(packet.state, "preview");
        assert_eq!(packet.npub, NPUB);
        assert_eq!(packet.tag, "17162c92");
        assert_eq!(packet.curve, "trainer-curve-v1");
        assert_eq!((packet.xp, packet.level), (300, 3));
        assert_eq!((packet.next_level_at, packet.to_next), (520, 220));
        assert_eq!(packet.awards.len(), 6);
        assert!(packet.awards.iter().all(|a| a.rule == "reproduce"
            && a.role == "reproducer"
            && a.xp == 50
            && a.link.starts_with("https://njump.me/note1")));
        assert!(packet.nsec.is_none());
        assert!(packet.note.contains("can't be spent"));
        let json = serde_json::to_string(&packet).unwrap();
        assert!(!json.contains(WORLD) && !json.contains("nsec1"));
    }

    #[test]
    fn playtest_xp_shows_beside_the_trainer_level_and_never_in_it() {
        let mut trainer = Trainer::default();
        let packet = trainer.packet(WORLD, false, true).unwrap();
        // The tutorial fixture's 300 trainer XP is level 3; the playtest
        // fixture's 55 XP is its own number and leaves the level alone.
        assert_eq!((packet.xp, packet.level), (300, 3));
        let playtest = &packet.playtest;
        assert_eq!(playtest.state, "preview");
        assert_eq!(playtest.xp, 55);
        assert_eq!(
            (
                playtest.accepted_reports,
                playtest.fixes_verified,
                playtest.sessions
            ),
            (1, 1, 1)
        );
        assert!(playtest.titles.iter().any(|t| t == "playtester"));
        assert!(packet.awards.iter().all(|a| a.rule != "playtest"));
        assert!(playtest.awards.iter().all(|a| a.rule == "playtest"));
    }

    #[test]
    fn only_an_explicit_reveal_returns_the_trainer_nsec() {
        let mut trainer = Trainer::default();
        let shown = trainer.packet(WORLD, true, true).unwrap();
        let nsec = shown.nsec.unwrap();
        assert_eq!(
            nostr::nip19::decode_nsec(&nsec).unwrap(),
            WORLD.parse::<SecretKey>().unwrap().secret_bytes()
        );
        assert!(trainer.packet(WORLD, false, true).unwrap().nsec.is_none());
    }

    #[test]
    fn a_bad_key_is_refused_without_echoing_it() {
        let mut trainer = Trainer::default();
        for bad in ["zz".repeat(32), "11".repeat(31), String::new()] {
            let error = trainer.packet(&bad, false, true).err().unwrap();
            assert_eq!(error, "Invalid trainer key");
        }
    }

    #[test]
    fn the_referee_is_the_openagents_referee() {
        let mut trainer = Trainer::default();
        let packet = trainer.packet(WORLD, false, true).unwrap();
        assert_eq!(
            packet.referee_npub,
            "npub1v59z5gklyzc4v7c8klhqd8nuffl426d3s7zyluu5suxyyjn4khrsrusf6k"
        );
        assert_eq!(packet.relay, "wss://relay.openagents.com");
    }
}
