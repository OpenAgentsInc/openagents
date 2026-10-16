//! Public JSON event and stop schemas for the existing cloud turn runtime.
use serde_json::{Value, json};
pub(crate) fn extend(doc: &mut Value) {
    let schemas = doc["components"]["schemas"].as_object_mut().unwrap();
    schemas.insert("ChatTurnError".into(),json!({"type":"object","required":["code","message","retry_after_ms"],"properties":{"code":{"type":"string","maxLength":128},"message":{"type":"string"},"retry_after_ms":{"type":["integer","null"],"minimum":0}}}));
    let mut variants = Vec::new();
    for kind in ["text", "tool", "cards", "done", "error"] {
        let mut props = json!({"type":{"const":kind},"request_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1}});
        let mut required = vec!["type", "request_id", "revision"];
        match kind {
            "text" | "tool" => {
                props["index"] = json!({"type":"integer","minimum":0,"maximum":3999});
                props["role"] = json!({"enum":["user","assistant","thinking","tool"]});
                props["text"] = json!({"type":"string"});
                props["replace"] = json!({"type":"boolean"});
                required.extend(["index", "role", "text", "replace"]);
            }
            "cards" => {
                props["cards"] = json!({"type":"object","description":"Display data with existing Gym/Lab card objects; preserve unknown fields. Cards do not grant permission to execute an offer."});
                required.push("cards");
            }
            "done" => {
                props["outcome"] = json!({"enum":["answered","stopped"]});
                required.push("outcome");
            }
            _ => {
                props["error"] = json!({"$ref":"#/components/schemas/ChatTurnError"});
                required.push("error");
            }
        }
        variants.push(json!({"type":"object","required":required,"properties":props}));
    }
    schemas.insert("ChatEvent".into(), json!({"oneOf":variants}));
    schemas.insert("ChatStop".into(),json!({"type":"object","required":["request_id"],"additionalProperties":false,"properties":{"request_id":{"type":"string","format":"uuid"}}}));
    schemas.insert("ChatStopped".into(),json!({"type":"object","required":["request_id","stopped","finished"],"properties":{"request_id":{"type":"string","format":"uuid"},"stopped":{"type":"boolean"},"finished":{"const":true}}}));
    schemas.get_mut("HistoryMessage").unwrap()["properties"]["reply"] = json!({"type":["object","null"],"properties":{"cards":{"type":"array","maxItems":16,"items":{"type":"object"}}},"description":"Person-visible cards only; model-routing evidence is not included."});
    schemas.get_mut("ThreadRow").unwrap()["properties"]["request_id"] =
        json!({"type":["string","null"],"format":"uuid"});
    schemas.get_mut("ThreadRow").unwrap()["properties"]["outcome"] =
        json!({"enum":["pending","answered","stopped","failed","unknown",null]});
    for (path, method, id) in [
        ("/v1/threads/{id}/events", "get", "streamCloudTurn"),
        ("/v1/threads/{id}/stop", "post", "stopCloudTurn"),
    ] {
        let mut responses = json!({});
        for status in ["400", "401", "403", "404", "409", "429", "503"] {
            responses[status] = json!({"description":"Invalid request, signed out, current access refused, missing chat/request, wrong home/changed state, throttled or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        let mut op = json!({"operationId":id,"tags":["Cloud chats"],"security":[{"appBearer":[]}],"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}],"responses":responses});
        if method == "get" {
            op["description"] = json!(
                "Follow one request in the existing account-store cloud runtime. Account session/sign-in key only; cookies and oak keys are refused. Owner or current shared-project member, rechecked while connected. SSE event names match JSON type: text, tool, cards, done, error. Each connection starts with replace:true snapshots of that request's current messages, followed by replace:false text suffixes; apply by index, never append a replacement twice. Reconnect with the same request_id after a lost connection; no resend is needed. Event IDs identify this connection's revision/sequence; Last-Event-ID is not used to replay events. A connection lasts at most five minutes; stream_timeout asks the client to reconnect. Closing/cancelling the stream leaves execution running; POST stop cancels the named request. A terminal done/error closes the stream. Tool/card events are display data, never local execution authority."
            );
            op["parameters"].as_array_mut().unwrap().push(json!({"name":"request_id","in":"query","required":true,"schema":{"type":"string","format":"uuid"}}));
            op["responses"]["200"] = json!({"description":"UTF-8 SSE. JSON data conforms to ChatEvent; comment keepalives every 15 seconds. Cache-Control: no-store.","content":{"text/event-stream":{"schema":{"type":"string"},"x-event-schema":{"$ref":"#/components/schemas/ChatEvent"}}}});
        } else {
            op["description"] = json!(
                "Stop only request_id in this cloud chat, using the existing stop transition. Owner or the current shared member who sent the pending turn. A delayed or repeated stop cannot cancel a newer request. stopped:true confirms the persisted stopped state; stopped:false with finished:true means the named request already finished. Partial text stays; the server drops the model call after observing the stopped state. A pending request owned by another execution path refuses 409 unsupported_turn, rather than claiming it finished. This does not stop terminal/phone sessions or background work."
            );
            op["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/ChatStop"}}}});
            op["responses"]["200"] = json!({"description":"Confirmed state of the named request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/ChatStopped"}}}});
        }
        doc["paths"][path][method] = op;
    }
}
