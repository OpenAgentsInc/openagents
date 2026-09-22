//! NIP-52 calendar events.
//!
//! Kind `31922` is a date-based event. Kind `31923` is a time-based
//! event. Kind `31924` is a calendar list. Kind `31925` is an RSVP.
//! All four are addressable. A `name` tag supplies the title only when
//! `title` is absent. A declined RSVP ignores `fb`.
//!
//! Time zone names are not checked against the IANA database. `D` tags
//! must include the start day and are not required to list every later
//! day. Images and links are not fetched. Recurring events are not
//! expanded. These kinds are not added to the NIP-11 list.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const DATE_KIND: u16 = 31_922;
const TIME_KIND: u16 = 31_923;
const CALENDAR_KIND: u16 = 31_924;
const RSVP_KIND: u16 = 31_925;
const DAY_SECONDS: u64 = 86_400;
const GEOHASH: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// Attendance on a kind `31925` RSVP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attendance {
    Accepted,
    Declined,
    Tentative,
}

/// Free or busy availability. A declined RSVP has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Free,
    Busy,
}

/// When a calendar event occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarSpan {
    /// Inclusive `start` and an exclusive `end`. An omitted end occupies `start`.
    Date { start: String, end: Option<String> },
    /// Inclusive unix `start` and an exclusive `end`. An omitted end is instantaneous.
    Time {
        start: u64,
        end: Option<u64>,
        start_tzid: Option<String>,
        end_tzid: Option<String>,
        days: Vec<u64>,
    },
}

/// A participant `p` tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
    pub pubkey: String,
    pub relay: String,
    pub role: Option<String>,
}

/// An `a` tag plus its optional relay hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarRef {
    pub address: ReplacementAddress,
    pub relay: String,
}

/// A kind `31922` or `31923` calendar event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEvent {
    pub kind: u16,
    pub identifier: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub image: Option<String>,
    pub locations: Vec<String>,
    pub geohashes: Vec<String>,
    pub participants: Vec<Participant>,
    pub hashtags: Vec<String>,
    pub links: Vec<String>,
    pub calendars: Vec<CalendarRef>,
    pub span: CalendarSpan,
}

/// A kind `31924` calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calendar {
    pub identifier: String,
    pub title: String,
    pub content: String,
    pub events: Vec<CalendarRef>,
}

/// A kind `31925` RSVP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rsvp {
    pub identifier: String,
    pub content: String,
    pub event: CalendarRef,
    pub revision: Option<String>,
    pub status: Attendance,
    pub availability: Option<Availability>,
    pub author: Option<String>,
    pub author_relay: String,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn geohash_ok(value: &str) -> bool {
    (1..=12).contains(&value.len()) && value.bytes().all(|byte| GEOHASH.contains(&byte))
}

fn tzid_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.split('/').all(|part| {
            let mut chars = part.chars();
            matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
                && chars.all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '+' | '-'))
        })
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "calendar pubkey")
        .map_err(|_| invalid("a calendar pubkey is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn tags_named<'a>(event: &'a Event, name: &str) -> Vec<&'a super::Tag> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect()
}

fn one_value<'a>(event: &'a Event, name: &str, reason: &str) -> Result<&'a str, DomainError> {
    let tags = tags_named(event, name);
    if tags.len() != 1 {
        return Err(invalid(reason));
    }
    tags[0]
        .value()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(reason))
}

fn optional_value<'a>(
    event: &'a Event,
    name: &str,
    reason: &str,
) -> Result<Option<&'a str>, DomainError> {
    let tags = tags_named(event, name);
    if tags.len() > 1 {
        return Err(invalid(reason));
    }
    match tags.first() {
        None => Ok(None),
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(reason)),
        },
    }
}

fn identifier(event: &Event) -> Result<String, DomainError> {
    let value = one_value(event, "d", "a calendar event has one d tag")?;
    if value.len() > 256 || value.chars().any(char::is_whitespace) {
        return Err(invalid("a calendar event has one d tag"));
    }
    Ok(value.to_owned())
}

