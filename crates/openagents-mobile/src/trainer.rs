//! The Account tab's trainer card: the level over your head in the Grid,
//! and the counted awards behind it.
//!
//! Your trainer key is the Verse world key, the one whose prefix the Grid
//! shows over your head. The card reads NIP-XP quests, awards, and
//! revocations from the public relay with Verse's reader
//! ([`verse::xp::Board`]), trusts the OpenAgents referee alone, re-checks
//! every award against the signed evidence it names, and names the curve
//! its level uses (`trainer-curve-v1`). It never spends or pays, and it
//! publishes one thing: the trainer profile (NIP-XP `13193`), signed by the
//! world key, and only after the person taps **Show my level** or **Hide
//! my level** and confirms; the key list on **Link a key** and
//! **Remove**; and the trainer card (`30194`) on **Export card**. Until a shown profile exists, no one's Grid
//! shows a level over this player's head.
//!
//! The world key's secret reaches the interface only in the direct reply
//! to a `trainer` request with `reveal: true`, which the Trainer Key screen
//! sends after the person taps **Reveal nsec** and confirms a warning, so
//! they can sign a reproduction with it on a computer.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use ::verse::xp::{Board, Snapshot};
use nostr::domain::Event;
use secp256k1::{Secp256k1, SecretKey};
use serde::Serialize;

/// Why XP exists and what it isn't, shown under the card.
pub const NOTE: &str =
    "XP shows work OpenAgents accepted. It can't be spent, traded, or converted into money.";

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
    /// The trainer profile: `none` (never published, so no level shows
    /// over this player's head in anyone's Grid), `shown`, or `hidden`.
    pub profile: &'static str,
    /// `idle`, `publishing`, or `failed` for the last profile change.
    pub profile_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_error: Option<String>,
    /// The other keys this trainer's profile lists, each `linked` (it
    /// signed a link back, so its XP counts here) or `waiting`.
    pub linked_keys: Vec<LinkedKey>,
    /// The trainer this world key is linked to both ways, as an npub, when
    /// it belongs to another key's trainer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_to: Option<String>,
    /// The last card export: `idle`, `publishing`, `published`, or
    /// `failed`.
    pub card_status: &'static str,
    /// Playtest XP and titles from the separate playtest referee, shown
    /// beside the trainer XP above and never summed into its level.
    pub playtest: PlaytestSection,
    /// The trainer key's secret in NIP-19 form, only on an explicit reveal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsec: Option<String>,
}

/// Why playtest XP is separate, shown under the playtest card.
pub const PLAYTEST_NOTE: &str = "Playtest XP is for playtest help we accepted: feedback, bugs we could reproduce, fixes, and sessions with a report. It doesn't count toward your trainer level. Joining earns nothing.";

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

/// The direct reply to a `trainer_export` request: the signed card as a
/// JSON file and its public link.
#[derive(Serialize)]
pub struct CardExport {
    pub schema: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// The signed `30194` event, pretty-printed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<String>,
    /// The card's `naddr` on a public Nostr gateway.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// A preview card is signed but never published.
    pub preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One key the trainer profile lists.
#[derive(Serialize)]
pub struct LinkedKey {
    pub npub: String,
    pub public_hex: String,
    /// `linked` or `waiting` (no link back yet).
    pub status: &'static str,
}

fn npub_of(hex: &str) -> String {
    hex.parse::<secp256k1::XOnlyPublicKey>()
        .map(|k| nostr::nip19::encode_npub(&k.serialize()))
        .unwrap_or_else(|_| hex.to_owned())
}

/// A public key typed as an npub or 64 hex characters, as lowercase hex.
fn parse_key(text: &str) -> Option<String> {
    let text = text.trim();
    if let Ok(bytes) = nostr::nip19::decode_npub(text) {
        return Some(bytes.iter().map(|b| format!("{b:02x}")).collect());
    }
    let hex = text.to_ascii_lowercase();
    (hex.len() == 64 && hex.parse::<secp256k1::XOnlyPublicKey>().is_ok()).then_some(hex)
}

/// Publishes an event signed by the world key. Blocking.
pub trait Publish: Send + Sync {
    /// Sends `event` to the public relay, authenticating as `secret`.
    ///
    /// # Errors
    ///
    /// When the relay can't be reached or refuses the event.
    fn publish(&self, secret: &SecretKey, event: &Event) -> Result<(), String>;
}

/// The live publisher: `wss://relay.openagents.com`, with NIP-42.
pub struct RelayPublish;

impl Publish for RelayPublish {
    fn publish(&self, secret: &SecretKey, event: &Event) -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "The phone could not start a Nostr connection.".to_owned())?;
        runtime
            .block_on(crate::payees::write_relay(
                ::verse::session::PUBLIC_RELAY,
                secret,
                event,
            ))
            .map_err(|_| "The relay didn't accept the change. Try again.".to_owned())
    }
}

