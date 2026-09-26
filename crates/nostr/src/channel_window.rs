//! NIP-CW channel windows.
//!
//! A window is a cursor-paged view of one channel's top-level rows. The
//! relay signs `kind:39006` bounds for every served page. A page without
//! that bounds event is not a window.

use std::collections::{HashMap, HashSet};

use crate::domain::{Event, RelaySigner, Tag};

/// Relay-signed thread summary.
pub const SUMMARY_KIND: u16 = 39_005;
/// Relay-signed window bounds. One per served page.
pub const BOUNDS_KIND: u16 = 39_006;
/// Rows requested when a filter omits `limit`.
pub const DEFAULT_LIMIT: usize = 50;
/// Largest row budget a window will serve.
pub const MAX_LIMIT: usize = 200;
const MAX_DEPTH: u32 = 100;
const AUX_CAP: usize = 1_000;

/// Composite cursor. `id` is 64 lowercase hex characters.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    /// Unix seconds.
    pub created_at: u64,
    /// Event id.
    pub id: String,
}

/// A parsed window filter. Absent `top_level: true` is not a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRequest {
    /// The single `#h` channel.
    pub channel: String,
    /// Row budget. Overlays do not count.
    pub limit: usize,
    /// Optional row-kind restriction.
    pub kinds: Option<Vec<u16>>,
    /// Include `kind:39005` summaries.
    pub include_summaries: bool,
    /// Include reactions, deletions, and edits that reference the rows.
    pub include_aux: bool,
    /// Absent on a head request.
    pub cursor: Option<Cursor>,
}

/// One served page, rows first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPage {
    /// Signed events: rows, then aux, summaries, and bounds.
    pub events: Vec<Event>,
    /// True when another page exists.
    pub has_more: bool,
    /// Cursor to echo, present only when `has_more` is true.
    pub next_cursor: Option<Cursor>,
}

/// Parse a filter object. `Ok(None)` means serve it as an ordinary filter.
///
/// # Errors
///
/// Returns a reason when `top_level` is true and the window fields are
/// malformed. Those requests must not be downgraded to a half cursor.
pub fn parse_window(value: &serde_json::Value) -> Result<Option<WindowRequest>, &'static str> {
    let object = value.as_object().ok_or("filter")?;
    if object.get("top_level") != Some(&serde_json::Value::Bool(true)) {
        return Ok(None);
    }
    let channels = string_list(object.get("#h"))?;
    let channel = match channels.as_slice() {
        [channel] if !channel.is_empty() => channel.clone(),
        _ => return Err("channel"),
    };
    let limit = match object.get("limit") {
        None => DEFAULT_LIMIT,
        Some(serde_json::Value::Number(number)) => {
            let limit = number.as_u64().ok_or("limit")?;
            usize::try_from(limit)
                .unwrap_or(MAX_LIMIT)
                .clamp(1, MAX_LIMIT)
        }
        Some(_) => return Err("limit"),
    };
    let kinds = match object.get("kinds") {
        None => None,
        Some(value) => Some(kind_list(value)?),
    };
    let cursor = match (object.get("until"), object.get("before_id")) {
        (None, None) => None,
        (Some(until), Some(before_id)) => Some(Cursor {
            created_at: until.as_u64().ok_or("until")?,
            id: hex_id(before_id.as_str().ok_or("before_id")?)?,
        }),
        _ => return Err("cursor"),
    };
    Ok(Some(WindowRequest {
        channel,
        limit,
        kinds,
        include_summaries: object.get("include_summaries") == Some(&serde_json::Value::Bool(true)),
        include_aux: object.get("include_aux") == Some(&serde_json::Value::Bool(true)),
        cursor,
    }))
}

