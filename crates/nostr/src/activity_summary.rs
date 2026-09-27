//! Audience-bound activity summaries (NIP-WS).
//!
//! A host publishes one bounded summary per task or session as a private
//! `3188` artifact, sealed separately to each enrolled device that holds the
//! `observe` right. A push wake carries nothing; the device fetches this
//! summary afterwards. The summary names a phase, a headline of at most 160
//! bytes, an attention reason, and an update time. It grants nothing.
//!
//! [`encode`] builds a summary from host state and applies the disclosure
//! rules: failure detail becomes a generic phrase, and headline tokens that
//! look like paths, URLs, addresses, assignments, or credentials become
//! `[redacted]`. [`verify`] refuses any body that encoding would change, so a
//! reader accepts only bodies that already satisfy those rules. Neither
//! function can prove that a headline did not come from a prompt or engine
//! output; the host must build headlines from its own typed state.

use std::str::FromStr;

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::{Map, Value, json};

use crate::contracts::{self, ContractError, RefusalCode};
use crate::domain::Event;
use crate::private_artifact;

/// Artifact schema and body version.
pub const SCHEMA: &str = "openagents.activity-summary.v1";
/// Media type of the inline artifact.
pub const MEDIA_TYPE: &str = "application/json";
/// Maximum headline length in UTF-8 bytes.
pub const MAX_HEADLINE_BYTES: usize = 160;
/// Maximum encoded body length in bytes.
pub const MAX_BODY_BYTES: usize = 1_024;
/// Maximum requested retention after `updated_at`, in seconds.
pub const MAX_RETENTION_SECONDS: u64 = 604_800;
/// Replacement text for a withheld headline token.
pub const REDACTED: &str = "[redacted]";

const FIELDS: [&str; 9] = [
    "v",
    "requires",
    "host",
    "subject",
    "sequence",
    "phase",
    "headline",
    "attention",
    "updated_at",
];
const CREDENTIAL_PREFIXES: [&str; 16] = [
    "sk-",
    "sk_",
    "pk_",
    "rk_",
    "nsec1",
    "ncryptsec",
    "oak_",
    "sess_",
    "ghp_",
    "gho_",
    "ghs_",
    "ghu_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "akia",
];

/// What the summary describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectKind {
    /// A durable host task.
    Task,
    /// An engine session.
    Session,
}

/// Coarse lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Admitted and not started.
    Queued,
    /// Running.
    Running,
    /// Paused until someone acts.
    Waiting,
    /// Finished.
    Completed,
    /// Ended in failure. The headline is the generic failure phrase.
    Failed,
    /// Cancelled.
    Cancelled,
    /// The host cannot establish the phase.
    Unknown,
}

/// Why the summary asks for attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// Nothing needs the reader.
    None,
    /// An approval waits. Requires [`Phase::Waiting`].
    Approval,
    /// Input waits. Requires [`Phase::Waiting`].
    Input,
    /// The work finished. Requires [`Phase::Completed`].
    Completed,
    /// The work failed. Required by, and requires, [`Phase::Failed`].
    Failed,
}

/// One verified activity summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivitySummary {
    /// Host pubkey, 64 lowercase hex characters. Equals the envelope signer.
    pub host: String,
    /// Task or session.
    pub subject_kind: SubjectKind,
    /// Host-issued subject ID, 64 lowercase hex characters.
    pub subject: String,
    /// Per-subject counter. A higher value supersedes a lower one.
    pub sequence: u64,
    /// Coarse lifecycle phase.
    pub phase: Phase,
    /// Bounded, redacted, single-line headline.
    pub headline: String,
    /// Attention reason.
    pub attention: Attention,
    /// Unix seconds of the state this summary reflects.
    pub updated_at: u64,
}

/// Host state before the disclosure rules apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryDraft<'a> {
    /// Host pubkey.
    pub host: &'a str,
    /// Task or session.
    pub subject_kind: SubjectKind,
    /// Subject ID.
    pub subject: &'a str,
    /// Per-subject counter.
    pub sequence: u64,
    /// Coarse lifecycle phase.
    pub phase: Phase,
    /// Host-authored title, such as an owner-set task title. Never a prompt,
    /// engine output, path, or credential; the encoder redacts only what it
    /// can recognize.
    pub headline: &'a str,
    /// Attention reason.
    pub attention: Attention,
    /// Unix seconds of the state.
    pub updated_at: u64,
}

