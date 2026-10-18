//! Public contracts for account chats and the existing computer sync protocol.
use serde_json::{Value, json};

fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/Thread{name}")})
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn nullable(kind: &str) -> Value {
    json!({"type":[kind,"null"]})
}
fn operation(
    doc: &mut Value,
    path: &str,
    method: &str,
    id: &str,
    description: &str,
    request: Option<Value>,
    status: &str,
    response: Option<Value>,
) {
    let mut parameters = Vec::new();
    for name in ["id", "session", "file", "ask", "name"] {
        if path.contains(&format!("{{{name}}}")) {
            parameters
                .push(json!({"name":name,"in":"path","required":true,"schema":{"type":"string"}}));
        }
    }
    let success = match response {
        Some(schema) => {
            json!({"description":"Success","content":{"application/json":{"schema":schema}}})
        }
        None => json!({"description":"Success; no response body"}),
    };
    let mut responses = json!({status:success});
    for code in [
        "400", "401", "403", "404", "409", "410", "413", "415", "422", "429", "503",
    ] {
        responses[code] = json!({"description":"Refusal. The error code distinguishes invalid input, missing authority/resource, changed/busy state, deleted sync sessions, size/content limits, secret screening, or unavailable service.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
    }
    let mut op = json!({"operationId":id,"tags":["Threads"],"description":format!("{description} Account session or sign-in key required; cookies and oak API keys do not authorize this operation."),"security":[{"appBearer":[]}],"parameters":parameters,"responses":responses});
    if let Some(schema) = request {
        op["requestBody"] =
            json!({"required":true,"content":{"application/json":{"schema":schema}}});
    }
    doc["paths"][path][method] = op;
}

pub(crate) fn extend(doc: &mut Value) {
    let string = json!({"type":"string"});
    let boolean = json!({"type":"boolean"});
    let uint = json!({"type":"integer","minimum":0});
    let role = json!({"enum":["user","assistant","tool","thinking"]});
    let surface = json!({"enum":["web","terminal","phone"]});
    let message = object(json!({"role":role,"text":string}), &["role", "text"]);
    let row = object(
        json!({"id":{"type":"string","format":"uuid"},"title":string,"line":string,"surface":surface,"computer":nullable("string"),"session":nullable("string"),"updated_unix":uint,"working":boolean,"online":boolean,"can_reply":boolean,"pinned":boolean,"project":nullable("string"),"messages":uint}),
        &[
            "id",
            "title",
            "line",
            "surface",
            "computer",
            "session",
            "updated_unix",
            "working",
            "online",
            "can_reply",
            "pinned",
            "project",
            "messages",
        ],
    );
    let thread = object(
        json!({"id":string,"object":{"const":"thread"},"title":string,"project":nullable("string"),"surface":surface,"computer":nullable("string"),"updated_at":{"type":"string","format":"date-time"},"messages":uint,"pinned":boolean,"archived":boolean,"started_by":object(json!({"name":string,"you":boolean}), &["name","you"])}),
        &[
            "id",
            "object",
            "title",
            "project",
            "surface",
            "computer",
            "updated_at",
            "messages",
            "pinned",
            "archived",
        ],
    );
    let reply = object(
        json!({"request_id":{"type":"string","format":"uuid"},"text":{"type":"string","minLength":1,"maxLength":4000},"files":array(string.clone()),"project":nullable("string")}),
        &["request_id", "text"],
    );
    let mut reply = reply;
    reply["additionalProperties"] = json!(false);
    let mut wire = message.clone();
    wire["additionalProperties"] = json!(false);
    wire["properties"]["step"] = json!({"anyOf":[{"type":"null"},{"type":"object","additionalProperties":false,"required":["name","summary","status"],"properties":{"name":{"type":"string"},"summary":{"type":"string"},"status":{"enum":["running","done","failed"]},"input":{"type":"string"},"output":{"type":"string"}}}]});
    let mut upload = object(
        json!({"computer":string,"title":string,"messages":{"type":"array","maxItems":4000,"items":wire},"repository":nullable("string"),"active_unix":nullable("integer")}),
        &["computer", "title", "messages"],
    );
    upload["additionalProperties"] = json!(false);
    let schemas = json!({
        "ThreadRow":row,"ThreadObject":thread,"ThreadReply":reply,"ThreadSyncUpload":upload,
        "ThreadComputer":object(json!({"name":string,"online":boolean,"seen_unix":nullable("integer"),"sync":{"enum":["all","local",null]}}), &["name","online","seen_unix","sync"]),
        "ThreadList":object(json!({"threads":array(r("Row")),"computers":array(r("Computer")),"total":uint,"more":boolean,"next_offset":nullable("integer")}), &["threads","computers","total","more","next_offset"]),
        "ThreadProjectList":object(json!({"data":array(r("Object")),"next":nullable("string")}), &["data","next"]),
        "ThreadRecent":object(json!({"thread":r("Row"),"messages":array(object(json!({"role":role,"text":string,"files":array(json!({"$ref":"#/components/schemas/HistoryFile"}))}), &["role","text"])),"earlier":uint,"waiting":uint}), &["thread","messages","earlier","waiting"]),
        "ThreadMessages":object(json!({"data":array(object(json!({"id":string,"object":{"const":"message"},"thread":string,"role":role,"content":array(object(json!({"type":{"const":"text"},"text":string}), &["type","text"])),"files":{"type":"null"},"generated_files":{"type":"null"},"artifacts":{"type":"null"}}), &["id","object","thread","role","content","files","generated_files","artifacts"])),"next":nullable("string")}), &["data","next"]),
        "ThreadAccepted":{"oneOf":[object(json!({"queued":{"const":true}}), &["queued"]),object(json!({"answering":{"const":true}}), &["answering"])]},
        "ThreadStarted":object(json!({"id":string,"answering":{"const":true}}), &["id","answering"]),
        "ThreadShare":object(json!({"shared":boolean,"url":nullable("string")}), &["shared","url"]),
        "ThreadSyncSaved":object(json!({"chat":string,"changed":boolean}), &["chat","changed"]),
        "ThreadSyncList":object(json!({"sessions":array(object(json!({"session":string,"deleted":boolean}), &["session","deleted"]))}), &["sessions"]),
        "ThreadSyncTaken":object(json!({"replies":array(object(json!({"id":string,"text":string}), &["id","text"])),"continued":array(message),"asks":array(object(json!({"id":string,"action":{"oneOf":[object(json!({"kind":{"const":"screenshot"}}), &["kind"]),object(json!({"kind":{"const":"pull"},"path":string}), &["kind","path"])]}}), &["id","action"]))}), &["replies","continued","asks"])
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    operation(
        doc,
        "/v1/threads",
        "get",
        "listThreads",
        "Without project: owned, nonarchived chats, pinned first then newest activity; offset pagination, limit defaults to 200 and clamps to 1..200, store discovery ceiling 10000. With project: current authorized project (ID or name:NAME; none means unfiled), after pagination oldest activity first, limit defaults to 100 and clamps to 1..200. These are different response envelopes; neither is a snapshot.",
        None,
        "200",
        Some(json!({"oneOf":[r("List"),r("ProjectList")]})),
    );
    for (name, schema, description) in [
        ("offset", uint.clone(), "Without project only"),
        ("limit", uint.clone(), "Clamped to 1..200"),
        (
            "after",
            string.clone(),
            "Project list only; last returned ID, unknown after value is 400 invalid_cursor",
        ),
        (
            "project",
            string.clone(),
            "Project ID, name:NAME, or none; changes envelope and order",
        ),
    ] {
        doc["paths"]["/v1/threads"]["get"]["parameters"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":name,"in":"query","description":description,"schema":schema}));
    }
    operation(
        doc,
        "/v1/threads",
        "post",
        "startThread",
        "Starts an account-store cloud chat using the existing server runtime and caller allowance. request_id names the chat; retry the same UUID after a lost response. Existing chat returns 200, first start 201. Project is allowed here only. Files must be uploaded under request_id first. Follow the request through /v1/threads/{id}/events and cancel that request through /v1/threads/{id}/stop. Browser HTML forms/events remain compatible.",
        Some(r("Reply")),
        "201",
        Some(r("Started")),
    );
    doc["paths"]["/v1/threads"]["post"]["responses"]["200"] =
        doc["paths"]["/v1/threads"]["post"]["responses"]["201"].clone();
    operation(
        doc,
        "/v1/threads/{id}",
        "get",
        "readThread",
        "Owner or current shared-project member. Newest 200 nonempty messages; earlier counts omitted messages. Use read-only /v1/history for revision-pinned rich transcript pages.",
        None,
        "200",
        Some(r("Recent")),
    );
    operation(
        doc,
        "/v1/threads/{id}",
        "patch",
        "moveThread",
        "Owner only. Send at least one of project, pinned, archived, title; an empty body refuses 400 invalid_request. project: the target requires current project editor/owner role; null removes filing; moving resets the branch. pinned: true pins after every earlier pin, false unpins. archived: true archives the chat and removes the cloud computers that ran it, false restores it. title: trimmed, one line, 1..120 characters, using the web rename rules. Revision compare-and-swap retries three times; conflicts return 503.",
        Some(
            json!({"type":"object","minProperties":1,"properties":{"project":nullable("string"),"pinned":{"type":"boolean"},"archived":{"type":"boolean"},"title":{"type":"string","minLength":1,"maxLength":120}}}),
        ),
        "200",
        Some(r("Object")),
    );
    operation(
        doc,
        "/v1/threads/{id}",
        "delete",
        "deleteThread",
        "Owner only. Deletes permanently; a running answer refuses 409 busy, a race refuses 409 changed. A repeated delete returns 404, not another effect.",
        None,
        "204",
        None,
    );
    operation(
        doc,
        "/v1/threads/{id}/messages",
        "get",
        "listThreadMessages",
        "Current owner/shared-project access, oldest first, limit defaults to 100 and clamps to 1..200; after is msg_INDEX. Unknown after value returns 400 invalid_cursor. Unversioned text-only pages; files/generated_files/artifacts are null. Use /v1/history for rich revision-pinned reads.",
        None,
        "200",
        Some(r("Messages")),
    );
    for (name, schema) in [("limit", uint.clone()), ("after", string.clone())] {
        doc["paths"]["/v1/threads/{id}/messages"]["get"]["parameters"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":name,"in":"query","schema":schema}));
    }
    operation(
        doc,
        "/v1/threads/{id}/messages",
        "post",
        "replyToThread",
        "Trimmed nonempty text, at most 4000 characters; secrets refuse 422 secret. Reuse request_id on retry. Terminal queues at most four replies for an online computer, remembers 16 reply IDs; offline/busy return 409. Phone chats refuse 409 phone. Web chats use the existing server turn and caller allowance; busy 409, unavailable 503. project must be absent/null. Shared-project editor/owner can send words to a web chat using their own allowance; viewers refuse 403 project_role, shared terminal/files refuse 400 words_only. This operation accepts a turn; it does not return generated output or an SSE stream.",
        Some(r("Reply")),
        "202",
        Some(r("Accepted")),
    );
    for method in ["get", "post"] {
        operation(
            doc,
            "/v1/threads/{id}/share",
            method,
            if method == "get" {
                "readThreadShare"
            } else {
                "setThreadShare"
            },
            "Owner only, otherwise 404 unknown. A project set to only_me refuses 409 not_shareable. The URL is a read-only bearer link to words, never attachments. Unsharing or deletion revokes it; resharing creates a new token. GET does not create a share.",
            if method == "post" {
                Some(
                    json!({"type":"object","required":["shared"],"additionalProperties":false,"properties":{"shared":{"type":"boolean"}}}),
                )
            } else {
                None
            },
            "200",
            Some(r("Share")),
        );
    }
    operation(
        doc,
        "/v1/threads/{id}/files",
        "post",
        "addThreadFile",
        "Owner-only raw upload for a cloud chat or a not-yet-started request UUID; at most 10 MiB (text 512 KiB), 32 files per chat, four per message. Supports images, PDF, text and supported CAD files. Returns formatted size, not byte count. A retry can create another pending file; keep the returned ID rather than blindly retrying. File validation errors use code file; pending limit is 409 full.",
        None,
        "201",
        Some(object(
            json!({"id":string,"name":string,"kind":{"enum":["png","jpeg","gif","webp","pdf","text","step","stl"]},"size":string,"url":string}),
            &["id", "name", "kind", "size", "url"],
        )),
    );
    doc["paths"]["/v1/threads/{id}/files"]["post"]["parameters"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"name","in":"query","schema":{"type":"string"}}));
    doc["paths"]["/v1/threads/{id}/files"]["post"]["requestBody"] = json!({"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary","maxLength":10485760}}}});
    operation(
        doc,
        "/v1/threads/{id}/files/{file}",
        "get",
        "readThreadFile",
        "Owner-only attachment bytes. Shared project access to transcript words does not grant this route. Missing bytes return 404 not_found.",
        None,
        "200",
        None,
    );
    doc["paths"]["/v1/threads/{id}/files/{file}"]["get"]["responses"]["200"] = json!({"description":"Attachment bytes with validated media type","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}});
    operation(
        doc,
        "/v1/threads/{id}/files/{file}",
        "delete",
        "removeUnsentThreadFile",
        "Owner-only removal of a pending attachment; sent attachments refuse. Success is 204; a repeated removal returns a file refusal.",
        None,
        "204",
        None,
    );
    operation(
        doc,
        "/v1/threads/synced",
        "get",
        "listSyncedThreads",
        "Account-owned sync session IDs and deletion tombstones, not transcript history. Older /coder/sessions alias remains supported.",
        None,
        "200",
        Some(r("SyncList")),
    );
    operation(
        doc,
        "/v1/threads/synced/{session}",
        "put",
        "syncThread",
        "Upsert under authenticated account and session (ASCII letters/digits/_/-, 1..128 bytes). Maximum body 8 MiB, 4000 messages, 256 KiB/message; invalid tool details are omitted (32 KiB input/output). Secret screening refuses 422. Repeated same content returns changed:false; website deletion returns 410 deleted. Does not execute a turn. Older /coder/sessions/{session} alias is unchanged.",
        Some(r("SyncUpload")),
        "200",
        Some(r("SyncSaved")),
    );
    operation(
        doc,
        "/v1/threads/synced/{session}",
        "delete",
        "forgetSyncedThread",
        "Account-owned session removal. Safe to repeat; deleted boolean; older /coder/sessions/{session} alias is unchanged.",
        None,
        "200",
        Some(object(json!({"deleted":boolean}), &["deleted"])),
    );
    operation(
        doc,
        "/v1/threads/synced/{session}/status",
        "post",
        "setSyncedThreadStatus",
        "Sets working status without running a turn; 404 unknown, 410 deleted. Older /coder/sessions/{session}/status alias remains.",
        Some(
            json!({"type":"object","required":["working"],"additionalProperties":false,"properties":{"working":{"type":"boolean"}}}),
        ),
        "200",
        Some(r("SyncSaved")),
    );
    operation(
        doc,
        "/v1/threads/synced/{session}/replies",
        "post",
        "takeSyncedThreadReplies",
        "Destructive take: queued replies and asks are delivered at most once. A lost response cannot be retried for redelivery; this is not exactly-once execution. Continued user/assistant messages are oldest first, bounded to 64, each at most 256 KiB. The computer executes work under its own authority. Older /coder/sessions/{session}/replies alias remains.",
        None,
        "200",
        Some(r("SyncTaken")),
    );
    operation(
        doc,
        "/v1/threads/synced/{session}/captures/{ask}",
        "put",
        "saveSyncedThreadCapture",
        "Raw PNG/file bytes for a previously taken ask; at most 15 MiB. Older /coder/sessions/{session}/captures/{ask} alias remains.",
        None,
        "200",
        Some(r("SyncSaved")),
    );
    doc["paths"]["/v1/threads/synced/{session}/captures/{ask}"]["put"]["requestBody"] = json!({"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary","maxLength":15728640}}}});
    operation(
        doc,
        "/v1/threads/synced/{session}/captures/{ask}/failed",
        "post",
        "failSyncedThreadCapture",
        "Records a failed capture, no execution. Older /coder/sessions/{session}/captures/{ask}/failed alias remains.",
        Some(
            json!({"type":"object","required":["message"],"additionalProperties":false,"properties":{"message":{"type":"string"}}}),
        ),
        "200",
        Some(r("SyncSaved")),
    );
    for method in ["get", "put"] {
        operation(
            doc,
            "/v1/computers/{name}/sync",
            method,
            if method == "get" {
                "readComputerSyncChoice"
            } else {
                "setComputerSyncChoice"
            },
            "Account-owned sync preference. null on GET means not chosen. PUT accepts all or local. Older /coder/sync alias accepts the computer query/body field. This choice does not itself upload transcripts or start execution.",
            if method == "put" {
                Some(
                    json!({"type":"object","required":["choice"],"additionalProperties":false,"properties":{"choice":{"enum":["all","local"]}}}),
                )
            } else {
                None
            },
            "200",
            Some(object(
                json!({"choice":{"enum":["all","local",null]}}),
                &["choice"],
            )),
        );
    }
    operation(
        doc,
        "/v1/computers/check-in",
        "post",
        "checkInSyncComputer",
        "Records account-owned presence; returns waiting sessions and sync choice. Does not run them. Older /coder/check-in alias remains.",
        Some(
            json!({"type":"object","required":["computer"],"additionalProperties":false,"properties":{"computer":{"type":"string"}}}),
        ),
        "200",
        Some(object(
            json!({"waiting":array(string),"sync":{"enum":["all","local",null]}}),
            &["waiting", "sync"],
        )),
    );
}

#[cfg(test)]
#[path = "thread_openapi_tests.rs"]
mod tests;
