//! Schemas for the implemented private agent definition API.
use serde_json::{Value, json};
fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/CustomAgent{name}")})
}
pub(crate) fn extend(doc: &mut Value) {
    let id = json!({"type":"string","pattern":"^[0-9a-f]{64}$"});
    let revision = json!({"type":"integer","minimum":1});
    let spec = json!({"type":"object","additionalProperties":false,"required":["name","description","instructions","runtime","model","access"],"properties":{
  "name":{"type":"string","minLength":1,"maxLength":80,"description":"Nonblank, no control characters; at most 80 UTF-8 bytes."},
  "description":{"type":"string","maxLength":512,"description":"At most 512 UTF-8 bytes; no control characters."},
  "instructions":{"type":"string","minLength":1,"maxLength":agent_contract::MAX_INSTRUCTIONS_BYTES,"description":"Nonblank, no NUL; at most 65,536 UTF-8 bytes, skills included."},
  "runtime":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$","description":"The runtime to use first. `any` leaves the choice to the computer. Not part of the agent's identity: changing it keeps the agent and applies from the next turn."},
  "model":{"type":["string","null"],"minLength":1,"maxLength":256},"access":{"enum":["host_default","read_only","workspace_write"]},
  "fallbacks":{"type":"array","maxItems":4,"description":"Runtimes to try in order when `runtime` isn't installed, turned on, or signed in on the computer; then the computer's own default. Each runtime appears once and is not `sdk` or `any`. Omitted when empty.","items":{"type":"object","additionalProperties":false,"required":["runtime"],"properties":{
    "runtime":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$"},
    "model":{"type":"string","minLength":1,"maxLength":256}}}},
  "autonomy":{"type":"object","description":"How the agent runs on its own. Not part of the agent's identity. Omitted when unset; unknown fields are ignored.","properties":{
    "check_ins":{"type":"boolean","description":"Hosts turn timed check-ins on for this agent's chat without being asked, when it is the computer's only such chat."},
    "every_minutes":{"type":"integer","minimum":5,"maximum":1440,"description":"Minutes between check-ins when the host turns them on."},
    "stall_minutes":{"type":"integer","minimum":5,"maximum":240,"description":"Minutes a chat or claim the agent manages may go with no progress before the host raises it. Hosts default to 20."},
    "authority":{"type":"boolean","description":"The agent asks for standing authority over other chats with its check-ins. The host grants it, logged; the owner can turn it off."},
    "shell_timeout_secs":{"type":"integer","minimum":10,"maximum":3600,"description":"The longest one shell command may run, in seconds. The host passes it to the runtime."},
    "wakes":{"type":"array","maxItems":16,"description":"What the agent wakes on; omitted or empty is every wake the host knows. Readers skip wakes they don't know.","items":{"type":"string","examples":["chat_finished","chat_errored","chat_needs_answer","issue_opened","issue_closed","claim_stale","chat_stalled","claim_no_progress"]}},
    "coalesce_secs":{"type":"integer","minimum":0,"maximum":1800,"description":"Seconds events wait for quiet before they start one check-in."}}}}});
    let scope = json!({"type":"string","enum":["chat.message","chat.start","chat.steer","chat.stop","land.submit","work.dispatch"]});
    let over = json!({"oneOf":[
        {"type":"string","enum":["account"]},
        {"type":"object","additionalProperties":false,"required":["project"],"properties":{"project":{"type":"string","minLength":1,"maxLength":128}}},
        {"type":"object","additionalProperties":false,"required":["chats"],"properties":{"chats":{"type":"array","minItems":1,"maxItems":64,"items":{"type":"string","minLength":1,"maxLength":128}}}}]});
    let schemas = json!({
     "CustomAgentSpec":spec,
     "CustomAgentDefinition":{"type":"object","additionalProperties":false,"required":["id","revision","spec","deleted"],"properties":{"id":id,"revision":revision,"spec":r("Spec"),"deleted":{"type":"boolean"}}},
     "CustomAgentCreate":{"type":"object","additionalProperties":false,"required":["request_id","spec"],"properties":{"request_id":id,"spec":r("Spec")}},
     "CustomAgentUpdate":{"type":"object","additionalProperties":false,"required":["request_id","expected_revision","spec"],"properties":{"request_id":id,"expected_revision":revision,"spec":r("Spec")}},
     "CustomAgentDelete":{"type":"object","additionalProperties":false,"required":["request_id","expected_revision"],"properties":{"request_id":id,"expected_revision":revision}},
     "CustomAgentPage":{"type":"object","required":["data","next"],"properties":{"data":{"type":"array","maxItems":100,"items":r("Definition")},"next":{"type":["string","null"]}}},
     "CustomAgentGrant":{"type":"object","additionalProperties":false,"required":["id","scopes","over","grantedAt"],"description":"A power the account owner gave the agent over other chats. Grants belong to the account, never to the definition: a fork asks its own owner.","properties":{"id":id,"scopes":{"type":"array","items":scope},"over":over,"grantedAt":{"type":"integer","description":"Unix milliseconds."}}},
     "CustomAgentGrantCreate":{"type":"object","additionalProperties":false,"required":["request_id","scopes","over"],"description":"Scopes are words. Money, keys, secrets, deploys and closing other agents' issues can't be granted to any agent and answer 400 not_grantable.","properties":{"request_id":id,"scopes":{"type":"array","minItems":1,"items":{"type":"string"}},"over":over}},
     "CustomAgentGrantList":{"type":"object","additionalProperties":false,"required":["data"],"properties":{"data":{"type":"array","maxItems":32,"items":r("Grant")}}},
     "CustomAgentGrantRevoked":{"type":"object","additionalProperties":false,"required":["revoked"],"properties":{"revoked":{"type":"boolean"}}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    for (path, method, operation, body, result) in [
        (
            "/v1/agent-definitions",
            "get",
            "listAgentDefinitions",
            None,
            "Page",
        ),
        (
            "/v1/agent-definitions",
            "post",
            "createAgentDefinition",
            Some("Create"),
            "Definition",
        ),
        (
            "/v1/agent-definitions/{id}",
            "get",
            "getAgentDefinition",
            None,
            "Definition",
        ),
        (
            "/v1/agent-definitions/{id}",
            "put",
            "updateAgentDefinition",
            Some("Update"),
            "Definition",
        ),
        (
            "/v1/agent-definitions/{id}",
            "delete",
            "deleteAgentDefinition",
            Some("Delete"),
            "Definition",
        ),
        (
            "/v1/agent-definitions/{id}/grants",
            "get",
            "listAgentGrants",
            None,
            "GrantList",
        ),
        (
            "/v1/agent-definitions/{id}/grants",
            "post",
            "grantAgent",
            Some("GrantCreate"),
            "Grant",
        ),
        (
            "/v1/agent-definitions/{id}/grants/{grant}",
            "delete",
            "revokeAgentGrant",
            None,
            "GrantRevoked",
        ),
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters.push(json!({"name":"id","in":"path","required":true,"schema":id}));
        }
        if path.contains("{grant}") {
            parameters.push(json!({"name":"grant","in":"path","required":true,"schema":id}));
        }
        if method == "get" && !path.ends_with("/grants") {
            if result == "Page" {
                parameters.push(json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":100,"default":100}}));
                parameters.push(json!({"name":"after","in":"query","schema":id}));
            } else {
                parameters.push(json!({"name":"revision","in":"query","schema":revision}));
            }
        }
        let mut op = json!({"operationId":operation,"tags":["Custom agents"],"security":[{"appBearer":[]}],"parameters":parameters,
   "description":"Private account-owned definition. Does not install or execute an agent. We keep each change's request_id: a retry with the same inputs returns the first result, and changed inputs conflict. Deleted definitions and old revisions remain readable to their owner. Lists exclude deleted definitions and continue in ID order. Capacity is 128 active definitions, 4096 mutations, and 4 MiB per account, including history; no automatic history pruning. Updates and deletion require expected_revision. Maximum JSON body is 24 KiB.",
   "responses":{"200":{"description":"The account-owned result; no-store, private.","content":{"application/json":{"schema":r(result)}}}}});
        if let Some(body) = body {
            op["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":r(body)}}});
        }
        for status in ["400", "401", "403", "404", "409", "413", "429", "503"] {
            op["responses"][status] = json!({"description":"Invalid fields, signed out, denied, missing, version_conflict/idempotency_conflict, too large, quota_exhausted, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = op;
    }
}
