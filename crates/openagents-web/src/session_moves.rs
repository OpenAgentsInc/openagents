//! Web moves wait for the owning computer's normal continuation path.
use crate::chat_store::{Error, Store, now_unix};
use chat_home::{MoveRequest, PartRef};
use std::collections::BTreeMap;

const KEY: &str = "sessions/moves.json";

async fn read(
    store: &Store,
    owner: &str,
) -> Result<(BTreeMap<String, MoveRequest>, Option<String>), Error> {
    match store.read_key(&Store::owner_key(owner, KEY)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Unavailable("The chat move could not be read."))?,
            Some(generation),
        )),
        None => Ok((BTreeMap::new(), None)),
    }
}

async fn change(
    store: &Store,
    owner: &str,
    update: impl Fn(&mut BTreeMap<String, MoveRequest>),
) -> Result<(), Error> {
    for _ in 0..6 {
        let (mut moves, generation) = read(store, owner).await?;
        moves.retain(|_, r| r.valid(now_unix()));
        update(&mut moves);
        if moves.len() > crate::session_index::MAX_ROWS {
            return Err(Error::Invalid("Finish another chat move first."));
        }
        let bytes = serde_json::to_vec(&moves)
            .map_err(|_| Error::Invalid("The chat move could not be saved."))?;
        match store
            .write_key(&Store::owner_key(owner, KEY)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

pub(crate) async fn begin(
    store: &Store,
    owner: &str,
    source: &str,
    target: PartRef,
) -> Result<(), Error> {
    let request = MoveRequest {
        source: source.into(),
        target,
        expires_unix: now_unix() + 10 * 60,
    };
    if !request.valid(now_unix()) {
        return Err(Error::Invalid("Choose a new conversation."));
    }
    change(store, owner, |moves| {
        moves
            .entry(source.into())
            .or_insert_with(|| request.clone());
    })
    .await
}

pub(crate) async fn pending(store: &Store, owner: &str, source: &str) -> Result<bool, Error> {
    Ok(read(store, owner)
        .await?
        .0
        .get(source)
        .is_some_and(|r| r.valid(now_unix())))
}

pub(crate) async fn complete(
    store: &Store,
    owner: &str,
    source: &str,
    target: &PartRef,
) -> Result<(), Error> {
    let (moves, _) = read(store, owner).await?;
    if !moves.get(source).is_some_and(|r| &r.target == target) {
        return Ok(());
    }
    change(store, owner, |moves| {
        if moves.get(source).is_some_and(|r| &r.target == target) {
            moves.remove(source);
        }
    })
    .await
}

pub(crate) async fn list(
    store: &Store,
    owner: &str,
    device: &str,
) -> Result<Vec<MoveRequest>, Error> {
    let index = crate::session_index::load_linked(store, owner).await?;
    let mut moves = read(store, owner)
        .await?
        .0
        .into_values()
        .filter(|r| {
            r.valid(now_unix())
                && index.rows.get(&r.source).is_some_and(|row| {
                    row.device == device && row.home_transfer && row.continued_by.is_none()
                })
        })
        .map(|r| (r.source.clone(), r))
        .collect::<BTreeMap<_, _>>();
    // Older web moves already have a cloud part. Tell the computer where
    // they went so it can repair its local read-only state too.
    for row in index
        .rows
        .values()
        .filter(|r| r.device == device && r.home_transfer)
    {
        if let Some(target) = row
            .continued_by
            .as_ref()
            .filter(|p| p.home == chat_home::Home::Cloud)
        {
            moves.entry(row.id.clone()).or_insert_with(|| MoveRequest {
                source: row.id.clone(),
                target: target.clone(),
                expires_unix: now_unix() + 10 * 60,
            });
        }
    }
    Ok(moves.into_values().collect())
}

pub(crate) async fn route(
    axum::extract::State(app): axum::extract::State<crate::App>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(device): axum::extract::Path<String>,
) -> axum::response::Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if chat_home::Home::computer(&device).is_none() {
        return crate::coder_sync::refused(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid",
            "Choose a computer.",
        );
    }
    match list(&app.config.chat_store, &owner, &device).await {
        Ok(moves) => crate::coder_sync::answer(
            axum::http::StatusCode::OK,
            serde_json::json!({"moves":moves}),
        ),
        Err(error) => crate::coder_sync::stored(&error),
    }
}
