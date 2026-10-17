//! OpenAPI from the request and resource types used by the Labs handlers.
use crate::*;
use schemars::JsonSchema;
use serde_json::{Value, json};

fn schema<T: JsonSchema>() -> Value {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| {
        s.inline_subschemas = true;
        s.meta_schema = None;
    });
    serde_json::to_value(settings.into_generator().into_root_schema_for::<T>()).expect("Lab schema")
}
pub const OPERATIONS: &[(&str, &str, &str, Option<&str>, &str)] = &[
    (
        "/v1/labs/workers",
        "post",
        "enrollLabWorker",
        Some("WorkerEnroll"),
        "WorkerEnrolled",
    ),
    (
        "/v1/labs/workers/{id}",
        "delete",
        "revokeLabWorker",
        None,
        "WorkerRevoked",
    ),
    (
        "/v1/labs/workers/poll",
        "post",
        "pollLabWorker",
        Some("WorkerPoll"),
        "WorkerAvailable",
    ),
    (
        "/v1/labs/workers/start",
        "post",
        "startLabWorkerJob",
        Some("WorkerAttempt"),
        "WorkerWork",
    ),
    (
        "/v1/labs/workers/heartbeat",
        "post",
        "heartbeatLabWorkerJob",
        Some("WorkerAttempt"),
        "WorkerWork",
    ),
    (
        "/v1/labs/workers/finish",
        "post",
        "finishLabWorkerJob",
        Some("WorkerFinish"),
        "JobState",
    ),
    (
        "/v1/labs/jobs/{id}/usage",
        "get",
        "getLabJobUsage",
        None,
        "WorkerUsage",
    ),
    ("/v1/labs/kinds", "get", "listLabKinds", None, "Kinds"),
    (
        "/v1/labs/kinds/{kind}/tools",
        "get",
        "listLabTools",
        None,
        "Tools",
    ),
    (
        "/v1/labs/kinds/{kind}/tools/{name}",
        "post",
        "callLabTool",
        Some("ToolCall"),
        "ToolResult",
    ),
    (
        "/v1/labs/documents",
        "get",
        "listLabDocuments",
        None,
        "Documents",
    ),
    (
        "/v1/labs/documents",
        "post",
        "createLabDocument",
        Some("CreateDocument"),
        "Version",
    ),
    (
        "/v1/labs/documents/{id}",
        "get",
        "getLabDocument",
        None,
        "DocumentMeta",
    ),
    (
        "/v1/labs/documents/{id}",
        "delete",
        "deleteLabDocument",
        None,
        "Deleted",
    ),
    (
        "/v1/labs/documents/{id}/versions",
        "get",
        "listLabVersions",
        None,
        "Versions",
    ),
    (
        "/v1/labs/documents/{id}/versions",
        "post",
        "createLabVersion",
        Some("EditDocument"),
        "Version",
    ),
    (
        "/v1/labs/documents/{id}/versions/{n}",
        "get",
        "getLabVersion",
        None,
        "Version",
    ),
    (
        "/v1/labs/documents/{id}/versions/{n}/card",
        "get",
        "getLabCard",
        None,
        "Card",
    ),
    (
        "/v1/labs/documents/{id}/notes",
        "get",
        "listLabNotes",
        None,
        "Notes",
    ),
    (
        "/v1/labs/documents/{id}/notes",
        "post",
        "addLabNote",
        Some("AddNote"),
        "Saved",
    ),
    (
        "/v1/labs/documents/{id}/events",
        "get",
        "readLabEvents",
        None,
        "Events",
    ),
    ("/v1/labs/jobs", "get", "listLabJobs", None, "Jobs"),
    (
        "/v1/labs/jobs",
        "post",
        "createLabJob",
        Some("CreateJob"),
        "JobState",
    ),
    ("/v1/labs/jobs/{id}", "get", "getLabJob", None, "JobState"),
    (
        "/v1/labs/jobs/{id}",
        "delete",
        "cancelLabJob",
        None,
        "JobState",
    ),
    (
        "/v1/labs/artifacts/{id}",
        "get",
        "getLabArtifact",
        None,
        "ArtifactLink",
    ),
    (
        "/v1/labs/artifacts/{id}/content",
        "get",
        "readLabArtifact",
        None,
        "Bytes",
    ),
    (
        "/v1/labs/uploads",
        "post",
        "uploadLabSource",
        Some("Upload"),
        "Artifact",
    ),
];