/// The card's reader. It starts on the first `trainer` request and stops
/// when the app leaves the foreground; the last ledger stays for the card.
#[derive(Default)]
pub struct Trainer {
    board: Option<Board>,
    snapshot: Option<Snapshot>,
    preview: bool,
    /// Where profile changes go; the live relay when unset.
    publisher: Option<Arc<dyn Publish>>,
    /// A profile change on its way to the relay.
    publishing: Option<Receiver<Result<Event, String>>>,
    /// The newest profile this phone published, until the reader sees it.
    published: Option<Event>,
    publish_error: Option<String>,
    /// A card export on its way to the relay.
    card_publishing: Option<Receiver<Result<(), String>>>,
    card_status: &'static str,
    /// When the last card was signed, so the next one is newer.
    card_at: u64,
    /// The trust list the ledger was read under, which a card names; the
    /// OpenAgents referee unless a preview or a test says otherwise.
    trust: Option<::verse::xp::XpTrust>,
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
    /// A trainer whose profile changes go to `publisher` and whose reader
    /// is an empty, fixed ledger, so a test opens no connection.
    #[cfg(test)]
    pub fn with_publisher(publisher: Arc<dyn Publish>) -> Self {
        Self {
            publisher: Some(publisher),
            board: Some(Board::fixed("wss://test.invalid", Snapshot::default())),
            ..Self::default()
        }
    }

    /// This key's newest trainer profile: the reader's or the one this
    /// phone just published, whichever is newer.
    fn own_profile(&self, public_hex: &str) -> Option<(u64, bool, Vec<String>)> {
        let read = self
            .snapshot
            .as_ref()
            .and_then(|s| s.trainers.profiles.get(public_hex))
            .map(|p| (p.created_at, p.shown, p.keys.clone()));
        let mine = self.published.as_ref().and_then(|event| {
            nostr::xp::parse_profile(event)
                .ok()
                .map(|p| (event.created_at, p.shown, p.keys))
        });
        match (read, mine) {
            (Some(a), Some(b)) => Some(if b.0 > a.0 { b } else { a }),
            (a, b) => a.or(b),
        }
    }

    /// Takes the answer to a profile change, when it has arrived.
    fn settle_publish(&mut self) {
        let Some(rx) = &self.publishing else { return };
        match rx.try_recv() {
            Ok(Ok(event)) => {
                self.published = Some(event);
                self.publish_error = None;
                self.publishing = None;
            }
            Ok(Err(error)) => {
                self.publish_error = Some(error);
                self.publishing = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.publish_error = Some("The change didn't finish. Try again.".into());
                self.publishing = None;
            }
        }
    }

