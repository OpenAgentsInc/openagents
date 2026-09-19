use std::collections::HashSet;

use super::hex::decode_lower_hex;
use super::{Event, Tag, validate_nip44_v2_content};

pub const AGENT_ENGRAM_KIND: u16 = 30_174;
pub const AGENT_PERSONA_KIND: u16 = 30_175;
pub const TEAM_CATALOG_KIND: u16 = 30_178;
pub const EVENT_REMINDER_KIND: u16 = 30_300;
pub const PUSH_LEASE_KIND: u16 = 30_350;
pub const PROJECT_KIND: u16 = 30_621;
pub const DM_VISIBILITY_KIND: u16 = 30_622;
pub const THREAD_SUMMARY_KIND: u16 = 39_005;
pub const WINDOW_BOUNDS_KIND: u16 = 39_006;
pub const READ_STATE_KIND: u16 = 30_078;
pub const IDENTITY_ARCHIVE_REQUEST_KIND: u16 = 9_035;
pub const IDENTITY_UNARCHIVE_REQUEST_KIND: u16 = 9_036;
pub const IDENTITY_ARCHIVED_KIND: u16 = 8_002;
pub const IDENTITY_UNARCHIVED_KIND: u16 = 8_003;
pub const IDENTITY_ARCHIVE_LIST_KIND: u16 = 13_535;
pub const WORKSPACE_PROFILE_KIND: u16 = 9_033;
pub const DM_OPEN_KIND: u16 = 41_010;
pub const DM_HIDE_KIND: u16 = 41_012;
pub const MAX_REMINDER_HORIZON_SECONDS: u64 = 31_536_000;

pub const RELAY_ONLY_BLOCK_KINDS: &[u16] = &[
    DM_VISIBILITY_KIND,
    THREAD_SUMMARY_KIND,
    WINDOW_BOUNDS_KIND,
    IDENTITY_ARCHIVED_KIND,
    IDENTITY_UNARCHIVED_KIND,
    IDENTITY_ARCHIVE_LIST_KIND,
];

pub const BLOCK_GLOBAL_ONLY_KINDS: &[u16] = &[
    READ_STATE_KIND,
    AGENT_ENGRAM_KIND,
    AGENT_PERSONA_KIND,
    TEAM_CATALOG_KIND,
    EVENT_REMINDER_KIND,
    PUSH_LEASE_KIND,
    PROJECT_KIND,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityArchiveRequest {
    pub archive: bool,
    pub target: String,
    pub reason: Option<String>,
    pub replaced_by: Option<String>,
}

pub fn validate_block_ingest(event: &Event, now: u64) -> Result<(), String> {
    match event.kind {
        AGENT_ENGRAM_KIND => validate_engram(event),
        AGENT_PERSONA_KIND => validate_persona(event),
        TEAM_CATALOG_KIND => validate_team_catalog(event),
        EVENT_REMINDER_KIND => validate_reminder(event, now),
        PROJECT_KIND => validate_project(event),
        PUSH_LEASE_KIND => validate_push_lease_envelope(event, now),
        _ => Ok(()),
    }
}

pub fn validate_engram(event: &Event) -> Result<(), String> {
    let d = single_tag_value(event, "d", "agent engram")?;
    lower_hex(d, 32, "agent engram d tag")?;
    let owner = single_tag_value(event, "p", "agent engram")?;
    lower_hex(owner, 32, "agent engram owner")?;
    validate_nip44_v2_content(&event.content, "agent engram")
}

pub fn validate_persona(event: &Event) -> Result<(), String> {
    validate_shared(event, "persona")?;
    let d = single_tag_value(event, "d", "persona")?;
    let bytes = d.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        || !bytes[1..].iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err("persona d tag must match ^[a-z0-9][a-z0-9_-]{0,63}$".to_owned());
    }
    Ok(())
}

pub fn validate_team_catalog(event: &Event) -> Result<(), String> {
    validate_shared(event, "team catalog")?;
    let d = single_tag_value(event, "d", "team catalog")?;
    if d.is_empty()
        || d.chars().count() > 64
        || d.chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err("team catalog d tag must be 1-64 non-whitespace characters".to_owned());
    }
    Ok(())
}

