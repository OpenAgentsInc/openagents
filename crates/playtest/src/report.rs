//! A playtest report and its private transport.
//!
//! The app fills in the [`Context`] (build, device, tab, screen, time) and
//! the tester writes what happened, what they expected, and the steps, with
//! a [`Kind`]. The report travels as the content of a NIP-17 kind-14
//! message (JSON, schema [`SCHEMA`]) to the triage key, sealed with NIP-44
//! and signed by the tester's Verse world key ([`wrap`]). The kind-14
//! rumor's event ID is the report's identity: its first eight hex digits,
//! as `PT-1A2B3C4D`, are the report code the tester can quote. The SHA-256
//! of the exact content bytes is the report's [`digest`]. Beside the
//! private message, [`wrap`] signs the public, content-free NIP-XP playtest
//! report (kind `3197`, [`public_record`]) with the same key: the build,
//! the platform, the kind, and that digest, and no text. It is what a
//! playtest award cites, so an accepted report can earn XP.
//!
//! A screenshot is never part of a report from the Wallet tab or a screen
//! that can show a key ([`crate::session::Route::sensitive`]); [`Report::check`]
//! refuses one, whatever the host sends.

use nostr::domain::{Event, Tag};
use nostr::nip17;
use secp256k1::{Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::session::{self, Route, Tab};

/// The report body's schema.
pub const SCHEMA: &str = "openagents.playtest-report.v1";
/// The NIP-17 `subject` of a report message.
pub const SUBJECT: &str = "OpenAgents playtest report";
/// The `t` tag that marks a kind-14 message as a playtest report.
pub const MARKER: &str = "oa:playtest:report:v1";
/// Characters each written field may have.
pub const MAX_TEXT_CHARS: usize = 2_000;
/// Bytes the report's JSON may have. NIP-44 seals at most 65,535 bytes,
/// and a report is sealed twice (the seal, then the gift wrap), so the
/// content stays well under two thirds of that.
pub const MAX_CONTENT_BYTES: usize = 40_000;

/// What kind of report it is. "Felt good" is on purpose: playtesting asks
/// what is fun, not only what is broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Bug,
    Confusing,
    Idea,
    FeltGood,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Ios,
    Android,
}

/// What the app fills in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// `1.0.0`.
    pub app_version: String,
    /// `15`.
    pub build: String,
    pub platform: Platform,
    /// The device model, such as `iPhone17,1`.
    pub device: String,
    /// The operating system version, such as `26.0`.
    pub os_version: String,
    pub tab: Tab,
    pub route: Route,
    /// Unix seconds when the report was written.
    pub at: u64,
}

impl Context {
    /// `1.0.0 (15)`, as the Changelog and the triage log name a build.
    #[must_use]
    pub fn build_label(&self) -> String {
        format!("{} ({})", self.app_version, self.build)
    }
}

/// A screenshot the tester previewed, cropped, and chose to attach.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screenshot {
    /// Standard base64 of a JPEG.
    pub jpeg_base64: String,
    pub width: u32,
    pub height: u32,
}

/// Something the app changed so the report would fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Note {
    /// The screenshot was too large to send and was left out.
    ScreenshotDropped,
    /// The oldest session events were left out to fit.
    SessionTrimmed,
    /// The oldest messages of a shared chat were left out to fit.
    ChatTrimmed,
}

/// Why a chat is in a report: evaluation data for the chat router
/// (`docs/coder/design/2026-09-28-chat-router.md`), sent only by the
/// tester's own choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareReason {
    /// The tester ticked **Share this chat** on Report a problem, after
    /// seeing the whole chat.
    Shared,
    /// The tester said a prepared answer was wrong: the question, that
    /// answer, and how the worker chose it.
    WrongAnswer,
}

/// Who wrote a shared chat message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

/// One message of a shared chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatTurn {
    pub role: ChatRole,
    pub text: String,
    /// The prepared answer that is this message's text, as `id@version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    /// What the worker decided to show first (`canned`, `opener`, `model`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// The worker's typed judgment of the turn, as the NIP-CJ `judgment`
    /// feedback carried it (JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<String>,
}