impl SubjectKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Session => "session",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "task" => Some(Self::Task),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

impl Phase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "waiting" => Self::Waiting,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

impl Attention {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Approval => "approval",
            Self::Input => "input",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "none" => Self::None,
            "approval" => Self::Approval,
            "input" => Self::Input,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

/// The generic phrase that replaces every failure detail.
#[must_use]
pub const fn failure_phrase(kind: SubjectKind) -> &'static str {
    match kind {
        SubjectKind::Task => "Task failed",
        SubjectKind::Session => "Session failed",
    }
}

/// The headline used when nothing disclosable remains.
#[must_use]
pub const fn generic_headline(kind: SubjectKind, phase: Phase) -> &'static str {
    match (kind, phase) {
        (_, Phase::Failed) => failure_phrase(kind),
        (SubjectKind::Task, Phase::Queued) => "Task queued",
        (SubjectKind::Task, Phase::Running) => "Task running",
        (SubjectKind::Task, Phase::Waiting) => "Task waiting",
        (SubjectKind::Task, Phase::Completed) => "Task finished",
        (SubjectKind::Task, Phase::Cancelled) => "Task cancelled",
        (SubjectKind::Task, Phase::Unknown) => "Task status unknown",
        (SubjectKind::Session, Phase::Queued) => "Session queued",
        (SubjectKind::Session, Phase::Running) => "Session running",
        (SubjectKind::Session, Phase::Waiting) => "Session waiting",
        (SubjectKind::Session, Phase::Completed) => "Session finished",
        (SubjectKind::Session, Phase::Cancelled) => "Session cancelled",
        (SubjectKind::Session, Phase::Unknown) => "Session status unknown",
    }
}

/// Apply the disclosure rules and bounds to host state.
///
/// # Errors
///
/// Refuses malformed identifiers, unsafe integers, and an attention reason
/// that contradicts the phase.
pub fn encode(draft: &SummaryDraft<'_>) -> Result<ActivitySummary, ContractError> {
    check_hex(draft.host, "host")?;
    check_hex(draft.subject, "subject.id")?;
    check_integer(draft.sequence, "sequence")?;
    check_integer(draft.updated_at, "updated_at")?;
    check_consistency(draft.phase, draft.attention)?;
    let headline = if draft.phase == Phase::Failed {
        failure_phrase(draft.subject_kind).to_owned()
    } else {
        let cleaned = sanitize_headline(draft.headline);
        if cleaned.is_empty() || cleaned.split(' ').all(|token| token == REDACTED) {
            generic_headline(draft.subject_kind, draft.phase).to_owned()
        } else {
            cleaned
        }
    };
    Ok(ActivitySummary {
        host: draft.host.to_owned(),
        subject_kind: draft.subject_kind,
        subject: draft.subject.to_owned(),
        sequence: draft.sequence,
        phase: draft.phase,
        headline,
        attention: draft.attention,
        updated_at: draft.updated_at,
    })
}

/// The canonical body for `summary`.
#[must_use]
pub fn to_value(summary: &ActivitySummary) -> Value {
    json!({
        "v": SCHEMA,
        "requires": [],
        "host": summary.host,
        "subject": { "kind": summary.subject_kind.as_str(), "id": summary.subject },
        "sequence": summary.sequence,
        "phase": summary.phase.as_str(),
        "headline": summary.headline,
        "attention": summary.attention.as_str(),
        "updated_at": summary.updated_at,
    })
}

/// Verify body bytes and return the summary.
///
/// # Errors
///
/// Refuses oversized or malformed bodies, unknown versions and features,
/// extra or missing members, an over-long or unredacted headline, a failure
/// headline other than the generic phrase, and contradictory attention.
pub fn verify(bytes: &[u8]) -> Result<ActivitySummary, ContractError> {
    let value = contracts::parse_strict_bounded(bytes, MAX_BODY_BYTES)?;
    verify_value(&value)
}

