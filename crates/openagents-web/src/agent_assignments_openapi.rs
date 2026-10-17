//! Public assignment requests; the host's session remains the transcript owner.
use serde_json::{Value, json};
fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/AgentAssignment{name}")})
}
pub(crate) fn extend(doc: &mut Value) {
    let id = json!({"type":"string","pattern":"^[0-9a-f]{64}$"});
    let rev = json!({"type":"integer","minimum":1});
    let state = json!({"enum":["queued","running","completed","failed","stopped","unknown"]});
    let host = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"});
    let task = json!({"type":"string","minLength":1,"maxLength":65536,"description":"Nonblank, no NUL, at most 65,536 UTF-8 bytes."});
    let spec = json!({"type":"object","additionalProperties":false,"required":["agent","agent_revision","host","task"],"properties":{"agent":id,"agent_revision":rev,"host":host,"project":{"type":["string","null"],"minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"},"task":task}});
    let action = json!({"oneOf":[
     {"type":"object","additionalProperties":false,"required":["action","text"],"properties":{"action":{"const":"follow_up"},"text":task}},
     {"type":"object","additionalProperties":false,"required":["action"],"properties":{"action":{"enum":["stop","archive"]}}}
    ]});
    let schemas = json!({
     "AgentAssignmentSpec":spec,
     "AgentAssignmentCreate":{"type":"object","additionalProperties":false,"required":["request_id","spec"],"properties":{"request_id":id,"spec":r("Spec")}},
     "AgentAssignmentCommand":{"type":"object","additionalProperties":false,"required":["request_id","action"],"properties":{"request_id":id,"action":action}},
     "AgentAssignmentReport":{"type":"object","additionalProperties":false,"required":["request_id","host","expected_revision","state"],"properties":{"active_command":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"},"after_seq":{"type":"integer","minimum":0},"request_id":id,"host":host,"expected_revision":rev,"state":{"enum":["running","completed","failed","stopped","unknown"]},"result":{"type":["string","null"],"maxLength":16384,"description":"At most 16,384 UTF-8 bytes; no NUL."}}},
     "AgentAssignmentValue":{"type":"object","required":["id","revision","spec","definition","chat","state","commands","result","archived"],"properties":{"active_command":{"type":["string","null"]},"after_seq":{"type":"integer","minimum":0},"id":id,"revision":rev,"spec":r("Spec"),"definition":{"$ref":"#/components/schemas/CustomAgentDefinition"},"chat":{"type":"string","format":"uuid"},"state":state,"commands":{"type":"array","maxItems":128,"items":r("Command")},"result":{"type":["string","null"],"maxLength":16384},"archived":{"type":"boolean"}}},
     "AgentAssignmentPage":{"type":"object","required":["data","next"],"properties":{"data":{"type":"array","maxItems":8,"items":r("Value")},"next":{"type":["string","null"]}}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    for (path, method, op, body, result) in [
        (
            "/v1/agent-assignments",
            "post",
            "createAgentAssignment",
            Some("Create"),
            "Value",
        ),
        (
            "/v1/agent-assignments",
            "get",
            "listAgentAssignments",
            None,
            "Page",
        ),
        (
            "/v1/agent-assignments/{id}",
            "get",
            "getAgentAssignment",
            None,
            "Value",
        ),
        (
            "/v1/agent-assignments/{id}/commands",
            "post",
            "commandAgentAssignment",
            Some("Command"),
            "Value",
        ),
        (
            "/v1/agent-assignments/{id}/report",
            "post",
            "reportAgentAssignment",
            Some("Report"),
            "Value",
        ),
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters.push(json!({"name":"id","in":"path","required":true,"schema":id}));
        }
        if result == "Page" {
            parameters.push(json!({"name":"host","in":"query","schema":host}));
            parameters.push(json!({"name":"after","in":"query","schema":id}));
        }
        let mut operation = json!({"operationId":op,"tags":["Custom agents"],"security":[{"appBearer":[]}],"parameters":parameters,
   "description":"Private account-owned assignment. Creation records intent; only the selected, account-authorized host may execute through its session ledger after its own permission checks. IDs and command IDs survive restart. Retain request_id and exact inputs for retry; changed inputs conflict. Follow-up and stop are intents, not proof of consumption or cancellation. Archive is refused while queued/running. Reports are account-authorized host claims, not remote attestation, and require the selected host ID and exact expected revision. The transcript remains on coder-sync under chat; result is only a bounded summary. Limits: 256 unarchived assignments, 128 commands per assignment, 4096 saved mutations and 4 MiB per account including history. Maximum JSON body 96 KiB; list pages contain at most eight assignments.",
   "responses":{"200":{"description":"Account-owned assignment or page, no-store/private.","content":{"application/json":{"schema":r(result)}}}}});
        if let Some(body) = body {
            operation["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":r(body)}}});
        }
        for status in ["400", "401", "403", "404", "409", "413", "429", "503"] {
            operation["responses"][status] = json!({"description":"Invalid assignment, signed out, denied, missing, assignment_conflict, too large, quota_exhausted, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = operation;
    }
}
