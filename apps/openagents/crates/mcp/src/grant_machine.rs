//! OpenAgents #12163: `grant_machine` and `revoke_machine`, a coordinator
//! granting machine scopes to other chats under the owner's `delegate`.
//!
//! The owner grants `delegate` once (`openagents lease grant machine CHAT
//! --scope delegate`). This server acts for its own chat: it checks the
//! engine's proof of that chat ID (#12067) and the `delegate` grant on this
//! computer, then sends the subgrant to the target chat's own host through
//! the device channel (`GrantMachine` / `RevokeMachine` with
//! `targetDeviceId`), where `coder_lease::machine::delegate` writes and logs
//! it. A remote send is logged here too, so `openagents lease actions` on
//! both computers shows it. A host whose app predates these methods is
//! named with its version, never skipped silently.

use std::path::Path;

use coder_lease::machine::{self, Entry, Outcome, Scope, delegate, proof};
use openagents_rpc::methods;
use serde_json::{Value, json};

use crate::openagents::OpenAgents;
use crate::tools::ToolDef;

pub const GRANT: &str = "grant_machine";
pub const REVOKE: &str = "revoke_machine";

pub fn tools() -> [ToolDef; 2] {
    [
        ToolDef {
            name: GRANT,
            description: "Grant another chat machine scopes on its own computer, under the owner's delegate grant to you: build.prune, process.stop, host.upgrade (never delegate, money, keys or secrets). The grant reaches the chat's host on any of the owner's computers. Needs a reason; every grant is logged with you, the chat, the scopes and the reason. Deploys are never stopped and kept paths are never pruned.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "chat": {"type": "string", "description": "Chat id, unique id prefix, or exact title."},
                    "scopes": {
                        "type": "array", "minItems": 1,
                        "items": {"type": "string", "enum": ["build.prune", "process.stop", "host.upgrade"]}
                    },
                    "for_minutes": {"type": "integer", "minimum": 1, "description": "How long it lasts; until revoked when omitted."},
                    "reason": {"type": "string", "description": "Why, in one line. It goes in the log."}
                },
                "required": ["chat", "scopes", "reason"]
            }),
        },
        ToolDef {
            name: REVOKE,
            description: "End the machine grant you gave a chat with grant_machine, on its own computer. Logged with the reason.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "chat": {"type": "string", "description": "Chat id, unique id prefix, or exact title."},
                    "reason": {"type": "string", "description": "Why, in one line. It goes in the log."}
                },
                "required": ["chat", "reason"]
            }),
        },
    ]
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    chat: String,
    #[serde(default)]
    scopes: Vec<String>,
    for_minutes: Option<u64>,
    reason: String,
}

/// Run `name` for the chat `chat_id`, under this computer's lease root.
pub async fn call(
    openagents: &OpenAgents,
    chat_id: Option<&str>,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    let (root, _) = machine::here()?;
    let chat_proof = std::env::var(proof::PROOF_VAR).ok();
    call_at(
        &root,
        openagents,
        chat_id,
        chat_proof.as_deref(),
        name,
        args,
    )
    .await
}

async fn call_at(
    root: &Path,
    openagents: &OpenAgents,
    chat_id: Option<&str>,
    chat_proof: Option<&str>,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    let args: Args = serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))?;
    let issuer = chat_id.ok_or(
        "machine grants are given only from an app chat that holds the owner's delegate grant",
    )?;
    let env = |key: &str| (key == proof::PROOF_VAR).then(|| chat_proof.map(str::to_owned))?;
    if let Err(why) = proof::identify(root, Some(issuer), &env, false, &String::new) {
        return Err(machine::refuse_unproven(
            root,
            issuer,
            Scope::Delegate,
            &args.reason,
            why,
        ));
    }
    delegate::holds(root, issuer, &args.chat, &args.reason)?;
    let target = openagents
        .resolve_chat(&args.chat)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let local = openagents
        .local_device_id()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let (method, params, did) = if name == GRANT {
        let mut scopes = Vec::new();
        for word in &args.scopes {
            scopes.push(Scope::parse(word).ok_or_else(|| {
                format!(
                    "`{word}` isn't a machine scope; use build.prune, process.stop, or host.upgrade"
                )
            })?);
        }
        let request = delegate::GrantRequest {
            agent: target.id.clone(),
            scopes,
            for_secs: args.for_minutes.map(|minutes| minutes.saturating_mul(60)),
            issuer: Some(issuer.to_owned()),
            reason: args.reason.clone(),
        };
        let did = format!("granted {}", args.scopes.join(", "));
        (methods::GRANT_MACHINE, serde_json::to_value(request), did)
    } else {
        let request = delegate::RevokeRequest {
            agent: target.id.clone(),
            issuer: Some(issuer.to_owned()),
            reason: args.reason.clone(),
        };
        (
            methods::REVOKE_MACHINE,
            serde_json::to_value(request),
            "revoked".to_owned(),
        )
    };
    let params = params.map_err(|e| e.to_string())?;
    let sent = send(openagents, &target.device_id, method, params).await;
    if target.device_id != local {
        // The target host logs the grant; this one logs that it was sent.
        let mut entry = Entry::new(&machine::System, issuer, Scope::Delegate, &args.reason);
        entry.target = Some(target.id.clone());
        match &sent {
            Ok(_) => {
                entry.outcome = Outcome::Done;
                entry.detail = format!("{did} on device {}", target.device_id);
            }
            Err(why) => {
                entry.outcome = Outcome::Failed;
                entry.detail.clone_from(why);
            }
        }
        let _ = machine::log(root, &entry);
    }
    let mut reply = sent?;
    reply["chat"] = json!(target.id);
    reply["deviceId"] = json!(target.device_id);
    Ok(reply)
}