pub fn validate_reminder(event: &Event, now: u64) -> Result<(), String> {
    let d = single_tag_value(event, "d", "event reminder")?;
    if d.is_empty() {
        return Err("event reminder d tag must not be empty".to_owned());
    }
    validate_nip44_v2_content(&event.content, "event reminder")?;
    let not_before = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("not_before"))
        .collect::<Vec<_>>();
    if not_before.len() > 1 {
        return Err("malformed not_before".to_owned());
    }
    let not_before = not_before
        .first()
        .map(|tag| {
            canonical_decimal(
                tag.value().unwrap_or_default(),
                9_007_199_254_740_991,
                "not_before",
            )
        })
        .transpose()?;
    if let Some(not_before) = not_before {
        if not_before > now.saturating_add(MAX_REMINDER_HORIZON_SECONDS) {
            return Err("not_before too far in future".to_owned());
        }
        let expirations = event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some("expiration"))
            .collect::<Vec<_>>();
        if expirations.len() > 1 {
            return Err("event reminder has duplicate expiration tags".to_owned());
        }
        if let Some(expiration) = expirations.first() {
            let expiration = canonical_decimal(
                expiration.value().unwrap_or_default(),
                u64::MAX,
                "expiration",
            )?;
            if expiration <= not_before {
                return Err("expiration before not_before".to_owned());
            }
        }
    }
    Ok(())
}

pub fn validate_project(event: &Event) -> Result<(), String> {
    let d = single_tag_value(event, "d", "project")?;
    if d.is_empty() {
        return Err("[d-empty] project d tag must not be empty".to_owned());
    }
    let members = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect::<Vec<_>>();
    if members.len() > 64 {
        return Err("[member-cap] project must have at most 64 member a tags".to_owned());
    }
    let mut seen = HashSet::with_capacity(members.len());
    for tag in members {
        if !(2..=3).contains(&tag.as_slice().len()) {
            return Err(
                "[member-tag-arity] project a tags require two or three elements".to_owned(),
            );
        }
        let coordinate = tag.value().unwrap_or_default();
        let mut parts = coordinate.splitn(3, ':');
        let (Some(kind), Some(owner), Some(repo_d)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err("[member-coordinate-malformed] malformed project member".to_owned());
        };
        if kind != "30617" || repo_d.is_empty() || lower_hex(owner, 32, "project owner").is_err() {
            return Err("[member-coordinate-malformed] malformed project member".to_owned());
        }
        if !seen.insert(coordinate) {
            return Err("[member-duplicate] duplicate project member".to_owned());
        }
    }
    for (name, maximum) in [
        ("name", 256_usize),
        ("description", 2_048),
        ("buzz-channel", 256),
        ("buzz-visibility", 256),
    ] {
        let tags = event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some(name))
            .collect::<Vec<_>>();
        if tags.len() > 1 {
            return Err(format!(
                "[metadata-cardinality] duplicate project {name} tag"
            ));
        }
        if tags
            .first()
            .and_then(|tag| tag.value())
            .is_some_and(|value| value.len() > maximum)
        {
            return Err(format!("[metadata-length] project {name} tag is too long"));
        }
    }
    Ok(())
}

pub fn validate_push_lease_envelope(event: &Event, now: u64) -> Result<(), String> {
    if event.content.len() > 65_536 {
        return Err("push lease content exceeds 65536 bytes".to_owned());
    }
    let mut names = HashSet::new();
    for tag in &event.tags {
        let name = tag
            .name()
            .ok_or_else(|| "push lease contains an empty tag".to_owned())?;
        if !matches!(name, "d" | "expiration" | "exec" | "alt") {
            return Err(format!("push lease contains unexpected public tag {name}"));
        }
        if tag.as_slice().len() != 2 || !names.insert(name) {
            return Err(format!(
                "push lease {name} tag must occur once with one value"
            ));
        }
    }
    let d = single_tag_value(event, "d", "push lease")?;
    if d.is_empty() || d.len() > 64 {
        return Err("push lease d tag must contain 1-64 bytes".to_owned());
    }
    let expiration = canonical_decimal(
        single_tag_value(event, "expiration", "push lease")?,
        u64::MAX,
        "push lease expiration",
    )?;
    if expiration <= now.saturating_sub(900) {
        return Err("push lease already expired".to_owned());
    }
    if expiration > now.saturating_add(2_592_000) {
        return Err("push lease ttl too long".to_owned());
    }
    if single_tag_value(event, "exec", "push lease")?.is_empty() {
        return Err("push lease exec tag must not be empty".to_owned());
    }
    validate_nip44_v2_content(&event.content, "push lease")
}