fn title_of(event: &Event) -> Result<String, DomainError> {
    let titles = tags_named(event, "title");
    if titles.len() > 1 {
        return Err(invalid("a calendar event has one title"));
    }
    if let Some(tag) = titles.first() {
        return tag
            .value()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| invalid("a calendar event has one title"));
    }
    let names = tags_named(event, "name");
    if names.len() != 1 {
        return Err(invalid("a calendar event has one title"));
    }
    names[0]
        .value()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid("a calendar event has one title"))
}

fn relay_hint(value: Option<&String>) -> Result<String, DomainError> {
    match value {
        None => Ok(String::new()),
        Some(value) if value.is_empty() || is_relay(value) => Ok(value.to_owned()),
        Some(_) => Err(invalid("a calendar relay hint is empty or ws:// or wss://")),
    }
}

fn reference(tag: &super::Tag, allowed: &[u16], reason: &str) -> Result<CalendarRef, DomainError> {
    let Some(value) = tag.value() else {
        return Err(invalid(reason));
    };
    let address = ReplacementAddress::from_str(value).map_err(|_| invalid(reason))?;
    if !allowed.contains(&address.kind) || address.identifier.is_empty() {
        return Err(invalid(reason));
    }
    Ok(CalendarRef {
        address,
        relay: relay_hint(tag.as_slice().get(2))?,
    })
}

/// Day stamp `floor(unix_seconds / 86_400)`.
#[must_use]
pub fn day_stamp(seconds: u64) -> u64 {
    seconds / DAY_SECONDS
}

fn calendar_date(value: &str) -> Result<String, DomainError> {
    let reason = "a calendar date is YYYY-MM-DD";
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(invalid(reason));
    }
    let year: i32 = value[0..4].parse().map_err(|_| invalid(reason))?;
    let month: u32 = value[5..7].parse().map_err(|_| invalid(reason))?;
    let day: u32 = value[8..10].parse().map_err(|_| invalid(reason))?;
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return Err(invalid(reason)),
    };
    if day == 0 || day > max {
        return Err(invalid(reason));
    }
    Ok(value.to_owned())
}

fn unix_seconds(value: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid("a calendar timestamp is unix seconds"));
    }
    value
        .parse()
        .map_err(|_| invalid("a calendar timestamp is unix seconds"))
}

fn participants(event: &Event) -> Result<Vec<Participant>, DomainError> {
    let mut people = Vec::new();
    for tag in tags_named(event, "p") {
        let Some(value) = tag.value() else {
            return Err(invalid("a calendar pubkey is 32 lowercase hex bytes"));
        };
        let role = match tag.as_slice().get(3) {
            None => None,
            Some(value) if value.is_empty() => None,
            Some(value) if value.len() <= 128 => Some(value.to_owned()),
            Some(_) => {
                return Err(invalid(
                    "a calendar participant role is 1 to 128 characters",
                ));
            }
        };
        people.push(Participant {
            pubkey: pubkey(value)?,
            relay: relay_hint(tag.as_slice().get(2))?,
            role,
        });
    }
    Ok(people)
}

fn text_list(
    event: &Event,
    name: &str,
    reason: &str,
    allow_space: bool,
) -> Result<Vec<String>, DomainError> {
    let mut values = Vec::new();
    for tag in tags_named(event, name) {
        let Some(value) = tag
            .value()
            .filter(|value| !value.is_empty() && value.len() <= 2_048)
        else {
            return Err(invalid(reason));
        };
        if !allow_space && value.chars().any(char::is_whitespace) {
            return Err(invalid(reason));
        }
        values.push(value.to_owned());
    }
    Ok(values)
}

struct Common {
    identifier: String,
    title: String,
    summary: Option<String>,
    image: Option<String>,
    locations: Vec<String>,
    geohashes: Vec<String>,
    participants: Vec<Participant>,
    hashtags: Vec<String>,
    links: Vec<String>,
    calendars: Vec<CalendarRef>,
}