pub fn document() -> Value {
    let mut doc = json!({"openapi":"3.1.0","info":{"title":"OpenAgents Labs API","version":"1.0.0"},"servers":[{"url":"https://api.openagents.com"}],"paths":{},"components":{"securitySchemes":{"appBearer":{"type":"http","scheme":"bearer"}},"schemas":{}}});
    extend(&mut doc);
    doc
}
pub fn extend(doc: &mut Value) {
    let schemas = [
        (
            "WorkerRevoked",
            json!({"type":"object","properties":{"revoked":{"const":true}},"required":["revoked"],"additionalProperties":false}),
        ),
        ("WorkerEnroll", schema::<lab_jobs::protocol::Enroll>()),
        ("WorkerEnrolled", schema::<lab_jobs::authority::Enrolled>()),
        ("WorkerPoll", schema::<lab_jobs::protocol::Poll>()),
        ("WorkerAvailable", schema::<lab_jobs::protocol::Available>()),
        ("WorkerAttempt", schema::<lab_jobs::protocol::Attempt>()),
        ("WorkerWork", schema::<lab_jobs::protocol::Work>()),
        ("WorkerFinish", schema::<lab_jobs::protocol::Finish>()),
        ("WorkerUsage", schema::<Vec<lab_jobs::authority::Usage>>()),
        ("CreateDocument", schema::<CreateDocument>()),
        ("EditDocument", schema::<EditDocument>()),
        ("CreateJob", schema::<CreateJob>()),
        ("Upload", schema::<Upload>()),
        ("AddNote", schema::<AddNote>()),
        ("ToolCall", schema::<ToolCall>()),
        ("DocumentMeta", schema::<lab_kit::DocumentMeta>()),
        ("Version", schema::<lab_kit::Version>()),
        ("Card", schema::<lab_kit::Card>()),
        ("JobState", schema::<lab_kit::JobState>()),
        ("Artifact", schema::<lab_kit::Artifact>()),
        ("ArtifactLink", schema::<ArtifactLink>()),
        ("Kinds", schema::<Data<Vec<lab_kit::KindManifest>>>()),
        ("Tools", schema::<Data<Vec<lab_kit::ToolDecl>>>()),
        ("Documents", schema::<List<lab_kit::DocumentMeta>>()),
        ("Versions", schema::<List<lab_kit::Version>>()),
        ("Jobs", schema::<List<lab_kit::JobState>>()),
        ("Notes", schema::<List<lab_kit::Note>>()),
        ("Events", schema::<List<lab_kit::api::Event>>()),
        ("Deleted", schema::<Deleted>()),
        ("Saved", schema::<Saved>()),
        ("ToolResult", schema::<ToolResult>()),
        ("Error", schema::<ErrorBody>()),
        ("Bytes", json!({"type":"string","format":"binary"})),
    ];
    for (name, schema) in schemas {
        doc["components"]["schemas"][format!("Lab{name}")] = schema;
    }
    let reference = |name: &str| json!({"$ref":format!("#/components/schemas/Lab{name}")});
    for &(path, method, operation, body, result) in OPERATIONS {
        let mut parameters = vec![];
        for name in ["id", "kind", "name", "n"] {
            if path.contains(&format!("{{{name}}}")) {
                parameters.push(json!({"name":name,"in":"path","required":true,"schema":if name=="n" {json!({"type":"integer","minimum":1})}else{json!({"type":"string","maxLength":256})}}));
            }
        }
        if body.is_none() && operation != "revokeLabWorker" {
            parameters.push(json!({"name":"space","in":"query","schema":{"type":"string","default":"me","description":"me or project:PROJECT_ID. Ownership comes from authentication."}}));
        }
        if method == "get"
            && [
                "listLabDocuments",
                "listLabVersions",
                "listLabJobs",
                "listLabNotes",
                "readLabEvents",
            ]
            .contains(&operation)
        {
            parameters.push(json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":100,"default":50}}));
            parameters
                .push(json!({"name":"after","in":"query","schema":{"type":"integer","minimum":0}}));
        }
        if operation == "listLabJobs" {
            parameters.push(
                json!({"name":"document","in":"query","required":true,"schema":{"type":"string"}}),
            );
        }
        if operation == "listLabDocuments" {
            parameters.push(json!({"name":"kind","in":"query","schema":{"type":"string"}}));
        }
        if operation == "readLabEvents" {
            parameters
                .push(json!({"name":"since","in":"query","schema":{"type":"integer","minimum":0}}));
            parameters
                .push(json!({"name":"Last-Event-ID","in":"header","schema":{"type":"string"}}));
        }
        if operation == "readLabArtifact" {
            parameters.push(json!({"name":"Range","in":"header","schema":{"type":"string","example":"bytes=0-99"}}));
        }
        if operation == "createLabJob" || operation == "callLabTool" {
            parameters.push(json!({"name":"Idempotency-Key","in":"header","required":operation=="createLabJob","schema":{"type":"string","minLength":1,"maxLength":128}}));
        }
        let worker_only = [
            "pollLabWorker",
            "startLabWorkerJob",
            "heartbeatLabWorkerJob",
            "finishLabWorkerJob",
        ]
        .contains(&operation);
        let status = if method == "post"
            && operation != "callLabTool"
            && !worker_only
            && operation != "enrollLabWorker"
        {
            "201"
        } else {
            "200"
        };
        let media = if result == "Bytes" {
            "application/octet-stream"
        } else {
            "application/json"
        };
        let mut op = json!({"operationId":operation,"tags":["Labs"],"security":[{"appBearer":[]}],"parameters":parameters,"description":"Authorizes the caller and space on every request. Experimental kinds use the Computers account gate. Reads never start jobs. Pages and payloads are bounded. Source uploads are capped at 512 KiB until resumable uploads are integrated.","responses":{status:{"description":"Private, no-store result.","content":{media:{"schema":reference(result)}}}}});
        if worker_only {
            doc["components"]["securitySchemes"]["labWorkerBearer"] = json!({"type":"http","scheme":"bearer","description":"A revocable job-only labw_ key for one named space. Account sign-in keys are not accepted."});
            op["security"] = json!([{"labWorkerBearer":[]}]);
            op["description"] = json!(
                "Outbound job-worker operation. Checks current sign-in, account, project membership, kind access and lease before each operation. Requests cannot select another space or executable."
            );
        }
        for code in [
            "400", "401", "402", "403", "404", "409", "413", "422", "429", "503",
        ] {
            op["responses"][code] = json!({"description":"Stable error code and plain-language message.","content":{"application/json":{"schema":reference("Error")}}});
        }
        for status in ["429", "503"] {
            op["responses"][status]["headers"]["Retry-After"] = json!({"schema":{"type":"integer","minimum":1},"description":"Seconds before retrying."});
        }
        if let Some(body) = body {
            op["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":reference(body)}}});
        }
        if operation == "readLabArtifact" {
            op["responses"]["206"] = op["responses"]["200"].clone();
            op["responses"]["416"] = op["responses"]["400"].clone();
        }
        if operation == "readLabEvents" {
            op["responses"]["200"]["content"]["text/event-stream"] = json!({"schema":{"type":"string"},"description":"changes events contain ordered batches; reconnect with Last-Event-ID. Authorization is rechecked before each batch; streams end after five minutes."});
        }
        doc["paths"][path][method] = op;
    }
}
