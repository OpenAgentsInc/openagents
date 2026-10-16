//! A bounded bridge to non-Send driver file calls. It never creates or wakes a computer.
use crate::{Store, api::Api, store::Desired};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use computer_driver::Driver;
use computer_files::{FileError, MAX_BYTES, MAX_TRANSFERS, TIMEOUT_SECONDS};
use serde::Deserialize;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

#[derive(Clone)]
pub struct Client {
    tx: mpsc::Sender<Transfer>,
    slots: Arc<Semaphore>,
}
struct Transfer {
    owner: String,
    computer: String,
    path: String,
    upload: Option<(Vec<u8>, bool)>,
    reply: oneshot::Sender<Result<Vec<u8>, Error>>,
    _permit: OwnedSemaphorePermit,
}
#[derive(Clone, Copy)]
enum Error {
    File(FileError),
    MissingComputer,
    NotReady,
    Busy,
}
impl From<FileError> for Error {
    fn from(e: FileError) -> Self {
        Self::File(e)
    }
}

/// Keep all driver futures on their own LocalSet, with two requests in flight.
pub fn spawn<D: Driver + Clone + Send + 'static>(store: Store, driver: D) -> Client {
    let (tx, mut rx) = mpsc::channel::<Transfer>(MAX_TRANSFERS);
    std::thread::Builder::new()
        .name("oa-computers-files".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("file worker runtime");
            tokio::task::LocalSet::new().block_on(&rt, async move {
                while let Some(t) = rx.recv().await {
                    let store = store.clone();
                    let driver = driver.clone();
                    tokio::task::spawn_local(async move {
                        let write = t.upload.is_some();
                        let effect = async {
                            let e = store
                                .get(&t.owner, &t.computer)
                                .await
                                .map_err(|_| Error::File(FileError::Unavailable))?
                                .ok_or(Error::MissingComputer)?;
                            if e.desired == Desired::Deleted {
                                return Err(Error::MissingComputer);
                            }
                            if e.desired != Desired::Running
                                || !matches!(e.phase.as_str(), "awake" | "turn")
                            {
                                return Err(Error::NotReady);
                            }
                            if driver.capabilities().workspace_file_bytes.is_none() {
                                return Err(FileError::Unsupported.into());
                            }
                            let resource = e.record.resource().ok_or(Error::NotReady)?;
                            if let Some((bytes, overwrite)) = &t.upload {
                                let info = driver
                                    .write_workspace_file(resource, &t.path, bytes, *overwrite)
                                    .await?;
                                if info.size != bytes.len()
                                    || info.sha256 != computer_files::digest(bytes)
                                {
                                    return Err(FileError::WriteUncertain.into());
                                }
                                serde_json::to_vec(&info)
                                    .map_err(|_| FileError::WriteUncertain.into())
                            } else {
                                let bytes = driver.read_workspace_file(resource, &t.path).await?;
                                if bytes.len()
                                    > driver
                                        .capabilities()
                                        .workspace_file_bytes
                                        .unwrap_or(0)
                                        .min(MAX_BYTES)
                                {
                                    return Err(FileError::TooLarge.into());
                                }
                                Ok(bytes)
                            }
                        };
                        let result =
                            tokio::time::timeout(Duration::from_secs(TIMEOUT_SECONDS), effect)
                                .await
                                .unwrap_or(Err(Error::File(if write {
                                    FileError::WriteUncertain
                                } else {
                                    FileError::Unavailable
                                })));
                        let _ = t.reply.send(result);
                    });
                }
            });
        })
        .expect("file worker starts");
    Client {
        tx,
        slots: Arc::new(Semaphore::new(MAX_TRANSFERS)),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileQuery {
    path: String,
    #[serde(default)]
    overwrite: bool,
}
fn error(e: Error) -> Response {
    let (status, code, message) = match e {
        Error::MissingComputer => (404, "not_found", "No such computer."),
        Error::NotReady => (
            409,
            "computer_not_ready",
            "Wake the computer and wait until it is ready before transferring files.",
        ),
        Error::Busy => (
            429,
            "transfer_limit",
            "Two file transfers are already running. Try again shortly.",
        ),
        Error::File(f) => (
            match f {
                FileError::Invalid => 400,
                FileError::Missing => 404,
                FileError::Escape => 403,
                FileError::Special => 415,
                FileError::TooLarge => 413,
                FileError::Changed => 409,
                FileError::Exists => 412,
                FileError::Incomplete => 422,
                FileError::Unsupported => 501,
                _ => 503,
            },
            match f {
                FileError::Invalid => "invalid_path",
                FileError::Missing => "file_not_found",
                FileError::Escape => "file_path_refused",
                FileError::Special => "not_a_file",
                FileError::TooLarge => "file_too_large",
                FileError::Changed => "file_changed",
                FileError::Exists => "file_exists",
                FileError::Incomplete => "incomplete_upload",
                FileError::Unsupported => "files_unsupported",
                FileError::WriteUncertain => "write_uncertain",
                FileError::Unavailable => "unavailable",
            },
            f.message(),
        ),
    };
    let mut response = (
        StatusCode::from_u16(status).unwrap(),
        Json(json!({"error":code,"message":message})),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    if status == 429 {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, "1".parse().unwrap());
    }
    response
}
async fn transfer(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Query(q): Query<FileQuery>,
    headers: HeaderMap,
    req: Request,
) -> Response {
    let owner = match super::api::account(&headers) {
        Ok(o) => o,
        Err(e) => return e,
    };
    if computer_files::validate_path(&q.path).is_err() {
        return error(FileError::Invalid.into());
    }
    // Ownership and readiness precede body buffering and provider access.
    match api.store.get(&owner, &id).await {
        Ok(Some(e))
            if e.desired == Desired::Running && matches!(e.phase.as_str(), "awake" | "turn") => {}
        Ok(Some(e)) if e.desired == Desired::Deleted => return error(Error::MissingComputer),
        Ok(Some(_)) => return error(Error::NotReady),
        Ok(None) => return error(Error::MissingComputer),
        Err(_) => return error(FileError::Unavailable.into()),
    }
    let Some(client) = api
        .files
        .as_ref()
        .filter(|_| api.caps.workspace_file_bytes.is_some())
    else {
        return error(FileError::Unsupported.into());
    };
    let permit = match client.slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(Error::Busy),
    };
    let max = api.caps.workspace_file_bytes.unwrap_or(0).min(MAX_BYTES);
    let write = req.method() == axum::http::Method::PUT;
    let upload = if write {
        if headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            != Some("application/octet-stream")
        {
            return error(FileError::Invalid.into());
        }
        let size = match headers
            .get(header::CONTENT_LENGTH)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse::<usize>().ok())
        {
            Some(n) if n <= max => n,
            Some(_) => return error(FileError::TooLarge.into()),
            None => return error(FileError::Incomplete.into()),
        };
        let checksum = match headers
            .get("x-content-sha256")
            .and_then(|h| h.to_str().ok())
            .filter(|s| computer_files::valid_digest(s))
        {
            Some(s) => s.to_owned(),
            None => return error(FileError::Incomplete.into()),
        };
        let bytes = match tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECONDS),
            axum::body::to_bytes(req.into_body(), max),
        )
        .await
        {
            Ok(Ok(b)) => b.to_vec(),
            _ => return error(FileError::Incomplete.into()),
        };
        if bytes.len() != size || computer_files::digest(&bytes) != checksum {
            return error(FileError::Incomplete.into());
        }
        Some((bytes, q.overwrite))
    } else {
        if q.overwrite {
            return error(FileError::Invalid.into());
        }
        None
    };
    let (reply, result) = oneshot::channel();
    let t = Transfer {
        owner,
        computer: id,
        path: q.path,
        upload,
        reply,
        _permit: permit,
    };
    if client.tx.try_send(t).is_err() {
        return error(Error::Busy);
    }
    match result.await {
        Ok(Ok(bytes)) if write => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Ok(Ok(bytes)) => {
            let checksum = computer_files::digest(&bytes);
            let mut response = Response::new(Body::from(bytes));
            let h = response.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                "application/octet-stream".parse().unwrap(),
            );
            h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
            h.insert("x-content-sha256", checksum.parse().unwrap());
            h.insert(header::ETAG, format!("\"{checksum}\"").parse().unwrap());
            response
        }
        Ok(Err(e)) => error(e),
        Err(_) => error(
            if write {
                FileError::WriteUncertain
            } else {
                FileError::Unavailable
            }
            .into(),
        ),
    }
}
pub fn routes() -> Router<Arc<Api>> {
    Router::new().route("/v1/computers/{id}/files", get(transfer).put(transfer))
}
