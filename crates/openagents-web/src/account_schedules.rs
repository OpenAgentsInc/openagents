//! Scheduled prompts on the account (#11177): prompts that run on the
//! person's computers on a schedule ("weekdays at 09:00", "every 2 hours"),
//! listed, made, paused, and deleted here (Settings, Scheduled prompts) and
//! in the apps. Each one runs on the computer it names, as a host
//! background rule Coder keeps there (`coder-new` `/schedule`): Coder sends
//! that computer's prompts while its sync choice is on and applies this
//! list back (`coder-new` `schedule_sync`), so the computer stays the one
//! that runs them. A prompt either starts a new Coder run or posts into one
//! of that computer's existing chats, which then shows the answer.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /v1/schedules/sync` `{computer, schedules: [record]}` | Coder sends its computer's prompts and deletions; the account keeps the newer of each by id (a delete wins a tie) and answers with that computer's whole list, `200 {schedules: [record]}` |
//! | `GET /v1/schedules` | For the apps: `200 {schedules, computers, chats}`: the live prompts, newest first; the computers that run Coder, most recently seen first; and their chats (`{session, computer, title}`, newest first) |
//! | `PUT /v1/schedules/{id}` `{computer, prompt, days, time, every_secs, chat, paused}` | Save one (`new` makes one). `200 {schedule}`; `400 invalid`; `404 unknown`; `413 full`; `422 secret` |
//! | `POST /v1/schedules/{id}/pause` `{paused}` | Pause or resume one. `200 {schedule}`; `404 unknown` |
//! | `DELETE /v1/schedules/{id}` | Delete one: kept as a deletion so its computer removes it. `200 {deleted}` |
//! | `GET /settings/schedules` | The person's prompts, each with Pause or Resume and Delete, and a form for a new one |
//! | `POST /settings/schedules/save`, `/pause`, `/delete` | The page's forms |
//!
//! Each `/v1/schedules` route also answers at `/coder/schedules`, beside
//! Coder's other account routes.
//!
//! A record is `{id, computer, prompt, days, time, every_secs, chat,
//! chat_title, workspace, paused, updated, deleted}`: `id` is Coder's rule
//! id (`prompt-…`); `time` is `HH:MM` on the computer's clock, on `days`
//! (0 Sunday to 6 Saturday, every day when empty), or `every_secs` (at
//! least a minute) instead; `chat` is the session id of the chat it posts
//! into (a new Coder run when unset); `updated` is Unix seconds. A
//! deletion carries only its id, computer, and time. The routes take
//! `Authorization: Bearer sess_…` (Coder's or an app's own token). The
//! list is private to the account: one object beside its chats, never
//! listed or shared. A prompt that looks like it holds a credential is
//! never kept.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::{Field, Input, InputType, Select, Textarea};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::{Error, Store, account_owner, now_unix};
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::{SessionError, Viewer};
use crate::coder_sync::{answer, line, refused, stored, valid_session};
use crate::settings::{page, viewer};
use crate::ui_page::action_link;

pub(crate) const PAGE: &str = "/settings/schedules";
const SAVE: &str = "/settings/schedules/save";
const PAUSE: &str = "/settings/schedules/pause";
const DELETE: &str = "/settings/schedules/delete";
/// The apps' and Coder's routes, under `/v1` (#11158).
const API: &str = "/v1/schedules";
const SYNC: &str = "/v1/schedules/sync";
/// The same, beside Coder's other account routes.
const CODER_API: &str = "/coder/schedules";
const CODER_SYNC: &str = "/coder/schedules/sync";
const SAVE_SCOPE: &str = "schedule-save";
const PAUSE_SCOPE: &str = "schedule-pause";
const DELETE_SCOPE: &str = "schedule-delete";

/// Where the list lives, under the account's folder.
const KEY: &str = "schedules/list.json";
const SCHEMA: &str = "openagents.web.schedules.v1";
/// The most live prompts an account keeps; a new one past it is not kept.
pub(crate) const MAX_SCHEDULES: usize = 200;
/// The most deletions an account remembers; the oldest go first.
const MAX_DELETIONS: usize = 500;
/// The longest prompt, as a background rule keeps it.
const MAX_PROMPT_BYTES: usize = 4000;
const MAX_COMPUTER_CHARS: usize = 64;
const MAX_TITLE_CHARS: usize = 120;
/// The longest interval: thirty days.
const MAX_EVERY_SECS: u64 = 30 * 24 * 3600;
/// The most chats offered to post into.
const MAX_CHATS: usize = 100;
/// The largest exchange Coder sends.
const MAX_UPLOAD: usize = 2 * 1024 * 1024;
/// How far ahead of this server's clock a change's time may be.
const FUTURE_SLACK: u64 = 24 * 60 * 60;
/// A rule id Coder makes for a scheduled prompt starts with this.
const PREFIX: &str = "prompt-";

/// Whether this site answers `path` itself (never the upstream).
pub(crate) fn owns(path: &str) -> bool {
    path.strip_prefix(API)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// One scheduled prompt, or the fact that one was deleted, as Coder's
/// sync record has it (`coder-new` `schedule_sync::Record`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Schedule {
    pub id: String,
    pub computer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Serialize, Deserialize)]
struct Record {
    schema: String,
    schedules: Vec<Schedule>,
}

