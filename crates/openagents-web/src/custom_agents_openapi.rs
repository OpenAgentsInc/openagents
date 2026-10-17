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
  "instructions":{"type":"string","minLength":1,"maxLength":16384,"description":"Nonblank, no NUL; at most 16,384 UTF-8 bytes."},
  "runtime":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$","description":"The runtime to use first. `any` leaves the choice to the computer. Not part of the agent's identity: changing it keeps the agent and applies from the next turn."},
  "model":{"type":["string","null"],"minLength":1,"maxLength":256},"access":{"enum":["host_default","read_only","workspace_write"]},
  "fallbacks":{"type":"array","maxItems":4,"description":"Runtimes to try in order when `runtime` isn't installed, turned on, or signed in on the computer; then the computer's own default. Each runtime appears once and is not `sdk` or `any`. Omitted when empty.","items":{"type":"object","additionalProperties":false,"required":["runtime"],"properties":{
    "runtime":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$"},
    "model":{"type":"string","minLength":1,"maxLength":256}}}}}});
    let schemas = json!({
     "CustomAgentSpec":spec,
     "CustomAgentDefinition":{"type":"object","additionalProperties":false,"required":["id","revision","spec","deleted"],"properties":{"id":id,"revision":revision,"spec":r("Spec"),"deleted":{"type":"boolean"}}},
     "CustomAgentCreate":{"type":"object","additionalProperties":false,"required":["request_id","spec"],"properties":{"request_id":id,"spec":r("Spec")}},
     "CustomAgentUpdate":{"type":"object","additionalProperties":false,"required":["request_id","expected_revision","spec"],"properties":{"request_id":id,"expected_revision":revision,"spec":r("Spec")}},
     "CustomAgentDelete":{"type":"object","additionalProperties":false,"required":["request_id","expected_revision"],"properties":{"request_id":id,"expected_revision":revision}},
     "CustomAgentPage":{"type":"object","required":["data","next"],"properties":{"data":{"type":"array","maxItems":100,"items":r("Definition")},"next":{"type":["string","null"]}}}
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
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters.push(json!({"name":"id","in":"path","required":true,"schema":id}));
        }
        if method == "get" {
            if result == "Page" {
                parameters.push(json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":100,"default":100}}));
                parameters.push(json!({"name":"after","in":"query","schema":id}));
            } else {
                parameters.push(json!({"name":"revision","in":"query","schema":revision}));
            }
        }
        let mut op = json!({"operationId":operation,"tags":["Custom agents"],"security":[{"appBearer":[]}],"parameters":parameters,
   "description":"Private account-owned definition. Does not install or execute an agent. Mutation request_id is retained durably: identical retries return the original result, changed inputs conflict. Deleted definitions and old revisions remain readable to their owner. Lists exclude deleted definitions and continue in ID order. Capacity is 128 active definitions, 4096 mutations, and 4 MiB per account, including history; no automatic history pruning. Updates and deletion require expected_revision. Maximum JSON body is 24 KiB.",
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
