//! Public run presence routes and limits.
use serde_json::{Value, json};
fn reference(name: &str) -> Value {
    json!({"$ref": format!("#/components/schemas/Run{name}")})
}
pub(crate) fn extend(doc: &mut Value) {
    let id = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$","not":{"enum":[".",".."]},"description":"An identifier other than . or .. ."});
    let time = json!({"type":"integer","format":"int64","minimum":0,"description":"Unix milliseconds, at most one minute ahead of server time."});
    let optional_id = json!({"anyOf":[id,{"type":"null"}]});
    let schemas = json!({
        "RunHeartbeat": {"type":"object","additionalProperties":false,"required":["run","kind","harness","computer","status","last_output_at"],"properties":{
            "run":id,"kind":{"type":"string","enum":["app_chat","terminal","cloud_work","landing"]},"harness":id,"harness_session":optional_id,"computer":id,
            "issues":{"type":"array","maxItems":32,"uniqueItems":true,"items":{"type":"integer","minimum":1,"maximum":18446744073709551615_u64},"default":[]},
            "status":{"type":"string","enum":["idle","working","awaiting_input","errored"]},
            "last_output_at":time,"last_progress_at":{"anyOf":[time,{"type":"null"}]},
            "note":{"type":["string","null"],"maxLength":199,"description":"One line, fewer than 200 UTF-8 bytes, with no control characters."},
            "agent":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$","description":"Agent definition ID, never an agent name."}
        }},
        "RunList":{"type":"object","required":["runs"],"properties":{"runs":{"type":"array","maxItems":512,"items":reference("Heartbeat")}}},
        "RunOk":{"type":"object","required":["ok"],"properties":{"ok":{"const":true}}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    for (path, method, operation, result) in [
        ("/v1/runs", "get", "listRuns", "List"),
        ("/v1/runs/{id}/heartbeat", "put", "heartbeatRun", "Ok"),
        ("/v1/runs/{id}/end", "post", "endRun", "Ok"),
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters.push(json!({"name":"id","in":"path","required":true,"schema":id}));
        }
        if method == "get" {
            parameters.push(json!({"name":"live","in":"query","schema":{"type":"boolean"}}));
            parameters.push(json!({"name":"issue","in":"query","schema":{"type":"integer","minimum":1,"maximum":18446744073709551615_u64}}));
        }
        let mut op = json!({"operationId":operation,"tags":["Runs"],"security":[{"appBearer":[]}],"parameters":parameters,
            "description":"Private to the signed-in account. Live means not ended and a heartbeat received less than three minutes ago. Output and progress are host-reported Unix milliseconds, separate from receipt time. List filters combine with AND; live=false selects expired or ended runs. Results are newest receipt first, with run ID breaking ties. At most 512 runs; reports disappear from the list after 24 hours without a heartbeat, or 24 hours after ending. Use a fresh ID for a new run. Heartbeat body run must match the path ID. Run kind, harness, harness session, computer, and agent stay fixed; output and progress cannot move backward. Send at most once a minute except on status changes. An exact retry within a minute succeeds without extending presence; the same report after a minute refreshes it. Limits: 12 accepted heartbeats per run and 120 per account in a rolling minute, including status changes; 8 KiB heartbeat body. Repeated end requests succeed for a known ID. Heartbeats for an ended ID conflict while its record is kept. Reports grant no execution or steering authority. Responses are private and no-store.",
            "responses":{"200":{"description":"The account's runs or a successful update.","content":{"application/json":{"schema":reference(result)}}}}});
        if method == "put" {
            op["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":reference("Heartbeat")}}});
        }
        for status in ["400", "401", "403", "404", "409", "413", "429", "503"] {
            op["responses"][status] = json!({"description":"Invalid request, signed out, denied, missing run, run_ended/run_conflict, body too large, rate_limited/quota_exhausted, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        op["responses"]["429"]["headers"] = json!({"Retry-After":{"description":"Seconds to wait when rate_limited.","schema":{"type":"integer","minimum":1}}});
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = op;
    }
}
