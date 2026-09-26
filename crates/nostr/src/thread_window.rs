//! Strict NIP-CW thread filters, signed bounds, and shared query budgets.
//!
//! Database adapters supply already authorized thread metadata and capture the
//! last retained scan candidate before reconstruction. These helpers cannot
//! establish complete auxiliary closure or a fresh batch-wide access check.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::channel_window::Cursor;
use crate::domain::{Event, RelaySigner, Tag};
use crate::read_state_snapshot::{hex_bytes, uuid_bytes};

pub const BOUNDS_KIND: u16 = 39_007;
pub const MAX_FILTERS: usize = 4;
pub const MAX_RAW_ROWS: usize = 8_192;
pub const MAX_AUX_SCANS: usize = 64;
pub const MAX_BYTES: usize = 8_388_608;
pub const DEADLINE_MS: u64 = 8_000;

/// Normalized request. Construct through [`parse_batch`] or call `validate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadWindowRequest {
    pub channel: String,
    pub root: String,
    pub kinds: Vec<u16>,
    pub depth_limit: u32,
    pub limit: usize,
    pub include_aux: bool,
    pub cursor: Option<Cursor>,
}

impl ThreadWindowRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        uuid_bytes(&self.channel).map_err(|_| "channel")?;
        hex_bytes::<32>(&self.root).map_err(|_| "root")?;
        if !(1..=200).contains(&self.limit) || !(1..=100).contains(&self.depth_limit) {
            return Err("limit");
        }
        if self.kinds.is_empty()
            || self.kinds.len() > 4
            || self.kinds.iter().any(|kind| !supported_root_kind(*kind))
            || self.kinds.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("kinds");
        }
        if let Some(cursor) = &self.cursor {
            hex_bytes::<32>(&cursor.id).map_err(|_| "cursor")?;
        }
        Ok(())
    }

    /// The wire filter, including explicit defaults, with a paired cursor.
    pub fn filter(&self) -> Result<Value, &'static str> {
        self.validate()?;
        let mut filter = serde_json::json!({
            "thread_window": true, "#h": [self.channel], "#e": [self.root],
            "kinds": self.kinds, "depth_limit": self.depth_limit,
            "limit": self.limit, "include_aux": self.include_aux,
        });
        if let Some(cursor) = &self.cursor {
            filter["until"] = cursor.created_at.into();
            filter["before_id"] = cursor.id.clone().into();
        }
        Ok(filter)
    }
}

/// True for supported thread roots and replies; a diff is not a root.
#[must_use]
pub fn supported_root_kind(kind: u16) -> bool {
    matches!(kind, 9 | 40002 | 45001 | 45003)
}