/// Coder's ids for scheduled prompts: `prompt-` and lowercase letters,
/// digits, and dashes, up to 40 bytes (a background rule's id).
fn valid_id(id: &str) -> bool {
    id.starts_with(PREFIX)
        && id.len() <= 40
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn credential(text: &str) -> bool {
    secret_screen::credential_in(text).is_some()
}

/// `text` as a time of day, `HH:MM`: `9`, `9am`, `9:30 pm`, `18:30`.
pub(crate) fn time_of_day(text: &str) -> Option<String> {
    let text = text.trim().to_ascii_lowercase().replace('.', "");
    let (clock, meridiem) = if let Some(base) = text.strip_suffix("am") {
        (base.trim(), Some(false))
    } else if let Some(base) = text.strip_suffix("pm") {
        (base.trim(), Some(true))
    } else {
        (text.as_str(), None)
    };
    let (hours, minutes) = match clock.split_once(':') {
        Some((h, m)) if m.len() == 2 => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        Some(_) => return None,
        None => (clock.parse::<u32>().ok()?, 0),
    };
    if clock.is_empty() || minutes > 59 {
        return None;
    }
    let hours = match meridiem {
        Some(_) if hours == 0 || hours > 12 => return None,
        Some(false) => hours % 12,
        Some(true) => hours % 12 + 12,
        None if hours > 23 => return None,
        None => hours,
    };
    Some(format!("{hours:02}:{minutes:02}"))
}

/// Why a prompt isn't kept.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Secret,
    Invalid(&'static str),
}

/// `schedule` as the account keeps it, or why not: a known id and
/// computer, a time, one prompt within its bound with no text that looks
/// like a credential, a time of day or an interval (not both), and a chat
/// id when it posts into a chat. A deletion keeps only its id, computer,
/// and time.
fn checked(mut schedule: Schedule, now: u64) -> Result<Schedule, Refusal> {
    if !valid_id(&schedule.id) {
        return Err(Refusal::Invalid("A scheduled prompt has an invalid id."));
    }
    schedule.computer = line(&schedule.computer, MAX_COMPUTER_CHARS);
    if schedule.computer.is_empty() {
        return Err(Refusal::Invalid("Pick the computer it runs on."));
    }
    if schedule.updated == 0 || schedule.updated > now + FUTURE_SLACK {
        return Err(Refusal::Invalid("A scheduled prompt has an invalid time."));
    }
    if schedule.deleted {
        return Ok(Schedule {
            id: schedule.id,
            computer: schedule.computer,
            updated: schedule.updated,
            deleted: true,
            ..Schedule::default()
        });
    }
    let prompt = schedule
        .prompt
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if prompt.is_empty() {
        return Err(Refusal::Invalid("Write the prompt to run."));
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(Refusal::Invalid("That prompt is longer than 4000 bytes."));
    }
    if credential(&prompt) {
        return Err(Refusal::Secret);
    }
    schedule.prompt = Some(prompt);
    match (schedule.time.as_deref(), schedule.every_secs) {
        (Some(time), None) => {
            schedule.time =
                Some(time_of_day(time).ok_or(Refusal::Invalid("Name a time such as 09:00."))?);
            if schedule.days.iter().any(|day| *day > 6) {
                return Err(Refusal::Invalid("A day is 0 (Sunday) to 6 (Saturday)."));
            }
            schedule.days.sort_unstable();
            schedule.days.dedup();
            if schedule.days.len() == 7 {
                schedule.days.clear();
            }
        }
        (None, Some(seconds)) => {
            if !(60..=MAX_EVERY_SECS).contains(&seconds) {
                return Err(Refusal::Invalid(
                    "A scheduled prompt runs at most once a minute and at least once a month.",
                ));
            }
            schedule.days.clear();
        }
        _ => {
            return Err(Refusal::Invalid(
                "Pick a time of day or an interval for it to run.",
            ));
        }
    }
    if schedule
        .chat
        .as_deref()
        .is_some_and(|chat| !valid_session(chat))
    {
        return Err(Refusal::Invalid("That chat can't be posted into."));
    }
    schedule.chat_title = schedule
        .chat
        .as_ref()
        .and(schedule.chat_title.as_deref())
        .map(|title| line(title, MAX_TITLE_CHARS))
        .filter(|title| !title.is_empty() && !credential(title));
    schedule.workspace = schedule.workspace.filter(|workspace| {
        (workspace.starts_with('/') || workspace.starts_with("~/"))
            && !workspace.contains("..")
            && workspace.len() <= 512
            && !workspace.chars().any(char::is_control)
    });
    Ok(schedule)
}

/// Keep `incoming` when it is newer than the prompt with its id (a delete
/// wins a tie), or new and there is room. Returns whether anything
/// changed.
fn merge(schedules: &mut Vec<Schedule>, incoming: Schedule) -> bool {
    match schedules.iter_mut().find(|have| have.id == incoming.id) {
        Some(have) => {
            let newer = incoming.updated > have.updated
                || (incoming.updated == have.updated && incoming.deleted && !have.deleted);
            if !newer || *have == incoming {
                return false;
            }
            *have = incoming;
            true
        }
        None => {
            if !incoming.deleted
                && schedules.iter().filter(|have| !have.deleted).count() >= MAX_SCHEDULES
            {
                return false;
            }
            schedules.push(incoming);
            true
        }
    }
}

/// Forget the oldest deletions past [`MAX_DELETIONS`].
fn prune(schedules: &mut Vec<Schedule>) {
    let mut deletions: Vec<(u64, String)> = schedules
        .iter()
        .filter(|s| s.deleted)
        .map(|s| (s.updated, s.id.clone()))
        .collect();
    if deletions.len() <= MAX_DELETIONS {
        return;
    }
    deletions.sort();
    let over = deletions.len() - MAX_DELETIONS;
    let gone: Vec<String> = deletions.into_iter().take(over).map(|(_, id)| id).collect();
    schedules.retain(|s| !(s.deleted && gone.contains(&s.id)));
}

/// Newest first.
fn newest_first(mut schedules: Vec<Schedule>) -> Vec<Schedule> {
    schedules.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    schedules
}

/// Read the account's list and change it with `change` (which says
/// whether it changed anything and what to answer), retrying on a
/// concurrent write.
async fn update<T>(
    store: &Store,
    owner: &str,
    change: impl Fn(&mut Vec<Schedule>) -> (bool, T),
) -> Result<T, Error> {
    let key = Store::owner_key(owner, KEY)?;
    for _ in 0..4 {
        let (mut schedules, generation) = match store.read_key(&key).await? {
            Some((bytes, generation)) => {
                let record: Record = serde_json::from_slice(&bytes)
                    .map_err(|_| Error::Corrupt("The scheduled prompts are invalid."))?;
                if record.schema != SCHEMA {
                    return Err(Error::Corrupt("The scheduled prompts are invalid."));
                }
                (record.schedules, Some(generation))
            }
            None => (Vec::new(), None),
        };
        let (changed, result) = change(&mut schedules);
        if !changed {
            return Ok(result);
        }
        prune(&mut schedules);
        let bytes = serde_json::to_vec(&Record {
            schema: SCHEMA.to_owned(),
            schedules,
        })
        .map_err(|_| Error::Invalid("The scheduled prompts are invalid."))?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// The account's prompts and deletions, newest first.
pub(crate) async fn schedules(store: &Store, owner: &str) -> Result<Vec<Schedule>, Error> {
    update(store, owner, |list| (false, newest_first(list.clone()))).await
}

/// Merge what Coder on `computer` sent (records it can't keep, or for
/// another computer, are skipped) and answer with that computer's whole
/// list, newest first.
pub(crate) async fn exchange(
    store: &Store,
    owner: &str,
    computer: &str,
    sent: &[Value],
) -> Result<Vec<Schedule>, Error> {
    let now = now_unix();
    let computer = line(computer, MAX_COMPUTER_CHARS);
    let incoming: Vec<Schedule> = sent
        .iter()
        .filter_map(|value| serde_json::from_value::<Schedule>(value.clone()).ok())
        .filter_map(|schedule| checked(schedule, now).ok())
        .filter(|schedule| schedule.computer == computer)
        .collect();
    update(store, owner, |list| {
        let mut changed = false;
        for schedule in &incoming {
            changed |= merge(list, schedule.clone());
        }
        (changed, ())
    })
    .await?;
    Ok(schedules(store, owner)
        .await?
        .into_iter()
        .filter(|schedule| schedule.computer == computer)
        .collect())
}

/// One of the account's Coder chats, to post into.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Chat {
    pub session: String,
    pub computer: String,
    pub title: String,
}

/// The computers that run Coder for the account, most recently seen
/// first, and their chats, newest first. Phone chats aren't a computer's.
async fn computers_and_chats(
    store: &Store,
    owner: &str,
) -> Result<(Vec<String>, Vec<Chat>), Error> {
    let seen = store.computers(owner).await?.seen;
    let mut computers: Vec<(String, u64)> = seen.into_iter().collect();
    computers.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut chats: Vec<(u64, Chat)> = store
        .list(owner)
        .await?
        .into_iter()
        .filter_map(|chat| {
            let terminal = chat.terminal?;
            if crate::phone_api::phone_session(&terminal.session) {
                return None;
            }
            Some((
                chat.updated_unix,
                Chat {
                    session: terminal.session,
                    computer: terminal.computer,
                    title: line(&chat.title, MAX_TITLE_CHARS),
                },
            ))
        })
        .collect();
    chats.sort_by(|a, b| b.0.cmp(&a.0));
    let mut names: Vec<String> = computers.into_iter().map(|(name, _)| name).collect();
    for (_, chat) in &chats {
        if !names.contains(&chat.computer) {
            names.push(chat.computer.clone());
        }
    }
    Ok((
        names,
        chats
            .into_iter()
            .map(|(_, chat)| chat)
            .take(MAX_CHATS)
            .collect(),
    ))
}

/// The live prompts with each chat's current title.
fn live_with_titles(schedules: Vec<Schedule>, chats: &[Chat]) -> Vec<Schedule> {
    schedules
        .into_iter()
        .filter(|schedule| !schedule.deleted)
        .map(|mut schedule| {
            if let Some(chat) = schedule
                .chat
                .as_ref()
                .and_then(|session| chats.iter().find(|chat| &chat.session == session))
            {
                schedule.chat_title = Some(chat.title.clone());
            }
            schedule
        })
        .collect()
}

/// What a person changes about one prompt, on the page or from an app.
#[derive(Debug, Default)]
pub(crate) struct Edit {
    /// `None` makes a new one.
    pub id: Option<String>,
    pub computer: String,
    pub prompt: String,
    pub days: Vec<u8>,
    pub time: Option<String>,
    pub every_secs: Option<u64>,
    pub chat: Option<String>,
    pub paused: bool,
}

/// The new id for a prompt made here, in Coder's shape.
fn new_id() -> String {
    let random: String = fresh_request()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .take(12)
        .collect();
    format!("{PREFIX}{random}")
}

/// What became of a save.
#[derive(Debug, PartialEq, Eq)]
enum Saved {
    Saved(Schedule),
    Unknown,
    Full,
    Refused(Refusal),
}

/// Save `edit`: a change keeps its computer and is newer than what is
/// kept; a new one runs on the computer it names. A chat must be one of
/// that computer's.
async fn save(store: &Store, owner: &str, edit: &Edit, chats: &[Chat]) -> Result<Saved, Error> {
    update(store, owner, |list| {
        let now = now_unix();
        let existing = edit
            .id
            .as_ref()
            .and_then(|id| list.iter().find(|have| &have.id == id && !have.deleted));
        let (id, computer, workspace, updated) = match (&edit.id, existing) {
            (Some(_), None) => return (false, Saved::Unknown),
            (Some(id), Some(have)) => {
                let computer = line(&edit.computer, MAX_COMPUTER_CHARS);
                if !computer.is_empty() && computer != have.computer {
                    return (
                        false,
                        Saved::Refused(Refusal::Invalid(
                            "A scheduled prompt stays on its computer. Make a new one for another computer.",
                        )),
                    );
                }
                (
                    id.clone(),
                    have.computer.clone(),
                    have.workspace.clone(),
                    now.max(have.updated + 1),
                )
            }
            (None, _) => (new_id(), edit.computer.clone(), None, now),
        };
        let chat_title = match &edit.chat {
            None => None,
            Some(session) => match chats
                .iter()
                .find(|chat| &chat.session == session && chat.computer == line(&computer, MAX_COMPUTER_CHARS))
            {
                Some(chat) => Some(chat.title.clone()),
                None => {
                    return (
                        false,
                        Saved::Refused(Refusal::Invalid(
                            "Pick one of that computer's chats, or a new Coder run.",
                        )),
                    );
                }
            },
        };
        let schedule = Schedule {
            id,
            computer,
            prompt: Some(edit.prompt.clone()),
            days: edit.days.clone(),
            time: edit.time.clone(),
            every_secs: edit.every_secs,
            chat: edit.chat.clone(),
            chat_title,
            workspace,
            paused: edit.paused,
            updated,
            deleted: false,
        };
        let schedule = match checked(schedule, now) {
            Ok(schedule) => schedule,
            Err(refusal) => return (false, Saved::Refused(refusal)),
        };
        if merge(list, schedule.clone()) {
            (true, Saved::Saved(schedule))
        } else if edit.id.is_none() {
            (false, Saved::Full)
        } else {
            (false, Saved::Saved(schedule))
        }
    })
    .await
}

/// Pause or resume the prompt `id`. `None` when there is no such prompt.
async fn set_paused(
    store: &Store,
    owner: &str,
    id: &str,
    paused: bool,
) -> Result<Option<Schedule>, Error> {
    update(store, owner, |list| {
        let Some(have) = list.iter().find(|have| have.id == id && !have.deleted) else {
            return (false, None);
        };
        if have.paused == paused {
            return (false, Some(have.clone()));
        }
        let changed = Schedule {
            paused,
            updated: now_unix().max(have.updated + 1),
            ..have.clone()
        };
        (merge(list, changed.clone()), Some(changed))
    })
    .await
}

/// Delete the prompt `id` everywhere: it is kept as a deletion newer than
/// the prompt, so its computer removes it. Returns false when there was no
/// such prompt.
async fn forget(store: &Store, owner: &str, id: &str) -> Result<bool, Error> {
    update(store, owner, |list| {
        let Some(have) = list.iter().find(|have| have.id == id && !have.deleted) else {
            return (false, false);
        };
        let gone = Schedule {
            id: have.id.clone(),
            computer: have.computer.clone(),
            updated: now_unix().max(have.updated + 1),
            deleted: true,
            ..Schedule::default()
        };
        (merge(list, gone), true)
    })
    .await
}

/// When a prompt runs, in plain words: "Weekdays at 09:00", "Every 2
/// hours".
pub(crate) fn when_words(schedule: &Schedule) -> String {
    if let Some(seconds) = schedule.every_secs {
        return every_words(seconds);
    }
    let time = schedule.time.as_deref().unwrap_or("?");
    match schedule.days.as_slice() {
        [] => format!("Every day at {time}"),
        [1, 2, 3, 4, 5] => format!("Weekdays at {time}"),
        [0, 6] => format!("Weekends at {time}"),
        days => {
            const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            let names: Vec<&str> = days
                .iter()
                .filter_map(|day| NAMES.get(usize::from(*day)).copied())
                .collect();
            format!("{} at {time}", names.join(", "))
        }
    }
}

fn every_words(seconds: u64) -> String {
    let (count, unit) = if seconds % 86_400 == 0 {
        (seconds / 86_400, "day")
    } else if seconds % 3600 == 0 {
        (seconds / 3600, "hour")
    } else {
        (seconds / 60, "minute")
    };
    if count == 1 {
        format!("Every {unit}")
    } else {
        format!("Every {count} {unit}s")
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(API, get(api_list))
        .route(SYNC, post(api_sync))
        .route(
            &format!("{API}/{{id}}"),
            axum::routing::put(api_put).delete(api_delete),
        )
        .route(&format!("{API}/{{id}}/pause"), post(api_pause))
        .route(CODER_API, get(api_list))
        .route(CODER_SYNC, post(api_sync))
        .route(
            &format!("{CODER_API}/{{id}}"),
            axum::routing::put(api_put).delete(api_delete),
        )
        .route(&format!("{CODER_API}/{{id}}/pause"), post(api_pause))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
        .route(PAGE, get(schedules_page))
        .route(SAVE, post(save_form))
        .route(PAUSE, post(pause_form))
        .route(DELETE, post(delete_form))
}

async fn api_list(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let store = &app.config.chat_store;
    let listed = match schedules(store, &owner).await {
        Ok(listed) => listed,
        Err(error) => return stored(&error),
    };
    match computers_and_chats(store, &owner).await {
        Ok((computers, chats)) => answer(
            StatusCode::OK,
            json!({
                "schedules": live_with_titles(listed, &chats),
                "computers": computers,
                "chats": chats,
            }),
        ),
        Err(error) => stored(&error),
    }
}

async fn api_sync(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Some((computer, sent)) = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|body| {
            Some((
                body["computer"].as_str()?.to_owned(),
                body["schedules"].as_array()?.clone(),
            ))
        })
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {computer, schedules}.",
        );
    };
    if sent.len() > MAX_SCHEDULES + MAX_DELETIONS {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "Send at most 700 scheduled prompts at once.",
        );
    }
    match exchange(&app.config.chat_store, &owner, &computer, &sent).await {
        Ok(list) => answer(StatusCode::OK, json!({ "schedules": list })),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct Put {
    #[serde(default)]
    computer: String,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    days: Vec<u8>,
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    every_secs: Option<u64>,
    #[serde(default)]
    chat: Option<String>,
    #[serde(default)]
    paused: bool,
}

/// The answer to a save, for the apps.
fn saved_answer(saved: Result<Saved, Error>) -> Response {
    match saved {
        Ok(Saved::Saved(schedule)) => answer(StatusCode::OK, json!({ "schedule": schedule })),
        Ok(Saved::Unknown) => refused(
            StatusCode::NOT_FOUND,
            "unknown",
            "No such scheduled prompt.",
        ),
        Ok(Saved::Full) => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "full",
            "You have as many scheduled prompts as an account keeps. Delete some first.",
        ),
        Ok(Saved::Refused(Refusal::Secret)) => refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "That prompt looks like it holds a password or key, so it wasn't saved.",
        ),
        Ok(Saved::Refused(Refusal::Invalid(message))) => {
            refused(StatusCode::BAD_REQUEST, "invalid", message)
        }
        Err(error) => stored(&error),
    }
}