/// Verify a parsed body. See [`verify`].
///
/// # Errors
///
/// Returns the same refusals as [`verify`], except the byte bound.
pub fn verify_value(value: &Value) -> Result<ActivitySummary, ContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed("summary is not an object"))?;
    match object.get("v").and_then(Value::as_str) {
        Some(SCHEMA) => {}
        Some(_) => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedVersion,
                "summary.v",
            ));
        }
        None => return Err(malformed("summary.v")),
    }
    match object.get("requires").and_then(Value::as_array) {
        Some(items) if items.is_empty() => {}
        Some(_) => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "summary.requires",
            ));
        }
        None => return Err(malformed("summary.requires")),
    }
    if object.len() != FIELDS.len() || object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err(malformed("summary members"));
    }
    let host = string(object, "host")?;
    check_hex(host, "host")?;
    let subject = object
        .get("subject")
        .and_then(Value::as_object)
        .filter(|subject| subject.len() == 2)
        .ok_or_else(|| malformed("summary.subject"))?;
    let subject_kind = subject
        .get("kind")
        .and_then(Value::as_str)
        .and_then(SubjectKind::parse)
        .ok_or_else(|| malformed("summary.subject.kind"))?;
    let subject_id = string(subject, "id")?;
    check_hex(subject_id, "subject.id")?;
    let sequence = integer(object, "sequence")?;
    let updated_at = integer(object, "updated_at")?;
    let phase = Phase::parse(string(object, "phase")?).ok_or_else(|| malformed("summary.phase"))?;
    let attention = Attention::parse(string(object, "attention")?)
        .ok_or_else(|| malformed("summary.attention"))?;
    check_consistency(phase, attention)?;
    let headline = string(object, "headline")?;
    if headline.is_empty() || headline.len() > MAX_HEADLINE_BYTES {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "summary.headline",
        ));
    }
    if phase == Phase::Failed && headline != failure_phrase(subject_kind) {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "summary.headline discloses failure detail",
        ));
    }
    if sanitize_headline(headline) != headline {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "summary.headline is not redacted",
        ));
    }
    Ok(ActivitySummary {
        host: host.to_owned(),
        subject_kind,
        subject: subject_id.to_owned(),
        sequence,
        phase,
        headline: headline.to_owned(),
        attention,
        updated_at,
    })
}

/// Seal `summary` for one enrolled device as a private `3188` artifact.
///
/// The caller establishes that `device` currently holds `observe` on this
/// host and supplies a fresh per-device mailbox and nonce. Sealing grants
/// nothing.
///
/// # Errors
///
/// Refuses a summary not signed by `host_secret`, a body that does not
/// verify, and a retention outside `(updated_at, updated_at + 7 days]`.
pub fn seal(
    summary: &ActivitySummary,
    host_secret: &SecretKey,
    device: &XOnlyPublicKey,
    mailbox: &str,
    retain_until: u64,
    nonce: [u8; 32],
) -> Result<Event, ContractError> {
    let host = Keypair::from_secret_key(&Secp256k1::new(), host_secret)
        .x_only_public_key()
        .0
        .to_string();
    if host != summary.host {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "summary.host",
        ));
    }
    if retain_until <= summary.updated_at
        || retain_until > summary.updated_at.saturating_add(MAX_RETENTION_SECONDS)
    {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "summary retention",
        ));
    }
    let inline = to_value(summary);
    let bytes = contracts::jcs(&inline)?;
    verify(&bytes)?;
    let body = json!({
        "v": "openagents.artifact-envelope.v1",
        "requires": [],
        "artifact": {
            "digest": contracts::digest_bytes(&bytes),
            "size": bytes.len(),
            "media_type": MEDIA_TYPE,
            "schema": SCHEMA,
        },
        "inline": inline,
        "issued_at": summary.updated_at,
        "retain_until": retain_until,
    });
    private_artifact::seal(
        &body,
        host_secret,
        device,
        mailbox,
        summary.updated_at,
        nonce,
    )
}

/// Open a summary envelope as the device it was sealed to.
///
/// # Errors
///
/// Refuses an envelope from a signer other than `expected_host`, one sealed
/// to another reader, a different schema or media type, missing inline
/// bytes, and a body that does not verify or names another host.
pub fn open(
    event: &Event,
    device_secret: &SecretKey,
    expected_host: &str,
) -> Result<ActivitySummary, ContractError> {
    let envelope = private_artifact::open(event, device_secret)?;
    if envelope.signer() != expected_host {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "summary signer",
        ));
    }
    let own = Keypair::from_secret_key(&Secp256k1::new(), device_secret)
        .x_only_public_key()
        .0
        .to_string();
    if envelope.recipient() != own {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "summary recipient",
        ));
    }
    let artifact = envelope.artifact();
    if artifact.schema.as_deref() != Some(SCHEMA) || artifact.media_type != MEDIA_TYPE {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "summary schema",
        ));
    }
    let bytes = envelope.inline_bytes().ok_or_else(|| {
        ContractError::new(RefusalCode::ContentUnavailable, "summary inline bytes")
    })?;
    let summary = verify(bytes)?;
    if summary.host != expected_host {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "summary.host",
        ));
    }
    Ok(summary)
}