/// A chat the tester chose to send with a report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedChat {
    pub reason: ShareReason,
    /// Oldest first.
    pub turns: Vec<ChatTurn>,
}

/// Characters one shared chat message may have.
pub const MAX_CHAT_TEXT_CHARS: usize = 4_000;
/// Bytes one shared judgment may have.
pub const MAX_JUDGMENT_BYTES: usize = 2_048;
/// Messages one shared chat may have.
pub const MAX_CHAT_TURNS: usize = 200;

impl SharedChat {
    /// The chat as the tester previews it, one line per message, exactly
    /// what [`chat_digest`] commits to.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.turns
            .iter()
            .map(|turn| {
                let who = match turn.role {
                    ChatRole::User => "You",
                    ChatRole::Assistant => "OpenAgents",
                };
                match &turn.answer {
                    Some(answer) => format!("{who} (prepared answer {answer}): {}", turn.text),
                    None => format!("{who}: {}", turn.text),
                }
            })
            .collect()
    }

    /// Checks the bounds, and refuses a chat with a secret key in it.
    ///
    /// # Errors
    ///
    /// A sentence for the tester naming the first problem.
    pub fn check(&self) -> Result<(), String> {
        if self.turns.is_empty() {
            return Err("There's no chat to share.".into());
        }
        if self.turns.len() > MAX_CHAT_TURNS {
            return Err("The chat is too long to share.".into());
        }
        for turn in &self.turns {
            if turn.text.chars().count() > MAX_CHAT_TEXT_CHARS
                || turn.answer.as_ref().is_some_and(|a| a.len() > 96)
                || turn.tier.as_ref().is_some_and(|t| t.len() > 16)
                || turn
                    .judgment
                    .as_ref()
                    .is_some_and(|j| j.len() > MAX_JUDGMENT_BYTES)
            {
                return Err("A message in the chat is too long to share.".into());
            }
            if turn.text.contains("nsec1") {
                return Err(
                    "This chat has a secret key in it, so it can't be shared. Describe the problem in words."
                        .into(),
                );
            }
        }
        Ok(())
    }
}

/// The lowercase hex SHA-256 of a shared chat's JSON: the preview's
/// identity, so a report attaches exactly the chat the tester saw.
#[must_use]
pub fn chat_digest(chat: &SharedChat) -> String {
    digest(&serde_json::to_string(chat).unwrap_or_default())
}

/// One report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub context: Context,
    pub kind: Kind,
    /// What happened.
    pub happened: String,
    /// What the tester expected.
    pub expected: String,
    /// The steps that led there.
    pub steps: String,
    /// The tester allows quoting their words in a public issue.
    pub quote: bool,
    /// The Coder chat's task ID, only when the tester ticked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The playtest log the tester previewed, only while playtest logging
    /// is on and the tester chose to attach it. The field keeps its
    /// original `session` name on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<Vec<session::Event>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<Screenshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    /// A chat the tester chose to send, from the Chat tab only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<SharedChat>,
}

fn version_like(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value.split('.').count() <= 4
        && value
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn bounded(name: &str, value: &str, required: bool) -> Result<(), String> {
    if required && value.trim().is_empty() {
        return Err(format!("Write {name}."));
    }
    if value.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("Keep {name} under {MAX_TEXT_CHARS} characters."));
    }
    if value.contains("nsec1") {
        return Err(
            "A report never needs a secret key. Take the nsec out and describe the problem in words."
                .into(),
        );
    }
    Ok(())
}