/// Serve a window from events the reader is already allowed to see.
///
/// `truncated` is true when the supplied slice stopped at a scan budget
/// and older channel events may exist. The function refuses a short page
/// in that case instead of reporting a false end.
///
/// # Errors
///
/// Returns `scan` when the budget hides the next row.
pub fn render_window(
    events: &[Event],
    request: &WindowRequest,
    truncated: bool,
    signer: &RelaySigner,
    now: u64,
) -> Result<WindowPage, &'static str> {
    let rows = candidate_rows(events, request);
    let mut matched = rows
        .into_iter()
        .filter(|event| cursor_keeps(event, request.cursor.as_ref()))
        .collect::<Vec<_>>();
    matched.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    if truncated && matched.len() <= request.limit {
        return Err("scan");
    }
    let has_more = matched.len() > request.limit;
    matched.truncate(request.limit);
    let next_cursor = has_more.then(|| Cursor {
        created_at: matched.last().map(|event| event.created_at).unwrap_or(0),
        id: matched
            .last()
            .map(|event| event.id.clone())
            .unwrap_or_default(),
    });
    if has_more && next_cursor.is_none() {
        return Err("cursor");
    }
    let mut page = matched
        .iter()
        .map(|event| (*event).clone())
        .collect::<Vec<_>>();
    if request.include_aux && !matched.is_empty() {
        page.extend(aux_closure(events, &matched));
    }
    if request.include_summaries {
        for summary in summaries(events, &matched) {
            page.push(sign_overlay(
                signer,
                now,
                SUMMARY_KIND,
                vec![
                    Tag::new(vec!["e".into(), summary.row_id.clone()]),
                    Tag::new(vec!["d".into(), summary.row_id.clone()]),
                    Tag::new(vec!["h".into(), request.channel.clone()]),
                ],
                summary.content,
            ));
        }
    }
    let suffix = match &request.cursor {
        None => "head".to_owned(),
        Some(cursor) => format!("{}:{}", cursor.created_at, cursor.id),
    };
    let content = serde_json::to_string(&serde_json::json!({
        "has_more": has_more,
        "next_cursor": next_cursor.as_ref().map(|cursor| serde_json::json!({
            "created_at": cursor.created_at,
            "id": cursor.id,
        })),
    }))
    .expect("bounds content is finite JSON");
    page.push(sign_overlay(
        signer,
        now,
        BOUNDS_KIND,
        vec![
            Tag::new(vec!["d".into(), format!("{}:{suffix}", request.channel)]),
            Tag::new(vec!["h".into(), request.channel.clone()]),
        ],
        content,
    ));
    Ok(WindowPage {
        events: page,
        has_more,
        next_cursor,
    })
}

/// Accept a served page. A missing, extra, or unbound bounds event is refused.
///
/// # Errors
///
/// Returns a static reason. Callers discard the page.
pub fn accept_window(
    events: &[Event],
    channel: &str,
    request_cursor: Option<&Cursor>,
    relay_pubkey: &str,
) -> Result<(), &'static str> {
    let bounds = events
        .iter()
        .filter(|event| event.kind == BOUNDS_KIND)
        .collect::<Vec<_>>();
    if bounds.len() != 1 {
        return Err("bounds");
    }
    let bounds = bounds[0];
    if bounds.pubkey != relay_pubkey {
        return Err("signer");
    }
    bounds.validate_crypto().map_err(|_| "signature")?;
    let tags = bounds.tags.iter().map(Tag::name).collect::<Vec<_>>();
    if tags != [Some("d"), Some("h")] || bounds.tags.iter().any(|tag| tag.as_slice().len() != 2) {
        return Err("tags");
    }
    if bounds.tag_values("h").next() != Some(channel) {
        return Err("channel");
    }
    let suffix = match request_cursor {
        None => "head".to_owned(),
        Some(cursor) => format!("{}:{}", cursor.created_at, cursor.id),
    };
    if bounds.tag_values("d").next() != Some(format!("{channel}:{suffix}").as_str()) {
        return Err("binding");
    }
    let content =
        crate::contracts::parse_strict(bounds.content.as_bytes()).map_err(|_| "content")?;
    let has_more = content
        .get("has_more")
        .and_then(serde_json::Value::as_bool)
        .ok_or("content")?;
    let next = content.get("next_cursor").ok_or("content")?;
    let next_present = !next.is_null();
    if has_more != next_present {
        return Err("exhaustion");
    }
    if next_present {
        let object = next.as_object().ok_or("content")?;
        hex_id(
            object
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or("content")?,
        )?;
        object
            .get("created_at")
            .and_then(serde_json::Value::as_u64)
            .ok_or("content")?;
    }
    Ok(())
}

fn candidate_rows<'a>(events: &'a [Event], request: &WindowRequest) -> Vec<&'a Event> {
    let by_id = events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect::<HashMap<_, _>>();
    let deleted = deleted_ids(events);
    events
        .iter()
        .filter(|event| event.tag_values("h").any(|value| value == request.channel))
        .filter(|event| !deleted.contains(event.id.as_str()))
        .filter(|event| !matches!(event.kind, SUMMARY_KIND | BOUNDS_KIND))
        .filter(|event| {
            request
                .kinds
                .as_ref()
                .is_none_or(|kinds| kinds.contains(&event.kind))
        })
        .filter(|event| is_top_level(event, &by_id))
        .collect()
}