    /// Publishes a new trainer profile for the world key `secret_hex`:
    /// `shown` when given, else the current setting, listing `keys` when
    /// given, else the current ones. In a preview it changes the card
    /// alone and publishes nothing.
    ///
    /// # Errors
    ///
    /// A bad key, a change already on its way, or a list the profile
    /// refuses.
    pub fn set_profile(
        &mut self,
        secret_hex: &str,
        shown: Option<bool>,
        keys: Option<Vec<String>>,
    ) -> Result<(), String> {
        let secret: SecretKey = secret_hex
            .parse()
            .map_err(|_| "Invalid trainer key".to_owned())?;
        let signer = nostr::domain::RelaySigner::from_secret_hex(secret_hex)
            .map_err(|_| "Invalid trainer key".to_owned())?;
        self.settle_publish();
        if self.publishing.is_some() {
            return Err("A change is still being published.".into());
        }
        let public_hex = signer.pubkey().to_owned();
        let current = self.own_profile(&public_hex);
        let shown = shown.unwrap_or_else(|| current.as_ref().is_some_and(|c| c.1));
        let keys =
            keys.unwrap_or_else(|| current.as_ref().map(|c| c.2.clone()).unwrap_or_default());
        let parts =
            nostr::xp::profile(&public_hex, shown, &keys).map_err(|error| error.to_string())?;
        // A replaceable event must be newer than the one it replaces.
        let created_at = current.map_or(unix_now(), |c| unix_now().max(c.0 + 1));
        let event = signer.sign(created_at, parts.kind, parts.tags, parts.content);
        self.publish_error = None;
        if self.preview {
            self.published = Some(event);
            return Ok(());
        }
        let publisher = self
            .publisher
            .clone()
            .unwrap_or_else(|| Arc::new(RelayPublish));
        let (tx, rx) = mpsc::channel();
        self.publishing = Some(rx);
        std::thread::Builder::new()
            .name("trainer-profile".into())
            .spawn(move || {
                let _ = tx.send(publisher.publish(&secret, &event).map(|()| event));
            })
            .map_err(|_| "The phone could not start publishing.".to_owned())?;
        Ok(())
    }