impl Report {
    /// Checks every field: the closed sets, the bounds, a JPEG screenshot
    /// of a reasonable size, and no screenshot from the Wallet or a key
    /// screen.
    ///
    /// # Errors
    ///
    /// A sentence for the tester naming the first problem.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err("This report is from another version of the app.".into());
        }
        let c = &self.context;
        if !version_like(&c.app_version)
            || c.build.is_empty()
            || c.build.len() > 8
            || !c.build.bytes().all(|b| b.is_ascii_digit())
            || !version_like(&c.os_version)
        {
            return Err("The app's version or build couldn't be read.".into());
        }
        if c.device.is_empty()
            || c.device.len() > 40
            || !c
                .device
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || " ,._-()".contains(ch))
        {
            return Err("The device model couldn't be read.".into());
        }
        bounded("what happened", &self.happened, true)?;
        bounded("what you expected", &self.expected, false)?;
        bounded("the steps", &self.steps, false)?;
        if let Some(task) = &self.task
            && (task.is_empty()
                || task.len() > 96
                || !task
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "_.:-".contains(ch)))
        {
            return Err("The chat's task ID couldn't be read.".into());
        }
        if let Some(session) = &self.session
            && session.len() > session::MAX_EVENTS
        {
            return Err("The session log is too long.".into());
        }
        if let Some(chat) = &self.chat {
            if c.tab != Tab::Coder {
                return Err("A chat is shared only from the Chat tab.".into());
            }
            chat.check()?;
        }
        if let Some(shot) = &self.screenshot {
            if c.route.sensitive(c.tab) {
                return Err(
                    "Screenshots are never sent from the Wallet or a key screen. Describe it in words."
                        .into(),
                );
            }
            let b64 = &shot.jpeg_base64;
            if !b64.starts_with("/9j/")
                || b64.len() % 4 != 0
                || !b64
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
                || shot.width == 0
                || shot.height == 0
                || shot.width > 4096
                || shot.height > 4096
            {
                return Err("The screenshot isn't a JPEG image.".into());
            }
        }
        Ok(())
    }

    /// The report's JSON: the exact content of the private message.
    #[must_use]
    pub fn content(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Makes the report fit [`MAX_CONTENT_BYTES`]: first without the
    /// screenshot, then without the oldest session events, then without a
    /// shared chat's oldest messages (keeping its newest), noting each.
    /// The written fields are bounded, so the result always fits.
    #[must_use]
    pub fn fit(mut self) -> Self {
        if self.content().len() > MAX_CONTENT_BYTES && self.screenshot.take().is_some() {
            self.notes.push(Note::ScreenshotDropped);
        }
        let mut trimmed = false;
        while self.content().len() > MAX_CONTENT_BYTES {
            match self.session.as_mut() {
                Some(events) if !events.is_empty() => {
                    let drop = events.len().div_ceil(4);
                    events.drain(..drop);
                    trimmed = true;
                }
                _ => break,
            }
        }
        if trimmed {
            self.notes.push(Note::SessionTrimmed);
        }
        let mut cut = false;
        while self.content().len() > MAX_CONTENT_BYTES {
            match self.chat.as_mut() {
                Some(chat) if chat.turns.len() > 1 => {
                    chat.turns.remove(0);
                    cut = true;
                }
                Some(chat) => {
                    // One message alone: keep its start.
                    let turn = &mut chat.turns[0];
                    let keep = turn.text.chars().count() / 2;
                    if keep == 0 {
                        self.chat = None;
                    } else {
                        turn.text = turn.text.chars().take(keep).collect();
                    }
                    cut = true;
                }
                None => break,
            }
        }
        if cut {
            self.notes.push(Note::ChatTrimmed);
        }
        self
    }
}