async fn api_put(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<Put>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {computer, prompt, days, time, every_secs, chat, paused}.",
        );
    };
    let store = &app.config.chat_store;
    let chats = match computers_and_chats(store, &owner).await {
        Ok((_, chats)) => chats,
        Err(error) => return stored(&error),
    };
    let edit = Edit {
        id: (id != "new").then_some(id),
        computer: sent.computer,
        prompt: sent.prompt,
        days: sent.days,
        time: sent.time,
        every_secs: sent.every_secs,
        chat: sent.chat.filter(|chat| !chat.is_empty()),
        paused: sent.paused,
    };
    saved_answer(save(store, &owner, &edit, &chats).await)
}

async fn api_pause(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Some(paused) = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|body| body["paused"].as_bool())
    else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {paused}.");
    };
    match set_paused(&app.config.chat_store, &owner, &id, paused).await {
        Ok(Some(schedule)) => answer(StatusCode::OK, json!({ "schedule": schedule })),
        Ok(None) => refused(
            StatusCode::NOT_FOUND,
            "unknown",
            "No such scheduled prompt.",
        ),
        Err(error) => stored(&error),
    }
}

async fn api_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match forget(&app.config.chat_store, &owner, &id).await {
        Ok(deleted) => answer(StatusCode::OK, json!({ "deleted": deleted })),
        Err(error) => stored(&error),
    }
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Scheduled prompts",
        text,
        (PAGE, "Scheduled prompts"),
    ))
}