/// Parse an entire raw `/query` batch before dropping extension fields.
///
/// Any `thread_window` key activates strict mode, even when false or malformed.
/// Mixed modes, unknown fields, and more than four filters are refused.
pub fn parse_batch(raw: &Value) -> Result<Option<Vec<ThreadWindowRequest>>, &'static str> {
    let filters = raw.as_array().ok_or("filters")?;
    if !filters
        .iter()
        .any(|filter| filter.get("thread_window").is_some())
    {
        return Ok(None);
    }
    if filters.is_empty() || filters.len() > MAX_FILTERS {
        return Err("filters");
    }
    filters
        .iter()
        .map(parse_filter)
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn parse_filter(value: &Value) -> Result<ThreadWindowRequest, &'static str> {
    let object = value.as_object().ok_or("filter")?;
    if object.keys().any(|key| {
        ![
            "thread_window",
            "#h",
            "#e",
            "kinds",
            "depth_limit",
            "limit",
            "include_aux",
            "until",
            "before_id",
        ]
        .contains(&key.as_str())
    }) || object.get("thread_window") != Some(&Value::Bool(true))
    {
        return Err("mode");
    }
    let single = |key: &str| -> Result<String, &'static str> {
        let list = object
            .get(key)
            .and_then(Value::as_array)
            .ok_or("selector")?;
        let [value] = list.as_slice() else {
            return Err("selector");
        };
        value.as_str().map(str::to_owned).ok_or("selector")
    };
    let raw_kinds = object
        .get("kinds")
        .and_then(Value::as_array)
        .ok_or("kinds")?;
    if raw_kinds.is_empty() || raw_kinds.len() > 4 {
        return Err("kinds");
    }
    let mut kinds = raw_kinds
        .iter()
        .map(|value| u16::try_from(value.as_u64().ok_or("kinds")?).map_err(|_| "kinds"))
        .collect::<Result<Vec<_>, _>>()?;
    kinds.sort_unstable();
    kinds.dedup();
    let integer = |key: &str, default: u64| -> Result<u64, &'static str> {
        object
            .get(key)
            .map_or(Ok(default), |value| value.as_u64().ok_or("limit"))
    };
    let request = ThreadWindowRequest {
        channel: single("#h")?,
        root: single("#e")?,
        kinds,
        limit: usize::try_from(integer("limit", 50)?).map_err(|_| "limit")?,
        depth_limit: u32::try_from(integer("depth_limit", 100)?).map_err(|_| "limit")?,
        include_aux: object
            .get("include_aux")
            .map_or(Ok(false), |value| value.as_bool().ok_or("include_aux"))?,
        cursor: match (object.get("until"), object.get("before_id")) {
            (None, None) => None,
            (Some(until), Some(id)) => Some(Cursor {
                created_at: until.as_u64().ok_or("cursor")?,
                id: id.as_str().ok_or("cursor")?.into(),
            }),
            _ => return Err("cursor"),
        },
    };
    request.validate()?;
    Ok(request)
}

/// Derive the exact ordered-array request binding.
///
/// `host` must be the server-resolved normalized authority, not a caller's
/// untrusted header. Clients use the authority of their trusted relay origin.
pub fn binding(
    host: &str,
    reader: &str,
    request: &ThreadWindowRequest,
) -> Result<String, &'static str> {
    request.validate()?;
    hex_bytes::<32>(reader).map_err(|_| "reader")?;
    if host.is_empty()
        || !host.is_ascii()
        || host
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err("host");
    }
    let cursor = request
        .cursor
        .as_ref()
        .map(|cursor| serde_json::json!([cursor.created_at, cursor.id]));
    let bytes = serde_json::to_vec(&serde_json::json!([
        "tw",
        1,
        "older",
        host,
        reader,
        request.channel,
        request.root,
        request.limit,
        request.depth_limit,
        request.kinds,
        cursor,
        request.include_aux,
    ]))
    .map_err(|_| "binding")?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Signed exhaustion facts. The cursor is a scan position, not a delivered row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadBounds {
    pub version: u8,
    pub direction: String,
    pub has_more: bool,
    pub next_cursor: Option<Cursor>,
}

impl ThreadBounds {
    fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1 || self.direction != "older" {
            return Err("version");
        }
        if self.has_more != self.next_cursor.is_some() {
            return Err("exhaustion");
        }
        if let Some(cursor) = &self.next_cursor {
            hex_bytes::<32>(&cursor.id).map_err(|_| "cursor")?;
        }
        Ok(())
    }
}

/// Sign one page only after the adapter proves selection and refreshed access.
pub fn sign_bounds(
    signer: &RelaySigner,
    now: u64,
    host: &str,
    reader: &str,
    request: &ThreadWindowRequest,
    has_more: bool,
    next_cursor: Option<Cursor>,
) -> Result<Event, &'static str> {
    let bounds = ThreadBounds {
        version: 1,
        direction: "older".into(),
        has_more,
        next_cursor,
    };
    bounds.validate()?;
    Ok(signer.sign(
        now,
        BOUNDS_KIND,
        vec![
            Tag::new(vec![
                "d".into(),
                format!("tw:1:{}", binding(host, reader, request)?),
            ]),
            Tag::new(vec!["h".into(), request.channel.clone()]),
            Tag::new(vec!["e".into(), request.root.clone()]),
        ],
        serde_json::to_string(&bounds).map_err(|_| "content")?,
    ))
}

