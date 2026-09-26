//! Finite cuts of local evidence, attributed as projections, never signed RUN.
use crate::host::number;
use crate::*;
use nostr::control;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Snapshot {
    grant: Value,
    scope: Value,
    policy: Value,
    view: String,
    captured_at: u64,
    items: Vec<Value>,
    /// Opaque tokens are retained with their precise finite-cut offsets.
    cursors: BTreeMap<String, usize>,
}

pub(crate) fn read(
    host: &mut Host,
    request: &Value,
    input_ref: &Value,
    principal: &str,
    now: u64,
) -> Result<(Value, Vec<Event>)> {
    let max_items = number(request, "max_items")? as usize;
    let max_bytes = number(request, "max_bytes")? as usize;
    if max_items > 128 || !(2048..=32768).contains(&max_bytes) {
        return Err("read bounds exceed the installed disclosure profile".into());
    }
    let (id, offset) = if let Some(cursor) = request["after"].as_str() {
        host.document
            .snapshots
            .iter()
            .find_map(|(id, snapshot)| {
                snapshot
                    .cursors
                    .get(cursor)
                    .map(|offset| (id.clone(), *offset))
            })
            .ok_or("read cursor is unavailable")?
    } else {
        if host.document.snapshots.len() >= 32 {
            return Err("retained control snapshot limit reached".into());
        }
        let id = client::random_id();
        let snapshot = capture(host, request, now)?;
        host.document.snapshots.insert(id.clone(), snapshot);
        host.save()?;
        (id, 0)
    };
    let snapshot = host
        .document
        .snapshots
        .get(&id)
        .ok_or("read snapshot missing")?
        .clone();
    if snapshot.grant != request["grant"]
        || snapshot.scope != request["scope"]
        || snapshot.policy != host.setup().policy
        || snapshot.view != request["view"]
    {
        return Err("read cursor belongs to another grant, scope, policy, or view".into());
    }
    let mut answer = json!({"v":control::VIEW,"requires":[],"request":input_ref,"authority":host.setup().authority,"scope":host.setup().scope,"captured_at":snapshot.captured_at,"policy":snapshot.policy,"items":[],"next":null,"coverage":"partial"});
    let mut items = vec![];
    let mut authorized_events = vec![];
    let mut payload_bytes = 0;
    for item in snapshot.items.iter().skip(offset).take(max_items) {
        let mut closure = vec![];
        for r in [&item["artifact"], &item["provenance"]] {
            let value = host.document.blobs.json(r)?;
            if r == &item["artifact"] {
                closure.push(host.document.blobs.json(&value["content"])?);
            }
            closure.push(value);
        }
        let events = closure
            .iter()
            .map(|value| host.sign_artifact(value, principal, now))
            .collect::<Result<Vec<_>>>()?;
        let additional = events
            .iter()
            .map(|event| serde_json::to_vec(event).map_or(usize::MAX, |v| v.len()))
            .sum::<usize>();
        let mut proposed = items.clone();
        proposed.push(item.clone());
        answer["items"] = json!(proposed);
        // Count encrypted event bytes too. The conservative framing allowance
        // includes all CJ artifact references, its result, and a cursor.
        answer["next"] = json!("f".repeat(64));
        let view_event = host.sign_artifact(&answer, principal, now)?;
        let size = serde_json::to_vec(&view_event)
            .map_err(|e| e.to_string())?
            .len()
            + payload_bytes
            + additional
            + (authorized_events.len() + events.len() + 1) * 512
            + 4096;
        if size > max_bytes {
            break;
        }
        payload_bytes += additional;
        items = proposed;
        authorized_events.extend(events);
    }
    if items.is_empty() && offset < snapshot.items.len() {
        return Err("one projected item exceeds the requested page bound".into());
    }
    let end = offset + items.len();
    answer["items"] = json!(items);
    answer["next"] = Value::Null;
    if end < snapshot.items.len() {
        let cursor = client::random_id();
        host.document
            .snapshots
            .get_mut(&id)
            .ok_or("snapshot vanished")?
            .cursors
            .insert(cursor.clone(), end);
        answer["next"] = json!(cursor);
    }
    host.save()?;
    Ok((answer, authorized_events))
}
fn capture(host: &mut Host, request: &Value, now: u64) -> Result<Snapshot> {
    let setup = host.setup().clone();
    let first = coder::task::view::read(&setup.task_directory, &setup.task_id, None, 200)
        .map_err(|e| e.to_string())?;
    if first.task.intent_digest != setup.intent_digest {
        return Err("task source differs from the admitted scope".into());
    }
    let original = serde_json::to_value(&first.task).map_err(|e| e.to_string())?;
    let state = json!({"v":"openagents.control-state-display.v1","requires":[],"task":setup.scope["task"],"title":first.task.intent.title,"prompt":first.task.effective_prompt(),"revision":first.task.revision,"status":first.task.status,"execution":first.task.execution,"checks":first.task.checks,"verification":first.verification,"integration":first.integration,"cost_usd":first.cost_usd,"cost_status":first.cost_status,"evidence_state":first.evidence.state});
    let mut items = vec![project(host, &state, &original, "redacted", now)?];
    if request["view"] == "history" {
        let initial_total = first.evidence.total_steps;
        let mut steps = first.evidence.steps;
        let mut cursor = first.evidence.next;
        while steps.len() < initial_total.min(1024) {
            let Some(next) = cursor else {
                break;
            };
            let page =
                coder::task::view::read(&setup.task_directory, &setup.task_id, Some(&next), 200)
                    .map_err(|e| e.to_string())?;
            cursor = page.evidence.next;
            if page.evidence.steps.is_empty() {
                break;
            }
            steps.extend(page.evidence.steps);
        }
        steps.truncate(initial_total.min(1024));
        let mut retained = 0;
        for (index, step) in steps.iter().enumerate() {
            let bytes = jcs(step).map_err(|e| e.to_string())?;
            if retained + bytes.len() > 2 * 1024 * 1024 {
                break;
            }
            retained += bytes.len();
            let display = if bytes.len() > 8192 {
                json!({"v":"openagents.control-trace-display.v1","requires":[],"index":index,"step":null,"omitted_bytes":bytes.len(),"source_digest":nostr::contracts::digest_bytes(&bytes)})
            } else {
                json!({"v":"openagents.control-trace-display.v1","requires":[],"index":index,"step":step,"omitted_bytes":0,"source_digest":nostr::contracts::digest_bytes(&bytes)})
            };
            items.push(project(
                host,
                &display,
                step,
                if bytes.len() > 8192 {
                    "bounded"
                } else {
                    "unverifiable"
                },
                now,
            )?);
        }
    }
    Ok(Snapshot {
        grant: request["grant"].clone(),
        scope: setup.scope,
        policy: setup.policy,
        view: request["view"].as_str().ok_or("view")?.into(),
        captured_at: now,
        items,
        cursors: BTreeMap::new(),
    })
}
fn project(
    host: &mut Host,
    display: &Value,
    source: &Value,
    reason: &str,
    now: u64,
) -> Result<Value> {
    let source_bytes = jcs(source).map_err(|e| e.to_string())?;
    let source_ref = host.document.blobs.insert(
        source_bytes,
        "application/json",
        "openagents.control-local-source.v1",
    )?;
    let display_bytes = jcs(display).map_err(|e| e.to_string())?;
    let bounded = if display_bytes.len() > 8192 {
        Some(
            json!({"v":display["v"],"requires":[],"omitted_bytes":display_bytes.len(),"source_digest":nostr::contracts::digest_bytes(&display_bytes),"message":"The exact local content exceeds this display bound."}),
        )
    } else {
        None
    };
    let reason = if bounded.is_some() { "bounded" } else { reason };
    let display = bounded.as_ref().unwrap_or(display);
    let content = host
        .document
        .blobs
        .insert_json(display, display["v"].as_str().ok_or("display schema")?)?;
    let projection = json!({"v":control::PROJECTION,"requires":[],"content":content,"sources":[source_ref],"reason":reason,"coverage":"partial"});
    let projection_ref = host
        .document
        .blobs
        .insert_json(&projection, control::PROJECTION)?;
    let mut evidence = json!({"v":"openagents.evidence.v1","requires":[],"content":projection_ref,"source":{"kind":"document","identity":format!("control-projection:{}",host.setup().scope["task"].as_str().ok_or("scope")?),"version":now.to_string()},"capture":{"complete":false,"omitted_bytes":null,"reason":"Current-authority projection of local task or ATIF bytes; the source is not an original signed RUN record."},"derived_from":[],"scope":{"task":host.setup().scope["task"],"recipients":[],"classification":"private"}});
    evidence["id"] = json!(nostr::contracts::digest_bytes(
        &jcs(&evidence).map_err(|e| e.to_string())?
    ));
    nostr::contracts::parse_evidence(&evidence).map_err(|e| e.to_string())?;
    let provenance = host
        .document
        .blobs
        .insert_json(&evidence, "openagents.evidence.v1")?;
    Ok(json!({"kind":"projection","artifact":projection_ref,"provenance":provenance}))
}
