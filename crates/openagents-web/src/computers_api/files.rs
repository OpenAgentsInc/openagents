//! Binary workspace files use account authority and bounded transfer slots.
use super::*;
use axum::{
    body::Body,
    extract::{Query, Request},
};
use computer_files::{FileError, FileInfo, MAX_BYTES, MAX_TRANSFERS, TIMEOUT_SECONDS};
use futures_util::StreamExt;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;

static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileQuery {
    path: String,
    #[serde(default)]
    overwrite: bool,
}
fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_path",
        FileError::Invalid.message(),
    )
}
fn mapped(status: u16, bytes: &[u8]) -> Response {
    let value: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let code = value["error"].as_str().unwrap_or_default();
    let error = match (status, code) {
        (404, "not_found") => {
            return refused(StatusCode::NOT_FOUND, "not_found", "No such computer.");
        }
        (409, "computer_not_ready") => {
            return refused(
                StatusCode::CONFLICT,
                code,
                "Wake the computer and wait until it is ready before transferring files.",
            );
        }
        (429, "transfer_limit") => return limited(),
        (400, "invalid_path") => FileError::Invalid,
        (404, "file_not_found") => FileError::Missing,
        (403, "file_path_refused") => FileError::Escape,
        (415, "not_a_file") => FileError::Special,
        (413, "file_too_large") => FileError::TooLarge,
        (409, "file_changed") => FileError::Changed,
        (412, "file_exists") => FileError::Exists,
        (422, "incomplete_upload") => FileError::Incomplete,
        (501, "files_unsupported") => FileError::Unsupported,
        (503, "write_uncertain") => FileError::WriteUncertain,
        _ => return unavailable(),
    };
    refused(StatusCode::from_u16(status).unwrap(), code, error.message())
}
fn uncertain() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "write_uncertain",
        FileError::WriteUncertain.message(),
    )
}
// Send 413 immediately, then keep the response open while discarding the
// rejected HTTP/1 upload. Closing it while Cloud Run is still sending the
// body can reset the connection and hide 413. The existing slot, byte bound,
// and deadline also bound clients that continue sending or stall.
fn reject_large_upload(req: Request, permit: tokio::sync::OwnedSemaphorePermit) -> Response {
    let response = refused(
        StatusCode::PAYLOAD_TOO_LARGE,
        "file_too_large",
        FileError::TooLarge.message(),
    );
    let (mut parts, error) = response.into_parts();
    parts.headers.remove(header::CONTENT_LENGTH);
    let first = futures_util::stream::once(async move {
        Ok::<_, std::convert::Infallible>(axum::body::to_bytes(error, 4096).await.unwrap())
    });
    let finish = futures_util::stream::once(async move {
        let _permit = permit;
        let mut body = req.into_body().into_data_stream();
        let discard = async {
            let mut bytes = 0_usize;
            while let Some(Ok(chunk)) = body.next().await {
                bytes = bytes.saturating_add(chunk.len());
                // Cloud Run's HTTP/1 request limit is 32 MiB.
                if bytes >= 32 * 1024 * 1024 {
                    break;
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECONDS), discard).await;
        Ok::<_, std::convert::Infallible>(axum::body::Bytes::new())
    });
    Response::from_parts(parts, Body::from_stream(first.chain(finish)))
}
fn limited() -> Response {
    let mut r = refused(
        StatusCode::TOO_MANY_REQUESTS,
        "transfer_limit",
        "Two file transfers are already running. Try again shortly.",
    );
    r.headers_mut()
        .insert(header::RETRY_AFTER, "1".parse().unwrap());
    r
}
pub(crate) async fn transfer(
    State(app): State<App>,
    Path(id): Path<String>,
    query: Result<Query<FileQuery>, axum::extract::rejection::QueryRejection>,
    req: Request,
) -> Response {
    let q = match query {
        Ok(Query(q)) => q,
        Err(_) => return invalid(),
    };
    let headers = req.headers();
    let i = match admitted(&app, headers).await {
        Ok(i) => i,
        Err(e) => return e,
    };
    if !cloud_id(&id) {
        return refused(StatusCode::NOT_FOUND, "not_found", "No such computer.");
    }
    if headers.get_all(header::ORIGIN).iter().count() > 1
        || headers
            .get(header::ORIGIN)
            .is_some_and(|h| h.to_str().ok() != app.config.cloud.as_deref().map(|c| c.origin()))
    {
        return refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Open this computer from OpenAgents.",
        );
    }
    if computer_files::validate_path(&q.path).is_err() {
        return invalid();
    }
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    // Check ownership and state before accepting a file body.
    match client
        .call(
            reqwest::Method::GET,
            &format!("/v1/computers/{id}"),
            &i.owner,
            None,
        )
        .await
    {
        Ok((200, v))
            if v["desired"] == "running"
                && matches!(v["state"].as_str(), Some("awake" | "turn")) => {}
        Ok((200, v)) if v["desired"] == "deleted" => {
            return refused(StatusCode::NOT_FOUND, "not_found", "No such computer.");
        }
        Ok((200, _)) => {
            return refused(
                StatusCode::CONFLICT,
                "computer_not_ready",
                "Wake the computer and wait until it is ready before transferring files.",
            );
        }
        Ok((404, _)) => return refused(StatusCode::NOT_FOUND, "not_found", "No such computer."),
        _ => return unavailable(),
    }
    let slots = SLOTS.get_or_init(|| Arc::new(Semaphore::new(MAX_TRANSFERS)));
    let Ok(_permit) = slots.clone().try_acquire_owned() else {
        return limited();
    };
    let write = req.method() == axum::http::Method::PUT;
    let (upload, checksum) = if write {
        if headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            != Some("application/octet-stream")
        {
            return invalid();
        }
        let size = match headers
            .get(header::CONTENT_LENGTH)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok())
        {
            Some(n) if n <= MAX_BYTES => n,
            Some(_) => {
                return reject_large_upload(req, _permit);
            }
            None => {
                return refused(
                    StatusCode::LENGTH_REQUIRED,
                    "length_required",
                    "Send the upload length in Content-Length.",
                );
            }
        };
        let checksum = match headers
            .get("x-content-sha256")
            .and_then(|h| h.to_str().ok())
            .filter(|s| computer_files::valid_digest(s))
        {
            Some(s) => s.to_owned(),
            None => {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "incomplete_upload",
                    FileError::Incomplete.message(),
                );
            }
        };
        let bytes = match tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS),
            axum::body::to_bytes(req.into_body(), MAX_BYTES),
        )
        .await
        {
            Ok(Ok(b)) => b.to_vec(),
            _ => {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "incomplete_upload",
                    FileError::Incomplete.message(),
                );
            }
        };
        if bytes.len() != size || computer_files::digest(&bytes) != checksum {
            return refused(
                StatusCode::UNPROCESSABLE_ENTITY,
                "incomplete_upload",
                FileError::Incomplete.message(),
            );
        }
        (Some(bytes), Some(checksum))
    } else {
        if q.overwrite {
            return invalid();
        }
        (None, None)
    };
    let mut query = reqwest::Url::parse("https://files.invalid/").unwrap();
    query
        .query_pairs_mut()
        .append_pair("path", &q.path)
        .append_pair("overwrite", if q.overwrite { "true" } else { "false" });
    let path = format!("/v1/computers/{id}/files?{}", query.query().unwrap());
    let method = if write {
        reqwest::Method::PUT
    } else {
        reqwest::Method::GET
    };
    let uploaded_size = upload.as_ref().map(Vec::len);
    match client
        .file_call(
            method,
            &path,
            &i.owner,
            upload,
            checksum.as_deref(),
            i.environment_attempt.as_deref(),
        )
        .await
    {
        Ok((200, bytes, _)) if write => {
            let Ok(info) = serde_json::from_slice::<FileInfo>(&bytes) else {
                return uncertain();
            };
            if Some(info.size) != uploaded_size
                || info.size > MAX_BYTES
                || Some(info.sha256.as_str()) != checksum.as_deref()
            {
                return uncertain();
            }
            answer(
                StatusCode::OK,
                json!({"size":info.size,"sha256":info.sha256}),
            )
        }
        Ok((200, bytes, Some(hash))) if !write => {
            if !computer_files::valid_digest(&hash) || computer_files::digest(&bytes) != hash {
                return unavailable();
            }
            let mut response = Response::new(Body::from(bytes));
            let h = response.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                "application/octet-stream".parse().unwrap(),
            );
            h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
            h.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
            h.insert("x-content-sha256", hash.parse().unwrap());
            h.insert(header::ETAG, format!("\"{hash}\"").parse().unwrap());
            response
        }
        Ok((status, _, _)) if write && matches!(status, 500 | 502 | 503 | 504) => uncertain(),
        Ok((status, bytes, _)) => mapped(status, &bytes),
        Err(_) if write => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "write_uncertain",
            FileError::WriteUncertain.message(),
        ),
        Err(_) => unavailable(),
    }
}

