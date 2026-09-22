//! `POST /v1/feedback` and `GET /v1/feedback/{id}`: structured agent
//! feedback with trackable receipts.
//!
//! A submission is a versioned envelope — `openagents.feedback.v1` —
//! carrying an observation, optional expected and actual behavior,
//! environment and version notes, bounded reproduction evidence, and a
//! bounded set of attachments. The service persists the record before
//! it acknowledges, seals a [`FeedbackReceipt`] (a distinct schema from
//! the inference receipt), and answers status lookups to the
//! submitting credential only.
//!
//! Triage is an operator act, not a model's: the `gateway-feedback`
//! binary appends transitions — `accepted`, `needs-information`,
//! `resolved`, `rejected`, `redact` — to the record's journal. An
//! identical resubmission names the earlier record as `duplicate`; the
//! idempotency key binds the credential and the content, so a changed
//! payload under a reused key is a conflict.
//!
//! Forwarding a report's private content to another system happens
//! only under the submitter's explicit `consent.forward`, recorded on
//! the record; nothing here forwards anything. Retention and deletion
//! are operator acts on the record directory — the operator CLI's
//! `redact` replaces stored content with its digests in place.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde_json::{Value, json};

use receipts::feedback::{FeedbackReceipt, FeedbackState};

use crate::serve::{ServeState, authenticate, now_utc};

/// The submission schema this module accepts.
pub const SCHEMA: &str = "openagents.feedback.v1";

/// The idempotency index beside the record directories.
const INDEX: &str = "index.jsonl";

/// Non-terminal submissions one credential may hold at once.
const PENDING_MAX: usize = 64;

/// The bounded field sizes a submission may carry.
const OBSERVATION_MAX: usize = 4_096;
const FIELD_MAX: usize = 4_096;
const REPRODUCTION_MAX: usize = 8_192;
const ENV_FIELD_MAX: usize = 256;
const ATTACHMENT_MAX: usize = 4;
const ATTACHMENT_BYTES_MAX: usize = 65_536;
const ATTACHMENTS_TOTAL_MAX: usize = 262_144;

// ---------- routes ----------

/// `POST /v1/feedback`: accept one structured report. The record and
/// its receipt persist before the service acknowledges; an identical
/// replay under the same `Idempotency-Key` returns the stored status,
/// a changed payload under the same key conflicts, and identical
/// content under a new key records `duplicate` against the first
/// submission.
pub(crate) async fn submit(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let Ok(envelope) = serde_json::from_slice::<Value>(&body) else {
        return fb_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "the body is not a JSON submission envelope",
            None,
        );
    };
    if let Err(message) = validate(&envelope) {
        return fb_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            &message,
            None,
        );
    }
    let (_registry, caller) = match authenticate(&state, &headers) {
        Ok(parts) => parts,
        Err((status, code, message)) => return fb_error(status, code, &message, None),
    };
    // A report must name who filed it — the anonymous caller the
    // serving paths admit is not a feedback submitter.
    if caller.tenant.is_none() {
        return fb_error(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "feedback requires a tenant bearer key",
            None,
        );
    }
    let idem = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("fbk_{}", hex(&secp256k1::rand::random::<[u8; 16]>())));
    let digest = receipts::execution::digest_request(&envelope);
    let scope = caller.key.clone();
    match index_lookup(&state.dir, &scope, &idem) {
        Some(entry) if entry.digest == digest => {
            let dir = record_dir(&state.dir, &entry.submission);
            return match read_json(&dir.join("status.json")) {
                Some(status) => (StatusCode::OK, Json(view(&dir, &status))).into_response(),
                None => fb_error(
                    StatusCode::GONE,
                    "submission_deleted",
                    "the submission's record was deleted; it is not replayed",
                    Some(&entry.submission),
                ),
            };
        }
        Some(_) => {
            return fb_error(
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "the idempotency key already named a different submission",
                None,
            );
        }
        None => {}
    }
    if pending(&state.dir, &scope) >= PENDING_MAX {
        return fb_error(
            StatusCode::TOO_MANY_REQUESTS,
            "capacity",
            "too many open feedback submissions under this credential",
            None,
        );
    }
    // Identical content under a different key is a duplicate, recorded
    // against the first submission rather than dropped.
    let duplicate_of = content_lookup(&state.dir, &scope, &digest);
    let submission = format!("fb_{}", hex(&submission_id(&scope, &idem, &digest)));
    let received = now_utc();
    let state_name = if duplicate_of.is_some() {
        "duplicate"
    } else {
        "submitted"
    };
    let mut receipt = FeedbackReceipt::for_submission(&submission, &digest, &received);
    receipt.tenant = caller.tenant.as_ref().map(|_| caller.key.clone());
    receipt.status = if duplicate_of.is_some() {
        FeedbackState::Duplicate
    } else {
        FeedbackState::Submitted
    };
    receipt.seal();
    let manifest = json!({
        "v": SCHEMA,
        "submission": submission,
        "tenant": caller.tenant,
        "key": caller.key,
        "workspace": caller.workspace,
        "idem": idem,
        "digest": digest,
        "received_at": received,
        "envelope": envelope,
    });
    let mut status = json!({
        "v": SCHEMA,
        "submission": submission,
        "status": state_name,
        "received_at": received,
        "receipt": serde_json::to_value(&receipt).unwrap_or(Value::Null),
    });
    if let Some(first) = &duplicate_of {
        status["duplicate_of"] = json!(first);
    }
    let dir = record_dir(&state.dir, &submission);
    if let Err(message) = persist(&dir, &manifest, &status, &receipt, &envelope) {
        return fb_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "submission_unavailable",
            &message,
            None,
        );
    }
    if let Err(message) = index_append(&state.dir, &scope, &idem, &submission, &digest) {
        let _ = fs::remove_dir_all(&dir);
        return fb_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "submission_unavailable",
            &message,
            None,
        );
    }
    (StatusCode::ACCEPTED, Json(view(&dir, &status))).into_response()
}

