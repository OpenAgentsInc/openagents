//! Schemas for the implemented, read-only saved-history surface.
use serde_json::{Value, json};
fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/History{name}")})
}
fn nullable(kind: &str) -> Value {
    json!({"type":[kind,"null"]})
}
pub(crate) fn extend(doc: &mut Value) {
    let schemas = json!({
        "HistorySummary":{"type":"object","required":["id","title","revision","updated_unix","pinned_unix","archived_unix","project","source","messages"],"properties":{
            "id":{"type":"string","format":"uuid"},"title":{"type":"string"},"revision":{"type":"integer","minimum":1},"updated_unix":{"type":"integer","minimum":0},
            "pinned_unix":nullable("integer"),"archived_unix":nullable("integer"),"project":nullable("string"),"source":{"enum":["web","terminal","phone"]},"messages":{"type":"integer","minimum":0}}},
        "HistoryProject":{"type":"object","required":["id","name","shared"],"properties":{"id":{"type":"string"},"name":{"type":"string"},"shared":{"type":"boolean"}}},
        "HistoryFile":{"type":"object","required":["id","name","kind","size"],"properties":{"id":{"type":"string","pattern":"^[0-9a-f]{32}$"},"name":{"type":"string"},"kind":{"enum":["png","jpeg","gif","webp","pdf","text","step","stl"]},"size":{"type":"integer","minimum":1}}},
        "HistoryTool":{"type":["object","null"],"required":["name","summary","status","input","output"],"properties":{"name":{"type":"string"},"summary":{"type":"string"},"status":{"enum":["running","done","failed"]},"input":{"type":"string"},"output":{"type":"string"}}},
        "HistoryMessage":{"type":"object","required":["id","role","text","files","tool","generated_files","artifacts"],"properties":{"id":{"type":"string","pattern":"^msg_[0-9]+$"},"role":{"enum":["user","assistant","tool","thinking"]},"text":{"type":"string"},"files":{"type":"array","items":r("File")},"tool":r("Tool"),"cards":{"type":"array","maxItems":16,"items":{"allOf":[{"$ref":"#/components/schemas/LabCard"},{"type":"object","properties":{"space":{"type":"string","description":"The space used for artifact reads with the reader's own credentials."}}}]}},"generated_files":{"type":"array","maxItems":0,"items":{}},"artifacts":{"type":"array","maxItems":0,"items":{}}}},
        "HistoryList":{"type":"object","required":["data","next"],"properties":{"data":{"type":"array","maxItems":200,"items":r("Summary")},"next":nullable("string")}},
        "HistoryProjects":{"type":"object","required":["data","next"],"properties":{"data":{"type":"array","maxItems":200,"items":r("Project")},"next":nullable("string")}},
        "HistoryCapture":{"type":"object","required":["id","name","state","size","media"],"properties":{"id":{"type":"string","format":"uuid"},"name":{"type":"string"},"state":{"enum":["saved","unavailable","not_saved"]},"size":nullable("integer"),"media":nullable("string")}},
        "HistoryMessages":{"type":"object","required":["thread","revision","data","next","captures"],"properties":{"captures":{"type":"array","maxItems":8,"items":r("Capture")},"thread":r("Summary"),"revision":{"type":"integer","minimum":1},"data":{"type":"array","maxItems":200,"items":r("Message")},"next":nullable("string")}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    for (path, id, schema, description) in [
        (
            "/v1/history",
            "listSavedChats",
            "List",
            "List your saved chats, newest first, or the chats of one shared project.",
        ),
        (
            "/v1/history/projects",
            "listHistoryProjects",
            "Projects",
            "List the projects your saved chats belong to.",
        ),
        (
            "/v1/history/{id}/messages",
            "readSavedMessages",
            "Messages",
            "Read one saved chat's messages, oldest first, a page at a time.",
        ),
        (
            "/v1/history/{id}/files/{file}",
            "readSavedFile",
            "",
            "Download a file attached to a saved chat.",
        ),
        (
            "/v1/history/{id}/captures/{capture}",
            "readSavedCapture",
            "",
            "Download a screen capture saved with a chat.",
        ),
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters.push(json!({"name":"id","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}));
        }
        if path.contains("{capture}") {
            parameters.push(json!({"name":"capture","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}));
        } else if path.contains("{file}") {
            parameters.push(json!({"name":"file","in":"path","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{32}$"}}));
        } else {
            parameters.push(json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":200,"default":100}}));
            parameters.push(json!({"name":"after","in":"query","description":"Last public ID from this same list. Empty filtered pages may still have next. ID-order discovery is not a snapshot; restart to find newly inserted earlier IDs.","schema":{"type":"string"}}));
            if schema == "Messages" {
                parameters.push(json!({"name":"revision","in":"query","description":"Required with after. A changed transcript returns 409; restart without after.","schema":{"type":"integer","minimum":1}}));
            }
            if schema == "List" {
                parameters.push(json!({"name":"query","in":"query","description":"Case-insensitive saved-title search. Continue next even when a matched page is empty; change the query only when restarting from the first page.","schema":{"type":"string","maxLength":200}}));
                parameters.push(json!({"name":"project","in":"query","description":"A currently authorized shared project ID. Omit for all owned chats, including archived.","schema":{"type":"string"}}));
            }
        }
        let success = if schema.is_empty() {
            json!({"description":"Saved attachment bytes. Current chat/project authorization is checked; missing saved bytes return 404.","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}})
        } else {
            json!({"description":"A read-only saved page. No hosts are contacted and no seen/send state changes.","content":{"application/json":{"schema":r(schema)}}})
        };
        let mut responses = json!({"200":success});
        for status in ["400", "401", "403", "404", "409", "429", "503"] {
            responses[status] = json!({"description":"Invalid page, signed out, denied, missing, changed, throttled, or unavailable. A failure is never an empty success.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        doc["paths"][path] = json!({"get":{"operationId":id,"description":description,"tags":["Saved history"],"security":[{"appBearer":[]}],"parameters":parameters,"responses":responses}});
    }
    doc["components"]["schemas"]["SavedPinOrder"] = json!({
        "type":"object","required":["revision","ids"],"properties":{
            "revision":{"type":"integer","minimum":0},
            "ids":{"type":"array","maxItems":4096,"uniqueItems":true,"items":{"type":"string","format":"uuid"}}
        }
    });
    let response = json!({"description":"Confirmed owner pin order.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/SavedPinOrder"}}}});
    doc["paths"]["/v1/history/pins"] = json!({
        "get":{"operationId":"readSavedPinOrder","tags":["Saved history"],"security":[{"appBearer":[]}],
            "description":"Read your saved pins. Discovery scans at most 10,000 chats in 200-row pages, keeps at most 4,096 pins, and times out after 10 seconds. Failure is not an empty order.",
            "responses":{"200":response.clone(),"401":{"description":"Signed out."},"503":{"description":"Discovery limit or timeout; retain the last known order."}}},
        "patch":{"operationId":"moveSavedPin","tags":["Saved history"],"security":[{"appBearer":[]}],
            "description":"Move one of your saved pins into an exact gap after removing that pin. Supply the last read revision and both neighbours (null at an edge). One owner-scoped CAS commits the order without changing chats. A stale revision or gap returns 409; refresh before retrying.",
            "requestBody":{"required":true,"content":{"application/json":{"schema":{
                "type":"object","required":["revision","id"],"additionalProperties":false,"properties":{
                    "revision":{"type":"integer","minimum":0},"id":{"type":"string","format":"uuid"},
                    "after":{"type":["string","null"],"format":"uuid"},"before":{"type":["string","null"],"format":"uuid"}
                }
            }}}},"responses":{"200":response,"400":{"description":"Invalid request."},"401":{"description":"Signed out."},"409":{"description":"pin_order_changed; no order was written."},"503":{"description":"Pinned chats could not be loaded; no order was written."}}}
    });
}
