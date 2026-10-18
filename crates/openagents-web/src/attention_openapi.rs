//! Public attention stream route.
use serde_json::{Value, json};
pub(crate) fn extend(doc: &mut Value) {
    let time = json!({"type":"integer","format":"int64","description":"Unix milliseconds."});
    let text = json!({"type":"string"});
    let issue = json!({"type":"integer","minimum":1,"maximum":18446744073709551615_u64});
    let variant = |kind: &str, fields: Value| {
        let mut properties = json!({"kind":{"const":kind}});
        let mut required = vec![json!("kind")];
        for (name, schema) in fields.as_object().unwrap() {
            properties[name] = schema.clone();
            required.push(json!(name));
        }
        json!({"type":"object","required":required,"properties":properties})
    };
    let kinds = json!([
        variant("run_stalled", json!({"run":text,"since":time})),
        variant("run_awaiting_input", json!({"run":text,"question":text})),
        variant("run_errored", json!({"run":text,"reason":text})),
        variant(
            "claim_no_progress",
            json!({"issue":issue,"holder":text,"since":time})
        ),
        variant(
            "landing_failed",
            json!({"entry":text,"issue":{"anyOf":[issue,{"type":"null"}]},"reason":text})
        ),
        variant(
            "work_finished",
            json!({"work":text,"issue":issue,"outcome":{"type":"string","enum":["done","failed","cancelled"]}})
        ),
        variant(
            "machine_low",
            json!({"computer":text,"resource":{"type":"string","enum":["disk","memory"]},"value":{"type":"integer","minimum":0}})
        ),
    ]);
    doc["components"]["schemas"]["AttentionEvent"] = json!({
        "type":"object",
        "description":"Sent as an SSE `attention` event whose `id` is `seq`. Clients ignore unknown kinds.",
        "required":["seq","at","kind"],
        "properties":{
            "seq":{"type":"integer","minimum":1,"description":"Strictly increasing per account."},
            "at":time,
            "agent":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$","description":"Agent definition ID, never an agent name."}
        },
        "oneOf":kinds
    });
    let mut op = json!({
        "operationId":"streamAttention","tags":["Attention"],"security":[{"appBearer":[]}],
        "parameters":[{"name":"after","in":"query","schema":{"type":"integer","minimum":0},"description":"Send events with seq greater than this; 0 or absent for every event still kept. Without it, a Last-Event-ID header is used."}],
        "description":"Private to the signed-in account. Server-sent events in seq order, derived from the account's run heartbeats and work runs: a working run with no output for 20 minutes (run_stalled), a run awaiting input or errored, a live run holding an issue with no progress for 20 minutes (claim_no_progress), a failed landing, and a finished work run. Each condition is raised once. The last 1000 events are kept; sources older than 24 hours raise none. The stream ends after about two hours; reconnect with the last seq. Comments keep the connection open. Events grant no execution or steering authority.",
        "responses":{"200":{"description":"An event stream of AttentionEvent.","content":{"text/event-stream":{"schema":{"$ref":"#/components/schemas/AttentionEvent"}}}}}
    });
    for status in ["400", "401", "403", "503"] {
        op["responses"][status] = json!({"description":"Invalid event number, signed out, denied, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
    }
    doc["paths"]["/v1/attention"] = json!({"get": op});
}