pub fn parse_identity_archive_request(
    event: &Event,
    now: u64,
) -> Result<IdentityArchiveRequest, String> {
    if !matches!(
        event.kind,
        IDENTITY_ARCHIVE_REQUEST_KIND | IDENTITY_UNARCHIVE_REQUEST_KIND
    ) {
        return Err("identity archive request has the wrong kind".to_owned());
    }
    if event.created_at.abs_diff(now) > 120 {
        return Err(
            "identity archive request is outside the 120-second freshness window".to_owned(),
        );
    }
    let protected = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice() == ["-"])
        .count();
    if protected != 1 {
        return Err("identity archive request requires exactly one protected tag".to_owned());
    }
    let target = single_tag_value(event, "p", "identity archive request")?.to_owned();
    lower_hex(&target, 32, "identity archive target")?;
    let reason = optional_single_value(event, "reason", "identity archive request")?;
    let replaced_by = optional_single_value(event, "replaced-by", "identity archive request")?;
    if event.kind == IDENTITY_UNARCHIVE_REQUEST_KIND && replaced_by.is_some() {
        return Err("unarchive request must not contain replaced-by".to_owned());
    }
    if let Some(replacement) = &replaced_by {
        lower_hex(replacement, 32, "replacement pubkey")?;
        if replacement == &target {
            return Err("replacement pubkey must differ from the archive target".to_owned());
        }
    }
    Ok(IdentityArchiveRequest {
        archive: event.kind == IDENTITY_ARCHIVE_REQUEST_KIND,
        target,
        reason,
        replaced_by,
    })
}

pub fn workspace_icon(event: &Event) -> Result<String, String> {
    if event.kind != WORKSPACE_PROFILE_KIND || !event.content.is_empty() {
        return Err("workspace profile command must be kind 9033 with empty content".to_owned());
    }
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("icon"))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err("workspace profile command has duplicate icon tags".to_owned());
    }
    let icon = tags.first().and_then(|tag| tag.value()).unwrap_or_default();
    if icon
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err("workspace icon contains whitespace or control characters".to_owned());
    }
    if icon.is_empty() {
        return Ok(String::new());
    }
    if icon.starts_with("data:image/") {
        if icon.len() > 98_304 {
            return Err("workspace icon data URL exceeds 98304 bytes".to_owned());
        }
    } else if icon.starts_with("https://") || icon.starts_with("http://") {
        if icon.len() > 2_048 {
            return Err("workspace icon URL exceeds 2048 bytes".to_owned());
        }
    } else {
        return Err("workspace icon must be http(s), data:image/*, or empty".to_owned());
    }
    Ok(icon.to_owned())
}

pub fn dm_visibility_channel(event: &Event) -> Result<&str, String> {
    if !matches!(event.kind, DM_HIDE_KIND | DM_OPEN_KIND) {
        return Err("DM visibility command has the wrong kind".to_owned());
    }
    let channel = single_tag_value(event, "h", "DM visibility command")?;
    if channel.is_empty() || channel.len() > 128 || channel.chars().any(char::is_control) {
        return Err("DM visibility channel id must contain 1-128 safe characters".to_owned());
    }
    Ok(channel)
}

fn validate_shared(event: &Event, subject: &str) -> Result<(), String> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("shared"))
        .collect::<Vec<_>>();
    if tags.len() > 1
        || tags
            .first()
            .is_some_and(|tag| tag.as_slice() != ["shared", "true"])
    {
        return Err(format!(
            "{subject} shared tag must be exactly [\"shared\",\"true\"] and unique"
        ));
    }
    Ok(())
}

fn single_tag_value<'a>(event: &'a Event, name: &str, subject: &str) -> Result<&'a str, String> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<&Tag>>();
    if tags.len() != 1 || tags[0].as_slice().len() < 2 {
        return Err(format!(
            "{subject} must contain exactly one {name} tag with a value"
        ));
    }
    Ok(tags[0].value().unwrap_or_default())
}

fn optional_single_value(
    event: &Event,
    name: &str,
    subject: &str,
) -> Result<Option<String>, String> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 || tags.first().is_some_and(|tag| tag.as_slice().len() < 2) {
        return Err(format!("{subject} has a malformed or duplicate {name} tag"));
    }
    Ok(tags.first().and_then(|tag| tag.value()).map(str::to_owned))
}

fn lower_hex(value: &str, bytes: usize, subject: &str) -> Result<(), String> {
    let valid = match bytes {
        32 => decode_lower_hex::<32>(value, "Block NIP hexadecimal value").is_ok(),
        _ => false,
    };
    valid.then_some(()).ok_or_else(|| {
        format!(
            "{subject} must be {} lowercase hexadecimal characters",
            bytes * 2
        )
    })
}

fn canonical_decimal(value: &str, maximum: u64, subject: &str) -> Result<u64, String> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(format!("{subject} must be a canonical decimal"));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{subject} is out of range"))?;
    (parsed <= maximum)
        .then_some(parsed)
        .ok_or_else(|| format!("{subject} is out of range"))
}