/// Whether `incoming` replaces `current` for the same host and subject.
///
/// Returns `false` for a stale or identical summary.
///
/// # Errors
///
/// Refuses a different host or subject, and two different bodies that claim
/// one sequence.
pub fn supersedes(
    current: &ActivitySummary,
    incoming: &ActivitySummary,
) -> Result<bool, ContractError> {
    if current.host != incoming.host
        || current.subject != incoming.subject
        || current.subject_kind != incoming.subject_kind
    {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "summary subject",
        ));
    }
    if incoming.sequence == current.sequence && incoming != current {
        return Err(ContractError::new(
            RefusalCode::Conflict,
            "summary sequence",
        ));
    }
    Ok(incoming.sequence > current.sequence)
}

/// Normalize a headline: one line, collapsed spaces, redacted tokens, and at
/// most [`MAX_HEADLINE_BYTES`] bytes. Idempotent.
#[must_use]
pub fn sanitize_headline(input: &str) -> String {
    let mut tokens: Vec<&str> = Vec::new();
    for token in input.split(|c: char| c.is_whitespace() || c.is_control()) {
        if token.is_empty() {
            continue;
        }
        let token = if sensitive(token) { REDACTED } else { token };
        if token == REDACTED && tokens.last() == Some(&REDACTED) {
            continue;
        }
        tokens.push(token);
    }
    let joined = tokens.join(" ");
    truncate(&joined)
}

fn truncate(text: &str) -> String {
    if text.len() <= MAX_HEADLINE_BYTES {
        return text.to_owned();
    }
    const ELLIPSIS: &str = "\u{2026}";
    let mut end = MAX_HEADLINE_BYTES - ELLIPSIS.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut kept = text[..end].trim_end().to_owned();
    // A cut through a word could leave a fragment that is no longer
    // recognized as sensitive; drop the partial final word.
    if !text[end..].starts_with(' ')
        && let Some(space) = kept.rfind(' ')
    {
        kept.truncate(space);
    }
    kept.push_str(ELLIPSIS);
    kept
}

/// Whether a whitespace-delimited token must be withheld.
fn sensitive(token: &str) -> bool {
    if token == REDACTED {
        return false;
    }
    if token.contains("://")
        || token.contains(['/', '\\', '=', '@', '`'])
        || token.starts_with(['~', '$', '%'])
    {
        return true;
    }
    let core = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-');
    let lower = core.to_ascii_lowercase();
    if CREDENTIAL_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        || lower.starts_with("npub1")
    {
        return true;
    }
    let hex_run = core.chars().filter(char::is_ascii_hexdigit).count();
    if core.len() >= 16 && hex_run == core.len() {
        return true;
    }
    let has_digit = core.chars().any(|c| c.is_ascii_digit());
    let has_alpha = core.chars().any(char::is_alphabetic);
    core.chars().count() >= 20 && has_digit && has_alpha
}

fn check_consistency(phase: Phase, attention: Attention) -> Result<(), ContractError> {
    let consistent = match attention {
        Attention::None => !matches!(phase, Phase::Failed),
        Attention::Approval | Attention::Input => phase == Phase::Waiting,
        Attention::Completed => phase == Phase::Completed,
        Attention::Failed => phase == Phase::Failed,
    };
    if consistent {
        Ok(())
    } else {
        Err(ContractError::new(
            RefusalCode::Incompatible,
            "summary attention and phase",
        ))
    }
}

fn check_hex(value: &str, field: &str) -> Result<(), ContractError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        if field == "host" && XOnlyPublicKey::from_str(value).is_err() {
            return Err(malformed(field));
        }
        Ok(())
    } else {
        Err(malformed(field))
    }
}

fn check_integer(value: u64, field: &str) -> Result<(), ContractError> {
    if i128::from(value) <= contracts::SAFE_INTEGER {
        Ok(())
    } else {
        Err(malformed(field))
    }
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, ContractError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(key))
}