fn is_top_level(event: &Event, by_id: &HashMap<&str, &Event>) -> bool {
    match depth(event, by_id, &mut HashSet::new()) {
        Some(0) | None => true,
        Some(1) => broadcast(event),
        Some(_) => false,
    }
}

fn depth(event: &Event, by_id: &HashMap<&str, &Event>, stack: &mut HashSet<String>) -> Option<u32> {
    if stack.len() as u32 >= MAX_DEPTH || !stack.insert(event.id.clone()) {
        return None;
    }
    let Some(parent_id) = reply_parent(event) else {
        return Some(0);
    };
    let parent = by_id.get(parent_id)?;
    depth(parent, by_id, stack).map(|parent_depth| parent_depth.saturating_add(1))
}

fn reply_parent(event: &Event) -> Option<&str> {
    event.tags.iter().find_map(|tag| {
        let values = tag.as_slice();
        (values.len() >= 4 && values[0] == "e" && values[3] == "reply" && is_hex(&values[1]))
            .then(|| values[1].as_str())
    })
}

fn broadcast(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.as_slice() == ["broadcast", "1"])
}

fn cursor_keeps(event: &Event, cursor: Option<&Cursor>) -> bool {
    let Some(cursor) = cursor else {
        return true;
    };
    event.created_at < cursor.created_at
        || (event.created_at == cursor.created_at && event.id.as_str() > cursor.id.as_str())
}

fn deleted_ids(events: &[Event]) -> HashSet<&str> {
    events
        .iter()
        .filter(|event| matches!(event.kind, 5 | 9_005))
        .flat_map(|event| event.tag_values("e"))
        .filter(|value| is_hex(value))
        .collect()
}

fn aux_closure(events: &[Event], rows: &[&Event]) -> Vec<Event> {
    let row_ids = rows
        .iter()
        .map(|event| event.id.as_str())
        .collect::<HashSet<_>>();
    let mut hop1 = Vec::new();
    for event in events {
        if hop1.len() >= AUX_CAP {
            break;
        }
        if matches!(event.kind, 7 | 5 | 9_005 | 40_003)
            && event.tag_values("e").any(|value| row_ids.contains(value))
        {
            hop1.push(event.clone());
        }
    }
    let hop1_ids = hop1
        .iter()
        .map(|event| event.id.as_str())
        .collect::<HashSet<_>>();
    let mut hop2 = Vec::new();
    for event in events {
        if hop2.len() >= AUX_CAP {
            break;
        }
        if matches!(event.kind, 5 | 9_005)
            && event.tag_values("e").any(|value| hop1_ids.contains(value))
            && hop1.iter().all(|existing| existing.id != event.id)
        {
            hop2.push(event.clone());
        }
    }
    hop1.extend(hop2);
    hop1
}

struct Summary {
    row_id: String,
    content: String,
}

fn summaries(events: &[Event], rows: &[&Event]) -> Vec<Summary> {
    let mut children: HashMap<&str, Vec<&Event>> = HashMap::new();
    for event in events {
        if let Some(parent) = reply_parent(event) {
            children.entry(parent).or_default().push(event);
        }
    }
    let mut summaries = Vec::new();
    for row in rows {
        let descendants = descendants(row.id.as_str(), &children);
        if descendants.is_empty() {
            continue;
        }
        let direct = children.get(row.id.as_str()).map(Vec::len).unwrap_or(0);
        let last_reply_at = descendants.iter().map(|event| event.created_at).max();
        let mut ordered = descendants.clone();
        ordered.sort_by_key(|event| std::cmp::Reverse(event.created_at));
        let mut participants = Vec::new();
        for event in ordered {
            if !participants.contains(&event.pubkey) {
                participants.push(event.pubkey.clone());
            }
            if participants.len() == 10 {
                break;
            }
        }
        let content = serde_json::to_string(&serde_json::json!({
            "reply_count": direct,
            "descendant_count": descendants.len(),
            "last_reply_at": last_reply_at,
            "participants": participants,
        }))
        .expect("summary content is finite JSON");
        summaries.push(Summary {
            row_id: row.id.clone(),
            content,
        });
    }
    summaries
}

