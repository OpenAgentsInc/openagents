//! The signed-in account's chats on openagents.com (#11504): the chats made
//! on the website, the phone, and synced terminals, read through the
//! website's `/v1/threads` routes under the same sign-in `openagents
//! projects` uses. `chat threads --account` lists them, `chat read` and
//! `chat export` fall back to them when this computer has no thread by that
//! name, and `projects read` prints a project's chats message by message.

use serde_json::{Value, json};

use crate::keys::Site;

/// The most list pages read before giving up on a list that keeps going.
const PAGES: usize = 50;

fn get(site: &Site, path: &str) -> Result<Value, String> {
    Site::answer(
        site.http
            .get(site.url(path))
            .bearer_auth(site.saved.token())
            .send(),
    )
}

/// Whether `id` has the shape of an account thread's ID (a UUID).
#[must_use]
pub(crate) fn account_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(part, len)| part.len() == len && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A cursor or ID as it may go into a URL.
fn safe(value: &str) -> Result<&str, String> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        Ok(value)
    } else {
        Err("The website sent an unexpected page cursor.".into())
    }
}

/// One thread row, the same whichever list shape the website sent: `id`,
/// `title`, `surface`, `computer`, `project`, `updated` (Unix seconds),
/// `messages` (when the list counts them), `pinned`, `archived`.
fn row(raw: &Value) -> Value {
    let updated = raw["updated_unix"]
        .as_u64()
        .or_else(|| raw["updated_at"].as_str().and_then(unix_of))
        .unwrap_or(0);
    json!({
        "id": raw["id"],
        "title": raw["title"].as_str().unwrap_or_default(),
        "surface": raw["surface"],
        "computer": raw["computer"],
        "project": raw["project"],
        "updated": updated,
        "messages": raw["messages"],
        "pinned": raw["pinned"].as_bool().unwrap_or(false),
        "archived": raw["archived"].as_bool().unwrap_or(false),
    })
}

/// Unix seconds of an RFC 3339 UTC time such as `2026-10-11T14:22:58Z`.
fn unix_of(text: &str) -> Option<u64> {
    let (date, time) = text.trim_end_matches('Z').split_once('T')?;
    let mut date = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let mut time = time
        .split(':')
        .map(|part| part.split('.').next().unwrap_or_default().parse::<i64>());
    let (hour, minute, second) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

/// The account's threads, or one project's, most recent first, at most
/// `limit` of them.
///
/// # Errors
/// Not signed in, or the website refused or couldn't be reached.
pub(crate) fn threads(
    site: &Site,
    project: Option<&str>,
    limit: usize,
) -> Result<Vec<Value>, String> {
    let base = match project {
        Some(project) => format!("/v1/threads?project={}&limit=200", safe(project)?),
        None => "/v1/threads?limit=200".to_owned(),
    };
    let mut rows = Vec::new();
    let mut next = String::new();
    for _ in 0..PAGES {
        let page = get(site, &format!("{base}{next}"))?;
        rows.extend(
            page["data"]
                .as_array()
                .or_else(|| page["threads"].as_array())
                .into_iter()
                .flatten()
                .map(row),
        );
        next = if let Some(cursor) = page["next"].as_str().filter(|c| !c.is_empty()) {
            format!("&after={}", safe(cursor)?)
        } else if page["more"] == true
            && let Some(offset) = page["next_offset"].as_u64()
        {
            format!("&offset={offset}")
        } else {
            break;
        };
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row["updated"].as_u64().unwrap_or(0)));
    rows.truncate(limit);
    Ok(rows)
}