/// `GET /v1/feedback/{id}`: the submission's status and triage
/// journal — the submitting credential's view only. Another
/// credential's lookup answers `submission_not_found`, so existence is
/// not disclosed across tenants.
pub(crate) async fn status(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let Some((dir, status)) = load_owned(&state, &headers, &id) else {
        return fb_error(
            StatusCode::NOT_FOUND,
            "submission_not_found",
            "no such feedback submission",
            None,
        );
    };
    Json(view(&dir, &status)).into_response()
}

// ---------- the operator's triage ----------

/// One lifecycle step on a stored record — `accept`,
/// `needs-information`, `resolve`, `reject`, or `redact`. The
/// `gateway-feedback` binary is the only writer; appending a line to
/// `transitions.jsonl` and rewriting `status.json` is the whole act.
/// A redaction additionally removes stored content in place, keeping
/// each removed field's digest.
///
/// # Errors
///
/// Returns a message when the record is missing, the transition is not
/// a known lifecycle step, or the files cannot be written.
pub fn transition(root: &Path, id: &str, step: &str, note: Option<&str>) -> Result<Value, String> {
    let dir = record_dir(root, id);
    let mut status = read_json(&dir.join("status.json"))
        .ok_or_else(|| format!("no such feedback submission: {id}"))?;
    let mapped: String = match step {
        "accept" => "accepted".to_string(),
        "needs-information" => "needs-information".to_string(),
        "resolve" => "resolved".to_string(),
        "reject" => "rejected".to_string(),
        "redact" => {
            redact(&dir)?;
            // A redaction keeps the record's lifecycle state.
            status["status"].as_str().unwrap_or("submitted").to_string()
        }
        other => return Err(format!("unknown transition: {other}")),
    };
    let line = json!({
        "at": now_utc(),
        "status": mapped,
        "note": note,
        "actor": "operator",
    });
    append_line(&dir.join("transitions.jsonl"), &line);
    status["status"] = json!(mapped);
    if step == "redact" {
        status["redacted"] = json!(true);
    }
    if let Some(note) = note {
        status["note"] = json!(note);
    }
    write_json(&dir.join("status.json"), &status)?;
    Ok(status)
}

/// Replace a record's stored content with its digests — attachments
/// and free-text fields alike. The record and its receipt stay; the
/// content is gone.
fn redact(dir: &Path) -> Result<(), String> {
    let path = dir.join("manifest.json");
    let mut manifest = read_json(&path).ok_or("no manifest to redact")?;
    let envelope = manifest["envelope"].clone();
    let mut digests = serde_json::Map::new();
    for field in [
        "observation",
        "expected",
        "actual",
        "reproduction",
        "environment",
        "attachments",
        "consent",
    ] {
        if let Some(value) = envelope.get(field) {
            digests.insert(
                field.to_string(),
                json!(receipts::execution::digest_request(value)),
            );
        }
    }
    manifest["envelope"] = json!({
        "v": SCHEMA,
        "redacted": true,
        "digests": digests,
    });
    let _ = fs::remove_dir_all(dir.join("attachments"));
    write_json(&path, &manifest)
}