pub(super) fn extend_openapi(doc: &mut Value) {
    doc["components"]["schemas"]["AppComputerFileInfo"] = json!({"type":"object","additionalProperties":false,"required":["size","sha256"],"properties":{"size":{"type":"integer","minimum":0,"maximum":MAX_BYTES},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}}});
    let mut responses = json!({"200":{"description":"Complete binary file; X-Content-SHA256 and ETag identify the returned bytes.","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}}});
    for (code, description) in [
        (400, "Invalid path or query."),
        (401, "Sign in again."),
        (403, "Access or path refused."),
        (404, "Computer, file, or parent directory missing."),
        (
            409,
            "Computer is not awake, or the file changed during the read.",
        ),
        (411, "Upload length required."),
        (412, "Create-only upload conflicts with a different file."),
        (413, "File exceeds 4 MiB."),
        (415, "Choose a regular file."),
        (422, "Upload length or checksum mismatch."),
        (
            429,
            "Two transfers are in progress; retry after one second.",
        ),
        (501, "Protected files are unsupported on this computer."),
        (
            503,
            "Transfer unavailable; a write may have completed, so read and verify before retrying.",
        ),
    ] {
        responses[code.to_string()] = json!({"description":description,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
    }
    let parameters = json!([{"name":"id","in":"path","required":true,"schema":{"type":"string","pattern":"^cmp_[0-9a-f]{20}$"}},{"name":"path","in":"query","required":true,"description":"Relative UTF-8 path beneath the command workspace. No empty or dot components, controls, backslashes, colon, percent signs, symlinks, or mount crossings. Parents must already exist.","schema":{"type":"string","minLength":1,"maxLength":1024}}]);
    let read = json!({"operationId":"readComputerFile","summary":"Read a workspace file","description":"Requires an awake computer; never wakes or allocates. Returns a bounded complete read after taking a read lease and checking file size and modification metadata. Files open for writing return409; files that cannot take a lease are unsupported. Concurrent changes can return409. File mode is not altered.","security":[{"appBearer":[]}],"parameters":parameters,"responses":responses});
    responses["200"] = json!({"description":"The complete upload was checked and atomically published with mode0600.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppComputerFileInfo"}}}});
    let mut write_parameters = parameters.as_array().unwrap().clone();
    write_parameters.push(json!({"name":"overwrite","in":"query","schema":{"type":"boolean","default":false},"description":"False: existing identical bytes succeed; different bytes return412. True: atomically replace; concurrent writers use last completed publication. No content compare-and-swap or idempotency key is offered."}));
    write_parameters.push(json!({"name":"X-Content-SHA256","in":"header","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}}));
    let write = json!({"operationId":"writeComputerFile","summary":"Upload a workspace file","description":"Requires awake computer, Content-Length, and SHA256. Validates the full body before writing. Publishes a same-directory temporary file by atomic rename; partial or mismatched uploads leave the previous file intact. Missing parents return404. Maximum4MiB, two transfers per instance,60second upload and60second execution bounds. A lost response can leave the result uncertain: read and compare SHA256 before retrying. Files stay on this computer's disk and are not shared artifacts.","security":[{"appBearer":[]}],"parameters":write_parameters,"requestBody":{"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary","maxLength":MAX_BYTES}}}},"responses":responses});
    doc["paths"]["/v1/computers/{id}/files"] = json!({"get":read,"put":write});
}