fn unavailable_page(error: &Error) -> Response {
    eprintln!("openagents-web: schedules: {error}");
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "Your scheduled prompts can't be read right now. Try again in a minute.",
    )
}

fn target(viewer: &Viewer, request: &str) -> String {
    format!("{}:schedules:{request}", viewer.account_id)
}

/// A form's ticket: its CSRF token and request id.
type Ticket<'a> = (&'a str, &'a str);

async fn schedules_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let store = &app.config.chat_store;
    let listed = match schedules(store, &owner).await {
        Ok(listed) => listed,
        Err(error) => return unavailable_page(&error),
    };
    let (computers, chats) = match computers_and_chats(store, &owner).await {
        Ok(found) => found,
        Err(error) => return unavailable_page(&error),
    };
    let mut tickets = Vec::new();
    for scope in [SAVE_SCOPE, PAUSE_SCOPE, DELETE_SCOPE] {
        let request = fresh_request();
        match service.csrf(&headers, &viewer, scope, &target(&viewer, &request)) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let live = live_with_titles(listed, &chats);
    let body = schedules_content(
        &live,
        &computers,
        &chats,
        [
            (&tickets[0].0, &tickets[0].1),
            (&tickets[1].0, &tickets[1].1),
            (&tickets[2].0, &tickets[2].1),
        ],
    );
    page(&headers, service, &viewer, "Scheduled prompts", PAGE, body)
}

