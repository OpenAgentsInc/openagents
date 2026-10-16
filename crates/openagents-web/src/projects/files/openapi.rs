//! Project file contracts. Chat attachments, computer files, and vault transport are separate.
use super::*;

fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/AppFile{name}")})
}

pub(crate) fn extend(doc: &mut Value) {
    let file = FileInfo {
        schema: FILE_SCHEMA.into(),
        id: "file_000000000000000000000001".into(),
        filename: "example.txt".into(),
        mime_type: "text/plain".into(),
        size_bytes: 12,
        sha256: format!("{:x}", Sha256::digest(b"example file")),
        created_unix: 1,
        project: Some("prj_0000000000000001".into()),
        chunks: None,
    };
    let note = DocumentInfo {
        schema: DOCUMENT_SCHEMA.into(),
        id: "pdoc_000000000000000000000001".into(),
        title: "Example note".into(),
        text: "Example text.".into(),
        created_unix: 1,
    };
    let mut schemas = json!({
        "AppFileObject":{"type":"object","required":["id","object","filename","mime_type","size_bytes","sha256","created_at","stored_as","project"],"properties":{
            "id":{"type":"string","pattern":"^file_[0-9a-f]{24}$"},"object":{"const":"file"},"filename":{"type":"string"},"mime_type":{"type":"string"},"size_bytes":{"type":"integer","minimum":1},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"created_at":{"type":"string","format":"date-time"},"stored_as":{"const":"original"},"project":{"type":["string","null"]}
        },"examples":[file.shown()]},
        "AppFileAttachment":{"type":"object","required":["id","object","type","protection","filename","mime_type","size_bytes","sha256","created_at"],"properties":{
            "id":{"type":"string","pattern":"^file_[0-9a-f]{24}$"},"object":{"const":"attachment"},"type":{"const":"file"},"protection":{"const":"standard"},"filename":{"type":"string"},"mime_type":{"type":"string"},"size_bytes":{"type":"integer","minimum":1},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"created_at":{"type":"string","format":"date-time"}
        },"examples":[file.attachment()]},
        "AppFileDocumentAttachment":{"type":"object","required":["id","object","type","protection","title","size_bytes","created_at"],"properties":{
            "id":{"type":"string","pattern":"^pdoc_[0-9a-f]{24}$"},"object":{"const":"attachment"},"type":{"const":"document"},"protection":{"const":"standard"},"title":{"type":"string","maxLength":MAX_TITLE_CHARS},"size_bytes":{"type":"integer","minimum":1,"maximum":MAX_DOCUMENT_BYTES},"created_at":{"type":"string","format":"date-time"}
        },"examples":[note.attachment()]},
        "AppFileVaultAttachment":{"type":"object","required":["id","object","type","protection","size_bytes","created_at"],"properties":{
            "id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"object":{"const":"attachment"},"type":{"const":"vault"},"protection":{"const":"only_me"},"size_bytes":{"type":"integer","minimum":0},"created_at":{"type":"string","format":"date-time"},"filename":false,"mime_type":false,"title":false
        },"description":"Encrypted vault object metadata, without a filename or MIME type. Open the object through the separate vault contract; these file-content routes cannot decrypt or download it."},
        "AppFileSourceAttachment":{"type":"object","required":["id","object","type","integration","kind","name","link","created_at"],"properties":{
            "id":{"type":"string"},"object":{"const":"attachment"},"type":{"const":"source"},"integration":{"type":"string"},"kind":{"type":"string"},"name":{"type":"string"},"link":{"type":"string","format":"uri"},"created_at":{"type":"string","format":"date-time"}
        },"description":"Stored connected-source metadata only. Source creation, grants and content reads have a separate contract."},
        "AppFileAttachmentResult":{"oneOf":[r("Attachment"),r("DocumentAttachment"),r("VaultAttachment")]},
        "AppFileAttachments":{"type":"object","required":["data","next","protection"],"properties":{
            "data":{"type":"array","maxItems":200,"items":{"oneOf":[r("Attachment"),r("DocumentAttachment"),r("VaultAttachment"),r("SourceAttachment")]}},"next":{"type":["string","null"]},"protection":{"enum":["standard","only_me"]}
        }},
        "AppFileNewDocument":{"type":"object","required":["title","text"],"properties":{
            "title":{"type":"string","description":"Control characters are removed, then the title is truncated to 200 characters and trimmed; it must remain nonempty."},"text":{"type":"string","minLength":1,"maxLength":MAX_DOCUMENT_BYTES,"description":"Nonblank text, at most 1 MiB in UTF-8 bytes. Credential-like text may be refused."}
        }},
        "AppFileAttach":{"type":"object","additionalProperties":false,"properties":{
            "file":{"type":["string","null"],"pattern":"^file_[0-9a-f]{24}$"},"document":{"anyOf":[r("NewDocument"),{"type":"null"}]},"vault":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"},"source":{"type":"null","description":"Null is ignored. Non-null sources are refused here; use the source routes."}
        },"oneOf":[{"required":["file"],"properties":{"file":{"type":"string"}}},{"required":["document"],"properties":{"document":{"type":"object"}}},{"required":["vault"],"properties":{"vault":{"type":"string"}}}],"examples":[{"document":{"title":"Example note","text":"Example text."}}]},
        "AppFileDeleted":{"type":"object","required":["id","object"],"properties":{"id":{"type":"string"},"object":{"const":"file.deleted"}}},
        "AppFileAttachmentDeleted":{"type":"object","required":["id","object"],"properties":{"id":{"type":"string"},"object":{"const":"attachment.deleted"}}},
        "AppFileUploadRequest":{"type":"object","additionalProperties":false,"required":["request_id","project","filename","size_bytes","sha256"],"properties":{
            "request_id":{"type":"string","pattern":"^[0-9a-f]{64}$","description":"Use the same request ID when retrying or resuming an upload after a restart."},"project":{"type":"string"},"filename":{"type":"string"},"size_bytes":{"type":"integer","minimum":1},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}
        }},
        "AppFileUpload":{"type":"object","required":["id","object","file","project","size_bytes","sha256","part_bytes","uploaded_bytes","next_part","expires_at","complete"],"properties":{
            "id":{"type":"string","pattern":"^upl_[0-9a-f]{24}$"},"object":{"const":"file.upload"},"file":{"type":"string","pattern":"^file_[0-9a-f]{24}$"},"project":{"type":"string"},"size_bytes":{"type":"integer","minimum":1},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"part_bytes":{"type":"integer","const":uploads::PART_BYTES},"uploaded_bytes":{"type":"integer","minimum":0},"next_part":{"type":"integer","minimum":0},"expires_at":{"type":"integer","minimum":0,"description":"Unix seconds; not an RFC 3339 string. Completed or finalizing uploads can be inspected after this time, subject to current permissions and limits."},"complete":{"type":"boolean"}
        }},
        "AppFileUploadDeleted":{"type":"object","required":["id","object"],"properties":{"id":{"type":"string"},"object":{"const":"file.upload.deleted"}}}
    });
    let mut document = schemas["AppFileDocumentAttachment"].clone();
    document["properties"]["object"]["const"] = json!("document");
    document["properties"]["project"] = json!({"type":"string"});
    document["properties"]["text"] = json!({"type":"string"});
    document["required"]
        .as_array_mut()
        .unwrap()
        .extend([json!("project"), json!("text")]);
    let mut example = note.attachment();
    example["object"] = json!("document");
    example["project"] = json!("prj_0000000000000001");
    example["text"] = json!(note.text);
    document["examples"] = json!([example]);
    schemas["AppFileDocument"] = document;
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    doc["components"]["securitySchemes"]["projectFileBearer"] = json!({"type":"http","scheme":"bearer","description":"A live account-service sess_ session or oa_sk_ sign-in key. Cookies and oak_ keys are not accepted. File requests revalidate the session; attached files also require current project membership and, for mutations, editor/owner rights, even without the optional project query."});
    for (path, method, id, request, answer, status, description) in [
        (
            "/v1/files",
            "post",
            "uploadProjectFile",
            "binary",
            "Object",
            "201",
            "Upload an unattached standard project file. purpose defaults to project; any other purpose is refused. The ordinary default is 10 MiB; configured/admin limits apply, with a hard 16 MiB direct-body ceiling. Empty files are refused. Names are cleaned to 120 characters; bytes stay original. No idempotency deduplication. Attaching later is separate and can fail, leaving this upload to delete. Never use this route for plaintext intended for only_me protection; use encrypted vault transport.",
        ),
        (
            "/v1/files/{id}",
            "get",
            "getProjectFile",
            "",
            "Object",
            "200",
            "Read project-file metadata from this account, or with project from that project's storage. Shared members normally need project. A storage account cannot bypass current roles after ownership transfer or removal. Chat attachment IDs are a separate transport.",
        ),
        (
            "/v1/files/{id}",
            "delete",
            "deleteProjectFile",
            "",
            "Deleted",
            "200",
            "Delete a standard project file and its attachment, chunk storage and quota reservation. Current project editor/owner rights are required for attached files. A repeated deletion returns 404; partial cleanup failures return an error, so inspect state before retrying.",
        ),
        (
            "/v1/files/{id}/content",
            "get",
            "downloadProjectFile",
            "",
            "binary",
            "200",
            "Download the entire original file, with its MIME type, attachment filename and a SHA-256 checksum header. This header uses hex, not base64. Large files stream; a later part failure can terminate an already-started response. Verify size and SHA-256 before replacing a local destination. No byte-range contract is offered.",
        ),
        (
            "/v1/projects/{project}/attachments",
            "get",
            "listProjectAttachments",
            "",
            "Attachments",
            "200",
            "List current files, documents, vault metadata and stored connected-source metadata, oldest first with ID tie breaks. Limit defaults to 100 and clamps to 1–200; after must still exist in this list. A project holds at most 200 file/document/vault attachments. Deleted vault objects are omitted. Protection describes the default for new attachments; changing it does not encrypt existing standard files.",
        ),
        (
            "/v1/projects/{project}/attachments",
            "post",
            "attachProjectFileOrDocument",
            "Attach",
            "AttachmentResult",
            "201",
            "Current editor or owner; send exactly one non-null file, document or vault ID. Direct sources are refused with use_the_sources_page. Standard files/documents cannot be added to only_me projects. Vault attachment needs the project's storage account and an existing encrypted object. Documents are new on every request. Known file/vault attachments can return 200, subject to current permissions and capacity; this is not a general mutation deduplication contract. An editor's uploaded standard file moves into the project's storage account, transferring its storage reservation.",
        ),
        (
            "/v1/projects/{project}/attachments/{attachment}",
            "delete",
            "deleteProjectAttachment",
            "",
            "AttachmentDeleted",
            "200",
            "Current editor or owner. Remove a file/document/vault attachment. A standard file and its bytes are deleted; removing a vault attachment does not delete the encrypted vault object. Sources require their own delete route. Missing attachments return 404.",
        ),
        (
            "/v1/projects/{project}/documents/{document}",
            "get",
            "getProjectDocument",
            "",
            "Document",
            "200",
            "Read a document's text and metadata under current project membership. This does not execute a model or modify the document.",
        ),
        (
            "/v1/files/uploads",
            "post",
            "startProjectFileUpload",
            "UploadRequest",
            "Upload",
            "201",
            "Start a resumable standard file upload. Creator must currently edit the project. request_id is scoped to the creator and resolved project; retries with the same cleaned filename, size and SHA-256 return the existing upload (200). Conflicting reuse returns 409. Reserves the complete size against the project's storage account, not a payment. File/account limits and the default seven-day expiry are configured by the service, not by a caller-supplied admin flag.",
        ),
        (
            "/v1/files/uploads/{upload}",
            "get",
            "getProjectFileUpload",
            "",
            "Upload",
            "200",
            "Inspect progress. Only the creating account, still an editor/owner of the supplied project, can inspect or resume it. Current protection and configured size limits are rechecked. Expired/cancelled uploads can return 410 upload_expired; a removed session returns 404.",
        ),
        (
            "/v1/files/uploads/{upload}/parts/{part}",
            "put",
            "putProjectFileUploadPart",
            "part",
            "Upload",
            "200",
            "Send the next complete part, at most 8 MiB, with X-Part-SHA256. Each part must have its exact expected length. An exact retry of a stored part is safe and does not advance progress twice; changed bytes or out-of-order parts return 409. If a concurrent update wins, inspect status before retrying. Finished/finalizing uploads refuse new parts.",
        ),
        (
            "/v1/files/uploads/{upload}/complete",
            "post",
            "completeProjectFileUpload",
            "",
            "Object",
            "201",
            "Verify all part bytes, total size, SHA-256, current rights, protection, capacity and credential-like text; then publish the file and attachment. An exact completed retry returns the same file (200). An interrupted finish can be retried with the same upload ID; finishing work survives expiry. A missing or wrong complete hash is an error, never an empty success.",
        ),
        (
            "/v1/files/uploads/{upload}",
            "delete",
            "cancelProjectFileUpload",
            "",
            "UploadDeleted",
            "200",
            "Cancel an unfinished upload, remove its parts and release quota. Current creator/project rights still apply; expired uploads may be cancelled. A finished/finalizing upload returns 409: delete the file instead. A later repeated cancel can return 404 or 410. No remote process is being cancelled.",
        ),
    ] {
        let mut parameters = vec![];
        for name in ["id", "project", "attachment", "document", "upload", "part"] {
            if path.contains(&format!("{{{name}}}")) {
                parameters.push(json!({"name":name,"in":"path","required":true,"schema":if name=="part" {json!({"type":"integer","minimum":0})} else {json!({"type":"string"})}}));
            }
        }
        if path.contains("{upload}") {
            parameters.push(json!({"name":"project","in":"query","required":true,"schema":{"type":"string"},"description":"Project ID or name:NAME. Use the ID returned when starting the upload."}));
        } else if path.starts_with("/v1/files/{id}") {
            parameters.push(json!({"name":"project","in":"query","schema":{"type":"string"},"description":"Read or mutate through current project membership; required for files in another account's project storage."}));
        }
        if id == "uploadProjectFile" {
            parameters.push(json!({"name":"purpose","in":"query","schema":{"const":"project","default":"project"}}));
            parameters.push(json!({"name":"filename","in":"query","schema":{"type":"string"},"description":"Preferred filename. name is an older alias; omit both for the service's fallback name."}));
            parameters.push(
                json!({"name":"name","in":"query","deprecated":true,"schema":{"type":"string"}}),
            );
        }
        if id == "listProjectAttachments" {
            parameters.push(json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":0,"default":100},"description":"Clamped to 1–200."}));
            parameters.push(json!({"name":"after","in":"query","schema":{"type":"string"}}));
        }
        if request == "part" {
            parameters.push(json!({"name":"X-Part-SHA256","in":"header","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}}));
        }
        let success = if answer == "binary" {
            json!({"description":"Whole original file bytes; see the operation's streaming/checksum contract.","headers":{"Digest":{"schema":{"type":"string","pattern":"^sha-256=[0-9a-f]{64}$"}},"Content-Disposition":{"schema":{"type":"string"}},"Content-Length":{"schema":{"type":"integer","minimum":1}}},"content":{"*/*":{"schema":{"type":"string","format":"binary"}}}})
        } else {
            json!({"description":"The requested file result.","content":{"application/json":{"schema":r(answer)}}})
        };
        let mut op = json!({"operationId":id,"tags":["Project files"],"description":description,"security":[{"projectFileBearer":[]}],"parameters":parameters,"responses":{}});
        op["responses"][status] = success.clone();
        if matches!(
            id,
            "attachProjectFileOrDocument" | "startProjectFileUpload" | "completeProjectFileUpload"
        ) {
            op["responses"]["200"] = success;
        }
        for status in [
            "400", "401", "403", "404", "409", "410", "413", "415", "422", "429", "502", "503",
        ] {
            op["responses"][status] = json!({"description":"Invalid request or `after` value; empty input; `looks_like_a_secret`, `signed_out`, `project_role`, `not_found`, `not_in_vault`, `full`, `only_me_project`, `in_another_project`, `upload_conflict`, `storage_full`, `upload_expired`, `too_large` or `digest_mismatch`; a service refused or could not complete the request. Codes remain strings; an error never means the request succeeded.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
            if matches!(status, "400" | "413" | "415" | "422") {
                op["responses"][status]["content"]["text/plain"] = json!({"schema":{"type":"string"},"description":"Framework query/JSON/body-limit failure before the handler."});
            }
        }
        if !request.is_empty() {
            op["requestBody"] = if matches!(request, "binary" | "part") {
                json!({"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary","maxLength":if request=="part" {uploads::PART_BYTES} else {16*1024*1024}}}}})
            } else {
                json!({"required":true,"content":{"application/json":{"schema":r(request)}}})
            };
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = op;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::{body::Body, extract::Request, middleware::Next};

    fn validator(document: &Value, schema: &Value) -> jsonschema::Validator {
        let mut schema = schema.clone();
        schema["components"] = document["components"].clone();
        schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        jsonschema::validator_for(&schema).unwrap()
    }

    #[test]
    fn file_contract_examples_and_attachment_choices_match_serializers() {
        let document = crate::app_openapi::document();
        for (name, schema) in document["components"]["schemas"].as_object().unwrap() {
            if !name.starts_with("AppFile") {
                continue;
            }
            let check = validator(&document, schema);
            if let Some(examples) = schema["examples"].as_array() {
                for example in examples {
                    assert!(check.is_valid(example), "{name}");
                }
            }
        }
        let check = validator(&document, &r("Attach"));
        assert!(check.is_valid(&json!({"document":{"title":"Note","text":"Example text"}})));
        assert!(!check.is_valid(&json!({})));
        assert!(!check.is_valid(&json!({"file":"file_000000000000000000000001","document":{"title":"Note","text":"Example"}})));
        assert!(!check.is_valid(&json!({"source":{"kind":"drive"}})));
        let check = validator(&document, &r("VaultAttachment"));
        let mut example = json!({"id":"a".repeat(64),"object":"attachment","type":"vault","protection":"only_me","size_bytes":100,"created_at":"1970-01-01T00:00:01Z"});
        assert!(check.is_valid(&example));
        example["filename"] = json!("private.txt");
        assert!(!check.is_valid(&example));
    }

    pub(crate) async fn check_response(request: Request, next: Next) -> Response {
        let path = request.uri().path().to_owned();
        let method = request.method().as_str().to_ascii_lowercase();
        let response = next.run(request).await;
        if !(path.starts_with("/v1/files") || path.starts_with("/v1/projects"))
            || !response
                .headers()
                .get("content-type")
                .is_some_and(|v| v.as_bytes().starts_with(b"application/json"))
        {
            return response;
        }
        let mut owned = json!({"paths":{},"components":{"schemas":{},"securitySchemes":{}}});
        super::extend(&mut owned);
        let paths = owned["paths"].as_object().unwrap();
        let item = paths.get(&path).or_else(|| {
            paths.iter().find_map(|(template, item)| {
                let wanted: Vec<_> = template.split('/').collect();
                let actual: Vec<_> = path.split('/').collect();
                (wanted.len() == actual.len()
                    && wanted
                        .iter()
                        .zip(actual)
                        .all(|(a, b)| a.starts_with('{') || *a == b))
                .then_some(item)
            })
        });
        let Some(item) = item else {
            return response;
        };
        let status = response.status().as_u16().to_string();
        let schema = &item[&method]["responses"][&status]["content"]["application/json"]["schema"];
        // A downloaded original JSON file is still bytes, not a metadata DTO.
        if response.status() == StatusCode::OK && path.ends_with("/content") {
            return response;
        }
        assert!(
            schema.is_object(),
            "Undocumented file response {method} {path} {status}"
        );
        let check = validator(&crate::app_openapi::document(), schema);
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 32 * 1024 * 1024).await.unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            check.is_valid(&value),
            "File response schema drift {method} {path} {status}"
        );
        Response::from_parts(parts, Body::from(bytes))
    }
}