    /// Adds `add` (an npub or hex key) to, or removes `remove` from, the
    /// keys the world key's trainer profile lists, and publishes it. The
    /// added key counts only after it signs a link back on its own device
    /// (`microcoder xp link`).
    ///
    /// # Errors
    ///
    /// A key that isn't a public key, the world key itself, or anything
    /// [`Trainer::set_profile`] refuses.
    pub fn set_link(
        &mut self,
        secret_hex: &str,
        add: Option<&str>,
        remove: Option<&str>,
    ) -> Result<(), String> {
        let signer = nostr::domain::RelaySigner::from_secret_hex(secret_hex)
            .map_err(|_| "Invalid trainer key".to_owned())?;
        let own = signer.pubkey().to_owned();
        let mut keys = self.own_profile(&own).map(|p| p.2).unwrap_or_default();
        if let Some(add) = add {
            let key = parse_key(add).ok_or("That isn't an npub or a hex public key.")?;
            if key == own {
                return Err("That's this phone's trainer key.".into());
            }
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        if let Some(remove) = remove {
            let key = parse_key(remove).ok_or("That isn't an npub or a hex public key.")?;
            keys.retain(|k| *k != key);
        }
        self.set_profile(secret_hex, None, Some(keys))
    }

    /// Signs the trainer card for the world key `secret_hex` from the
    /// current ledger (the OpenAgents referee, the public relay, the keys
    /// linked both ways) and publishes it at its address, so the link
    /// resolves. A preview signs the labeled fixture's card and publishes
    /// nothing.
    ///
    /// # Errors
    ///
    /// A bad key, no ledger read yet, or a card the protocol refuses.
    pub fn export(&mut self, secret_hex: &str) -> Result<CardExport, String> {
        let secret: SecretKey = secret_hex
            .parse()
            .map_err(|_| "Invalid trainer key".to_owned())?;
        let signer = nostr::domain::RelaySigner::from_secret_hex(secret_hex)
            .map_err(|_| "Invalid trainer key".to_owned())?;
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or("The card is still reading your XP. Try again in a moment.")?;
        let relay = ::verse::session::PUBLIC_RELAY.to_owned();
        let trust = self
            .trust
            .clone()
            .unwrap_or_else(::verse::xp::openagents_trust);
        let now = unix_now().max(self.card_at + 1);
        let card = ::verse::xp::trainer_card(
            snapshot,
            signer.pubkey(),
            &trust,
            std::slice::from_ref(&relay),
            now,
        );
        let parts = nostr::xp::card(&card).map_err(|e| e.to_string())?;
        let event = signer.sign(now, parts.kind, parts.tags, parts.content);
        self.card_at = now;
        let mut pubkey = [0u8; 32];
        for (i, byte) in pubkey.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&event.pubkey[2 * i..2 * i + 2], 16).unwrap_or(0);
        }
        let naddr = nostr::nip19::encode_naddr(&nostr::nip19::Naddr {
            identifier: nostr::xp::CARD_ADDRESS.to_owned(),
            pubkey,
            kind: u32::from(nostr::xp::CARD_KIND),
            relays: vec![relay],
        })
        .map_err(|_| "The card's link could not be made.".to_owned())?;
        let export = CardExport {
            schema: "openagents.trainer-card.v1",
            file_name: Some(format!("trainer-card-{}.json", &event.pubkey[..8])),
            json: serde_json::to_string_pretty(&event).ok(),
            link: Some(format!("https://njump.me/{naddr}")),
            preview: self.preview,
            error: None,
        };
        if !self.preview {
            let publisher = self
                .publisher
                .clone()
                .unwrap_or_else(|| Arc::new(RelayPublish));
            let (tx, rx) = mpsc::channel();
            self.card_publishing = Some(rx);
            self.card_status = "publishing";
            std::thread::Builder::new()
                .name("trainer-card".into())
                .spawn(move || {
                    let _ = tx.send(publisher.publish(&secret, &event));
                })
                .map_err(|_| "The phone could not start publishing.".to_owned())?;
        }
        Ok(export)
    }

    fn settle_card(&mut self) {
        let Some(rx) = &self.card_publishing else {
            return;
        };
        let status = match rx.try_recv() {
            Ok(Ok(())) => "published",
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => "failed",
            Err(mpsc::TryRecvError::Empty) => return,
        };
        self.card_status = status;
        self.card_publishing = None;
    }

    /// Shows `error` on the card as the last change's failure, so the
    /// screen can say why nothing was published.
    ///
    /// # Errors
    ///
    /// Never; it returns `Ok` so the caller answers with the card.
    #[allow(clippy::unnecessary_wraps)]
    pub fn refuse(&mut self, error: String) -> Result<(), String> {
        self.publish_error = Some(error);
        Ok(())
    }

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

    /// The Gym's view of this trainer from the ledger: name, level, XP,
    /// titles, what they made, and the eval-check quest's shares when a
    /// trusted quest states them. It starts the reader when none runs.
    pub(crate) fn standing(&mut self, secret: &SecretKey) -> crate::gym::Standing {
        let (key, _) = secret.x_only_public_key(&Secp256k1::new());
        let public_hex = key.to_string();
        if !self.preview {
            let board = self.board.get_or_insert_with(|| {
                Board::start_with(
                    ::verse::session::PUBLIC_RELAY,
                    ::verse::xp::openagents_trust(),
                    None,
                    nostr::domain::RelaySigner::from_secret_hex(
                        &secret.display_secret().to_string(),
                    )
                    .ok(),
                )
            });
            board.tick();
            if let Some(snapshot) = board.snapshot.take() {
                self.snapshot = Some(snapshot);
            }
        }
        let name = crate::eval_cards::trainer_name(&public_hex);
        let Some(snapshot) = self.snapshot.as_ref() else {
            return crate::gym::Standing {
                name,
                public_hex,
                ..crate::gym::Standing::default()
            };
        };
        let keys = ::verse::xp::trainer_keys(snapshot, &public_hex);
        let card = ::verse::xp::card(snapshot, &keys);
        let made = ::verse::xp::made(snapshot, &keys);
        let level = ::verse::xp::level_of(card.xp);
        let share = |role: &str| {
            snapshot
                .quests
                .iter()
                .find(|q| q.trusted && q.rule == nostr::xp::EVAL_CHECK)
                .and_then(|q| q.split.iter().find(|(r, _)| r == role).map(|(_, xp)| *xp))
        };
        let row = |r: &xp_ledger::eval::MadeResult, check: bool| crate::gym::MadeRow {
            id: r.id.clone(),
            verdict: r.verdict.clone(),
            confirmed_by: r.confirmed_by,
            standing: serde_json::to_value(r.standing)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            xp: r.xp,
            check,
            suite: r.suite.clone(),
        };
        crate::gym::Standing {
            name,
            public_hex,
            read: true,
            xp: card.xp,
            level,
            level_at: ::verse::xp::xp_to_reach(level),
            next_at: ::verse::xp::xp_to_reach(level + 1),
            titles: card.titles.clone(),
            suites: made
                .suites
                .iter()
                .map(|s| (s.release.clone(), s.results, s.xp))
                .collect(),
            results: made
                .results
                .iter()
                .map(|r| row(r, false))
                .chain(made.checks.iter().map(|r| row(r, true)))
                .collect(),
            adoptions: made
                .adoptions
                .iter()
                .map(|c| (c.title.clone(), c.xp))
                .collect(),
            pending: made.pending,
            eval_xp: made.xp,
            checker_xp: share("checker"),
            evaluator_xp: share("evaluator"),
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
            self.trust = Some(trust);
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
        // XP sums over the keys linked to this trainer both ways.
        let keys = ::verse::xp::trainer_keys(snapshot, &public_hex);
        let card = ::verse::xp::card(snapshot, &keys);
        let linked_to = snapshot
            .trainers
            .trainer_of(&public_hex)
            .filter(|t| *t != public_hex)
            .map(npub_of);
        let open_quests =
            ::verse::xp::open_quests(snapshot, std::slice::from_ref(&public_hex), unix_now());
        let playtest = self.playtest(&public_hex, &signer);
        self.settle_publish();
        self.settle_card();
        let own = self.own_profile(&public_hex);
        let linked_keys = own
            .as_ref()
            .map(|p| p.2.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|key| LinkedKey {
                npub: npub_of(&key),
                status: if self.snapshot.as_ref().is_some_and(|s| {
                    s.trainers.linked.get(&key).map(String::as_str) == Some(public_hex.as_str())
                }) {
                    "linked"
                } else {
                    "waiting"
                },
                public_hex: key,
            })
            .collect();
        let profile = match self.own_profile(&public_hex) {
            None => "none",
            Some((_, true, _)) => "shown",
            Some((_, false, _)) => "hidden",
        };
        let profile_status = match (&self.publishing, &self.publish_error) {
            (Some(_), _) => "publishing",
            (None, Some(_)) => "failed",
            (None, None) => "idle",
        };
        Ok(TrainerPacket {
            profile,
            profile_status,
            profile_error: self.publish_error.clone(),
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
            linked_keys,
            linked_to,
            card_status: if self.card_status.is_empty() {
                "idle"
            } else {
                self.card_status
            },
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

    /// Records what would reach the relay.
    #[derive(Default)]
    struct Recorded(std::sync::Mutex<Vec<Event>>);

    impl Publish for Recorded {
        fn publish(&self, _secret: &SecretKey, event: &Event) -> Result<(), String> {
            self.0.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    fn settle(trainer: &mut Trainer) -> TrainerPacket {
        for _ in 0..200 {
            let packet = trainer.packet(WORLD, false, false).unwrap();
            if packet.profile_status != "publishing" {
                return packet;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the profile never published");
    }

    #[test]
    fn a_level_shows_only_after_the_trainer_publishes_a_shown_profile() {
        let relay = Arc::new(Recorded::default());
        let mut trainer = Trainer::with_publisher(relay.clone());
        // Nothing is published by reading the card.
        let packet = trainer.packet(WORLD, false, false).unwrap();
        assert_eq!((packet.profile, packet.profile_status), ("none", "idle"));
        assert!(relay.0.lock().unwrap().is_empty());

        trainer.set_profile(WORLD, Some(true), None).unwrap();
        let packet = settle(&mut trainer);
        assert_eq!((packet.profile, packet.profile_status), ("shown", "idle"));
        let sent = relay.0.lock().unwrap().clone();
        assert_eq!(sent.len(), 1);
        let world = WORLD.parse::<SecretKey>().unwrap();
        let (public, _) = world.x_only_public_key(&Secp256k1::new());
        assert_eq!(sent[0].pubkey, public.to_string());
        assert_eq!(sent[0].kind, 13_193);
        let profile = nostr::xp::parse_profile(&sent[0]).unwrap();
        assert!(profile.shown && profile.keys.is_empty());
        let json = serde_json::to_string(&packet).unwrap();
        assert!(!json.contains(WORLD) && !json.contains("nsec1"));

        // Hiding replaces it with a newer profile.
        trainer.set_profile(WORLD, Some(false), None).unwrap();
        let packet = settle(&mut trainer);
        assert_eq!(packet.profile, "hidden");
        let sent = relay.0.lock().unwrap().clone();
        assert_eq!(sent.len(), 2);
        assert!(sent[1].created_at > sent[0].created_at);
        assert!(!nostr::xp::parse_profile(&sent[1]).unwrap().shown);
    }

    #[test]
    fn a_refused_profile_says_so_and_changes_nothing() {
        struct Refuses;
        impl Publish for Refuses {
            fn publish(&self, _: &SecretKey, _: &Event) -> Result<(), String> {
                Err("The relay didn't accept the change. Try again.".into())
            }
        }
        let mut trainer = Trainer::with_publisher(Arc::new(Refuses));
        trainer.set_profile(WORLD, Some(true), None).unwrap();
        let packet = settle(&mut trainer);
        assert_eq!((packet.profile, packet.profile_status), ("none", "failed"));
        assert!(packet.profile_error.unwrap().contains("Try again"));
    }

    #[test]
    fn a_preview_profile_change_publishes_nothing() {
        let relay = Arc::new(Recorded::default());
        let mut trainer = Trainer::with_publisher(relay.clone());
        assert_eq!(trainer.packet(WORLD, false, true).unwrap().profile, "shown");
        trainer.set_profile(WORLD, Some(false), None).unwrap();
        assert_eq!(
            trainer.packet(WORLD, false, false).unwrap().profile,
            "hidden"
        );
        assert!(relay.0.lock().unwrap().is_empty());
    }

    #[test]
    fn linking_a_key_lists_it_and_its_link_back_counts_its_xp() {
        let relay = Arc::new(Recorded::default());
        let mut trainer = Trainer::with_publisher(relay.clone());
        let laptop = ::verse::xp::fixture::signer(0x1a_97);
        let laptop_hex = laptop.pubkey().to_owned();
        let laptop_npub = npub_of(&laptop_hex);
        // Bad keys and this phone's own key are refused.
        assert!(trainer.set_link(WORLD, Some("npub1nope"), None).is_err());
        let world = WORLD.parse::<SecretKey>().unwrap();
        let own = world.x_only_public_key(&Secp256k1::new()).0.to_string();
        assert!(trainer.set_link(WORLD, Some(&own), None).is_err());
        assert!(relay.0.lock().unwrap().is_empty());

        trainer.set_link(WORLD, Some(&laptop_npub), None).unwrap();
        let packet = settle(&mut trainer);
        assert_eq!(packet.linked_keys.len(), 1);
        assert_eq!(packet.linked_keys[0].status, "waiting");
        assert_eq!(packet.linked_keys[0].npub, laptop_npub);
        // Adding a key keeps the level hidden until the person shows it.
        assert_eq!(packet.profile, "hidden");
        let sent = relay.0.lock().unwrap().clone();
        let profile = nostr::xp::parse_profile(&sent[0]).unwrap();
        assert_eq!(profile.keys, std::slice::from_ref(&laptop_hex));

        // The laptop signs its link back; its tutorial XP counts here.
        let referee = ::verse::xp::fixture::signer(0x0a_de_fe_ee);
        let mut events = ::verse::xp::fixture::tutorial_events(&referee, &laptop, 2, unix_now());
        events.push(sent[0].clone());
        let parts = nostr::xp::link(&laptop_hex, Some(&own)).unwrap();
        events.push(laptop.sign(unix_now(), parts.kind, parts.tags, parts.content));
        let mut trust = ::verse::xp::openagents_trust();
        trust.referees = std::collections::BTreeSet::from([referee.pubkey().to_owned()]);
        trainer.board = Some(Board::fixed(
            "wss://test.invalid",
            ::verse::xp::snapshot(&events, &trust),
        ));
        let packet = trainer.packet(WORLD, false, false).unwrap();
        assert_eq!(packet.linked_keys[0].status, "linked");
        assert_eq!((packet.xp, packet.level), (100, 2));

        // Removing it publishes a profile without it.
        trainer.set_link(WORLD, None, Some(&laptop_hex)).unwrap();
        let packet = settle(&mut trainer);
        assert!(packet.linked_keys.is_empty());
        let sent = relay.0.lock().unwrap().clone();
        assert!(nostr::xp::parse_profile(&sent[1]).unwrap().keys.is_empty());
    }

    #[test]
    fn exporting_signs_a_card_that_checks_and_publishes_it_with_a_link() {
        let relay = Arc::new(Recorded::default());
        let mut trainer = Trainer::with_publisher(relay.clone());
        // Nothing to export before the ledger is read.
        let mut empty = Trainer::with_publisher(relay.clone());
        empty.board = None;
        assert!(empty.export(WORLD).is_err());

        // A real ledger: two tutorial awards to this key.
        let referee = ::verse::xp::fixture::signer(0x0a_de_fe_ee);
        let signer = nostr::domain::RelaySigner::from_secret_hex(WORLD).unwrap();
        let events = ::verse::xp::fixture::tutorial_events(&referee, &signer, 2, unix_now());
        let mut trust = ::verse::xp::openagents_trust();
        trust.referees = std::collections::BTreeSet::from([referee.pubkey().to_owned()]);
        let snapshot = ::verse::xp::snapshot(&events, &trust);
        trainer.board = Some(Board::fixed("wss://test.invalid", snapshot.clone()));
        trainer.trust = Some(trust.clone());
        trainer.packet(WORLD, false, false).unwrap();
        let export = trainer.export(WORLD).unwrap();
        assert!(!export.preview);
        let link = export.link.unwrap();
        assert!(link.starts_with("https://njump.me/naddr1"));
        let naddr =
            nostr::nip19::decode_naddr(link.trim_start_matches("https://njump.me/")).unwrap();
        assert_eq!(
            (naddr.identifier.as_str(), naddr.kind),
            ("trainer-card", 30_194)
        );
        let json = export.json.unwrap();
        assert!(!json.contains(WORLD) && !json.contains("nsec1"));
        let event: Event = serde_json::from_str(&json).unwrap();
        let card = nostr::xp::parse_card(&event).unwrap();
        // The card names the trust list its ledger was read under, and a
        // reader under that list derives the same card.
        assert_eq!(card.referees, [referee.pubkey().to_owned()]);
        assert_eq!((card.xp, card.level), (100, 2));
        let check = ::verse::xp::check_card(&card, &event.pubkey, &snapshot);
        assert!(check.differences.is_empty(), "{:?}", check.differences);
        // A reader trusting only the OpenAgents referee disagrees, and says so.
        let other = ::verse::xp::snapshot(&events, &::verse::xp::openagents_trust());
        assert!(
            !::verse::xp::check_card(&card, &event.pubkey, &other)
                .differences
                .is_empty()
        );
        // It reached the relay, and the card says so.
        for _ in 0..200 {
            if trainer.packet(WORLD, false, false).unwrap().card_status != "publishing" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            trainer.packet(WORLD, false, false).unwrap().card_status,
            "published"
        );
        assert_eq!(relay.0.lock().unwrap().last().unwrap().id, event.id);
        // A second export is newer, so it replaces the first at its address.
        let again: Event =
            serde_json::from_str(&trainer.export(WORLD).unwrap().json.unwrap()).unwrap();
        assert!(again.created_at > event.created_at);
    }

    #[test]
    fn a_preview_card_is_signed_but_never_published() {
        let relay = Arc::new(Recorded::default());
        let mut trainer = Trainer::with_publisher(relay.clone());
        trainer.packet(WORLD, false, true).unwrap();
        let export = trainer.export(WORLD).unwrap();
        assert!(export.preview);
        let event: Event = serde_json::from_str(&export.json.unwrap()).unwrap();
        let card = nostr::xp::parse_card(&event).unwrap();
        assert_eq!((card.xp, card.level, card.awards.len()), (300, 3, 6));
        assert!(relay.0.lock().unwrap().is_empty());
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