/// Validate exactly one bounds event. Invalid bounds never authorize fallback.
///
/// `missing_bounds` is the only result that can indicate an older relay, and
/// only after the caller has separately ruled out auth, timeout, and incomplete
/// response failures. Explicit legacy fallback must clear descending cursors.
pub fn accept_bounds(
    events: &[Event],
    host: &str,
    reader: &str,
    request: &ThreadWindowRequest,
    relay_pubkey: &str,
) -> Result<ThreadBounds, &'static str> {
    let bounds = events
        .iter()
        .filter(|event| event.kind == BOUNDS_KIND)
        .collect::<Vec<_>>();
    let event = match bounds.as_slice() {
        [] => return Err("missing_bounds"),
        [event] => *event,
        _ => return Err("bounds"),
    };
    if event.pubkey != relay_pubkey {
        return Err("signer");
    }
    event.validate_nip01_structure().map_err(|_| "signature")?;
    event.validate_crypto().map_err(|_| "signature")?;
    let expected = [
        ("d", format!("tw:1:{}", binding(host, reader, request)?)),
        ("h", request.channel.clone()),
        ("e", request.root.clone()),
    ];
    if event.tags.len() != 3
        || expected.iter().any(|(name, value)| {
            event
                .tags
                .iter()
                .filter(|tag| tag.as_slice() == [name.to_string(), value.clone()])
                .count()
                != 1
        })
    {
        return Err("binding");
    }
    let value = crate::contracts::parse_strict(event.content.as_bytes()).map_err(|_| "content")?;
    // serde treats an absent Option as None; the wire requires an explicit null.
    if value.get("next_cursor").is_none() {
        return Err("content");
    }
    let parsed: ThreadBounds = serde_json::from_value(value).map_err(|_| "content")?;
    parsed.validate()?;
    Ok(parsed)
}

/// Shared budget for the whole query, including retries. Exhaustion is sticky.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThreadBudget {
    aux_scans: usize,
    raw_rows: usize,
    stored_bytes: usize,
    output_bytes: usize,
    elapsed_ms: u64,
    failed: bool,
}

