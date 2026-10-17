//! The gated account connection and project-source contracts.

use serde_json::{Value, json};

use super::{API, API_CONNECT, API_DRIVE, API_GOOGLE, API_TOOL};
use crate::cloud::connections::{MAX_PROJECT_SOURCES, MAX_SOURCES, Source};

fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/AppConnection{name}")})
}

pub(crate) fn extend(doc: &mut Value) {
    let source = Source {
        project: "prj_0000000000000001".into(),
        integration: "google".into(),
        id: "folder-example-0001".into(),
        name: "Example folder".into(),
        kind: "folder".into(),
        added_at: 1,
    };
    let mut schemas = json!({
        "AppConnectionState":{"type":"string","enum":["connected","not_connected","reconnect","unavailable"]},
        "AppConnectionKind":{"type":"string","enum":["folder","document","spreadsheet","presentation","pdf","photo","video","file"]},
        "AppConnectionSource":{"type":"object","required":["id","object","project","integration","name","kind","link","added_at"],"properties":{
            "id":{"type":"string"},"object":{"const":"source"},"project":{"type":"string"},"integration":{"const":"google"},"name":{"type":"string","maxLength":120},"kind":r("Kind"),"link":{"type":"string","format":"uri"},"added_at":{"type":"string","format":"date-time"}
        },"examples":[super::sources::source_object(&source)]},
        "AppConnectionSources":{"type":"object","required":["project","google","data"],"properties":{
            "project":{"type":"string"},"google":r("State"),"data":{"type":"array","maxItems":MAX_PROJECT_SOURCES,"items":r("Source")}
        }},
        "AppConnectionAttach":{"type":"object","anyOf":[{"required":["link"],"properties":{"link":{"type":"string"}}},{"required":["id"],"properties":{"id":{"type":"string"}}}],"properties":{
            "link":{"type":["string","null"]},"id":{"type":["string","null"]}
        },"description":"A Google Drive link or ID. A non-null link takes precedence over id, including when the link is invalid. Unknown fields are ignored.","examples":[{"id":source.id}]},
        "AppConnectionSourceDeleted":{"type":"object","required":["id","object","project"],"properties":{"id":{"type":"string"},"object":{"const":"source.deleted"},"project":{"type":"string"}}},
        "AppConnectionDriveFile":{"type":"object","required":["id","object","name","kind","folder","link","modified_at"],"properties":{
            "id":{"type":"string"},"object":{"const":"drive_file"},"name":{"type":"string"},"kind":r("Kind"),"folder":{"type":"boolean"},"link":{"type":"string","format":"uri"},"modified_at":{"type":["string","null"]}
        }},
        "AppConnectionDrivePage":{"type":"object","required":["data","folder","query","next"],"properties":{
            "data":{"type":"array","items":r("DriveFile")},"folder":{"type":"string"},"query":{"type":["string","null"]},"next":{"type":["string","null"]}
        }},
        "AppConnectionFeatures":{"type":"object","required":[],"properties":{}},
        "AppConnectionGoogle":{"type":"object","required":["status","email","scopes","sheets","features","connected_at"],"properties":{
            "status":r("State"),"email":{"type":["string","null"]},"scopes":{"type":"array","items":{"type":"string"}},"sheets":{"type":"boolean"},"features":r("Features"),"connected_at":{"type":["string","null"],"format":"date-time"}
        }},
        "AppConnectionTool":{"type":"object","required":["id","address","policy","description","parameters"],"properties":{
            "id":{"type":"string"},"address":{"type":"string"},"policy":{"enum":["allow","require_approval","block"]},"description":{"type":"string"},"parameters":{"type":"object"}
        }},
        "AppConnectionStored":{"type":"object","required":["integration","name","identity","status","scopes","sheets","connected_at","reconnect","tools"],"properties":{
            "integration":{"type":"string"},"name":{"type":"string"},"identity":{"type":["string","null"]},"status":r("State"),"scopes":{"type":"array","items":{"type":"string"}},"sheets":{"type":"boolean"},"connected_at":{"type":"string","format":"date-time"},"reconnect":{"type":"boolean"},"tools":{"type":"array","items":r("Tool")}
        }},
        "AppConnectionList":{"type":"object","required":["available","google","connections","sources"],"properties":{
            "available":{"type":"boolean"},"google":r("Google"),"connections":{"type":"array","items":r("Stored")},"sources":{"type":"array","maxItems":MAX_SOURCES,"items":{"type":"object","required":["project","id","name","kind","link"],"properties":{"project":{"type":"string"},"id":{"type":"string"},"name":{"type":"string"},"kind":r("Kind"),"link":{"type":"string","format":"uri"}}}}
        },"description":"Stored status is not a live grant check. A later Drive request can discover a revoked grant. No refresh or access token is returned. Tools may include other separately qualified Google features."},
        "AppConnectionConnect":{"type":"object","properties":{
            "sheets":{"type":"boolean","default":false},"feature":{"type":["string","null"],"enum":["drive","sheets","gmail","gmail_compose","calendar","calendar_events",null]},"return_to":{"type":["string","null"],"description":"A same-site return path. Unsafe paths fall back to Settings, Connections."}
        },"description":"Empty or whitespace-only bodies are accepted. feature takes precedence over sheets. This only returns a consent link; it does not connect or grant access."},
        "AppConnectionConsent":{"type":"object","required":["object","integration","url","scopes","status","message"],"properties":{
            "object":{"const":"connection.consent"},"integration":{"const":"google"},"url":{"type":"string","format":"uri"},"scopes":{"type":"array","items":{"type":"string"}},"status":r("State"),"message":{"type":"string"}
        }},
        "AppConnectionDeleted":{"type":"object","required":["object","integration","deleted","sources_kept"],"properties":{
            "object":{"const":"connection.deleted"},"integration":{"const":"google"},"deleted":{"type":"boolean"},"sources_kept":{"type":"integer","minimum":0,"maximum":MAX_SOURCES}
        }},
        "AppConnectionToolSearch":{"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string"},"folder":{"type":["string","null"]}}},
        "AppConnectionToolListFolder":{"type":"object","additionalProperties":false,"required":["folder"],"properties":{"folder":{"type":"string"},"page_token":{"type":["string","null"]}}},
        "AppConnectionToolRead":{"type":"object","additionalProperties":false,"required":["file"],"properties":{"file":{"type":"string"}}},
        "AppConnectionToolArguments":{"oneOf":[r("ToolSearch"),r("ToolListFolder"),r("ToolRead")]},
        "AppConnectionToolFile":{"type":"object","required":["id","name","kind","link"],"properties":{"id":{"type":"string"},"name":{"type":"string"},"kind":r("Kind"),"link":{"type":"string","format":"uri"},"modified":{"type":"string"},"seconds":{"type":"integer","minimum":0}}},
        "AppConnectionToolResult":{"type":"object","required":["result","read"],"properties":{
            "result":{"oneOf":[{"type":"object","required":["files"],"properties":{"files":{"type":"array","items":r("ToolFile")},"next_page_token":{"type":"string"}}},{"type":"object","required":["file","format","text","truncated"],"properties":{"file":r("ToolFile"),"format":{"enum":["text","csv","description"]},"text":{"type":"string"},"truncated":{"type":"boolean"},"note":{"type":"string"}}}]},
            "read":{"type":"array","items":{"type":"object","required":["name","link"],"properties":{"name":{"type":"string"},"link":{"type":"string","format":"uri"}}}},"usage":{"type":"object","required":["input","output"],"properties":{"input":{"type":"integer","minimum":0},"output":{"type":"integer","minimum":0}}},"cost_usd":{"type":"number","minimum":0}
        }}
    });
    for (feature, _) in oa_connections::google::FEATURES {
        schemas["AppConnectionFeatures"]["required"]
            .as_array_mut()
            .unwrap()
            .push(json!(feature));
        schemas["AppConnectionFeatures"]["properties"][feature] = json!({"type":"boolean"});
    }
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    doc["components"]["securitySchemes"]["connectionBearer"] = json!({"type":"http","scheme":"bearer","description":"A current account sess_ session or oa_sk_ sign-in key. Cookies and oak_ keys are refused. Connections must also be enabled for this account by the Google availability access list; other accounts receive 404 not_found. Google consent and Drive access are additional requirements, never implied by project membership."});
    for (path, method, id, request, answer, description) in [
        (
            API,
            "get",
            "listAccountConnections",
            "",
            "List",
            "Read this account's stored connection metadata and sources. available describes server configuration, not Google permission or an access-token check. The response is not paginated.",
        ),
        (
            API_CONNECT,
            "post",
            "createGoogleConsentLink",
            "Connect",
            "Consent",
            "Return a browser consent link. Open it signed in as the same account. The finish step checks the account again. This does not automate consent; Google must separately allow the person to connect while app verification is pending.",
        ),
        (
            API_GOOGLE,
            "delete",
            "disconnectAccountGoogle",
            "",
            "Deleted",
            "Delete the local connection before asking Google to revoke its refresh token. Remote revocation is best effort; a successful response proves only local deletion. Existing project sources stay. Repeated removal succeeds with deleted=false. No idempotency key or transaction across Google and local storage is promised.",
        ),
        (
            API_DRIVE,
            "get",
            "browseAccountDrive",
            "",
            "DrivePage",
            "Browse the caller's own Drive. Folder listing asks Google for up to 50 entries and passes through next as an opaque page token; keep the same folder on subsequent pages. Empty or over-512-byte page values are ignored. q selects a trimmed 1–200-character search, optionally within folder; search ignores page, requests up to 50 matches, and returns next=null. Files and permissions can change between pages; this is not a snapshot.",
        ),
        (
            API_TOOL,
            "post",
            "runAccountDriveTool",
            "ToolArguments",
            "ToolResult",
            "This schema qualifies the Drive tools only. Choose arguments for the named tool: query and optional folder for search; folder and optional page_token for list_folder; file for read. Each call checks the stored grant and policy. Search requests up to 20 matches; list_folder up to 50 with next_page_token. Read may call a model for photos/videos and return usage/cost_usd; it is not a free-read guarantee. Gmail and Calendar tools on this same route have separate contracts.",
        ),
        (
            "/v1/projects/{id}/sources",
            "get",
            "listConnectedProjectSources",
            "",
            "Sources",
            "List the account's own project's stored Drive sources in attachment order. There is no pagination or live Drive permission check. Stored metadata remains visible after disconnect or grant revocation. Shared project members cannot use this source contract.",
        ),
        (
            "/v1/projects/{id}/sources",
            "post",
            "attachConnectedProjectSource",
            "Attach",
            "Source",
            "Attach a file or folder after checking it with the caller's own live Drive grant. Only the account's own project is accepted; shared members, including editors, are refused. Up to 20 sources per project and 50 per account. A duplicate returns 200 with the original metadata, name and added_at, even if Drive changed; a new attachment returns 201. A retry checks Drive again, so it can fail after an earlier successful write. No Idempotency-Key handling is offered.",
        ),
        (
            "/v1/projects/{id}/sources/{source}",
            "delete",
            "detachConnectedProjectSource",
            "",
            "SourceDeleted",
            "Remove stored metadata from the account's own project, without deleting or changing the Drive file. A live Google grant is not required. A repeated deletion returns 404. Account/project permission and the Google availability gate still apply.",
        ),
    ] {
        let mut operation = json!({"operationId":id,"tags":["Connected project sources"],"security":[{"connectionBearer":[]}],"description":description,"parameters":[],"responses":{}});
        let response = |schema: Value, description: &str| json!({"description":description,"content":{"application/json":{"schema":schema}}});
        operation["responses"]["200"] = response(r(answer), "The current result.");
        if id == "attachConnectedProjectSource" {
            operation["responses"]["201"] = response(r(answer), "A new source was attached.");
        }
        for (status, description) in [
            (
                "400",
                "invalid_request or invalid: malformed JSON, unsupported feature, Drive link or tool arguments.",
            ),
            (
                "401",
                "signed_out: missing, unsupported, expired or revoked account credential.",
            ),
            (
                "403",
                "project_role: shared project sources are unavailable; drive_forbidden: Google refused access.",
            ),
            (
                "404",
                "not_found: Connections is disabled for this account/server, the project is absent, the source is absent, or Google cannot find the file.",
            ),
            (
                "409",
                "google_not_connected, google_reconnect, full or ambiguous_name. A revoked refresh grant is marked reconnect for later requests.",
            ),
            (
                "422",
                "invalid_request or tool_failed: Drive validation, missing tool scope, denied policy, unsupported input, or tool execution failure. Tool failures use this status even for a failed downstream read.",
            ),
            (
                "429",
                "rate_limited: the project account service could not complete the request.",
            ),
            (
                "502",
                "google_unavailable or github_unavailable: Google or the project account service could not be reached.",
            ),
            (
                "503",
                "unavailable: connection configuration or account/storage service is unavailable.",
            ),
        ] {
            operation["responses"][status] = response(
                json!({"$ref":"#/components/schemas/AppAccountError"}),
                description,
            );
        }
        operation["responses"]["400"]["content"]["text/plain"] =
            json!({"schema":{"type":"string"}});
        if matches!(id, "createGoogleConsentLink" | "runAccountDriveTool") {
            operation["responses"]["413"] = json!({"description":"The framework refused an oversized request body.","content":{"text/plain":{"schema":{"type":"string"}}}});
        }
        if !request.is_empty() {
            operation["requestBody"] = json!({"required":id!="createGoogleConsentLink","content":{"application/json":{"schema":r(request)}}});
        }
        let parameters = operation["parameters"].as_array_mut().unwrap();
        for name in ["id", "source", "tool"] {
            if path.contains(&format!("{{{name}}}")) {
                let schema = if name == "tool" {
                    json!({"type":"string","enum":["drive_search","drive_list_folder","drive_read","drive.search","drive.list_folder","drive.read"]})
                } else {
                    json!({"type":"string"})
                };
                parameters.push(json!({"name":name,"in":"path","required":true,"schema":schema,"description":if name=="id" {"Own project ID or name:NAME. Duplicate exact names return 409 ambiguous_name."} else {"The source ID or supported Drive tool name."}}));
            }
        }
        if id == "browseAccountDrive" {
            for name in ["folder", "q", "page"] {
                parameters.push(json!({"name":name,"in":"query","schema":{"type":"string"}}));
            }
        }
        doc["paths"][path][method] = operation;
    }
}
