//! Test runs never land in a person's lists (#11601).
//!
//! A request that carries [`HEADER`] (`X-OpenAgents-Test: 1`) is a test
//! run: a gate, a smoke, or an agent's check. What it does still works,
//! so the check proves the real path, but nothing it leaves shows up:
//!
//! - a chat it starts (`POST /v1/threads`, the web's `/chat`, a synced
//!   Coder session) is written to the [`INDEX_KEY`] index, left out of
//!   every list ([`crate::chat_store::Store::list`]), and deleted for good
//!   by [`sweep`] once it is [`KEEP_SECONDS`] old (the sweep runs every
//!   [`SWEEP_EVERY`], so it is gone within the hour);
//! - a computer check-in, sync choice, or activity report it sends is
//!   answered but never recorded, so no test computer is listed.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::chat_store::{Error, Store, now_unix};

/// The request header that marks a test run.
pub const HEADER: &str = "x-openagents-test";
/// The store key of the test chats waiting to be deleted.
pub(crate) const INDEX_KEY: &str = "test-runs/chats.json";
const SCHEMA: &str = "openagents.web.test_runs.v1";
/// How long a test chat is kept, so the check that made it can read it back.
pub const KEEP_SECONDS: u64 = 40 * 60;
/// How often the sweep runs.
pub const SWEEP_EVERY: Duration = Duration::from_secs(10 * 60);
/// The most test chats the index holds; past it the oldest are deleted at
/// the next sweep even when young.
const MAX_ENTRIES: usize = 4_096;

/// Whether the request is a test run.
pub fn marked(headers: &HeaderMap) -> bool {
    headers
        .get(HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| matches!(value.trim(), "1" | "true"))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    chats: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    owner: String,
    id: String,
    made_unix: u64,
}

async fn read(store: &Store) -> Result<(Index, Option<String>), Error> {
    match store.read_key(INDEX_KEY).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The test run index is invalid."))?,
            Some(generation),
        )),
        None => Ok((Index::default(), None)),
    }
}

/// Change the index with `change` (which says whether it changed),
/// retrying on a concurrent write.
async fn update(store: &Store, change: impl Fn(&mut Index) -> bool) -> Result<Index, Error> {
    for _ in 0..6 {
        let (mut index, generation) = read(store).await?;
        if !change(&mut index) {
            return Ok(index);
        }
        index.schema = SCHEMA.to_owned();
        let bytes =
            serde_json::to_vec(&index).map_err(|_| Error::Invalid("The test index is invalid."))?;
        match store
            .write_key(INDEX_KEY, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(index),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// Remember chat `id` of `owner` as a test run's.
pub(crate) async fn note(store: &Store, owner: &str, id: &str) -> Result<(), Error> {
    update(store, |index| {
        if index.chats.iter().any(|e| e.owner == owner && e.id == id) {
            return false;
        }
        index.chats.push(Entry {
            owner: owner.to_owned(),
            id: id.to_owned(),
            made_unix: now_unix(),
        });
        true
    })
    .await
    .map(|_| ())
}

/// [`note`] when `headers` mark a test run; a failure is logged, never
/// shown (the sweep can't find the chat, but the person can delete it).
pub(crate) async fn note_if_marked(store: &Store, headers: &HeaderMap, owner: &str, id: &str) {
    if marked(headers)
        && let Err(error) = note(store, owner, id).await
    {
        eprintln!("openagents-web: test runs: {error}");
    }
}

/// The ids of `owner`'s chats made by test runs, to leave out of lists.
pub(crate) async fn hidden(store: &Store, owner: &str) -> Result<BTreeSet<String>, Error> {
    let (index, _) = read(store).await?;
    Ok(index
        .chats
        .into_iter()
        .filter(|entry| entry.owner == owner)
        .map(|entry| entry.id)
        .collect())
}

/// Delete the test chats older than [`KEEP_SECONDS`] at `now` (and the
/// oldest past [`MAX_ENTRIES`]) for good, with their files. Returns how
/// many were deleted.
pub async fn sweep(store: &Store, now: u64) -> Result<usize, Error> {
    let (index, _) = read(store).await?;
    let overflow = index.chats.len().saturating_sub(MAX_ENTRIES);
    let due: Vec<Entry> = index
        .chats
        .iter()
        .enumerate()
        .filter(|(at, entry)| *at < overflow || now.saturating_sub(entry.made_unix) >= KEEP_SECONDS)
        .map(|(_, entry)| entry.clone())
        .collect();
    let mut removed = 0;
    let mut done = Vec::new();
    for entry in due {
        match store.load(&entry.owner, &entry.id).await {
            Ok(Some(loaded)) => match store
                .delete(&entry.owner, &entry.id, &loaded.generation)
                .await
            {
                Ok(true) => {
                    removed += 1;
                    if let Err(error) =
                        crate::chat_files::purge(store, &entry.owner, &entry.id).await
                    {
                        eprintln!("openagents-web: test runs: chat files: {error}");
                    }
                    done.push(entry);
                }
                Ok(false) => done.push(entry),
                // Written again since it was read: the next sweep takes it.
                Err(Error::Conflict) => {}
                Err(error) => eprintln!("openagents-web: test runs: delete: {error}"),
            },
            Ok(None) | Err(Error::Invalid(_)) => done.push(entry),
            Err(error) => eprintln!("openagents-web: test runs: load: {error}"),
        }
    }
    if !done.is_empty() {
        update(store, |index| {
            let before = index.chats.len();
            index.chats.retain(|e| !done.contains(e));
            index.chats.len() != before
        })
        .await?;
    }
    Ok(removed)
}

/// Run [`sweep`] now and every [`SWEEP_EVERY`] for the life of the server.
pub fn spawn_sweep(store: Arc<Store>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(SWEEP_EVERY);
        loop {
            ticks.tick().await;
            match sweep(&store, now_unix()).await {
                Ok(0) => {}
                Ok(removed) => println!("openagents-web: deleted {removed} test-run chats"),
                Err(error) => eprintln!("openagents-web: test runs sweep: {error}"),
            }
        }
    })
}