/// The account thread `arg` names: its ID, a unique ID prefix, or its
/// title.
///
/// # Errors
/// As [`threads`]; or, in words, that no thread or several are named so.
pub(crate) fn find(site: &Site, arg: &str) -> Result<Value, String> {
    let rows = threads(site, None, usize::MAX)?;
    let id = |row: &Value| row["id"].as_str().unwrap_or_default().to_owned();
    if let Some(row) = rows.iter().find(|row| id(row) == arg) {
        return Ok(row.clone());
    }
    let lowered = arg.trim().to_lowercase();
    let prefixed: Vec<&Value> = rows.iter().filter(|row| id(row).starts_with(arg)).collect();
    let titled: Vec<&Value> = rows
        .iter()
        .filter(|row| {
            row["title"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_lowercase()
                == lowered
        })
        .collect();
    for found in [&prefixed, &titled] {
        if let [one] = found.as_slice() {
            return Ok((*one).clone());
        }
    }
    let several = if prefixed.len() > 1 {
        &prefixed
    } else {
        &titled
    };
    if several.len() > 1 {
        let mut message = format!(
            "{} chats in your account are named `{arg}`; give one's ID:",
            several.len()
        );
        for row in several.iter().take(10) {
            message.push_str(&format!("\n  {}  made on {}", id(row), place(row)));
        }
        return Err(message);
    }
    Err(format!(
        "no thread is named `{arg}` on this computer or in your account on openagents.com; \
         give its ID, a unique ID prefix, or its title (`openagents chat threads` and \
         `openagents chat threads --account` list them)"
    ))
}

/// A thread's messages, oldest first, each `{role, text}`. A message's
/// files and artifacts are named after its text.
///
/// # Errors
/// As [`threads`]; an ID that isn't a thread's.
pub(crate) fn messages(site: &Site, id: &str) -> Result<Vec<Value>, String> {
    let id = safe(id)?;
    let mut turns = Vec::new();
    let mut after = String::new();
    for _ in 0..PAGES {
        let page = get(site, &format!("/v1/threads/{id}/messages{after}"))?;
        for message in page["data"].as_array().into_iter().flatten() {
            let mut text = message["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            for key in ["files", "generated_files", "artifacts"] {
                for item in message[key].as_array().into_iter().flatten() {
                    if let Some(name) = item["name"].as_str().or_else(|| item["title"].as_str()) {
                        text.push_str(&format!("\n[{name}]"));
                    }
                }
            }
            turns.push(json!({"role": message["role"], "text": text}));
        }
        match page["next"].as_str().filter(|cursor| !cursor.is_empty()) {
            Some(cursor) => after = format!("?after={}", safe(cursor)?),
            None => break,
        }
    }
    Ok(turns)
}

/// A whole account thread as `chat read --json` prints one.
pub(crate) fn whole(row: &Value, turns: Vec<Value>) -> Value {
    json!({
        "thread": row["id"],
        "title": row["title"],
        "backend": "account",
        "surface": row["surface"],
        "computer": row["computer"],
        "project": row["project"],
        "updated": row["updated"],
        "turns": turns,
    })
}

/// The turns as text: who said it, then what they said.
pub(crate) fn turns_text(turns: &Value) -> String {
    let mut out = String::new();
    for turn in turns.as_array().into_iter().flatten() {
        // A step and the model's thinking (#11543) say what they are.
        let who = match turn["role"].as_str() {
            Some("user") => "you",
            Some("tool") => "step",
            Some("thinking") => "thinking",
            _ => "openagents",
        };
        out.push_str(&format!(
            "\n{who}:\n{}\n",
            turn["text"].as_str().unwrap_or("")
        ));
    }
    out
}

/// Where a thread was made, in words.
pub(crate) fn place(row: &Value) -> String {
    match (row["surface"].as_str(), row["computer"].as_str()) {
        (Some("web"), _) => "the website".into(),
        (Some("phone"), _) => "the phone".into(),
        (Some("terminal"), Some(computer)) => format!("the terminal on {computer}"),
        (Some("terminal"), None) => "a terminal".into(),
        (Some(other), _) => other.to_owned(),
        (None, _) => "openagents.com".into(),
    }
}

/// `chat read` of an account thread, as text.
pub(crate) fn read_text(value: &Value) -> String {
    format!(
        "{}\n(made on {})\n{}\nthread {}",
        value["title"].as_str().unwrap_or(""),
        place(value),
        turns_text(&value["turns"]),
        value["thread"].as_str().unwrap_or("")
    )
}

/// The account thread as a chat [`openagents_chat::thread::Thread`], for
/// `chat export`'s trajectory.
pub(crate) fn as_thread(row: &Value, turns: &[Value]) -> openagents_chat::thread::Thread {
    use openagents_chat::basic_coder::Turn;
    let updated = row["updated"].as_u64().unwrap_or(0);
    openagents_chat::thread::Thread {
        summary: openagents_chat::basic_chats::Summary {
            id: row["id"].as_str().unwrap_or_default().to_owned(),
            title: row["title"].as_str().unwrap_or_default().to_owned(),
            started: updated,
            updated,
            coder: None,
            archived: row["archived"].as_bool().unwrap_or(false),
            pinned: row["pinned"].as_bool().unwrap_or(false),
            named: false,
        },
        turns: turns
            .iter()
            .map(|turn| {
                let text = turn["text"].as_str().unwrap_or_default();
                if turn["role"] == "user" {
                    Turn::user(text)
                } else {
                    Turn::assistant(text, None)
                }
            })
            .collect(),
        busy: false,
        failure: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_ids_are_uuids() {
        assert!(account_id("95d8be69-9509-4333-ba58-b9c4032f8b25"));
        assert!(!account_id("95d8be69"));
        assert!(!account_id("0123456789abcdef0123456789abcdef"));
        assert!(!account_id("95d8be69-9509-4333-ba58-b9c4032f8b2z"));
    }

    #[test]
    fn both_list_shapes_read_as_one_row() {
        let phone = row(&json!({"id": "a", "title": "Who are you", "surface": "web",
            "computer": null, "updated_unix": 1_791_642_670, "pinned": false}));
        let api = row(
            &json!({"id": "a", "object": "thread", "title": "Who are you",
            "surface": "web", "updated_at": "2026-10-10T14:31:10Z", "messages": 8,
            "project": "prj_1"}),
        );
        assert_eq!(phone["updated"], 1_791_642_670);
        assert_eq!(api["updated"], 1_791_642_670);
        assert_eq!(api["messages"], 8);
        assert_eq!(place(&phone), "the website");
    }

    #[test]
    fn a_read_shows_each_message_and_exports_as_a_thread() {
        let row = row(&json!({"id": "95d8be69-9509-4333-ba58-b9c4032f8b25",
            "title": "Summarize this repo.", "surface": "web", "updated_unix": 10}));
        let turns = vec![
            json!({"role": "user", "text": "Summarize this repo."}),
            json!({"role": "assistant", "text": "It is a monorepo."}),
        ];
        let text = read_text(&whole(&row, turns.clone()));
        assert!(text.starts_with("Summarize this repo.\n(made on the website)\n\nyou:\n"));
        assert!(text.contains("\nyou:\nSummarize this repo.\n"));
        assert!(text.contains("\nopenagents:\nIt is a monorepo.\n"));
        assert!(text.ends_with("thread 95d8be69-9509-4333-ba58-b9c4032f8b25"));
        let thread = as_thread(&row, &turns);
        assert_eq!(thread.turns.len(), 2);
        let document = openagents_chat::thread::trajectory(&thread, "test");
        assert!(atif::validate(&document).is_empty());
    }
}