/// Sends `method` to the device `device_id`'s engine, through this one. A
/// device whose app doesn't know the method, or doesn't answer, is named
/// with its version.
///
/// # Errors
/// The send failed or the host refused; the message says which host.
pub async fn send(
    openagents: &OpenAgents,
    device_id: &str,
    method: &str,
    mut params: Value,
) -> Result<Value, String> {
    params["targetDeviceId"] = json!(device_id);
    match openagents.call(method, params).await {
        Ok(reply) => Ok(reply),
        Err(error) => {
            let error = format!("{error:#}");
            let device = openagents
                .devices()
                .await
                .ok()
                .and_then(|devices| devices.into_iter().find(|device| device.id == device_id));
            let name = device
                .as_ref()
                .map_or(device_id, |device| device.name.as_str());
            let version = device
                .as_ref()
                .and_then(|device| device.version.as_deref())
                .unwrap_or("an unknown version");
            if error.contains("unknown method") {
                Err(format!(
                    "{name} runs openagents {version}, which can't take delegated machine grants yet; upgrade that computer's app first (upgrade_host from a chat on it)"
                ))
            } else {
                Err(format!("{name} (openagents {version}): {error}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use futures::StreamExt as _;
    use openagents_rpc::{RpcError, RpcReply, RpcService};

    use super::*;
    use crate::openagents::Origin;

    /// The Mac's engine with Foreman on it, forwarding to a newer computer
    /// (`dev-new`, whose lease root is `remote`) and an older one
    /// (`dev-old`) that doesn't know `GrantMachine`.
    struct Engine {
        remote: std::path::PathBuf,
        sent: Mutex<Vec<Value>>,
    }

    #[async_trait]
    impl RpcService for Engine {
        async fn handle(&self, method: &str, params: Value) -> Result<RpcReply, RpcError> {
            let stream = |item: Value| RpcReply::Stream(futures::stream::iter(vec![item]).boxed());
            Ok(match method {
                methods::LOCAL_DEVICE => RpcReply::Value(json!({ "deviceId": "dev-mac" })),
                methods::WATCH_DEVICES => stream(json!([
                    {"id": "dev-mac", "name": "Tron", "platform": "macos", "lastSeenAt": null},
                    {"id": "dev-new", "name": "coderos-5090", "platform": "linux", "lastSeenAt": null, "version": "1.0.1"},
                    {"id": "dev-old", "name": "coderos-4080", "platform": "linux", "lastSeenAt": null, "version": "1.0.0"}
                ])),
                methods::WATCH_CHATS => stream(json!([
                    {"id": "chat-android", "deviceId": "dev-new", "title": "Android lead", "archived": false,
                     "config": {"harness": "codex", "sandbox": "workspace-write"}, "createdAt": "2026-10-18T00:00:00Z"},
                    {"id": "chat-old", "deviceId": "dev-old", "title": "Old host", "archived": false,
                     "config": {"harness": "codex", "sandbox": "workspace-write"}, "createdAt": "2026-10-18T00:00:00Z"}
                ])),
                methods::GRANT_MACHINE | methods::REVOKE_MACHINE => {
                    self.sent.lock().unwrap().push(params.clone());
                    if params["targetDeviceId"] == "dev-old" {
                        return Err(RpcError::UnknownMethod(method.into()));
                    }
                    if method == methods::GRANT_MACHINE {
                        let request: delegate::GrantRequest =
                            serde_json::from_value(params).unwrap();
                        let granted = delegate::serve_grant(&self.remote, &request)
                            .map_err(RpcError::Failed)?;
                        RpcReply::Value(json!({ "granted": granted }))
                    } else {
                        let request: delegate::RevokeRequest =
                            serde_json::from_value(params).unwrap();
                        let revoked = delegate::serve_revoke(&self.remote, &request)
                            .map_err(RpcError::Failed)?;
                        RpcReply::Value(json!({ "revoked": revoked }))
                    }
                }
                other => return Err(RpcError::UnknownMethod(other.into())),
            })
        }
    }

    #[tokio::test]
    async fn a_delegate_holder_grants_on_the_chats_own_computer_and_old_hosts_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let (mac, remote) = (dir.path().join("mac"), dir.path().join("remote"));
        let engine = Arc::new(Engine {
            remote: remote.clone(),
            sent: Mutex::new(Vec::new()),
        });
        let openagents = OpenAgents::with_client(
            openagents_rpc::memory_client(engine.clone()),
            Origin::default(),
        );
        let foreman = proof::proof_for(&mac, "foreman").unwrap();
        let grant =
            json!({"chat": "Android lead", "scopes": ["build.prune"], "reason": "disk at 12 GiB"});
        let run = |name: &'static str, args: Value, proof: Option<String>| {
            let (mac, openagents) = (mac.clone(), &openagents);
            async move {
                call_at(
                    &mac,
                    openagents,
                    Some("foreman"),
                    proof.as_deref(),
                    name,
                    args,
                )
                .await
            }
        };

        // No proof, then no delegate grant: refused before anything is sent.
        assert!(
            run(GRANT, grant.clone(), None)
                .await
                .unwrap_err()
                .starts_with("refused:")
        );
        let err = run(GRANT, grant.clone(), Some(foreman.clone()))
            .await
            .unwrap_err();
        assert!(err.contains("no delegate grant"), "{err}");
        assert!(engine.sent.lock().unwrap().is_empty());

        machine::grant(&mac, "foreman", &[Scope::Delegate], None).unwrap();
        let out = run(GRANT, grant.clone(), Some(foreman.clone()))
            .await
            .unwrap();
        assert_eq!(out["deviceId"], "dev-new", "{out}");
        let now = coder_lease::now_ms();
        let there = machine::Grants::load(&remote).unwrap();
        assert_eq!(
            there.scopes_of("chat-android", now),
            vec![Scope::BuildPrune]
        );
        assert_eq!(engine.sent.lock().unwrap()[0]["issuer"], "foreman");

        // Never delegate itself, even through the tool.
        let err = run(
            GRANT,
            json!({"chat": "chat-android", "scopes": ["delegate"], "reason": "x"}),
            Some(foreman.clone()),
        )
        .await
        .unwrap_err();
        assert!(err.contains("isn't delegable"), "{err}");

        let err = run(
            GRANT,
            json!({"chat": "Old host", "scopes": ["build.prune"], "reason": "disk"}),
            Some(foreman.clone()),
        )
        .await
        .unwrap_err();
        assert!(err.contains("coderos-4080 runs openagents 1.0.0"), "{err}");

        let out = run(
            REVOKE,
            json!({"chat": "chat-android", "reason": "done"}),
            Some(foreman),
        )
        .await
        .unwrap();
        assert_eq!(out["revoked"], true);
        assert!(
            machine::Grants::load(&remote)
                .unwrap()
                .scopes_of("chat-android", now)
                .is_empty()
        );

        // Both computers' logs show it: sent here, written there.
        let here = machine::read_log(&mac, 20).unwrap();
        assert!(here.iter().any(|e| e.outcome == Outcome::Done
            && e.target.as_deref() == Some("chat-android")
            && e.detail == "granted build.prune on device dev-new"));
        assert!(
            here.iter()
                .any(|e| e.outcome == Outcome::Failed && e.detail.contains("coderos-4080"))
        );
        let there = machine::read_log(&remote, 20).unwrap();
        assert!(
            there
                .iter()
                .any(|e| e.agent == "foreman" && e.reason == "disk at 12 GiB")
        );
        assert!(there.iter().any(|e| e.detail == "revoked build.prune"));
    }
}
