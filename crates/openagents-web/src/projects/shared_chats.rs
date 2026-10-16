//! The chats of a shared project (#11586).
//!
//! A chat stays in the store of the person who started it. A shared
//! project keeps, in its home folder (beside its files), one small entry
//! per chat that entered it: `project-files/projects/{project}/c-{chat}.json`
//! naming the chat and whose store holds it. Every member lists the
//! project's chats from those entries; each is read fresh and kept only
//! while it is still in the project and not deleted. Entries are written
//! when a chat is started in or moved into a shared project, and when
//! someone joins (the chats already in the project move in with it).
//!
//! Who may read: any current member (the account service's answer for
//! this request). Who may send: editors and the owner. Each message is
//! answered under its sender's own allowance ([`crate::pages::chat`]'s
//! `payer`), never the chat's starter's.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, header};
use oa_auth::repos::Project;
use serde::{Deserialize, Serialize};

use crate::App;
use crate::chat_store::{Conversation, Error, Loaded, Store, account_owner};

const SCHEMA: &str = "openagents.web.project-chat.v1";
const ROOT: &str = "project-files";
/// The most chats one shared project lists.
const MAX_CHATS: usize = 2_000;

#[derive(Serialize, Deserialize)]
struct Entry {
    schema: String,
    chat: String,
    owner: String,
}

/// Whether other people are in the project.
pub(crate) fn shared(project: &Project) -> bool {
    project.sharing.as_ref().is_some_and(|s| s.shared())
}

/// The chat-store owner whose folder keeps the project's files and chat
/// entries: `viewer`'s own, unless the project was shared with them.
pub(crate) fn home_owner(project: &Project, viewer: &str) -> String {
    project
        .sharing
        .as_ref()
        .and_then(|s| s.home.as_deref())
        .map_or_else(|| viewer.to_owned(), account_owner)
}

fn folder(home: &str, project: &str) -> Result<String, Error> {
    if !oa_auth::repos::project_id(project) {
        return Err(Error::Invalid("The project id is invalid."));
    }
    Store::owner_key(home, &format!("{ROOT}/projects/{project}"))
}

fn key(home: &str, project: &str, chat: &str) -> Result<String, Error> {
    if !crate::chat_store::valid_id(chat) {
        return Err(Error::Invalid("The chat id is invalid."));
    }
    Ok(format!("{}/c-{chat}.json", folder(home, project)?))
}

/// Note that `chat` is in `project` when the project is shared, so every
/// member finds it. `viewer` is the owner of the request that put it
/// there.
pub(crate) async fn note(
    app: &App,
    project: &Project,
    viewer: &str,
    chat: &Conversation,
) -> Result<(), Error> {
    if !shared(project) || chat.project.as_deref() != Some(project.id.as_str()) {
        return Ok(());
    }
    write(
        &app.config.chat_store,
        &home_owner(project, viewer),
        &project.id,
        chat,
    )
    .await
}

async fn write(store: &Store, home: &str, project: &str, chat: &Conversation) -> Result<(), Error> {
    let key = key(home, project, &chat.id)?;
    if store.read_key(&key).await?.is_some() {
        return Ok(());
    }
    let entry = Entry {
        schema: SCHEMA.into(),
        chat: chat.id.clone(),
        owner: chat.owner.clone(),
    };
    let bytes =
        serde_json::to_vec(&entry).map_err(|_| Error::Invalid("The entry could not be saved."))?;
    match store.write_key(&key, bytes, None).await {
        Ok(_) | Err(Error::Conflict) => Ok(()),
        Err(error) => Err(error),
    }
}

/// When someone joins: the chats `starter` already has in the project
/// move in with it.
pub(crate) async fn take_in(
    app: &App,
    project: &str,
    home: &str,
    starter: &str,
) -> Result<(), Error> {
    let store = &app.config.chat_store;
    for chat in store.list(starter).await? {
        if chat.project.as_deref() == Some(project) {
            write(store, home, project, &chat).await?;
        }
    }
    Ok(())
}

async fn entries(store: &Store, home: &str, project: &str) -> Result<Vec<Entry>, Error> {
    let mut found = Vec::new();
    for key in store.folder_keys(&folder(home, project)?).await? {
        let name = key.rsplit('/').next().unwrap_or_default();
        if !name.starts_with("c-") {
            continue;
        }
        if let Some((bytes, _)) = store.read_key(&key).await?
            && let Ok(entry) = serde_json::from_slice::<Entry>(&bytes)
            && entry.schema == SCHEMA
        {
            found.push(entry);
            if found.len() >= MAX_CHATS {
                break;
            }
        }
    }
    Ok(found)
}

/// Whether `chat` is a live chat of `project`.
fn in_project(chat: &Conversation, project: &str) -> bool {
    chat.project.as_deref() == Some(project) && !chat.deleted()
}

/// The project's chats as `viewer` sees them: their own chats in it (from
/// `own`), and, in a shared project, everyone's.
pub(crate) async fn chats_in(
    app: &App,
    project: &Project,
    viewer: &str,
    own: &[Conversation],
) -> Result<Vec<Conversation>, Error> {
    let mut chats: Vec<Conversation> = own
        .iter()
        .filter(|chat| in_project(chat, &project.id))
        .cloned()
        .collect();
    if !shared(project) {
        return Ok(chats);
    }
    let store = &app.config.chat_store;
    let home = home_owner(project, viewer);
    for entry in entries(store, &home, &project.id).await? {
        if chats.iter().any(|c| c.id == entry.chat) {
            continue;
        }
        if let Ok(Some(loaded)) = store.load(&entry.owner, &entry.chat).await
            && in_project(&loaded.conversation, &project.id)
        {
            chats.push(loaded.conversation);
        }
    }
    Ok(chats)
}