fn common(event: &Event) -> Result<Common, DomainError> {
    let mut geohashes = Vec::new();
    for tag in tags_named(event, "g") {
        let Some(value) = tag.value().filter(|value| geohash_ok(value)) else {
            return Err(invalid("a calendar geohash is 1 to 12 characters"));
        };
        geohashes.push(value.to_owned());
    }
    let image = match optional_value(
        event,
        "image",
        "a calendar image is an http:// or https:// URL",
    )? {
        None => None,
        Some(value) if http_url(value) => Some(value.to_owned()),
        Some(_) => return Err(invalid("a calendar image is an http:// or https:// URL")),
    };
    let mut calendars = Vec::new();
    for tag in tags_named(event, "a") {
        calendars.push(reference(
            tag,
            &[CALENDAR_KIND],
            "a calendar event requests inclusion in a kind 31924 calendar",
        )?);
    }
    Ok(Common {
        identifier: identifier(event)?,
        title: title_of(event)?,
        summary: optional_value(event, "summary", "a calendar summary is non-empty")?
            .map(str::to_owned),
        image,
        locations: text_list(
            event,
            "location",
            "a calendar location is 1 to 2048 characters",
            true,
        )?,
        geohashes,
        participants: participants(event)?,
        hashtags: text_list(event, "t", "a calendar hashtag is non-empty", false)?,
        links: text_list(event, "r", "a calendar link is non-empty", false)?,
        calendars,
    })
}

/// Read a kind `31922` or `31923` calendar event.
///
/// # Errors
///
/// Returns a sentence when a date, timestamp, title, or reference is refused.
pub fn open_calendar_event(event: &Event) -> Result<CalendarEvent, DomainError> {
    if !matches!(event.kind, DATE_KIND | TIME_KIND) {
        return Err(invalid("a calendar event has kind 31922 or 31923"));
    }
    let common = common(event)?;
    let span = if event.kind == DATE_KIND {
        let start = calendar_date(one_value(event, "start", "a calendar date is YYYY-MM-DD")?)?;
        let end = match optional_value(event, "end", "a calendar date is YYYY-MM-DD")? {
            None => None,
            Some(value) => Some(calendar_date(value)?),
        };
        if let Some(end) = &end
            && start.as_str() >= end.as_str()
        {
            return Err(invalid("a calendar start date is before the end date"));
        }
        CalendarSpan::Date { start, end }
    } else {
        let start = unix_seconds(one_value(
            event,
            "start",
            "a calendar timestamp is unix seconds",
        )?)?;
        let end = match optional_value(event, "end", "a calendar timestamp is unix seconds")? {
            None => None,
            Some(value) => Some(unix_seconds(value)?),
        };
        if let Some(end) = end
            && start >= end
        {
            return Err(invalid("a calendar start time is before the end time"));
        }
        let start_tzid =
            match optional_value(event, "start_tzid", "a calendar time zone has IANA form")? {
                None => None,
                Some(value) if tzid_ok(value) => Some(value.to_owned()),
                Some(_) => return Err(invalid("a calendar time zone has IANA form")),
            };
        let end_tzid =
            match optional_value(event, "end_tzid", "a calendar time zone has IANA form")? {
                None => start_tzid.clone(),
                Some(value) if tzid_ok(value) => Some(value.to_owned()),
                Some(_) => return Err(invalid("a calendar time zone has IANA form")),
            };
        let mut days = Vec::new();
        for tag in tags_named(event, "D") {
            let Some(value) = tag.value() else {
                return Err(invalid(
                    "a calendar day stamp is floor(unix seconds / 86400)",
                ));
            };
            days.push(
                unix_seconds(value)
                    .map_err(|_| invalid("a calendar day stamp is floor(unix seconds / 86400)"))?,
            );
        }
        if !days.contains(&day_stamp(start)) {
            return Err(invalid(
                "a calendar day stamp is floor(unix seconds / 86400)",
            ));
        }
        CalendarSpan::Time {
            start,
            end,
            start_tzid,
            end_tzid,
            days,
        }
    };
    Ok(CalendarEvent {
        kind: event.kind,
        identifier: common.identifier,
        title: common.title,
        content: event.content.clone(),
        summary: common.summary,
        image: common.image,
        locations: common.locations,
        geohashes: common.geohashes,
        participants: common.participants,
        hashtags: common.hashtags,
        links: common.links,
        calendars: common.calendars,
        span,
    })
}