// ---------- validation ----------

/// Whether an envelope is an `openagents.feedback.v1` submission this
/// service can store.
fn validate(envelope: &Value) -> Result<(), String> {
    let Some(object) = envelope.as_object() else {
        return Err("the submission is not a JSON object".to_string());
    };
    if object.get("v").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(format!("the submission does not carry `v: {SCHEMA}`"));
    }
    let observation = object
        .get("observation")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "a submission needs a non-empty `observation`".to_string())?;
    bounded("observation", observation, OBSERVATION_MAX)?;
    for field in ["expected", "actual"] {
        if let Some(value) = object.get(field) {
            let value = value
                .as_str()
                .ok_or_else(|| format!("`{field}` is a string or absent"))?;
            bounded(field, value, FIELD_MAX)?;
        }
    }
    if let Some(reproduction) = object.get("reproduction") {
        let reproduction = reproduction
            .as_str()
            .ok_or_else(|| "`reproduction` is a string or absent".to_string())?;
        bounded("reproduction", reproduction, REPRODUCTION_MAX)?;
    }
    if let Some(environment) = object.get("environment") {
        let environment = environment
            .as_object()
            .ok_or_else(|| "`environment` is an object or absent".to_string())?;
        for (field, value) in environment {
            let value = value
                .as_str()
                .ok_or_else(|| format!("`environment.{field}` is a string"))?;
            bounded(&format!("environment.{field}"), value, ENV_FIELD_MAX)?;
        }
    }
    if let Some(attachments) = object.get("attachments") {
        let attachments = attachments
            .as_array()
            .ok_or_else(|| "`attachments` is an array or absent".to_string())?;
        if attachments.len() > ATTACHMENT_MAX {
            return Err(format!("at most {ATTACHMENT_MAX} attachments"));
        }
        let mut total = 0usize;
        for attachment in attachments {
            let attachment = attachment
                .as_object()
                .ok_or_else(|| "an attachment is an object".to_string())?;
            let name = attachment
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "an attachment needs a `name`".to_string())?;
            bounded("attachment name", name, ENV_FIELD_MAX)?;
            let encoded = attachment
                .get("content_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| "an attachment needs `content_base64`".to_string())?;
            let bytes = decode_base64(encoded)
                .ok_or_else(|| format!("attachment `{name}` is not base64"))?;
            if bytes.len() > ATTACHMENT_BYTES_MAX {
                return Err(format!(
                    "attachment `{name}` exceeds {ATTACHMENT_BYTES_MAX} bytes"
                ));
            }
            total += bytes.len();
        }
        if total > ATTACHMENTS_TOTAL_MAX {
            return Err(format!(
                "attachments exceed {ATTACHMENTS_TOTAL_MAX} bytes in total"
            ));
        }
    }
    if let Some(consent) = object.get("consent") {
        let consent = consent
            .as_object()
            .ok_or_else(|| "`consent` is an object or absent".to_string())?;
        if let Some(forward) = consent.get("forward")
            && !forward.is_boolean()
        {
            return Err("`consent.forward` is a boolean".to_string());
        }
    }
    Ok(())
}

fn bounded(field: &str, value: &str, max: usize) -> Result<(), String> {
    if value.len() > max {
        return Err(format!("`{field}` exceeds {max} bytes"));
    }
    Ok(())
}

/// Decode a base64 body — enough alphabet for attachment blobs, no
/// dependency beyond what the workspace already carries.
fn decode_base64(encoded: &str) -> Option<Vec<u8>> {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = encoded
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    let mut chunk = [0u8; 4];
    let mut size = 0usize;
    for byte in clean {
        if byte == b'=' {
            break;
        }
        chunk[size] = nibble(byte)?;
        size += 1;
        if size == 4 {
            out.extend_from_slice(&[
                chunk[0] << 2 | chunk[1] >> 4,
                chunk[1] << 4 | chunk[2] >> 2,
                chunk[2] << 6 | chunk[3],
            ]);
            size = 0;
        }
    }
    match size {
        0 => {}
        2 => out.push(chunk[0] << 2 | chunk[1] >> 4),
        3 => out.extend_from_slice(&[chunk[0] << 2 | chunk[1] >> 4, chunk[1] << 4 | chunk[2] >> 2]),
        _ => return None,
    }
    Some(out)
}

// ---------- persistence ----------

fn record_dir(root: &Path, id: &str) -> PathBuf {
    root.join("feedback").join(id)
}