fn integer(object: &Map<String, Value>, key: &str) -> Result<u64, ContractError> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed(key))?;
    check_integer(value, key)?;
    Ok(value)
}

fn malformed(field: &str) -> ContractError {
    ContractError::new(RefusalCode::Malformed, field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }

    fn public(n: u8) -> XOnlyPublicKey {
        Keypair::from_secret_key(&Secp256k1::new(), &secret(n))
            .x_only_public_key()
            .0
    }

    fn draft<'a>(
        host: &'a str,
        headline: &'a str,
        phase: Phase,
        attention: Attention,
    ) -> SummaryDraft<'a> {
        SummaryDraft {
            host,
            subject_kind: SubjectKind::Task,
            subject: "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12",
            sequence: 3,
            phase,
            headline,
            attention,
            updated_at: 1_000,
        }
    }

    #[test]
    fn a_summary_round_trips_through_a_private_artifact() {
        let host = public(1).to_string();
        let summary = encode(&draft(
            &host,
            "Review the release notes",
            Phase::Waiting,
            Attention::Approval,
        ))
        .unwrap();
        assert_eq!(summary.headline, "Review the release notes");
        let event = seal(
            &summary,
            &secret(1),
            &public(2),
            &"c".repeat(64),
            2_000,
            [7; 32],
        )
        .unwrap();
        assert_eq!(event.kind, contracts::ARTIFACT_ENVELOPE_KIND);
        assert!(!event.content.contains("release"));
        let opened = open(&event, &secret(2), &host).unwrap();
        assert_eq!(opened, summary);
        // The host can reread its own copy but it is not a device read.
        assert_eq!(
            open(&event, &secret(1), &host).unwrap_err().code,
            RefusalCode::NotAdmitted
        );
        // A third key cannot open it, and another signer is refused.
        assert!(open(&event, &secret(3), &host).is_err());
        assert_eq!(
            open(&event, &secret(2), &public(3).to_string())
                .unwrap_err()
                .code,
            RefusalCode::IdentityMismatch
        );
        // Only the host key may seal a summary naming the host.
        assert!(
            seal(
                &summary,
                &secret(3),
                &public(2),
                &"c".repeat(64),
                2_000,
                [7; 32]
            )
            .is_err()
        );
    }

    #[test]
    fn headlines_are_bounded_to_160_bytes_on_a_character_boundary() {
        let host = public(1).to_string();
        let long = "déjà vu ".repeat(40);
        let summary = encode(&draft(&host, &long, Phase::Running, Attention::None)).unwrap();
        assert!(summary.headline.len() <= MAX_HEADLINE_BYTES);
        assert!(summary.headline.ends_with('\u{2026}'));
        let bytes = contracts::jcs(&to_value(&summary)).unwrap();
        assert_eq!(verify(&bytes).unwrap(), summary);

        let mut over = to_value(&summary);
        over["headline"] = Value::String("a".repeat(161));
        let bytes = serde_json::to_vec(&over).unwrap();
        assert_eq!(verify(&bytes).unwrap_err().code, RefusalCode::LimitExceeded);

        let mut empty = to_value(&summary);
        empty["headline"] = Value::String(String::new());
        let bytes = serde_json::to_vec(&empty).unwrap();
        assert_eq!(verify(&bytes).unwrap_err().code, RefusalCode::LimitExceeded);

        let padded = format!(r#"{{"pad":"{}"}}"#, "x".repeat(MAX_BODY_BYTES));
        assert_eq!(
            verify(padded.as_bytes()).unwrap_err().code,
            RefusalCode::LimitExceeded
        );
    }

    #[test]
    fn paths_urls_and_credentials_are_redacted() {
        let host = public(1).to_string();
        for (input, expected) in [
            ("Edit /Users/alice/secret.txt now", "Edit [redacted] now"),
            ("Fetch https://example.com/x?token=1", "Fetch [redacted]"),
            ("Use key sk-live-0123456789abcdef", "Use key [redacted]"),
            (
                "Set API_KEY=hunter2 then retry",
                "Set [redacted] then retry",
            ),
            ("Mail alice@example.com", "Mail [redacted]"),
            (
                "Open C:\\Windows\\system32 and ~/notes",
                "Open [redacted] and [redacted]",
            ),
            ("Key nsec1qqqqqqqq", "Key [redacted]"),
            (
                "Id 0123456789abcdef0123456789abcdef here",
                "Id [redacted] here",
            ),
            ("Line one\nline\ttwo", "Line one line two"),
            ("a /x /y b", "a [redacted] b"),
        ] {
            let summary = encode(&draft(&host, input, Phase::Running, Attention::None)).unwrap();
            assert_eq!(summary.headline, expected, "{input:?}");
            let bytes = contracts::jcs(&to_value(&summary)).unwrap();
            assert_eq!(verify(&bytes).unwrap().headline, expected);
        }
        // A headline that is only sensitive falls back to the generic phrase.
        let summary = encode(&draft(
            &host,
            "/etc/passwd",
            Phase::Running,
            Attention::None,
        ))
        .unwrap();
        assert_eq!(summary.headline, "Task running");

        // Verification refuses a body that encoding would have redacted.
        let mut leaked = to_value(&summary);
        leaked["headline"] = Value::String("Read /etc/passwd".into());
        let bytes = serde_json::to_vec(&leaked).unwrap();
        assert_eq!(verify(&bytes).unwrap_err().code, RefusalCode::NotAdmitted);
    }

    #[test]
    fn failure_detail_becomes_the_generic_phrase() {
        let host = public(1).to_string();
        let summary = encode(&draft(
            &host,
            "panicked at src/lib.rs:10: index out of bounds",
            Phase::Failed,
            Attention::Failed,
        ))
        .unwrap();
        assert_eq!(summary.headline, "Task failed");
        let mut detailed = to_value(&summary);
        detailed["headline"] = Value::String("Task failed: out of memory".into());
        let bytes = serde_json::to_vec(&detailed).unwrap();
        assert_eq!(verify(&bytes).unwrap_err().code, RefusalCode::NotAdmitted);
        // Failure always asks for attention, and only failure may.
        assert!(encode(&draft(&host, "x", Phase::Failed, Attention::None)).is_err());
        assert!(encode(&draft(&host, "x", Phase::Running, Attention::Failed)).is_err());
        assert!(encode(&draft(&host, "x", Phase::Running, Attention::Approval)).is_err());
        assert!(
            encode(&draft(
                &host,
                "Done",
                Phase::Completed,
                Attention::Completed
            ))
            .is_ok()
        );
    }

    #[test]
    fn verification_is_closed_and_versioned() {
        let host = public(1).to_string();
        let summary = encode(&draft(&host, "Build", Phase::Queued, Attention::None)).unwrap();
        let base = to_value(&summary);

        let mut extra = base.clone();
        extra["prompt"] = Value::String("hidden".into());
        assert_eq!(
            verify(&serde_json::to_vec(&extra).unwrap())
                .unwrap_err()
                .code,
            RefusalCode::Malformed
        );
        let mut meta = base.clone();
        meta["meta"] = json!({});
        assert!(verify(&serde_json::to_vec(&meta).unwrap()).is_err());
        let mut version = base.clone();
        version["v"] = Value::String("openagents.activity-summary.v2".into());
        assert_eq!(
            verify(&serde_json::to_vec(&version).unwrap())
                .unwrap_err()
                .code,
            RefusalCode::UnsupportedVersion
        );
        let mut feature = base.clone();
        feature["requires"] = json!(["x"]);
        assert_eq!(
            verify(&serde_json::to_vec(&feature).unwrap())
                .unwrap_err()
                .code,
            RefusalCode::UnsupportedFeature
        );
        let duplicate = serde_json::to_string(&base).unwrap().replacen(
            "\"phase\":\"queued\"",
            "\"phase\":\"queued\",\"phase\":\"queued\"",
            1,
        );
        assert!(verify(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn a_higher_sequence_supersedes_and_a_fork_conflicts() {
        let host = public(1).to_string();
        let first = encode(&draft(&host, "Build", Phase::Running, Attention::None)).unwrap();
        let mut next = first.clone();
        next.sequence = 4;
        assert!(supersedes(&first, &next).unwrap());
        assert!(!supersedes(&next, &first).unwrap());
        assert!(!supersedes(&first, &first).unwrap());
        let mut fork = first.clone();
        fork.headline = "Other".into();
        assert_eq!(
            supersedes(&first, &fork).unwrap_err().code,
            RefusalCode::Conflict
        );
    }
}