/// The intervals the page offers, in seconds and words.
const EVERY: [(u64, &str); 7] = [
    (900, "Every 15 minutes"),
    (1800, "Every 30 minutes"),
    (3600, "Every hour"),
    (7200, "Every 2 hours"),
    (14_400, "Every 4 hours"),
    (21_600, "Every 6 hours"),
    (43_200, "Every 12 hours"),
];

/// The form for a new prompt.
fn new_form(computers: &[String], chats: &[Chat], save: Ticket<'_>) -> Markup {
    let prompt = Field::new("schedule-prompt", "Prompt");
    let computer = Field::new("schedule-computer", "Runs on");
    let repeat = Field::new("schedule-repeat", "When");
    let time = Field::new("schedule-time", "At").description(
        "A time of day on that computer's clock, such as 09:00 or 6:30pm. Not used for \"every …\".",
    );
    let place = Field::new("schedule-chat", "Where the answer goes");
    let mut computer_select = Select::new("computer").aria(computer.aria());
    for name in computers {
        computer_select = computer_select.option(name.as_str(), name.as_str());
    }
    let mut repeat_select = Select::new("repeat")
        .aria(repeat.aria())
        .option("daily", "Every day")
        .option("weekdays", "Weekdays")
        .option("weekends", "Weekends");
    for (seconds, words) in EVERY {
        repeat_select = repeat_select.option(format!("every-{seconds}"), words);
    }
    let mut chat_select = Select::new("chat")
        .aria(place.aria())
        .option("", "Start a new Coder run");
    for chat in chats {
        chat_select = chat_select.option(
            chat.session.as_str(),
            format!(
                "Post in \u{201c}{}\u{201d} on {}",
                chat.title, chat.computer
            ),
        );
    }
    html! {
        form method="post" action=(SAVE) autocomplete="off" {
            input type="hidden" name="csrf" value=(save.0);
            input type="hidden" name="request" value=(save.1);
            (prompt.clone().control(
                Textarea::new("prompt")
                    .rows(3)
                    .maxlength(MAX_PROMPT_BYTES as u32)
                    .required(true)
                    .aria(prompt.aria()),
            ))
            (computer.clone().control(computer_select))
            (repeat.clone().control(repeat_select))
            (time.clone().control(
                Input::new("time")
                    .input_type(InputType::Text)
                    .value("09:00")
                    .maxlength(10)
                    .aria(time.aria()),
            ))
            (place.clone().control(chat_select))
            p { (Button::new("Add scheduled prompt").kind(ButtonType::Submit)) }
        }
    }
}

