//! Source-backed continuation contract shared by account and computer homes.
use serde_json::{Value, json};
pub(crate) fn extend(doc: &mut Value) {
    let schemas = doc["components"]["schemas"].as_object_mut().unwrap();
    schemas.insert(
        "ChatHome".into(),
        json!({"type":"string","pattern":"^(cloud|computer:[A-Za-z0-9._-]{1,128})$"}),
    );
    schemas.insert("ChatPartRef".into(),json!({"type":"object","additionalProperties":false,"required":["id","home"],"properties":{"id":{"type":"string","format":"uuid"},"home":{"$ref":"#/components/schemas/ChatHome"}}}));
    schemas.insert("ChatContextTurn".into(),json!({"type":"object","additionalProperties":false,"required":["role","text"],"properties":{"role":{"enum":["user","assistant"]},"text":{"type":"string","maxLength":1024}}}));
    schemas.insert("ChatContext".into(),json!({"type":"object","additionalProperties":false,"required":["title","summary","recent"],"properties":{"title":{"type":"string","maxLength":256},"summary":{"type":"string","maxLength":1024},"recent":{"type":"array","maxItems":12,"items":{"$ref":"#/components/schemas/ChatContextTurn"}}},"description":"At most 16 KiB of serialized UTF-8 JSON. Summary and recent user/assistant text only; no file or tool output. Credential-shaped text is screened."}));
    schemas.insert("ChatContinuationClaim".into(),json!({"type":"object","additionalProperties":false,"required":["revision","target"],"properties":{"revision":{"type":"integer","minimum":1},"target":{"$ref":"#/components/schemas/ChatPartRef"},"seed":{"$ref":"#/components/schemas/ChatContext"}}}));
    schemas.insert("ChatContinuation".into(),json!({"type":"object","additionalProperties":false,"required":["account","source","target","seed"],"properties":{"account":{"type":"string"},"source":{"$ref":"#/components/schemas/ChatPartRef"},"target":{"$ref":"#/components/schemas/ChatPartRef"},"seed":{"$ref":"#/components/schemas/ChatContext"}}}));
    for name in ["HistorySummary", "ThreadRow"] {
        let summary = schemas.get_mut(name).unwrap();
        summary["properties"]["home"] = json!({"$ref":"#/components/schemas/ChatHome"});
        for key in ["continues", "continued_by"] {
            summary["properties"][key] =
                json!({"oneOf":[{"type":"null"},{"$ref":"#/components/schemas/ChatPartRef"}]});
        }
    }
    let mut responses = json!({"200":{"description":"The same linked destination, including a safe context seed","content":{"application/json":{"schema":{"$ref":"#/components/schemas/ChatContinuation"}}}}});
    for status in ["400", "401", "404", "409", "429", "503"] {
        responses[status] = json!({"description":"Invalid reference, signed out, not owned/missing, changed/busy/already continued, throttled, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
    }
    doc["paths"]["/v1/threads/{id}/continue"]["post"] = json!({"operationId":"continueChatOnComputer","tags":["Chat homes"],"security":[{"appBearer":[]}],"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}],"requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/ChatContinuationClaim"}}}},"responses":responses,"description":"Owner-only transition of an idle source to a new cloud or computer part. The host first creates its normal session, then calls with that session ID and its device home. A changed source refuses; the same destination is retry-safe, while another destination refuses. The source becomes read-only and records continued_by before execution starts. The host records continues in its document and registry and passes the bounded seed through its existing harness path. This endpoint never starts a harness or creates execution authority. Computer sources supply a bounded seed after freezing their local command queue. Cloud targets are created with that context outside the visible transcript; the normal cloud send route starts their turns. Older terminal sources use stable legacy-{chat-id} computer references until their original computer can be identified."});
}
