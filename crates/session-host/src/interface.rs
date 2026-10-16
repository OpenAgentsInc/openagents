//! Pinned CAP definitions for the managed-session host's actual operations.
use crate::{Result, engine::Profile};
use nostr::contracts::{self, SCHEMA_MEDIA_TYPE};
use serde_json::{Value, json};
use workspace_host::artifact::Artifacts;

pub(crate) fn native_action(
    store: &Artifacts,
    host: &str,
    profile: &Profile,
    action: &crate::engine::ActionProfile,
) -> Result<Value> {
    let input = store.put(
        &contracts::jcs(&action.input_schema).map_err(|e| e.to_string())?,
        SCHEMA_MEDIA_TYPE,
        None,
    )?;
    let output = store.put(
        &contracts::jcs(&action.output_schema).map_err(|e| e.to_string())?,
        SCHEMA_MEDIA_TYPE,
        None,
    )?;
    let engine = store.put(&profile.engine, "application/octet-stream", None)?;
    let protocol = store.put(&profile.protocol, SCHEMA_MEDIA_TYPE, None)?;
    let id = format!("{host}:session/{}-action-{}", profile.name, action.method);
    let definition = json!({"v":1,"requires":[],"id":id,"profile":"native",
        "summary":format!("Run the admitted {} native action.",action.method),
        "input":input,"output":output,"effects":action.effects,"minimum":{},
        "support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"none","evidence":["openagents.session-native.v1"]},
        "binding_contract":{"operation":"session-native-action","interface":"openagents.session-native-action.v1"},
        "meta":{"engine":engine,"protocol":protocol,"method":action.method,"input_max_bytes":32768}});
    nostr::cap::parse_definition(&definition).map_err(|e| e.to_string())?;
    Ok(
        json!({"id":id,"artifact":store.put(&contracts::jcs(&definition).map_err(|e|e.to_string())?,"application/json",Some("openagents.cap.v1"))?}),
    )
}

fn schema(
    store: &Artifacts,
    feature: Option<&str>,
    output: bool,
    profile: &Profile,
) -> Result<Value> {
    let mut session: Value =
        serde_json::from_slice(nostr::session::SCHEMA_BYTES).map_err(|e| e.to_string())?;
    if feature == Some("approvals") && !output {
        session =
            serde_json::from_slice(nostr::sovereign::SCHEMA_BYTES).map_err(|e| e.to_string())?;
    }
    let reference = |name: &str| json!({"$ref":format!("#/$defs/{name}")});
    let shape = if output && matches!(feature, None | Some("submit" | "queue")) {
        json!({"oneOf":[reference("session-control-result"),reference("session-turn-result")]})
    } else if output {
        reference(if feature == Some("history") {
            "session-history"
        } else {
            "session-control-result"
        })
    } else {
        match feature {
            None => json!({"anyOf":[reference("session-open"),reference("session-command"),
                reference("session-interaction-response")]}),
            Some(name @ ("create" | "resume" | "fork" | "import")) => {
                let source = if name == "create" { "new" } else { name };
                json!({"allOf":[reference("session-open"),
                    {"properties":{"source":{"properties":{"kind":{"const":source}}}}}]})
            }
            Some("questions") => reference("session-interaction-response"),
            Some("approvals") => reference("approval-decision"),
            Some("history") => json!({"type":"object","additionalProperties":false,
                "required":["session"],"properties":{"session":{"type":"string","minLength":64,"maxLength":64}}}),
            Some(name) => {
                let actions = if name == "queue" {
                    vec!["enqueue", "cancel_queued", "promote"]
                } else {
                    vec![name]
                };
                json!({"allOf":[reference("session-command"),{"properties":{"action":{"enum":actions}}}]})
            }
        }
    };
    let mut body = json!({"$schema":"https://json-schema.org/draft/2020-12/schema",
        "$defs":session["$defs"],"allOf":[shape],
        "description":"Managed-session operation; current control rights and runtime admission are checked by the host."});
    if let Some(name) = feature {
        body["description"] = json!(format!(
            "Managed-session {} operation; support is {}. Current control rights and runtime admission are checked by the host.",
            name,
            profile.features.get(name).ok_or("feature")?.as_str()
        ));
        if name == "steer" {
            body["description"] = json!(format!(
                "Steering mode: {}; emulation: {}; acknowledgment: {}. Consumption requires the advertised engine evidence.",
                profile.steering.native,
                profile.steering.emulation.unwrap_or("none"),
                profile.steering.acknowledgment
            ));
        }
        if name == "close" {
            body["description"] = json!(
                "Close only after known turn quiescence; an unknown close keeps its resume binding."
            );
        }
    }
    store.put(
        &contracts::jcs(&body).map_err(|e| e.to_string())?,
        SCHEMA_MEDIA_TYPE,
        None,
    )
}

/// A definition is inert. Actual dispatch separately checks the exact admitted
/// adapter, signed command, current controller, runtime, and operation input.
pub(crate) fn definition(
    store: &Artifacts,
    host: &str,
    profile: &Profile,
    feature: Option<&str>,
) -> Result<Value> {
    let slug = match feature {
        Some(name) => format!("{}-{name}", profile.name),
        None => profile.name.to_string(),
    };
    let operation = feature
        .map(|name| format!("session-{name}"))
        .unwrap_or_else(|| "session-control".into());
    let id = format!("{host}:session/{slug}");
    let input = schema(store, feature, false, profile)?;
    let output = schema(store, feature, true, profile)?;
    let engine = store.put(&profile.engine, "application/octet-stream", None)?;
    let protocol = store.put(&profile.protocol, SCHEMA_MEDIA_TYPE, None)?;
    let body = json!({"v":1,"requires":[],"id":id,"profile":"native",
        "summary":format!("Manage {} sessions using the admitted host operation.", profile.name),
        "input":input,"output":output,"effects":profile.effects,"minimum":{},
        "support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"none",
            "evidence":["openagents.session-control-result.v1","openagents.session-control-receipt.v1",
                "openagents.session-native.v1","openagents.session-history.v1"]},
        "binding_contract":{"operation":operation,"interface":"openagents.session-host.v1"},
        "meta":{"engine":engine,"protocol":protocol,"feature":feature,
            "features":profile.features.iter().map(|(name,support)|
                (name.to_string(),json!(support.as_str()))).collect::<serde_json::Map<_,_>>() }});
    nostr::cap::parse_definition(&body).map_err(|e| e.to_string())?;
    let artifact = store.put(
        &contracts::jcs(&body).map_err(|e| e.to_string())?,
        "application/json",
        Some("openagents.cap.v1"),
    )?;
    Ok(json!({"id":id,"artifact":artifact}))
}