/// The lowercase hex SHA-256 of a report's exact content bytes.
#[must_use]
pub fn digest(content: &str) -> String {
    Sha256::digest(content.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `PT-1A2B3C4D`: the first eight hex digits of the report message's ID.
#[must_use]
pub fn code(rumor_id: &str) -> String {
    format!(
        "PT-{}",
        rumor_id.get(..8).unwrap_or(rumor_id).to_ascii_uppercase()
    )
}

/// A sealed report, ready for the relay.
#[derive(Clone, Debug)]
pub struct Sealed {
    /// The kind-14 rumor's event ID.
    pub rumor_id: String,
    pub code: String,
    pub digest: String,
    /// The kind-1059 gift wrap addressed to the triage key.
    pub wrap: Event,
    /// The public, content-free NIP-XP playtest report (kind `3197`),
    /// signed by the tester, committing to [`Sealed::digest`].
    pub public: Event,
}

/// The randomness a seal needs, supplied by the caller so tests repeat.
pub struct Randomness {
    /// The one-time key that signs the gift wrap.
    pub wrapper: SecretKey,
    pub seal_nonce: [u8; 32],
    pub wrap_nonce: [u8; 32],
    /// Seconds (at most two days) to move each outer timestamp earlier.
    pub seal_earlier: u64,
    pub wrap_earlier: u64,
}

/// Seals `report` from `tester` (the Verse world key) to `triage`.
///
/// # Errors
///
/// When the report fails [`Report::check`] or is too large to seal.
pub fn wrap(
    report: &Report,
    tester: &SecretKey,
    triage: &XOnlyPublicKey,
    random: &Randomness,
) -> Result<Sealed, String> {
    report.check()?;
    let content = report.content();
    if content.len() > MAX_CONTENT_BYTES {
        return Err("The report is too large to send.".into());
    }
    let (author, _) = tester.x_only_public_key(&Secp256k1::new());
    let at = report.context.at;
    let tags = vec![
        Tag::new(vec!["p".into(), triage.to_string()]),
        Tag::new(vec!["subject".into(), SUBJECT.into()]),
        Tag::new(vec!["t".into(), MARKER.into()]),
    ];
    let failed = |_| "The report couldn't be sealed.".to_string();
    let rumor = nip17::chat_rumor(&author.to_string(), at, &content, tags).map_err(failed)?;
    let seal = nip17::seal(
        &rumor,
        tester,
        triage,
        nip17::hidden_timestamp(at, random.seal_earlier).map_err(failed)?,
        random.seal_nonce,
        None,
    )
    .map_err(failed)?;
    let wrap = nip17::gift_wrap(
        &seal,
        &random.wrapper,
        triage,
        nip17::hidden_timestamp(at, random.wrap_earlier).map_err(failed)?,
        random.wrap_nonce,
        None,
    )
    .map_err(failed)?;
    let digest = digest(&content);
    let public = public_record(report, &digest, tester)?;
    Ok(Sealed {
        code: code(&rumor.id),
        digest,
        rumor_id: rumor.id,
        wrap,
        public,
    })
}

/// The public NIP-XP playtest report (kind `3197`) for `report`, whose
/// private content hashes to `digest`, signed by `tester` at the report's
/// time. It holds the build, the platform, and the kind, and no text.
///
/// # Errors
///
/// When a field is outside the NIP-XP grammar.
pub fn public_record(report: &Report, digest: &str, tester: &SecretKey) -> Result<Event, String> {
    let parts = nostr::xp::playtest::playtest_report(
        &report.context.build_label(),
        &session::name(&report.context.platform),
        &session::name(&report.kind),
        digest,
        None,
    )
    .map_err(|_| "The report's public record couldn't be made.".to_string())?;
    let signer = nostr::domain::RelaySigner::from_secret_hex(&tester.display_secret().to_string())
        .map_err(|_| "The report's public record couldn't be signed.".to_string())?;
    Ok(signer.sign(report.context.at, parts.kind, parts.tags, parts.content))
}

/// A report the triage key opened.
#[derive(Clone, Debug)]
pub struct Opened {
    /// The tester's key (hex), which signed the seal.
    pub tester: String,
    pub rumor_id: String,
    pub code: String,
    pub digest: String,
    /// The gift wrap's event ID, for deduplicating deliveries.
    pub wrap_id: String,
    pub report: Report,
}

/// Opens a gift wrap with the triage key and reads the report in it.
///
/// # Errors
///
/// When the wrap isn't for this key, isn't a playtest report, or holds a
/// report that fails [`Report::check`].
pub fn open(wrap: &Event, triage: &SecretKey) -> Result<Opened, String> {
    let rumor = nip17::open_direct_message(wrap, triage).map_err(|e| e.to_string())?;
    let message = nip17::chat_message(&rumor).map_err(|e| e.to_string())?;
    let marked = rumor
        .tags
        .iter()
        .any(|t| t.name() == Some("t") && t.value() == Some(MARKER));
    if !marked {
        return Err("not a playtest report".into());
    }
    let report: Report = serde_json::from_str(&message.content)
        .map_err(|e| format!("the report doesn't parse: {e}"))?;
    report.check()?;
    Ok(Opened {
        tester: rumor.pubkey.clone(),
        code: code(&rumor.id),
        digest: digest(&message.content),
        rumor_id: rumor.id,
        wrap_id: wrap.id.clone(),
        report,
    })
}

#[cfg(test)]
mod tests;
