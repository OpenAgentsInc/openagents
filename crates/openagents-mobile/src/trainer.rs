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
    /// Open quests from the trusted referee that no counted award has taken.
    pub open_quests: usize,
    pub note: &'static str,
    /// The trainer key's secret in NIP-19 form, only on an explicit reveal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsec: Option<String>,
}

/// The card's reader. It starts on the first `trainer` request and stops
/// when the app leaves the foreground; the last ledger stays for the card.
#[derive(Default)]
pub struct Trainer {
    board: Option<Board>,
    snapshot: Option<Snapshot>,
    preview: bool,
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

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Trainer {
    /// The app left the foreground: close the relay connection.
    pub fn pause(&mut self) {
        self.board = None;
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
                    Some(signer),
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
        let now = unix_now();
        let taken: std::collections::BTreeSet<&str> =
            snapshot.credits.iter().map(|c| c.quest.as_str()).collect();
        let open_quests = snapshot
            .quests
            .iter()
            .filter(|q| {
                q.trusted
                    && !q.conflict
                    && q.season.opens_at <= now
                    && now <= q.season.closes_at
                    && !taken.contains(q.address.as_str())
            })
            .count();
        Ok(TrainerPacket {
            schema: "openagents.trainer.v1",
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
            awards: card
                .awards
                .into_iter()
                .map(|a| AwardRow {
                    link: gateway_link(&a.award),
                    title: a.title,
                    quest: a.quest,
                    season: a.season,
                    rule: a.rule,
                    role: a.role,
                    xp: a.xp,
                    award: a.award,
                })
                .collect(),
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