/// Read a kind `31924` calendar.
///
/// # Errors
///
/// Returns a sentence when the title or an event reference is refused.
pub fn open_calendar(event: &Event) -> Result<Calendar, DomainError> {
    if event.kind != CALENDAR_KIND {
        return Err(invalid("a calendar has kind 31924"));
    }
    let mut events = Vec::new();
    for tag in tags_named(event, "a") {
        events.push(reference(
            tag,
            &[DATE_KIND, TIME_KIND],
            "a calendar references a kind 31922 or 31923 event",
        )?);
    }
    Ok(Calendar {
        identifier: identifier(event)?,
        title: title_of(event)?,
        content: event.content.clone(),
        events,
    })
}

/// Read a kind `31925` RSVP.
///
/// A declined RSVP drops `fb`. The relay does not decide who may attend.
///
/// # Errors
///
/// Returns a sentence when the event reference or status is refused.
pub fn open_rsvp(event: &Event) -> Result<Rsvp, DomainError> {
    if event.kind != RSVP_KIND {
        return Err(invalid("a calendar RSVP has kind 31925"));
    }
    let addresses = tags_named(event, "a");
    if addresses.len() != 1 {
        return Err(invalid("a calendar RSVP references one calendar event"));
    }
    let target = reference(
        addresses[0],
        &[DATE_KIND, TIME_KIND],
        "a calendar RSVP references one calendar event",
    )?;
    let revisions = tags_named(event, "e");
    if revisions.len() > 1 {
        return Err(invalid("a calendar RSVP has one event revision"));
    }
    let revision = match revisions.first() {
        None => None,
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid(
                    "a calendar RSVP revision is 32 lowercase hex bytes",
                ));
            };
            decode_lower_hex::<32>(value, "calendar revision")
                .map_err(|_| invalid("a calendar RSVP revision is 32 lowercase hex bytes"))?;
            relay_hint(tag.as_slice().get(2))?;
            Some(value.to_owned())
        }
    };
    let status = match one_value(
        event,
        "status",
        "a calendar RSVP status is accepted, declined, or tentative",
    )? {
        "accepted" => Attendance::Accepted,
        "declined" => Attendance::Declined,
        "tentative" => Attendance::Tentative,
        _ => {
            return Err(invalid(
                "a calendar RSVP status is accepted, declined, or tentative",
            ));
        }
    };
    let availability = if status == Attendance::Declined {
        None
    } else {
        match optional_value(event, "fb", "a calendar RSVP availability is free or busy")? {
            None => None,
            Some("free") => Some(Availability::Free),
            Some("busy") => Some(Availability::Busy),
            Some(_) => return Err(invalid("a calendar RSVP availability is free or busy")),
        }
    };
    let authors = tags_named(event, "p");
    if authors.len() > 1 {
        return Err(invalid("a calendar RSVP has one author"));
    }
    let (author, author_relay) = match authors.first() {
        None => (None, String::new()),
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a calendar pubkey is 32 lowercase hex bytes"));
            };
            (Some(pubkey(value)?), relay_hint(tag.as_slice().get(2))?)
        }
    };
    Ok(Rsvp {
        identifier: identifier(event)?,
        content: event.content.clone(),
        event: target,
        revision,
        status,
        availability,
        author,
        author_relay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_calendar_event_keeps_its_span_and_an_rsvp_names_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/52.md"
        ))
        .unwrap();
        assert!(text.contains("kind:31922"));
        assert!(text.contains("kind:31923"));
        assert!(text.contains("kind:31924") || text.contains("kind `31924`"));
        assert!(text.contains("kind:31925"));
        assert!(text.contains("accepted"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "52.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "52.md")
        );

        let author = signer("c5");
        let guest = signer("d6");
        let dated = author.sign(
            1_700_000_000,
            DATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "offsite".into()]),
                Tag::new(vec!["title".into(), "Offsite".into()]),
                Tag::new(vec!["name".into(), "ignored".into()]),
                Tag::new(vec!["start".into(), "2026-09-22".into()]),
                Tag::new(vec!["end".into(), "2026-09-24".into()]),
                Tag::new(vec!["location".into(), "Room 4".into()]),
                Tag::new(vec!["g".into(), "u4xsu".into()]),
                Tag::new(vec![
                    "p".into(),
                    guest.pubkey().to_owned(),
                    "wss://relay.example".into(),
                    "required".into(),
                ]),
                Tag::new(vec![
                    "a".into(),
                    format!("31924:{}:work", author.pubkey()),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "image".into(),
                    "https://example.com/offsite.png".into(),
                ]),
            ],
            "Two days away.".into(),
        );
        dated.validate_structure().unwrap();
        assert_eq!(dated.class(), EventClass::Addressable);
        let opened = open_calendar_event(&dated).unwrap();
        assert_eq!(opened.title, "Offsite");
        assert_eq!(opened.locations, vec!["Room 4".to_owned()]);
        assert_eq!(opened.participants[0].role.as_deref(), Some("required"));
        assert_eq!(opened.calendars[0].address.kind, CALENDAR_KIND);
        match opened.span {
            CalendarSpan::Date { start, end } => {
                assert_eq!(start, "2026-09-22");
                assert_eq!(end.as_deref(), Some("2026-09-24"));
            }
            CalendarSpan::Time { .. } => panic!("date event"),
        }
        let named = author.sign(
            1_700_000_010,
            DATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "named".into()]),
                Tag::new(vec!["name".into(), "Legacy title".into()]),
                Tag::new(vec!["start".into(), "2024-02-29".into()]),
            ],
            String::new(),
        );
        assert_eq!(open_calendar_event(&named).unwrap().title, "Legacy title");
        let newer = author.sign(
            1_700_000_100,
            DATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "offsite".into()]),
                Tag::new(vec!["title".into(), "Offsite".into()]),
                Tag::new(vec!["start".into(), "2026-09-22".into()]),
            ],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&dated, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );
        let backwards = author.sign(
            1_700_000_110,
            DATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "backwards".into()]),
                Tag::new(vec!["title".into(), "Backwards".into()]),
                Tag::new(vec!["start".into(), "2026-09-24".into()]),
                Tag::new(vec!["end".into(), "2026-09-22".into()]),
            ],
            String::new(),
        );
        assert!(backwards.validate_structure().is_err());
        let leap = author.sign(
            1_700_000_120,
            DATE_KIND,
            vec![
                Tag::new(vec!["d".into(), "leap".into()]),
                Tag::new(vec!["title".into(), "Leap".into()]),
                Tag::new(vec!["start".into(), "2023-02-29".into()]),
            ],
            String::new(),
        );
        assert!(leap.validate_structure().is_err());

        let start = 1_700_000_000_u64;
        let timed = author.sign(
            1_700_000_200,
            TIME_KIND,
            vec![
                Tag::new(vec!["d".into(), "offsite".into()]),
                Tag::new(vec!["title".into(), "Offsite call".into()]),
                Tag::new(vec!["start".into(), start.to_string()]),
                Tag::new(vec!["end".into(), (start + 3_600).to_string()]),
                Tag::new(vec!["start_tzid".into(), "America/Costa_Rica".into()]),
                Tag::new(vec!["D".into(), day_stamp(start).to_string()]),
                Tag::new(vec!["summary".into(), "One hour".into()]),
            ],
            "Bring notes.".into(),
        );
        timed.validate_structure().unwrap();
        let timed_open = open_calendar_event(&timed).unwrap();
        match timed_open.span {
            CalendarSpan::Time {
                start: seen,
                end,
                start_tzid,
                end_tzid,
                days,
            } => {
                assert_eq!(seen, start);
                assert_eq!(end, Some(start + 3_600));
                assert_eq!(start_tzid.as_deref(), Some("America/Costa_Rica"));
                assert_eq!(end_tzid.as_deref(), Some("America/Costa_Rica"));
                assert_eq!(days, vec![day_stamp(start)]);
            }
            CalendarSpan::Date { .. } => panic!("time event"),
        }
        assert!(matches!(
            compare_replacement(&dated, &timed),
            Err(DomainError::ReplacementAddressMismatch)
        ));
        let missing_day = author.sign(
            1_700_000_210,
            TIME_KIND,
            vec![
                Tag::new(vec!["d".into(), "missing-day".into()]),
                Tag::new(vec!["title".into(), "Missing".into()]),
                Tag::new(vec!["start".into(), start.to_string()]),
                Tag::new(vec!["D".into(), (day_stamp(start) + 1).to_string()]),
            ],
            String::new(),
        );
        assert!(missing_day.validate_structure().is_err());

        let calendar = author.sign(
            1_700_000_300,
            CALENDAR_KIND,
            vec![
                Tag::new(vec!["d".into(), "work".into()]),
                Tag::new(vec!["title".into(), "Work".into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("31922:{}:offsite", author.pubkey()),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "a".into(),
                    format!("31923:{}:offsite", author.pubkey()),
                ]),
            ],
            String::new(),
        );
        calendar.validate_structure().unwrap();
        assert_eq!(calendar.class(), EventClass::Addressable);
        let calendar = open_calendar(&calendar).unwrap();
        assert_eq!(calendar.events.len(), 2);
        assert_eq!(calendar.events[0].address.kind, DATE_KIND);
        assert_eq!(calendar.events[1].address.kind, TIME_KIND);

        let rsvp = guest.sign(
            1_700_000_400,
            RSVP_KIND,
            vec![
                Tag::new(vec![
                    "a".into(),
                    format!("31922:{}:offsite", author.pubkey()),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    dated.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["d".into(), "guest-offsite".into()]),
                Tag::new(vec!["status".into(), "accepted".into()]),
                Tag::new(vec!["fb".into(), "busy".into()]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
            ],
            "I will be there.".into(),
        );
        rsvp.validate_structure().unwrap();
        let rsvp = open_rsvp(&rsvp).unwrap();
        assert_eq!(rsvp.status, Attendance::Accepted);
        assert_eq!(rsvp.availability, Some(Availability::Busy));
        assert_eq!(rsvp.revision.as_deref(), Some(dated.id.as_str()));
        assert_eq!(rsvp.event.address.identifier, "offsite");
        let declined = guest.sign(
            1_700_000_410,
            RSVP_KIND,
            vec![
                Tag::new(vec![
                    "a".into(),
                    format!("31922:{}:offsite", author.pubkey()),
                ]),
                Tag::new(vec!["d".into(), "guest-declined".into()]),
                Tag::new(vec!["status".into(), "declined".into()]),
                Tag::new(vec!["fb".into(), "free".into()]),
            ],
            String::new(),
        );
        let declined = open_rsvp(&declined).unwrap();
        assert_eq!(declined.status, Attendance::Declined);
        assert!(declined.availability.is_none());
        let bad = guest.sign(
            1_700_000_420,
            RSVP_KIND,
            vec![
                Tag::new(vec![
                    "a".into(),
                    format!("31922:{}:offsite", author.pubkey()),
                ]),
                Tag::new(vec!["d".into(), "guest-bad".into()]),
                Tag::new(vec!["status".into(), "maybe".into()]),
            ],
            String::new(),
        );
        assert!(bad.validate_structure().is_err());
    }
}
