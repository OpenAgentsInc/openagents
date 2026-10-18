//! Public commands carried to a run's enrolled host.
use serde_json::{Value, json};
pub(crate) fn extend(doc: &mut Value) {
    let id = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$","not":{"enum":[".",".."]}});
    let text = json!({"type":"string","minLength":1,"maxLength":16384,"description":"Nonblank text, at most 16 KiB of UTF-8, without NUL."});
    let commands = [
        json!({"type":"object","additionalProperties":false,"required":["kind","text"],"properties":{"kind":{"const":"message"},"text":text}}),
        json!({"type":"object","additionalProperties":false,"required":["kind","text"],"properties":{"kind":{"const":"steer"},"text":text}}),
        json!({"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"stop"}}}),
        json!({"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"restart"},"brief":{"anyOf":[text,{"type":"null"}]}}}),
    ];
    let schemas = json!({
        "SessionCommand":{"oneOf":commands},
        "RunCommand":{"type":"object","additionalProperties":false,"required":["request_id","command"],"properties":{"request_id":id,"command":{"$ref":"#/components/schemas/SessionCommand"}}},
        "RunCommandReceipt":{"type":"object","additionalProperties":false,"required":["run","request_id","outcome"],"properties":{"run":id,"request_id":id,"outcome":{"oneOf":[
            {"type":"object","additionalProperties":false,"required":["status"],"properties":{"status":{"const":"accepted"}}},
            {"type":"object","additionalProperties":false,"required":["status","reason"],"properties":{"status":{"const":"refused"},"reason":{"type":"string","enum":["not_steerable","no_grant","run_ended","offline","unavailable"]}}}
        ]}}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    doc["paths"]["/v1/runs/{id}/commands"] = json!({"post":{
        "operationId":"commandRun","tags":["Runs"],"security":[{"appBearer":[]}],
        "parameters":[{"name":"id","in":"path","required":true,"schema":id}],
        "description":"Send a message, steer a running turn, stop, or restart through this account's explicitly enabled NIP-HOST binding. The host checks the native operate grant and any calling agent's definition grant. The host resolves the signed native grant through an operator-set caller binding; a request cannot choose its agent identity. Unbound grants refuse. Agent controls default to refused until the shared account-grant checker is installed. The run's computer selects the binding. App chats resolve by run ID or one matching harness session on that host. Other run kinds currently answer not_steerable. Acceptance means the host recorded a command, not that a turn finished. Keep the request ID and command unchanged on retry; changed retries conflict. An unavailable result may have reached the host: retry the same ID. Requests save the exact signed host packet before sending; retries never create a second native operation. At most 4096 command records and 8 MiB per account; records are not evicted. Maximum body 20 KiB. Responses are private and no-store.",
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/RunCommand"}}}},
        "responses":{"200":{"description":"Host receipt or a typed refusal.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/RunCommandReceipt"}}}},"400":{"description":"Invalid command or storage limit."},"401":{"description":"Sign in required."},"404":{"description":"Unknown account-owned run."},"409":{"description":"Changed request or host binding."},"413":{"description":"Body too large."},"503":{"description":"Account or storage service unavailable."}}
    }});
}