impl ThreadBudget {
    /// Charge each SQL auxiliary scan before starting it.
    pub fn charge_aux_scan(&mut self) -> Result<(), &'static str> {
        self.aux_scans = self.aux_scans.saturating_add(1);
        self.check()
    }

    /// Charge raw rows, including tombstones and probes, before reconstruction.
    pub fn charge_raw(
        &mut self,
        rows: usize,
        content_and_tag_bytes: usize,
    ) -> Result<(), &'static str> {
        self.raw_rows = self.raw_rows.saturating_add(rows);
        self.stored_bytes = self.stored_bytes.saturating_add(content_and_tag_bytes);
        self.check()
    }

    /// Charge compact output bytes for the entire batch, including bounds.
    pub fn charge_output(&mut self, bytes: usize) -> Result<(), &'static str> {
        self.output_bytes = self.output_bytes.saturating_add(bytes);
        self.check()
    }

    /// Observe elapsed time from one monotonic start shared across all retries.
    pub fn observe_elapsed(&mut self, milliseconds: u64) -> Result<(), &'static str> {
        if milliseconds < self.elapsed_ms {
            self.failed = true;
        }
        self.elapsed_ms = milliseconds;
        self.check()
    }

    fn check(&mut self) -> Result<(), &'static str> {
        self.failed |= self.aux_scans > MAX_AUX_SCANS
            || self.raw_rows > MAX_RAW_ROWS
            || self.stored_bytes > MAX_BYTES
            || self.output_bytes > MAX_BYTES
            || self.elapsed_ms >= DEADLINE_MS;
        if self.failed { Err("budget") } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{:064x}", 1)).unwrap()
    }
    fn request() -> ThreadWindowRequest {
        parse_batch(&serde_json::json!([{"thread_window":true,"#h":["00000000-0000-0000-0000-000000000001"],"#e":["11".repeat(32)],"kinds":[40002,9,9]}])).unwrap().unwrap().remove(0)
    }

    #[test]
    fn normalizes_only_allowed_requests_and_refuses_mixed_batches() {
        let request = request();
        assert_eq!(request.kinds, [9, 40002]);
        let valid = request.filter().unwrap();
        for (key, value) in [
            ("limit", serde_json::json!(0)),
            ("depth_limit", serde_json::json!(101)),
            ("top_level", serde_json::json!(true)),
            ("include_aux", serde_json::json!(1)),
            ("until", serde_json::json!(4)),
            ("kinds", serde_json::json!([40008])),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            assert!(parse_batch(&serde_json::json!([invalid])).is_err(), "{key}");
        }
        assert!(parse_batch(&serde_json::json!([valid, {}])).is_err());
        assert!(parse_batch(&serde_json::json!(vec![request.filter().unwrap(); 5])).is_err());
    }

    #[test]
    fn signed_binding_covers_host_reader_and_every_selection_field() {
        let request = request();
        let signer = signer();
        let reader = signer.pubkey();
        let cursor = Cursor {
            created_at: 3,
            id: "22".repeat(32),
        };
        let event = sign_bounds(
            &signer,
            1,
            "relay.example",
            reader,
            &request,
            true,
            Some(cursor.clone()),
        )
        .unwrap();
        assert_eq!(
            accept_bounds(
                std::slice::from_ref(&event),
                "relay.example",
                reader,
                &request,
                reader
            )
            .unwrap()
            .next_cursor,
            Some(cursor)
        );
        assert!(
            accept_bounds(
                std::slice::from_ref(&event),
                "other.example",
                reader,
                &request,
                reader
            )
            .is_err()
        );
        let mut changed = request.clone();
        changed.include_aux = true;
        assert!(
            accept_bounds(
                std::slice::from_ref(&event),
                "relay.example",
                reader,
                &changed,
                reader
            )
            .is_err()
        );
        changed = request.clone();
        changed.depth_limit = 1;
        assert!(
            accept_bounds(
                std::slice::from_ref(&event),
                "relay.example",
                reader,
                &changed,
                reader
            )
            .is_err()
        );
        changed = request.clone();
        changed.limit = 1;
        assert!(
            accept_bounds(
                std::slice::from_ref(&event),
                "relay.example",
                reader,
                &changed,
                reader
            )
            .is_err()
        );
        assert_eq!(
            accept_bounds(&[], "relay.example", reader, &request, reader),
            Err("missing_bounds")
        );
        assert_eq!(
            accept_bounds(
                &[event.clone(), event],
                "relay.example",
                reader,
                &request,
                reader
            ),
            Err("bounds")
        );
    }

    #[test]
    fn malformed_signed_bounds_never_become_exhaustion() {
        let request = request();
        let signer = signer();
        let reader = signer.pubkey();
        let event =
            sign_bounds(&signer, 1, "relay.example", reader, &request, false, None).unwrap();
        for content in [
            r#"{"version":1,"direction":"older","has_more":false}"#,
            r#"{"version":1,"direction":"older","has_more":true,"next_cursor":null}"#,
            r#"{"version":1,"direction":"older","has_more":false,"has_more":false,"next_cursor":null}"#,
        ] {
            let bad = signer.sign(1, BOUNDS_KIND, event.tags.clone(), content.into());
            assert!(accept_bounds(&[bad], "relay.example", reader, &request, reader).is_err());
        }
    }

    #[test]
    fn budgets_count_probes_and_retry_work_and_cannot_reset_after_exhaustion() {
        let mut budget = ThreadBudget::default();
        budget.charge_raw(MAX_RAW_ROWS, MAX_BYTES).unwrap();
        assert!(budget.charge_raw(1, 0).is_err());
        assert!(budget.charge_output(0).is_err());
        let mut budget = ThreadBudget::default();
        for _ in 0..MAX_AUX_SCANS {
            budget.charge_aux_scan().unwrap();
        }
        assert!(budget.charge_aux_scan().is_err());
        let mut budget = ThreadBudget::default();
        assert!(budget.charge_output(MAX_BYTES + 1).is_err());
        let mut budget = ThreadBudget::default();
        budget.observe_elapsed(DEADLINE_MS - 1).unwrap();
        assert!(budget.observe_elapsed(DEADLINE_MS).is_err());
    }
}
