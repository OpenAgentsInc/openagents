//! Device-only adapter. The existing host store admits every operation.
//! Protobuf is the native boundary; original signed Nostr bytes retain authority.
use coder_access::{
    Client, RelayPolicy,
    client::Pending,
    protocol::{Access, Operation, Outcome},
};
use coder_reach::{
    channel,
    hints::{Class, Transport},
    presence::Freshness,
};
use engine_proto::{
    AgentEvent, AgentProgress, ToolActivity, common::v1 as common, host::v1 as wire,
    reach::v1 as reach,
};
use prost::Message;
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};
use std::{
    ffi::{CStr, c_char},
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
pub mod refs;
const LIMIT: usize = 1_048_576;
#[derive(Debug)]
pub struct Failure(pub common::Refusal);
impl From<&str> for Failure {
    fn from(s: &str) -> Self {
        Self(common::Refusal {
            domain: "native".into(),
            code: "malformed".into(),
            message: s.into(),
            ..Default::default()
        })
    }
}
impl From<String> for Failure {
    fn from(s: String) -> Self {
        Self(common::Refusal {
            domain: "transport".into(),
            code: "unavailable".into(),
            message: s,
            retryable: true,
            ..Default::default()
        })
    }
}
impl Failure {
    fn host(e: coder_access::Error) -> Self {
        Self(common::Refusal {
            domain: "host".into(),
            code: serde_json::to_value(e.code)
                .unwrap()
                .as_str()
                .unwrap()
                .into(),
            missing_right: e.missing.map(|r| r.as_str().into()),
            message: e.message,
            retryable: matches!(
                e.code,
                coder_access::Code::Unavailable
                    | coder_access::Code::Transport
                    | coder_access::Code::RateLimited
            ),
            ..Default::default()
        })
    }
}
type Result<T> = std::result::Result<T, Failure>;
pub type NetworkIo = unsafe extern "C" fn(u64, *const u8, usize, *mut u8, usize) -> i64;
struct Stream {
    id: u64,
    io: NetworkIo,
}
impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let target = buf.initialize_unfilled();
        let n = unsafe {
            (self.io)(
                self.id,
                std::ptr::null(),
                0,
                target.as_mut_ptr(),
                target.len(),
            )
        };
        if n < 0 || n as usize > target.len() {
            return Poll::Ready(Err(io::ErrorKind::ConnectionAborted.into()));
        };
        buf.advance(n as usize);
        Poll::Ready(Ok(()))
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let n = unsafe { (self.io)(self.id, buf.as_ptr(), buf.len(), std::ptr::null_mut(), 0) };
        Poll::Ready(if n < 0 || n as usize > buf.len() {
            Err(io::ErrorKind::ConnectionAborted.into())
        } else {
            Ok(n as usize)
        })
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
fn now() -> Result<u64> {
    coder_access::unix_time().map_err(Failure::host)
}
fn project_access(access: &Access) -> Result<wire::Access> {
    if access.grant.scope.is_some() {
        return Err("Scoped grants are not supported by this device adapter".into());
    }
    let g = &access.grant;
    Ok(wire::Access {
        grant: Some(wire::Grant {
            id: g.grant.clone(),
            host: g.host.clone(),
            owner: g.owner.clone(),
            device: g.device.clone(),
            relay: g.relay.clone(),
            rights: Vec::<coder_access::Right>::from(g.rights.clone())
                .iter()
                .map(|r| r.as_str().into())
                .collect(),
            epoch: g.epoch,
            origin: Some(wire::GrantOrigin {
                kind: match g.origin.kind {
                    coder_access::protocol::OriginKind::Invitation => "invitation",
                    coder_access::protocol::OriginKind::Approval => "approval",
                }
                .into(),
                id: g.origin.id.clone(),
                issuer: g.origin.issuer.clone(),
            }),
            issued_at: g.issued_at,
            expires_at: g.expires_at,
        }),
        authorization: Some(common::SignedEvent {
            json: serde_json::to_string(&access.authorization)
                .map_err(|_| "Grant encoding failed")?,
        }),
    })
}
fn access(q: &wire::AgentRequest, key: &SecretKey) -> Result<Access> {
    let a = q.access.as_ref().ok_or("Pair this computer first")?;
    let g = a.grant.as_ref().ok_or("Missing host grant")?;
    let original = a.authorization.as_ref().ok_or("Missing signed grant")?;
    let event = serde_json::from_value(
        nostr::contracts::parse_strict(original.json.as_bytes())
            .map_err(|_| "Invalid grant JSON")?,
    )
    .map_err(|_| "Invalid signed grant")?;
    let access = Access::from_authorization(event, key, &g.host, now()?, RelayPolicy::Production)
        .map_err(|e| e.to_string())?;
    if project_access(&access)? != *a {
        return Err("Grant projection differs from its signed original".into());
    };
    Ok(access)
}
fn project_reach(r: &coder_host::client::Reach) -> reach::ReachState {
    let p = &r.presence.presence;
    reach::ReachState {
        presence: Some(reach::Presence {
            host: p.host.clone(),
            owner: p.owner.clone(),
            generation: p.generation,
            protocol: p.protocol,
            compatibility: Some(reach::VersionRange {
                min: p.compatibility.min,
                max: p.compatibility.max,
            }),
            capabilities: p.capabilities.clone(),
            observed_at: p.observed_at,
            telemetry: p.telemetry.map(|t| reach::Telemetry {
                cpu_count: t.cpu_count,
                cpu_utilization_pct: t.cpu_utilization_pct.into(),
                memory_available_pct: t.memory_available_pct.into(),
            }),
        }),
        hints: r
            .hints
            .hints
            .iter()
            .map(|h| reach::Hint {
                class_name: serde_json::to_value(h.class)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .into(),
                transport: serde_json::to_value(h.transport)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .into(),
                address: h.address.clone(),
                status: serde_json::to_value(h.status)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .into(),
                observed_at: h.observed_at,
            })
            .collect(),
        issued_at: r.hints.issued_at,
        expires_at: r.hints.expires_at,
        received_at: r.presence.received_at,
    }
}
fn task_id(host: &str, run: &str) -> Result<String> {
    if run.len() < 8
        || run.len() > 128
        || !run
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("Invalid phone task identity".into());
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(format!("openagents.phone-task.v1\0{host}\0{run}"))
    ))
}
fn native_cursor(c: wire::TaskCursor) -> Result<coder_access::task_read::Cursor> {
    let s = c.scope.ok_or("Missing cursor scope")?;
    Ok(coder_access::task_read::Cursor {
        scope: coder_access::task_read::Scope {
            workspace: s.workspace,
            task: s.task,
            revision: s.revision,
            attempt: s.attempt,
            intent_digest: s.intent_digest,
        },
        source: c.source,
        source_digest: c.source_digest,
        source_bytes: c.source_bytes,
        next_step: c.next_step,
        prefix_digest: c.prefix_digest,
    })
}
fn project_cursor(c: coder_access::task_read::Cursor) -> wire::TaskCursor {
    wire::TaskCursor {
        scope: Some(wire::TaskScope {
            workspace: c.scope.workspace,
            task: c.scope.task,
            revision: c.scope.revision,
            attempt: c.scope.attempt,
            intent_digest: c.scope.intent_digest,
        }),
        source: c.source,
        source_digest: c.source_digest,
        source_bytes: c.source_bytes,
        next_step: c.next_step,
        prefix_digest: c.prefix_digest,
    }
}
fn operation(q: &wire::AgentRequest, host: &str) -> Result<Operation> {
    let expected = task_id(host, &q.phone_run)?;
    if !q.remote_task.is_empty() && q.remote_task != expected {
        return Err("Task binding differs from this phone run".into());
    }
    match q.operation.as_str() {
        "start" => {
            if q.objective.is_empty() || q.objective.len() > 4000 {
                return Err("Task objective exceeds its bound".into());
            };
            let mut task = coder_access::client::tasks::input(&q.objective, &q.workspace);
            task.engine = Some(nostr::cj_conversation::Engine::Codex);
            task.read_only = (!q.allow_writes).then_some(true);
            Ok(Operation::CreateTask { task })
        }
        "read" => Ok(Operation::ReadTask {
            query: coder_access::task_read::PageQuery {
                workspace: q.workspace.clone(),
                task: expected,
                revision: None,
                cursor: q.cursor.clone().map(native_cursor).transpose()?,
                limit: 16,
            },
        }),
        "cancel" => Ok(Operation::CancelTask {
            task: expected,
            revision: q.revision,
            reason: "Stopped from OpenAgents phone".into(),
        }),
        _ => Err("Unsupported phone host operation".into()),
    }
}
fn base() -> wire::AgentReply {
    wire::AgentReply {
        schema: 1,
        ..Default::default()
    }
}
async fn reach(device: &coder_host::client::Device) -> Result<coder_host::client::Reach> {
    let r = coder_host::client::fetch_reach(device, device.relay())
        .await
        .map_err(|e| e.to_string())?;
    r.presence
        .judge(now()?, Freshness::default())
        .map_err(|e| e.to_string())?;
    Ok(r)
}
async fn direct(
    client: &Client,
    access: &Access,
    key: &SecretKey,
    pending: &Pending,
    r: &coder_host::client::Reach,
    stream: Stream,
) -> Result<Outcome> {
    let config = channel::ClientConfig {
        device: *key,
        host: access.grant.host.clone(),
        grant: access.grant.grant.clone(),
        epoch: access.grant.epoch,
        generation: r.presence.presence.generation,
        timeout: Duration::from_secs(10),
    };
    let mut channel = channel::connect(stream, &config, now()?)
        .await
        .map_err(|e| e.to_string())?;
    for fragment in
        coder_host_wire::fragments(&coder_host_wire::ToHost::Call(pending.event.clone()).encode())
            .map_err(|_| "Host message exceeds its bound")?
    {
        channel.send(&fragment).await.map_err(|e| e.to_string())?;
    }
    let mut assembler = coder_host_wire::Assembler::default();
    for _ in 0..128 {
        let Some(frame) = channel.recv().await.map_err(|e| e.to_string())? else {
            return Err("Host closed the channel".into());
        };
        if let Some(message) = assembler.push(&frame).map_err(|_| "Malformed host frame")? {
            match coder_host_wire::ToDevice::decode(&message)
                .map_err(|_| "Malformed host message")?
            {
                coder_host_wire::ToDevice::Answer(event) => {
                    return client
                        .verify_reply(pending, &event, now()?)
                        .map_err(Failure::host);
                }
                coder_host_wire::ToDevice::Closing(_) => {
                    return Err("Host revoked or closed this channel".into());
                }
                coder_host_wire::ToDevice::Renewal(_) | coder_host_wire::ToDevice::Pong(_) => {}
                _ => return Err("Unexpected host message".into()),
            }
        }
    }
    Err("Host response frame limit".into())
}
fn cut(value: &str, n: usize) -> String {
    let mut end = value.len().min(n);
    while !value.is_char_boundary(end) {
        end -= 1
    }
    value[..end].into()
}
fn events(page: &coder_access::task_read::Page, after: u64) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    for item in &page.evidence.steps {
        match item {
            coder_access::task_read::Step::Gap { index, .. } => {
                let seq = index * 128 + 1;
                if seq > after {
                    out.push(AgentEvent {
                        seq,
                        kind: "status".into(),
                        text: "Original evidence exceeds the inline limit".into(),
                        ..Default::default()
                    })
                }
            }
            coder_access::task_read::Step::Original { index, step } => {
                let seq = index * 128 + 1;
                let text = step["message"].as_str().unwrap_or("");
                if seq > after && !text.is_empty() {
                    out.push(AgentEvent {
                        seq,
                        kind: if step["source"] == "user" {
                            "user"
                        } else if step["source"] == "agent" {
                            "agent_message"
                        } else {
                            "status"
                        }
                        .into(),
                        text: cut(text, 4096),
                        speaker: if step["source"] == "agent" {
                            step["model_name"].as_str().unwrap_or("Agent")
                        } else {
                            ""
                        }
                        .into(),
                        ..Default::default()
                    })
                }
                if let Some(calls) = step["tool_calls"].as_array() {
                    for (i, call) in calls.iter().take(126).enumerate() {
                        let seq = seq + 1 + i as u64;
                        if seq <= after {
                            continue;
                        };
                        let id = call["tool_call_id"].as_str().unwrap_or("");
                        if id.is_empty() {
                            continue;
                        }
                        let result = step["observation"]["results"].as_array().and_then(|v| {
                            v.iter().find(|r| r["source_call_id"].as_str() == Some(id))
                        });
                        out.push(AgentEvent {
                            seq,
                            kind: "tool".into(),
                            tool: Some(ToolActivity {
                                id: cut(id, 128),
                                kind: "command".into(),
                                name: cut(call["function_name"].as_str().unwrap_or("Tool"), 80),
                                state: match result.and_then(|r| r["extra"]["status"].as_str()) {
                                    Some("completed") => "succeeded",
                                    Some("failed") => "failed",
                                    Some("cancelled") => "cancelled",
                                    _ => "running",
                                }
                                .into(),
                                input: cut(&call["arguments"].to_string(), 4096),
                                output: result
                                    .map(|r| cut(r["content"].as_str().unwrap_or(""), 8192))
                                    .unwrap_or_default(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
    out
}
fn progress(outcome: Outcome, p: &wire::PreparedCall) -> Result<wire::AgentReply> {
    let mut reply = base();
    reply.remote_task = p.remote_task.clone();
    let mut progress = AgentProgress {
        v: 1,
        run: p.phone_run.clone(),
        state: "running".into(),
        seq: p.after,
        ..Default::default()
    };
    match outcome {
        Outcome::Dispatched { receipt } => {
            reply.host_phase = if p.operation == "cancel" {
                "stopping"
            } else {
                "queued"
            }
            .into();
            if receipt.reference != p.remote_task {
                return Err("Host returned another task identity".into());
            };
        }
        Outcome::Task { task: page } => {
            if page.scope.task != p.remote_task || page.scope.workspace != p.workspace {
                return Err("Host returned another task scope".into());
            };
            reply.revision = page.scope.revision;
            let mut collected = events(&page, p.after);
            reply.host_phase = serde_json::to_value(&page).unwrap()["phase"]
                .as_str()
                .unwrap_or("unknown")
                .into();
            reply.cursor = if collected.len() <= 64 {
                page.next.clone().map(project_cursor)
            } else {
                p.cursor.clone()
            };
            progress.more = page.more_available || collected.len() > 64;
            collected.truncate(64);
            progress.seq = collected.last().map(|e| e.seq).unwrap_or(p.after);
            progress.events = collected;
            if !progress.more {
                use nostr::activity_summary::Phase;
                progress.state = match page.phase {
                    Phase::Completed => {
                        if page.execution == "finished" {
                            "succeeded"
                        } else {
                            "failed"
                        }
                    }
                    Phase::Failed => "failed",
                    Phase::Cancelled => "cancelled",
                    _ => "running",
                }
                .into();
            }
            if progress.state != "running" {
                progress.exit = Some(if progress.state == "succeeded" { 0 } else { -1 });
                progress.result = page
                    .evidence
                    .steps
                    .iter()
                    .rev()
                    .find_map(|step| match step {
                        coder_access::task_read::Step::Original { step, .. }
                            if step["source"] == "agent" && step.get("tool_calls").is_none() =>
                        {
                            step["message"].as_str()
                        }
                        _ => None,
                    })
                    .map(|s| cut(s, 32768))
                    .unwrap_or_else(|| {
                        format!(
                            "Host task {} · execution {}",
                            progress.state, page.execution
                        )
                    });
            }
        }
        _ => return Err("Host answered a different operation".into()),
    }
    reply.progress = Some(progress);
    Ok(reply)
}
pub async fn call(
    q: wire::AgentRequest,
    key: SecretKey,
    io: Option<(u64, NetworkIo)>,
) -> Result<wire::AgentReply> {
    if q.schema != 1 {
        return Err("Unsupported phone host schema".into());
    }
    if q.operation == "pair" {
        let a = coder_access::client::redeem(&q.invitation, &key, RelayPolicy::Production)
            .await
            .map_err(Failure::host)?;
        let mut r = base();
        r.access = Some(project_access(&a)?);
        return Ok(r);
    }
    let a = access(&q, &key)?;
    let client = Client::device(a.clone(), key, RelayPolicy::Production).map_err(Failure::host)?;
    let device = coder_host::client::Device::new(a.clone(), key, RelayPolicy::Production)
        .map_err(|e| e.to_string())?;
    let r = reach(&device).await?;
    if q.operation == "validate" {
        let mut reply = base();
        reply.access = Some(project_access(&a)?);
        return Ok(reply);
    }
    if q.operation == "reach" {
        let mut reply = base();
        reply.reach = Some(project_reach(&r));
        return Ok(reply);
    }
    if q.operation != "exchange" {
        let op = operation(&q, &a.grant.host)?;
        if q.operation == "start"
            && !q.allow_writes
            && !r
                .presence
                .presence
                .capabilities
                .iter()
                .any(|c| c == coder_access::protocol::TASK_READ_ONLY)
        {
            return Err("Update this host to support read-only tasks".into());
        }
        let remote = task_id(&a.grant.host, &q.phone_run)?;
        let id = if q.operation == "start" {
            remote.clone()
        } else {
            coder_access::protocol::random_id()
        };
        let pending = client
            .prepare_with_id(op, now()?, id)
            .map_err(Failure::host)?;
        let mut reply = base();
        reply.reach = Some(project_reach(&r));
        reply.remote_task = remote.clone();
        reply.pending = Some(wire::PreparedCall {
            pending_json: serde_json::to_string(&pending).map_err(|_| "Request encoding failed")?,
            operation: q.operation,
            phone_run: q.phone_run,
            workspace: q.workspace,
            remote_task: remote,
            after: q.after,
            cursor: q.cursor,
        });
        return Ok(reply);
    }
    let p = q.pending.ok_or("Missing prepared host request")?;
    let value = nostr::contracts::parse_strict_bounded(p.pending_json.as_bytes(), 524288)
        .map_err(|_| "Invalid saved host request")?;
    let pending: Pending =
        serde_json::from_value(value).map_err(|_| "Invalid saved host request")?;
    client
        .validate_pending(&pending, now()?)
        .map_err(Failure::host)?;
    if task_id(&a.grant.host, &p.phone_run)? != p.remote_task {
        return Err("Saved task binding differs".into());
    }
    match (&*p.operation, &pending.request.op) {
        ("start", Operation::CreateTask { task })
            if task.workspace == p.workspace && pending.request.request == p.remote_task => {}
        ("read", Operation::ReadTask { query })
            if query.workspace == p.workspace && query.task == p.remote_task => {}
        ("cancel", Operation::CancelTask { task, .. }) if task == &p.remote_task => {}
        _ => return Err("Saved request differs from its task binding".into()),
    }
    let outcome = if let Some((id, callback)) = io {
        if !r.hints.hints.iter().any(|h| {
            h.class == Class::Tailnet
                && h.transport == Transport::Tcp
                && h.address == q.stream_address
        }) {
            return Err("Tailnet endpoint is not in authenticated host hints".into());
        }
        direct(&client, &a, &key, &pending, &r, Stream { id, io: callback }).await?
    } else {
        client.send(&pending).await.map_err(Failure::host)?
    };
    let mut reply = progress(outcome, &p)?;
    reply.route = if io.is_some() { "tailnet" } else { "relay" }.into();
    Ok(reply)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_phone_host_call(
    input: *const u8,
    size: usize,
    secret: *const c_char,
    stream: u64,
    callback: Option<NetworkIo>,
    length: *mut usize,
) -> *mut u8 {
    if length.is_null() {
        return std::ptr::null_mut();
    };
    unsafe { *length = 0 };
    let result = std::panic::catch_unwind(|| -> Result<wire::AgentReply> {
        if input.is_null() || secret.is_null() || size > LIMIT {
            return Err("Invalid native request".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(input, size) };
        engine_proto::wire::validate("openagents.host.v1.AgentRequest", bytes)
            .map_err(str::to_owned)?;
        let key = unsafe { CStr::from_ptr(secret) }
            .to_str()
            .map_err(|_| "Invalid device key")?
            .parse::<SecretKey>()
            .map_err(|_| "Invalid device key")?;
        let q = wire::AgentRequest::decode(bytes).map_err(|_| "Invalid protobuf request")?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Native runtime unavailable")?
            .block_on(call(q, key, callback.map(|f| (stream, f))))
    });
    let reply = match result {
        Ok(Ok(reply)) => reply,
        other => {
            let mut reply = base();
            reply.error = Some(match other {
                Ok(Err(failure)) => failure.0,
                _ => Failure::from("Native host operation failed").0,
            });
            reply
        }
    };
    let bytes = reply.encode_to_vec().into_boxed_slice();
    unsafe { *length = bytes.len() };
    Box::into_raw(bytes).cast::<u8>()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_phone_host_free(bytes: *mut u8, size: usize) {
    if !bytes.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                bytes, size,
            )))
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn openagents_phone_host_key() -> *mut c_char {
    std::ffi::CString::new(
        SecretKey::new(&mut secp256k1::rand::rng())
            .display_secret()
            .to_string(),
    )
    .unwrap()
    .into_raw()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_phone_host_key_free(key: *mut c_char) {
    if !key.is_null() {
        unsafe { drop(std::ffi::CString::from_raw(key)) }
    }
}