/// Every shared project's chats `viewer` can see but didn't start.
pub(crate) async fn others(app: &App, projects: &[Project], viewer: &str) -> Vec<Conversation> {
    let mut found = Vec::new();
    for project in projects.iter().filter(|p| shared(p)) {
        if let Ok(chats) = chats_in(app, project, viewer, &[]).await {
            found.extend(chats.into_iter().filter(|chat| chat.owner != viewer));
        }
    }
    found
}

/// A chat someone else started in a shared project `viewer` is in, with
/// that project.
pub(crate) async fn find(
    app: &App,
    projects: &[Project],
    viewer: &str,
    id: &str,
) -> Result<Option<(Loaded, Project)>, Error> {
    let store = &app.config.chat_store;
    for project in projects.iter().filter(|p| shared(p)) {
        let Ok(key) = key(&home_owner(project, viewer), &project.id, id) else {
            return Ok(None);
        };
        let Some((bytes, _)) = store.read_key(&key).await? else {
            continue;
        };
        let Ok(entry) = serde_json::from_slice::<Entry>(&bytes) else {
            continue;
        };
        if entry.chat != id {
            continue;
        }
        if let Some(loaded) = store.load(&entry.owner, id).await?
            && in_project(&loaded.conversation, &project.id)
        {
            return Ok(Some((loaded, project.clone())));
        }
    }
    Ok(None)
}

/// The signed-in person's own token on this request: an app's bearer, or
/// the browser's session cookie.
pub(crate) fn token(headers: &HeaderMap) -> Option<String> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .map(str::to_owned);
    let cookie = || {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .find(|(name, _)| *name == "oa_cloud_session")
            .map(|(_, value)| value.to_owned())
    };
    bearer
        .or_else(cookie)
        .filter(|t| crate::cloud::session::session_token(t))
}

type Names = HashMap<String, String>;

fn kept() -> &'static Mutex<HashMap<String, (Names, Instant)>> {
    static KEPT: OnceLock<Mutex<HashMap<String, (Names, Instant)>>> = OnceLock::new();
    KEPT.get_or_init(Default::default)
}

/// The project's members' names by their chat-store owner, read once a
/// minute: who started each chat. Empty when they can't be read.
pub(crate) async fn starters(app: &App, headers: &HeaderMap, project: &Project) -> Names {
    const FOR: Duration = Duration::from_secs(60);
    if !shared(project) {
        return Names::new();
    }
    if let Ok(kept) = kept().lock()
        && let Some((names, at)) = kept.get(&project.id)
        && at.elapsed() < FOR
    {
        return names.clone();
    }
    let (Some(service), Some(token)) = (app.config.cloud.as_deref(), token(headers)) else {
        return Names::new();
    };
    let Ok(body) = service
        .project_call_as(
            &token,
            reqwest::Method::GET,
            &format!("/v1/projects/{}/members", project.id),
            None,
        )
        .await
    else {
        return Names::new();
    };
    let names: Names = body["members"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some((
                        account_owner(row["account"].as_str()?),
                        row["name"].as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    if let Ok(mut kept) = kept().lock() {
        kept.retain(|_, (_, at)| at.elapsed() < FOR);
        kept.insert(project.id.clone(), (names.clone(), Instant::now()));
    }
    names
}

/// Who started `chat`, for showing: "You", a member's name, or "A former
/// member".
pub(crate) fn started_by(chat: &Conversation, viewer: &str, names: &Names) -> String {
    if chat.owner == viewer {
        return "You".into();
    }
    names
        .get(&chat.owner)
        .cloned()
        .unwrap_or_else(|| "A former member".into())
}

/// A bounded page of the shared project's index, including archived chats.
pub(crate) async fn history_page<T>(
    app: &App,
    project: &Project,
    viewer: &str,
    after: Option<&str>,
    limit: usize,
    map: impl Fn(&Conversation) -> T,
) -> Result<(Vec<T>, Option<String>), Error> {
    let store = &app.config.chat_store;
    let home = home_owner(project, viewer);
    let (ids, next) = store
        .chat_ids_page(&folder(&home, &project.id)?, true, after, limit)
        .await?;
    let mut chats = Vec::new();
    for id in ids {
        let Some((bytes, _)) = store.read_key(&key(&home, &project.id, &id)?).await? else {
            continue;
        };
        let entry: Entry = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Corrupt("The chat list is invalid."))?;
        if entry.schema != SCHEMA || entry.chat != id {
            return Err(Error::Corrupt("The chat list is invalid."));
        }
        if let Some(loaded) = store.load(&entry.owner, &id).await?
            && in_project(&loaded.conversation, &project.id)
            && !crate::test_runs::hidden(store, &entry.owner)
                .await?
                .contains(&id)
        {
            chats.push(map(&loaded.conversation));
        }
    }
    Ok((chats, next))
}