/// The submission id: the credential, the key, and the content, so a
/// replay under the same key names the same record.
fn submission_id(scope: &str, key: &str, digest: &str) -> [u8; 16] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(scope.as_bytes());
    hasher.update([0]);
    hasher.update(key.as_bytes());
    hasher.update([0]);
    hasher.update(digest.as_bytes());
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

/// The first submission id under this scope carrying the same content
/// digest — the record a repeat submission duplicates.
fn content_lookup(root: &Path, scope: &str, digest: &str) -> Option<String> {
    read_lines(&root.join("feedback").join(INDEX))
        .iter()
        .find(|line| {
            line["scope"].as_str() == Some(scope) && line["digest"].as_str() == Some(digest)
        })
        .and_then(|line| line["submission"].as_str().map(str::to_string))
}

struct IndexEntry {
    digest: String,
    submission: String,
}

fn index_lookup(root: &Path, scope: &str, key: &str) -> Option<IndexEntry> {
    read_lines(&root.join("feedback").join(INDEX))
        .iter()
        .rfind(|line| line["scope"].as_str() == Some(scope) && line["key"].as_str() == Some(key))
        .map(|line| IndexEntry {
            digest: line["digest"].as_str().unwrap_or_default().to_string(),
            submission: line["submission"].as_str().unwrap_or_default().to_string(),
        })
}

fn index_append(
    root: &Path,
    scope: &str,
    key: &str,
    submission: &str,
    digest: &str,
) -> Result<(), String> {
    let path = root.join("feedback").join(INDEX);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|trouble| trouble.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|trouble| trouble.to_string())?;
    let line = json!({
        "scope": scope,
        "key": key,
        "submission": submission,
        "digest": digest,
        "at": now_utc(),
    });
    writeln!(file, "{line}").map_err(|trouble| trouble.to_string())?;
    file.sync_data().map_err(|trouble| trouble.to_string())
}

/// This credential's open submissions — the states that still need
/// triage. A duplicate is already answered by its first record, and a
/// resolved or rejected record is done.
fn pending(root: &Path, scope: &str) -> usize {
    const OPEN: [&str; 3] = ["submitted", "accepted", "needs-information"];
    read_lines(&root.join("feedback").join(INDEX))
        .iter()
        .filter(|line| line["scope"].as_str() == Some(scope))
        .filter_map(|line| line["submission"].as_str().map(str::to_string))
        .filter(|id| {
            read_json(&record_dir(root, id).join("status.json"))
                .and_then(|status| status["status"].as_str().map(str::to_string))
                .is_some_and(|state| OPEN.contains(&state.as_str()))
        })
        .count()
}

/// Write the record directory: manifest, status, sealed receipt, and
/// each attachment's decoded bytes beside it.
fn persist(
    dir: &Path,
    manifest: &Value,
    status: &Value,
    receipt: &FeedbackReceipt,
    envelope: &Value,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|trouble| trouble.to_string())?;
    if let Some(attachments) = envelope.get("attachments").and_then(Value::as_array) {
        let attachments_dir = dir.join("attachments");
        fs::create_dir_all(&attachments_dir).map_err(|trouble| trouble.to_string())?;
        for (index, attachment) in attachments.iter().enumerate() {
            let bytes = decode_base64(attachment["content_base64"].as_str().unwrap_or_default())
                .unwrap_or_default();
            fs::write(attachments_dir.join(index.to_string()), &bytes)
                .map_err(|trouble| trouble.to_string())?;
        }
    }
    write_json(&dir.join("manifest.json"), manifest)?;
    write_json(&dir.join("status.json"), status)?;
    fs::write(dir.join("receipt.json"), receipt.to_json()).map_err(|trouble| trouble.to_string())
}

/// Authenticate, then load the record only if the caller's credential
/// owns it — another credential's submission is not disclosed.
fn load_owned(state: &ServeState, headers: &HeaderMap, id: &str) -> Option<(PathBuf, Value)> {
    let (_registry, caller) = authenticate(state, headers).ok()?;
    let dir = record_dir(&state.dir, id);
    let manifest = read_json(&dir.join("manifest.json"))?;
    let status = read_json(&dir.join("status.json"))?;
    if manifest["key"].as_str().unwrap_or_default() != caller.key
        || manifest["tenant"].as_str() != caller.tenant.as_deref()
    {
        return None;
    }
    Some((dir, status))
}