fn descendants<'a>(root: &str, children: &HashMap<&str, Vec<&'a Event>>) -> Vec<&'a Event> {
    let mut out = Vec::new();
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        for child in children.get(id).into_iter().flatten() {
            out.push(*child);
            pending.push(child.id.as_str());
        }
    }
    out
}

fn sign_overlay(
    signer: &RelaySigner,
    now: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: String,
) -> Event {
    signer.sign(now, kind, tags, content)
}

fn string_list(value: Option<&serde_json::Value>) -> Result<Vec<String>, &'static str> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or("channel")?
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or("channel"))
        .collect()
}

fn kind_list(value: &serde_json::Value) -> Result<Vec<u16>, &'static str> {
    value
        .as_array()
        .ok_or("kinds")?
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|kind| u16::try_from(kind).ok())
                .ok_or("kinds")
        })
        .collect()
}

fn hex_id(value: &str) -> Result<String, &'static str> {
    is_hex(value).then(|| value.to_owned()).ok_or("before_id")
}

fn is_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(id_byte: u8, created_at: u64, kind: u16, tags: Vec<Tag>) -> Event {
        Event {
            id: format!("{id_byte:02x}{}", "ab".repeat(31)),
            pubkey: "11".repeat(32),
            created_at,
            kind,
            tags,
            content: String::new(),
            sig: "22".repeat(64),
        }
    }

    fn channel_tag() -> Tag {
        Tag::new(vec!["h".into(), "room".into()])
    }

    #[test]
    fn same_second_pages_keep_every_row_and_a_missing_bound_is_refused() {
        let signer = RelaySigner::from_secret_hex(&"33".repeat(32)).unwrap();
        let rows = (1..=3)
            .map(|byte| event(byte, 100, 1, vec![channel_tag()]))
            .collect::<Vec<_>>();
        let request = parse_window(&json!({"#h": ["room"], "top_level": true, "limit": 1}))
            .unwrap()
            .unwrap();
        let first = render_window(&rows, &request, false, &signer, 200).unwrap();
        assert!(first.has_more);
        assert_eq!(first.events[0].id, rows[0].id);
        accept_window(&first.events, "room", None, signer.pubkey()).unwrap();
        assert!(
            accept_window(
                &first.events[..first.events.len() - 1],
                "room",
                None,
                signer.pubkey()
            )
            .is_err()
        );
        let cursor = first.next_cursor.clone().unwrap();
        let continued = parse_window(&json!({
            "#h": ["room"],
            "top_level": true,
            "limit": 1,
            "until": cursor.created_at,
            "before_id": cursor.id,
        }))
        .unwrap()
        .unwrap();
        let second = render_window(&rows, &continued, false, &signer, 200).unwrap();
        assert_eq!(second.events[0].id, rows[1].id);
        accept_window(&second.events, "room", Some(&cursor), signer.pubkey()).unwrap();
    }

    #[test]
    fn a_reply_stays_out_of_the_window_unless_it_is_broadcast() {
        let signer = RelaySigner::from_secret_hex(&"44".repeat(32)).unwrap();
        let root = event(1, 50, 1, vec![channel_tag()]);
        let reply = event(
            2,
            60,
            1,
            vec![
                channel_tag(),
                Tag::new(vec!["e".into(), root.id.clone(), "".into(), "reply".into()]),
            ],
        );
        let broadcast = event(
            3,
            70,
            1,
            vec![
                channel_tag(),
                Tag::new(vec!["e".into(), root.id.clone(), "".into(), "reply".into()]),
                Tag::new(vec!["broadcast".into(), "1".into()]),
            ],
        );
        let request =
            parse_window(&json!({"#h": ["room"], "top_level": true, "include_summaries": true}))
                .unwrap()
                .unwrap();
        let page = render_window(
            &[root.clone(), reply, broadcast.clone()],
            &request,
            false,
            &signer,
            80,
        )
        .unwrap();
        let row_ids = page
            .events
            .iter()
            .filter(|event| event.kind == 1)
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(row_ids, vec![broadcast.id.as_str(), root.id.as_str()]);
        assert!(page.events.iter().any(|event| event.kind == SUMMARY_KIND));
        assert!(!page.has_more);
    }

    #[test]
    fn a_half_cursor_is_rejected() {
        let error =
            parse_window(&json!({"#h": ["room"], "top_level": true, "until": 10})).unwrap_err();
        assert_eq!(error, "cursor");
        assert!(
            parse_window(&json!({"#h": ["room"], "limit": 10}))
                .unwrap()
                .is_none()
        );
    }
}