/// The page: what scheduled prompts are, the person's prompts, and a form
/// for a new one (when a computer can run it).
fn schedules_content(
    schedules: &[Schedule],
    computers: &[String],
    chats: &[Chat],
    [save, pause, delete]: [Ticket<'_>; 3],
) -> Markup {
    let row = |schedule: &Schedule| {
        let place = match (&schedule.chat, &schedule.chat_title) {
            (Some(_), Some(title)) => format!("Posts in \u{201c}{title}\u{201d}"),
            (Some(_), None) => "Posts in one of its chats".to_owned(),
            (None, _) => "Starts a new Coder run".to_owned(),
        };
        html! {
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { (schedule.prompt.as_deref().unwrap_or_default()) }
                    span class="oa-settings-hint" {
                        (when_words(schedule)) " · Runs on " (schedule.computer) " · " (place)
                        @if schedule.paused { " · Paused" }
                    }
                }
                div class="oa-settings-control" {
                    form method="post" action=(PAUSE) {
                        input type="hidden" name="csrf" value=(pause.0);
                        input type="hidden" name="request" value=(pause.1);
                        input type="hidden" name="id" value=(schedule.id);
                        input type="hidden" name="paused" value=(if schedule.paused { "false" } else { "true" });
                        (Button::new(if schedule.paused { "Resume" } else { "Pause" })
                            .kind(ButtonType::Submit)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary))
                    }
                    form method="post" action=(DELETE) {
                        input type="hidden" name="csrf" value=(delete.0);
                        input type="hidden" name="request" value=(delete.1);
                        input type="hidden" name="id" value=(schedule.id);
                        (Button::new("Delete")
                            .kind(ButtonType::Submit)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary))
                    }
                }
            }
        }
    };
    html! {
        p { (action_link("Settings", crate::settings::PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Scheduled prompts" }
            p {
                "Prompts that run on your computers on a schedule, such as every weekday at 9. Each one runs on the computer you pick, while Coder there has sync on (type "
                code { "/sync on" } " in Coder). Make them here, in the app, or with "
                code { "/schedule" } " in Coder."
            }
            p { "A prompt starts a new Coder run, or posts into one of that computer's chats, where the answer shows." }
        }))
        @if schedules.is_empty() {
            p { "You have no scheduled prompts yet." }
        } @else {
            section class="oa-settings-group" aria-labelledby="schedules-list" {
                h2 #schedules-list { "Your prompts" }
                @for schedule in schedules { (row(schedule)) }
            }
        }
        section class="oa-settings-group" aria-labelledby="schedules-add" {
            h2 #schedules-add { "Add a scheduled prompt" }
            @if computers.is_empty() {
                p {
                    "No computer runs Coder for your account yet. Type "
                    code { "/sync on" } " in Coder on your computer, and it shows here."
                }
            } @else {
                (new_form(computers, chats, save))
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveForm {
    csrf: String,
    request: String,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    computer: String,
    #[serde(default)]
    repeat: String,
    #[serde(default)]
    time: String,
    #[serde(default)]
    chat: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PauseForm {
    csrf: String,
    request: String,
    id: String,
    paused: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteForm {
    csrf: String,
    request: String,
    id: String,
}

/// The page's form as an edit: the repeat choice becomes days and a time,
/// or an interval.
fn form_edit(form: SaveForm) -> Result<Edit, &'static str> {
    let (days, time, every_secs) = match form.repeat.as_str() {
        "daily" => (Vec::new(), Some(form.time), None),
        "weekdays" => (vec![1, 2, 3, 4, 5], Some(form.time), None),
        "weekends" => (vec![0, 6], Some(form.time), None),
        other => {
            let seconds = other
                .strip_prefix("every-")
                .and_then(|seconds| seconds.parse::<u64>().ok())
                .filter(|seconds| EVERY.iter().any(|(every, _)| every == seconds))
                .ok_or("Pick when it runs.")?;
            (Vec::new(), None, Some(seconds))
        }
    };
    Ok(Edit {
        id: None,
        computer: form.computer,
        prompt: form.prompt,
        days,
        time,
        every_secs,
        chat: Some(form.chat).filter(|chat| !chat.is_empty()),
        paused: false,
    })
}

async fn save_form(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SaveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        SAVE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    let edit = match form_edit(form) {
        Ok(edit) => edit,
        Err(message) => return problem(StatusCode::BAD_REQUEST, message),
    };
    let store = &app.config.chat_store;
    let chats = match computers_and_chats(store, &owner).await {
        Ok((_, chats)) => chats,
        Err(error) => return unavailable_page(&error),
    };
    match save(store, &owner, &edit, &chats).await {
        Ok(Saved::Saved(_)) => protect(Redirect::to(PAGE).into_response()),
        Ok(Saved::Unknown) => problem(
            StatusCode::NOT_FOUND,
            "That scheduled prompt was deleted, maybe on another computer.",
        ),
        Ok(Saved::Full) => problem(
            StatusCode::PAYLOAD_TOO_LARGE,
            "You have as many scheduled prompts as an account keeps. Delete some first.",
        ),
        Ok(Saved::Refused(Refusal::Secret)) => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "That prompt looks like it holds a password or key, so it wasn't saved.",
        ),
        Ok(Saved::Refused(Refusal::Invalid(message))) => problem(StatusCode::BAD_REQUEST, message),
        Err(error) => unavailable_page(&error),
    }
}

async fn pause_form(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<PauseForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        PAUSE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    let paused = form.paused == "true";
    match set_paused(&app.config.chat_store, &owner, &form.id, paused).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => unavailable_page(&error),
    }
}

async fn delete_form(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<DeleteForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        DELETE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    match forget(&app.config.chat_store, &owner, &form.id).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => unavailable_page(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> String {
        account_owner("acct_schedules")
    }

    fn record(id: &str, computer: &str, prompt: &str, updated: u64) -> Value {
        json!({"id": id, "computer": computer, "prompt": prompt,
               "days": [1, 2, 3, 4, 5], "time": "09:00", "updated": updated})
    }

    fn chat(session: &str, computer: &str, title: &str) -> Chat {
        Chat {
            session: session.into(),
            computer: computer.into(),
            title: title.into(),
        }
    }

    #[test]
    fn times_and_intervals_read_as_plain_words() {
        assert_eq!(time_of_day("9am").as_deref(), Some("09:00"));
        assert_eq!(time_of_day("6:30 pm").as_deref(), Some("18:30"));
        assert_eq!(time_of_day("12am").as_deref(), Some("00:00"));
        assert_eq!(time_of_day("18:05").as_deref(), Some("18:05"));
        assert_eq!(time_of_day("24:00"), None);
        assert_eq!(time_of_day("9:5"), None);
        assert_eq!(time_of_day("soon"), None);
        let at = |days: Vec<u8>| Schedule {
            days,
            time: Some("09:00".into()),
            ..Schedule::default()
        };
        assert_eq!(when_words(&at(vec![])), "Every day at 09:00");
        assert_eq!(when_words(&at(vec![1, 2, 3, 4, 5])), "Weekdays at 09:00");
        assert_eq!(when_words(&at(vec![0, 6])), "Weekends at 09:00");
        assert_eq!(when_words(&at(vec![1, 3])), "Mon, Wed at 09:00");
        let every = |seconds| Schedule {
            every_secs: Some(seconds),
            ..Schedule::default()
        };
        assert_eq!(when_words(&every(7200)), "Every 2 hours");
        assert_eq!(when_words(&every(3600)), "Every hour");
        assert_eq!(when_words(&every(1800)), "Every 30 minutes");
        assert!(valid_id(&new_id()), "{}", new_id());
    }

    #[tokio::test]
    async fn coder_prompts_merge_by_id_per_computer_and_a_delete_wins_a_tie() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let listed = exchange(
            &store,
            &owner(),
            "mac",
            &[
                record("prompt-triage", "mac", "triage the new issues", 100),
                // Another computer's record isn't this computer's to send.
                record("prompt-other", "pc", "x", 100),
                // Nor is one that can't be kept.
                record("prompt-fast", "mac", "", 100),
                json!({"id": "prompt-every", "computer": "mac", "prompt": "p",
                       "every_secs": 30, "updated": 100}),
            ],
        )
        .await
        .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].prompt.as_deref(), Some("triage the new issues"));
        // An older change doesn't replace a newer one.
        let listed = exchange(
            &store,
            &owner(),
            "mac",
            &[record("prompt-triage", "mac", "old", 50)],
        )
        .await
        .unwrap();
        assert_eq!(listed[0].prompt.as_deref(), Some("triage the new issues"));
        // A delete at the same time wins.
        let gone =
            json!({"id": "prompt-triage", "computer": "mac", "updated": 100, "deleted": true});
        let listed = exchange(&store, &owner(), "mac", &[gone]).await.unwrap();
        assert!(listed[0].deleted && listed[0].prompt.is_none());
        // Another account sees none of it.
        assert!(
            schedules(&store, &account_owner("acct_other"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_prompt_made_on_the_website_reaches_its_computer_and_posts_into_a_chat() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let chats = [
            chat("2026-10-10-abc", "mac", "Morning triage"),
            chat("2026-10-10-pc", "pc", "Elsewhere"),
        ];
        let edit = Edit {
            computer: "mac".into(),
            prompt: "what changed overnight?".into(),
            days: vec![5, 4, 3, 2, 1],
            time: Some("9am".into()),
            chat: Some("2026-10-10-abc".into()),
            ..Edit::default()
        };
        let Saved::Saved(made) = save(&store, &owner(), &edit, &chats).await.unwrap() else {
            panic!("not saved");
        };
        assert!(made.id.starts_with(PREFIX) && valid_id(&made.id));
        assert_eq!(made.time.as_deref(), Some("09:00"));
        assert_eq!(made.days, vec![1, 2, 3, 4, 5]);
        assert_eq!(made.chat_title.as_deref(), Some("Morning triage"));
        // Coder on that computer gets it with its next exchange; another
        // computer doesn't.
        let mac = exchange(&store, &owner(), "mac", &[]).await.unwrap();
        assert_eq!(mac.len(), 1);
        assert_eq!(mac[0].chat.as_deref(), Some("2026-10-10-abc"));
        assert!(
            exchange(&store, &owner(), "pc", &[])
                .await
                .unwrap()
                .is_empty()
        );
        // A chat on another computer can't be posted into.
        let elsewhere = Edit {
            chat: Some("2026-10-10-pc".into()),
            ..Edit {
                computer: "mac".into(),
                prompt: "x".into(),
                time: Some("09:00".into()),
                ..Edit::default()
            }
        };
        assert!(matches!(
            save(&store, &owner(), &elsewhere, &chats).await.unwrap(),
            Saved::Refused(Refusal::Invalid(_))
        ));
        // Paused here: newer, so Coder's older copy doesn't undo it.
        let paused = set_paused(&store, &owner(), &made.id, true)
            .await
            .unwrap()
            .unwrap();
        assert!(paused.paused && paused.updated > made.updated);
        let stale = serde_json::to_value(&made).unwrap();
        let mac = exchange(&store, &owner(), "mac", &[stale]).await.unwrap();
        assert!(mac[0].paused);
        assert!(
            set_paused(&store, &owner(), "prompt-none", true)
                .await
                .unwrap()
                .is_none()
        );
        // Deleted here: kept as a deletion for the computer to remove.
        assert!(forget(&store, &owner(), &made.id).await.unwrap());
        assert!(!forget(&store, &owner(), &made.id).await.unwrap());
        let mac = exchange(&store, &owner(), "mac", &[]).await.unwrap();
        assert!(mac[0].deleted);
        // A change to a deleted one is refused.
        let change = Edit {
            id: Some(made.id.clone()),
            ..edit
        };
        assert_eq!(
            save(&store, &owner(), &change, &chats).await.unwrap(),
            Saved::Unknown
        );
    }

    #[tokio::test]
    async fn a_prompt_with_a_key_or_no_time_is_not_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        // Assembled at run time so no credential-shaped literal sits here.
        let key = format!("sk-ant-{}", "a1".repeat(20));
        let secret = Edit {
            computer: "mac".into(),
            prompt: format!("deploy with {key}"),
            every_secs: Some(3600),
            ..Edit::default()
        };
        assert_eq!(
            save(&store, &owner(), &secret, &[]).await.unwrap(),
            Saved::Refused(Refusal::Secret)
        );
        let neither = Edit {
            computer: "mac".into(),
            prompt: "x".into(),
            ..Edit::default()
        };
        assert!(matches!(
            save(&store, &owner(), &neither, &[]).await.unwrap(),
            Saved::Refused(Refusal::Invalid(_))
        ));
        assert!(schedules(&store, &owner()).await.unwrap().is_empty());
    }

    #[test]
    fn the_form_reads_its_repeat_choice() {
        let form = |repeat: &str| SaveForm {
            csrf: String::new(),
            request: String::new(),
            prompt: "p".into(),
            computer: "mac".into(),
            repeat: repeat.into(),
            time: "9am".into(),
            chat: String::new(),
        };
        let weekdays = form_edit(form("weekdays")).unwrap();
        assert_eq!(weekdays.days, vec![1, 2, 3, 4, 5]);
        assert_eq!(weekdays.time.as_deref(), Some("9am"));
        assert!(weekdays.chat.is_none());
        let every = form_edit(form("every-7200")).unwrap();
        assert_eq!(every.every_secs, Some(7200));
        assert!(every.time.is_none());
        assert!(form_edit(form("every-5")).is_err());
        assert!(form_edit(form("sometimes")).is_err());
    }

    #[test]
    fn the_page_lists_prompts_and_offers_a_form_only_with_a_computer() {
        let schedule = Schedule {
            id: "prompt-triage".into(),
            computer: "mac".into(),
            prompt: Some("triage the new issues".into()),
            days: vec![1, 2, 3, 4, 5],
            time: Some("09:00".into()),
            chat: Some("2026-10-10-abc".into()),
            chat_title: Some("Morning triage".into()),
            paused: true,
            updated: 10,
            ..Schedule::default()
        };
        let tickets = [("c", "r"), ("c2", "r2"), ("c3", "r3")];
        let chats = [chat("2026-10-10-abc", "mac", "Morning triage")];
        let html = schedules_content(
            std::slice::from_ref(&schedule),
            &["mac".to_owned()],
            &chats,
            tickets,
        )
        .into_string();
        assert!(html.contains("triage the new issues"));
        assert!(html.contains("Weekdays at 09:00 · Runs on mac · Posts in"));
        assert!(html.contains("Morning triage") && html.contains("· Paused"));
        assert!(html.contains("Resume") && html.contains("Delete"));
        assert!(html.contains("Add scheduled prompt") && html.contains("Start a new Coder run"));
        // No computer: no form to fill that couldn't run.
        let empty = schedules_content(&[], &[], &[], tickets).into_string();
        assert!(empty.contains("no scheduled prompts yet"));
        assert!(!empty.contains("Add scheduled prompt"));
    }
}