/// The status document a caller sees: the stored record plus its
/// triage journal.
fn view(dir: &Path, status: &Value) -> Value {
    let mut view = status.clone();
    view["transitions"] = json!(read_lines(&dir.join("transitions.jsonl")));
    view
}

// ---------- small shared shapes ----------

fn fb_error(status: StatusCode, code: &str, message: &str, submission: Option<&str>) -> Response {
    let mut body = json!({
        "v": SCHEMA,
        "error": {"code": code, "message": message},
    });
    if let Some(submission) = submission {
        body["submission"] = json!(submission);
    }
    (status, Json(body)).into_response()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(value).map_err(|t| t.to_string())?,
    )
    .map_err(|trouble| trouble.to_string())?;
    fs::rename(&tmp, path).map_err(|trouble| trouble.to_string())
}

fn append_line(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{value}");
        let _ = file.sync_data();
    }
}

fn read_lines(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> Value {
        json!({
            "v": SCHEMA,
            "observation": "POST /v1/systemone answered `pending` for a closed door.",
            "expected": "a typed refusal",
            "actual": "an answer",
            "environment": {"version": "oak 0.1.0", "os": "macOS"},
            "reproduction": "oak ask --door kev-latest ...",
            "consent": {"forward": false},
        })
    }

    #[test]
    fn a_well_formed_submission_validates() {
        validate(&envelope()).unwrap();
    }

    #[test]
    fn a_submission_needs_the_schema_and_an_observation() {
        let mut wrong = envelope();
        wrong["v"] = json!("openagents.feedback.v0");
        assert!(validate(&wrong).is_err());
        wrong["v"] = json!(SCHEMA);
        wrong["observation"] = json!("   ");
        assert!(validate(&wrong).is_err());
    }

    #[test]
    fn oversized_fields_refuse() {
        let mut too_long = envelope();
        too_long["observation"] = json!("x".repeat(OBSERVATION_MAX + 1));
        assert!(validate(&too_long).is_err());
    }

    #[test]
    fn attachments_are_bounded_and_decoded() {
        let mut good = envelope();
        good["attachments"] = json!([
            {"name": "trace.txt", "media_type": "text/plain", "content_base64": "aGVsbG8="},
        ]);
        validate(&good).unwrap();
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");

        let mut too_many = envelope();
        too_many["attachments"] = json!([
            {"name": "a", "content_base64": "AA=="},
            {"name": "b", "content_base64": "AA=="},
            {"name": "c", "content_base64": "AA=="},
            {"name": "d", "content_base64": "AA=="},
            {"name": "e", "content_base64": "AA=="},
        ]);
        assert!(validate(&too_many).is_err());

        let mut bad = envelope();
        bad["attachments"] = json!([{"name": "x", "content_base64": "!!not-base64!!"}]);
        assert!(validate(&bad).is_err());
    }

    #[test]
    fn a_redaction_keeps_digests_and_drops_content() {
        let dir = tempfile::tempdir().unwrap();
        let record = record_dir(dir.path(), "fb_test");
        fs::create_dir_all(&record).unwrap();
        let manifest = json!({
            "v": SCHEMA,
            "submission": "fb_test",
            "key": "key-1",
            "tenant": "acme",
            "envelope": envelope(),
        });
        write_json(&record.join("manifest.json"), &manifest).unwrap();
        fs::create_dir_all(record.join("attachments")).unwrap();
        fs::write(record.join("attachments").join("0"), b"hello").unwrap();
        redact(&record).unwrap();
        let manifest = read_json(&record.join("manifest.json")).unwrap();
        assert_eq!(manifest["envelope"]["redacted"], true);
        assert!(manifest["envelope"]["digests"]["observation"].is_string());
        assert!(!record.join("attachments").exists());
    }

    #[test]
    fn a_transition_appends_and_restates_the_status() {
        let dir = tempfile::tempdir().unwrap();
        let record = record_dir(dir.path(), "fb_test");
        fs::create_dir_all(&record).unwrap();
        let status = json!({"v": SCHEMA, "submission": "fb_test", "status": "submitted"});
        write_json(&record.join("status.json"), &status).unwrap();
        let updated = transition(dir.path(), "fb_test", "accept", Some("triaged")).unwrap();
        assert_eq!(updated["status"], "accepted");
        let journal = read_lines(&record.join("transitions.jsonl"));
        assert_eq!(journal.len(), 1);
        assert_eq!(journal[0]["status"], "accepted");
        assert!(transition(dir.path(), "fb_test", "teleport", None).is_err());
        assert!(transition(dir.path(), "fb_missing", "accept", None).is_err());
    }
}
