//! Public Snap Station builds. Every write and read verifies the signed run.
use crate::{App, chat_store::Store};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    routing::get,
};
use snap_share::Shared;

type Error = (StatusCode, &'static str);
const FOLDER: &str = "snap-shares";
fn key(code: &str) -> String {
    format!("{FOLDER}/{code}.json")
}
fn unavailable() -> Error {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "Builds could not be loaded. Try again.",
    )
}
async fn checked(shared: Shared) -> Result<Shared, Error> {
    static REPLAYS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let permit = REPLAYS.try_acquire().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Other builds are opening. Try again in a moment.",
        )
    })?;
    let result = tokio::task::spawn_blocking(move || {
        shared.check().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "This run did not play back correctly.",
            )
        })?;
        Ok(shared)
    })
    .await
    .map_err(|_| unavailable())?;
    drop(permit);
    result
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/games/snap-station/shares", get(gallery).post(publish))
        .route("/games/snap-station/shares/{code}", get(open))
        .layer(DefaultBodyLimit::max(snap_share::MAX_BYTES))
}
async fn save(store: &Store, shared: Shared) -> Result<String, Error> {
    let shared = checked(shared).await?;
    let code = shared.code();
    let key = key(&code);
    let bytes = shared.encode().into_bytes();
    if let Some((old, _)) = store.read_key(&key).await.map_err(|_| unavailable())? {
        if old != bytes {
            return Err((
                StatusCode::CONFLICT,
                "Share this build again to get a new code.",
            ));
        }
    } else {
        // Bound storage; existing codes still open at capacity.
        if store
            .folder_keys_limit(FOLDER, 4096)
            .await
            .map_err(|_| unavailable())?
            .len()
            >= 4096
        {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "The gallery is full. Download your build to share it.",
            ));
        }
        match store.write_key(&key, bytes.clone(), None).await {
            Ok(_) => {}
            Err(crate::chat_store::Error::Conflict) => {
                if store
                    .read_key(&key)
                    .await
                    .map_err(|_| unavailable())?
                    .map(|p| p.0)
                    != Some(bytes)
                {
                    return Err(unavailable());
                }
            }
            Err(_) => return Err(unavailable()),
        }
    }
    if shared.gallery {
        // Keep the public listing separate so unlisted builds cannot crowd it out.
        // Retrying a publish also repairs an interrupted index write.
        match store
            .write_key(&format!("snap-gallery/{code}.json"), b"true".to_vec(), None)
            .await
        {
            Ok(_) | Err(crate::chat_store::Error::Conflict) => {}
            Err(_) => return Err(unavailable()),
        }
    }
    Ok(code)
}

async fn publish(
    State(app): State<App>,
    Json(shared): Json<Shared>,
) -> Result<Json<serde_json::Value>, Error> {
    let code = save(&app.config.chat_store, shared).await?;
    Ok(Json(serde_json::json!({"code": code})))
}
async fn load(store: &Store, code: &str) -> Result<Shared, Error> {
    let code =
        snap_share::code(code).ok_or((StatusCode::NOT_FOUND, "This build could not be found."))?;
    let (bytes, _) = store
        .read_key(&key(&code))
        .await
        .map_err(|_| unavailable())?
        .ok_or((StatusCode::NOT_FOUND, "This build could not be found."))?;
    let shared = Shared::decode(std::str::from_utf8(&bytes).map_err(|_| unavailable())?)
        .map_err(|_| unavailable())?;
    if shared.code() != code {
        return Err(unavailable());
    }
    checked(shared).await
}
async fn open(State(app): State<App>, Path(code): Path<String>) -> Result<Json<Shared>, Error> {
    load(&app.config.chat_store, &code).await.map(Json)
}
async fn gallery(State(app): State<App>) -> Result<Json<Vec<Shared>>, Error> {
    list(&app.config.chat_store).await.map(Json)
}
async fn list(store: &Store) -> Result<Vec<Shared>, Error> {
    let mut builds = Vec::new();
    let keys = store
        .folder_keys_limit("snap-gallery", 4096)
        .await
        .map_err(|_| unavailable())?;
    for key in keys.into_iter().rev().take(128) {
        let Some(code) = key.rsplit('/').next().and_then(|s| s.strip_suffix(".json")) else {
            continue;
        };
        if let Ok(shared) = load(store, code).await {
            if shared.gallery {
                builds.push(shared);
            }
        }
        if builds.len() == 24 {
            break;
        }
    }
    Ok(builds)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn shared_runs_persist_and_changed_runs_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().join("shares"));
        let level = snap_rules::level::job(1);
        let plan = snap_rules::bot::solve(&level).unwrap();
        let (_, run) = snap_rules::bot::record_plan(&level, &plan);
        let b = Shared::sign(1, &run, &[4; 32], &[8; 16], true).unwrap();
        let code = save(&store, b.clone()).await.unwrap();
        assert_eq!(save(&store, b.clone()).await.unwrap(), code);
        assert_eq!(list(&store).await.unwrap(), vec![b.clone()]);
        let unlisted = Shared::sign(1, &run, &[4; 32], &[9; 16], false).unwrap();
        save(&store, unlisted).await.unwrap();
        assert_eq!(list(&store).await.unwrap(), vec![b.clone()]);
        let reopened = Store::local(root.path().join("shares"));
        assert_eq!(load(&reopened, &code).await.unwrap(), b);
        let mut changed = b.clone();
        changed.gallery = false;
        assert_eq!(
            save(&store, changed).await.unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
        let (mut bytes, generation) = store.read_key(&key(&code)).await.unwrap().unwrap();
        bytes[0] = b'x';
        store
            .write_key(&key(&code), bytes, Some(&generation))
            .await
            .unwrap();
        assert!(load(&store, &code).await.is_err());
        assert!(list(&store).await.unwrap().is_empty());
        assert!(load(&store, "../secret").await.is_err());
    }
}
