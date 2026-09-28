//! `openagents session`: observe the chats a paired computer discloses under
//! the NIP-SESS history-observer profile, through `coder-connect`.
//!
//! `pair` redeems a `coder-pair:` invitation with this profile's key and
//! keeps the public connection code in the session store; `list`, `read`,
//! and `tail` send bounded observer requests over the connection's relay.
//! The observer grant is read-only by construction: `steer` and `interrupt`
//! read the grant and refuse unless it admits control, which
//! `openagents.history-observer-grant.v1` never does.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder_connect::coder_history::{
    CatalogRequest, Chat, RecordChunk, TranscriptCursor, TranscriptPage, TranscriptRequest,
};
use coder_connect::pairing::Invitation;
use coder_connect::{Client, ConnectionCode, Observation, Query, RelayPolicy};
use serde_json::{Value, json};

use crate::{Args, EXIT_FAILURE, Output};

const USAGE: &str = "usage: openagents session COMMAND [OPTIONS]
  pair INVITATION [--relay URL] [--timeout SECONDS] [--as PROFILE]
        Redeem a `coder-pair:` invitation from `openagents pair` or `coder
        pair` and keep the connection. --relay must match the invitation.
  list [--host PUBKEY] [--relay URL] [--timeout SECONDS] [--limit N]
        List the chats every paired computer discloses.
  read SESSION [--limit N] [--host PUBKEY] [--relay URL] [--timeout SECONDS]
        Print a chat's records from the start, at most N (default 200).
  tail SESSION [--host PUBKEY] [--relay URL] [--timeout SECONDS]
        Print a chat's records, then new ones until SECONDS pass (default 30).
  steer SESSION TEXT [--host PUBKEY]
  interrupt SESSION [--host PUBKEY]
        Control a chat when the grant admits it. A history-observer grant
        is read-only, so these are refused with exit code 1.
  connections
        List the saved connections and their grants.
  forget GRANT
        Remove a saved connection. This does not revoke the grant.
Every command also takes --store PATH to use another connection directory.
SESSION is a chat ID from `list`, or its source ID. --as names the profile
key that redeemed the invitation (default: default). --timeout defaults to
20 seconds for a request and bounds the whole command. Connections live in
~/.openagents/session/ (OPENAGENTS_SESSION_HOME overrides the directory).";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_TAIL: Duration = Duration::from_secs(30);
const DEFAULT_READ_LIMIT: usize = 200;
/// How long `tail` waits between polls for appended bytes.
const TAIL_POLL: Duration = Duration::from_secs(2);
/// The most catalog pages one lookup walks before giving up.
const MAX_CATALOG_PAGES: usize = 64;
const GRANT_SCHEMA: &str = coder_connect::protocol::GRANT;
const CONNECTION_SCHEMA: &str = coder_connect::protocol::CONNECTION;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("session", "a command is required", USAGE);
    };
    if command == "--help" || command == "-h" || command == "help" {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("session", &message, USAGE),
    };
    let timeout = match args.number::<u64>("timeout", 0) {
        Ok(0) => None,
        Ok(seconds) => Some(Duration::from_secs(seconds)),
        Err(message) => return output.usage("session", &message, USAGE),
    };
    let store = Store::new(args.option("store"));
    match command.as_str() {
        "pair" => {
            let Some(invitation) = args.positional().first() else {
                return output.usage("session", "an invitation is required", USAGE);
            };
            let identity = match crate::relay::identity_for(args.option("as")) {
                Ok(identity) => identity,
                Err(message) => return output.fail("session", &message),
            };
            let parsed = match Invitation::parse(
                invitation,
                crate::relay::unix_now(),
                RelayPolicy::Production,
            ) {
                Ok(parsed) => parsed,
                Err(error) => return output.fail("session", &error.to_string()),
            };
            if let Some(relay) = args.option("relay")
                && relay != parsed.relay
            {
                return output.fail(
                    "session",
                    &format!(
                        "the invitation names relay {}, not {relay}; pairing uses the invitation's relay",
                        parsed.relay
                    ),
                );
            }
            let wait = timeout.unwrap_or(DEFAULT_TIMEOUT);
            let redeemed = crate::runtime().block_on(async {
                tokio::time::timeout(
                    wait,
                    coder_connect::pairing::redeem(
                        invitation,
                        &identity.secret,
                        RelayPolicy::Production,
                    ),
                )
                .await
            });
            let code = match redeemed {
                Ok(Ok(code)) => code,
                Ok(Err(error)) => return output.fail("session", &error.to_string()),
                Err(_) => {
                    return output.fail(
                        "session",
                        &format!("{} did not pair within {wait:?}", parsed.relay),
                    );
                }
            };
            if let Err(message) = store.save(&code) {
                return output.fail("session", &message);
            }
            output.emit(&connection_value(&code, &identity.profile), |value| {
                format!(
                    "paired with {} over {}\ngrant {} expires at {}\nsources: {}",
                    value["host"].as_str().unwrap_or(""),
                    value["relay"].as_str().unwrap_or(""),
                    value["grant"].as_str().unwrap_or(""),
                    value["expires_at"],
                    source_labels(value)
                )
            });
            0
        }
        "connections" => {
            let connections = match store.load_all(&args) {
                Ok(connections) => connections,
                Err(message) => return output.fail("session", &message),
            };
            let values: Vec<Value> = connections
                .iter()
                .map(|code| connection_value(code, ""))
                .collect();
            output.emit(&json!({ "connections": values }), |value| {
                let rows: Vec<Vec<String>> = value["connections"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .map(|item| {
                                vec![
                                    item["grant"].as_str().unwrap_or("").to_owned(),
                                    item["host"].as_str().unwrap_or("").to_owned(),
                                    item["relay"].as_str().unwrap_or("").to_owned(),
                                    item["expires_at"].to_string(),
                                    source_labels(item),
                                ]
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                crate::out::table(&rows)
            });
            0
        }
        "forget" => {
            let Some(grant) = args.positional().first() else {
                return output.usage("session", "a grant ID is required", USAGE);
            };
            match store.forget(grant) {
                Ok(()) => {
                    output.emit(&json!({ "forgotten": grant }), |value| {
                        format!("forgot {}", value["forgotten"].as_str().unwrap_or(""))
                    });
                    0
                }
                Err(message) => output.fail("session", &message),
            }
        }
        "list" => {
            let limit = match args.number::<u16>("limit", 0) {
                Ok(limit) => limit,
                Err(message) => return output.usage("session", &message, USAGE),
            };
            let clients = match store.clients(&args) {
                Ok(clients) => clients,
                Err(message) => return output.fail("session", &message),
            };
            let wait = timeout.unwrap_or(DEFAULT_TIMEOUT);
            let mut chats = Vec::new();
            let mut errors = Vec::new();
            crate::runtime().block_on(async {
                for client in &clients {
                    let host = client.connection().host.clone();
                    match catalog(client, limit, wait, |_| false).await {
                        Ok(found) => chats.extend(found.into_iter().map(|chat| {
                            let mut value = serde_json::to_value(&chat).unwrap_or(Value::Null);
                            value["host"] = host.clone().into();
                            value
                        })),
                        Err(message) => errors.push(json!({ "host": host, "error": message })),
                    }
                }
            });
            output.emit(&json!({ "chats": chats, "errors": errors }), |value| {
                let mut rows = vec![vec![
                    "CHAT".to_owned(),
                    "HARNESS".to_owned(),
                    "UPDATED".to_owned(),
                    "STATUS".to_owned(),
                    "TITLE".to_owned(),
                ]];
                for chat in value["chats"].as_array().into_iter().flatten() {
                    rows.push(vec![
                        chat["id"].as_str().unwrap_or("").to_owned(),
                        chat["harness"].as_str().unwrap_or("").to_owned(),
                        chat["updated_at"].as_str().unwrap_or("-").to_owned(),
                        chat["status"].as_str().unwrap_or("").to_owned(),
                        chat["title"].as_str().unwrap_or("").to_owned(),
                    ]);
                }
                let mut text = crate::out::table(&rows);
                for error in value["errors"].as_array().into_iter().flatten() {
                    text.push_str(&format!(
                        "\n{}: {}",
                        error["host"].as_str().unwrap_or(""),
                        error["error"].as_str().unwrap_or("")
                    ));
                }
                text
            });
            if errors.is_empty() { 0 } else { EXIT_FAILURE }
        }
        "read" | "tail" => {
            let Some(session) = args.positional().first() else {
                return output.usage("session", "a SESSION is required", USAGE);
            };
            let limit = match args.number::<usize>("limit", DEFAULT_READ_LIMIT) {
                Ok(0) => return output.usage("session", "--limit is at least 1", USAGE),
                Ok(limit) => limit,
                Err(message) => return output.usage("session", &message, USAGE),
            };
            let clients = match store.clients(&args) {
                Ok(clients) => clients,
                Err(message) => return output.fail("session", &message),
            };
            let live = command == "tail";
            let wait = timeout.unwrap_or(if live { DEFAULT_TAIL } else { DEFAULT_TIMEOUT });
            let deadline = Instant::now() + wait;
            crate::runtime().block_on(async {
                let (client, chat) = match locate(&clients, session, wait).await {
                    Ok(found) => found,
                    Err(message) => return output.fail("session", &message),
                };
                let Some(source_id) = chat.source_id.clone() else {
                    return output.fail(
                        "session",
                        &format!(
                            "chat {} has no readable source ({:?})",
                            chat.id, chat.status
                        ),
                    );
                };
                if live {
                    tail(output, client, &chat, &source_id, deadline).await
                } else {
                    read(output, client, &chat, &source_id, limit, wait).await
                }
            })
        }
        "steer" | "interrupt" => {
            let Some(session) = args.positional().first() else {
                return output.usage("session", "a SESSION is required", USAGE);
            };
            if command == "steer" && args.positional().len() < 2 {
                return output.usage("session", "steer takes SESSION and TEXT", USAGE);
            }
            let connections = match store.load_all(&args) {
                Ok(connections) => connections,
                Err(message) => return output.fail("session", &message),
            };
            match control_refusal(&connections, command, session) {
                Some(message) => output.fail("session", &message),
                None => output.fail(
                    "session",
                    &format!("{command} is not implemented for any saved grant"),
                ),
            }
        }
        other => output.usage("session", &format!("unknown command `{other}`"), USAGE),
    }
}

/// Why the saved grants cannot `steer` or `interrupt` `session`. The observer
/// profile admits observation only, so every saved grant refuses; a grant of
/// another schema is refused as unsupported rather than trusted.
fn control_refusal(connections: &[ConnectionCode], command: &str, session: &str) -> Option<String> {
    if connections.is_empty() {
        return Some("no saved connection; run `openagents session pair INVITATION` first".into());
    }
    if connections.iter().all(|code| code.v == CONNECTION_SCHEMA) {
        Some(format!(
            "{command} refused: every saved grant is a read-only {GRANT_SCHEMA} \
             ({} of {}); it can observe chat {session} but cannot control it. \
             Control a task with `openagents computer steer` or `openagents computer cancel` \
             under a NIP-HOST grant instead.",
            connections.len(),
            connections.len()
        ))
    } else {
        Some(format!(
            "{command} refused: a saved connection has an unsupported schema"
        ))
    }
}

/// Where connections are kept: `--store`, `OPENAGENTS_SESSION_HOME`, or
/// `~/.openagents/session`.
struct Store {
    dir: PathBuf,
}

impl Store {
    fn new(flag: Option<&str>) -> Self {
        let dir = flag
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("OPENAGENTS_SESSION_HOME").map(PathBuf::from))
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map_or_else(|| PathBuf::from("."), PathBuf::from)
                    .join(".openagents")
                    .join("session")
            });
        Self { dir }
    }

    fn connections_dir(&self) -> PathBuf {
        self.dir.join("connections")
    }

    fn path(&self, grant: &str) -> PathBuf {
        self.connections_dir().join(format!("{grant}.json"))
    }

    fn save(&self, code: &ConnectionCode) -> Result<(), String> {
        let dir = self.connections_dir();
        create_private_dir(&dir)?;
        let text = serde_json::to_vec(code).map_err(|error| error.to_string())?;
        write_private(&self.path(&code.grant), &text)
    }

    fn forget(&self, grant: &str) -> Result<(), String> {
        let path = self.path(grant);
        std::fs::remove_file(&path)
            .map_err(|error| format!("cannot remove {}: {error}", path.display()))
    }

    /// Every saved connection, narrowed by `--host` and `--relay`.
    fn load_all(&self, args: &Args) -> Result<Vec<ConnectionCode>, String> {
        let dir = self.connections_dir();
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("cannot read {}: {error}", dir.display())),
        };
        let mut connections = Vec::new();
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let code = ConnectionCode::parse(&bytes)
                .map_err(|error| format!("{} is not a connection: {error}", path.display()))?;
            if args.option("host").is_some_and(|host| host != code.host) {
                continue;
            }
            if args
                .option("relay")
                .is_some_and(|relay| relay != code.relay)
            {
                continue;
            }
            connections.push(code);
        }
        connections.sort_by(|a, b| a.grant.cmp(&b.grant));
        Ok(connections)
    }

    /// A verified client for every saved connection this profile's key owns.
    fn clients(&self, args: &Args) -> Result<Vec<Client>, String> {
        let connections = self.load_all(args)?;
        if connections.is_empty() {
            return Err(
                "no saved connection; run `openagents session pair INVITATION` first".into(),
            );
        }
        let identity = crate::relay::identity_for(args.option("as"))?;
        let mut clients = Vec::new();
        for code in connections {
            let grant = code.grant.clone();
            let client = Client::new(code, identity.secret)
                .map_err(|error| format!("connection {grant} cannot be used: {error}"))?;
            clients.push(client);
        }
        Ok(clients)
    }
}

fn create_private_dir(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn connection_value(code: &ConnectionCode, profile: &str) -> Value {
    let mut value = json!({
        "grant": code.grant,
        "host": code.host,
        "client": code.client,
        "relay": code.relay,
        "expires_at": code.expires_at,
        "sources": code.sources,
        "read_only": true,
    });
    if !profile.is_empty() {
        value["profile"] = profile.into();
    }
    value
}

fn source_labels(value: &Value) -> String {
    value["sources"]
        .as_array()
        .map(|sources| {
            sources
                .iter()
                .map(|source| {
                    format!(
                        "{} ({})",
                        source["label"].as_str().unwrap_or(""),
                        source["kind"].as_str().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

async fn observe(client: &Client, query: Query, wait: Duration) -> Result<Observation, String> {
    match tokio::time::timeout(wait, client.observe(query)).await {
        Ok(Ok(observation)) => Ok(observation),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!(
            "{} did not answer within {wait:?}",
            client.connection().relay
        )),
    }
}

/// Walk the catalog until `stop` accepts a chat or the pages run out. With
/// `limit` above zero, stop after that many chats.
async fn catalog(
    client: &Client,
    limit: u16,
    wait: Duration,
    mut stop: impl FnMut(&Chat) -> bool,
) -> Result<Vec<Chat>, String> {
    let mut chats = Vec::new();
    let mut cursor = None;
    for _ in 0..MAX_CATALOG_PAGES {
        let request = CatalogRequest {
            cursor,
            limit: CatalogRequest::default().limit,
        };
        let page = match observe(client, Query::Catalog(request), wait).await? {
            Observation::Catalog(page) => page,
            Observation::Page(_) => return Err("host answered a catalog with a transcript".into()),
        };
        for chat in page.entries {
            let found = stop(&chat);
            chats.push(chat);
            if found || (limit > 0 && chats.len() >= usize::from(limit)) {
                return Ok(chats);
            }
        }
        match page.next {
            Some(next) => cursor = Some(next),
            None => return Ok(chats),
        }
    }
    Ok(chats)
}

/// The client and chat `session` names, by chat ID or source ID.
async fn locate<'a>(
    clients: &'a [Client],
    session: &str,
    wait: Duration,
) -> Result<(&'a Client, Chat), String> {
    let mut errors = Vec::new();
    for client in clients {
        let matches =
            |chat: &Chat| chat.id == session || chat.source_id.as_deref() == Some(session);
        match catalog(client, 0, wait, matches).await {
            Ok(chats) => {
                if let Some(chat) = chats.into_iter().find(|chat| matches(chat)) {
                    return Ok((client, chat));
                }
            }
            Err(message) => errors.push(format!("{}: {message}", client.connection().host)),
        }
    }
    if errors.is_empty() {
        Err(format!("no paired computer discloses a chat `{session}`"))
    } else {
        Err(format!(
            "no paired computer discloses a chat `{session}`; {}",
            errors.join("; ")
        ))
    }
}

async fn page(
    client: &Client,
    source_id: &str,
    cursor: Option<TranscriptCursor>,
    wait: Duration,
) -> Result<TranscriptPage, String> {
    let request = TranscriptRequest {
        source_id: source_id.to_owned(),
        cursor,
        max_bytes: coder_connect::coder_history::MAX_PAGE_BYTES,
        end: None,
    };
    match observe(client, Query::Page(request), wait).await? {
        Observation::Page(page) => Ok(page),
        Observation::Catalog(_) => Err("host answered a transcript with a catalog".into()),
    }
}

async fn read(
    output: &Output,
    client: &Client,
    chat: &Chat,
    source_id: &str,
    limit: usize,
    wait: Duration,
) -> u8 {
    let mut records = Vec::new();
    let mut cursor = None;
    let mut has_more = true;
    let mut notices = Vec::new();
    while has_more && records.len() < limit {
        let current = match page(client, source_id, cursor, wait).await {
            Ok(current) => current,
            Err(message) => return output.fail("session", &message),
        };
        notices.extend(current.notices.iter().cloned());
        records.extend(current.chunks.iter().map(record_value));
        has_more = current.has_more;
        cursor = Some(current.next);
    }
    records.truncate(limit);
    output.emit(
        &json!({
            "chat": chat,
            "host": client.connection().host,
            "records": records,
            "has_more": has_more,
            "cursor": cursor,
            "notices": notices,
        }),
        |value| {
            let mut lines: Vec<String> = value["records"]
                .as_array()
                .into_iter()
                .flatten()
                .map(record_line)
                .collect();
            if value["has_more"].as_bool().unwrap_or(false) {
                lines.push("... more records; raise --limit".to_owned());
            }
            lines.join("\n")
        },
    );
    0
}

async fn tail(
    output: &Output,
    client: &Client,
    chat: &Chat,
    source_id: &str,
    deadline: Instant,
) -> u8 {
    let mut cursor: Option<TranscriptCursor> = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return 0;
        }
        let current = match page(
            client,
            source_id,
            cursor.clone(),
            remaining.min(DEFAULT_TIMEOUT),
        )
        .await
        {
            Ok(current) => current,
            Err(message) => return output.fail("session", &message),
        };
        for chunk in &current.chunks {
            let mut value = record_value(chunk);
            value["chat"] = chat.id.clone().into();
            output.line(&value, record_line);
        }
        cursor = Some(current.next);
        if !current.has_more {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return 0;
            }
            tokio::time::sleep(remaining.min(TAIL_POLL)).await;
        }
    }
}

fn record_value(chunk: &RecordChunk) -> Value {
    let mut value = json!({
        "id": chunk.id,
        "index": chunk.index,
        "offset": chunk.offset,
        "end_offset": chunk.end_offset,
        "complete": chunk.complete,
        "oversized": chunk.oversized,
        "raw_base64": chunk.raw_base64,
    });
    if let Some(readable) = &chunk.readable {
        value["readable"] = serde_json::to_value(readable).unwrap_or(Value::Null);
    }
    value
}

/// One text line for a record: its index, who spoke, and the projected text.
fn record_line(value: &Value) -> String {
    let index = &value["index"];
    let readable = &value["readable"];
    if readable.is_null() {
        let bytes = value["end_offset"]
            .as_u64()
            .unwrap_or(0)
            .saturating_sub(value["offset"].as_u64().unwrap_or(0));
        return format!("{index} [raw {bytes} bytes]");
    }
    let who = readable["role"]
        .as_str()
        .or_else(|| readable["tool_name"].as_str())
        .or_else(|| readable["kind"].as_str())
        .unwrap_or("record");
    let text = readable["text"].as_str().unwrap_or("").replace('\n', " ");
    let marker = if readable["text_truncated"].as_bool().unwrap_or(false) {
        " …"
    } else {
        ""
    };
    format!("{index} {who}: {text}{marker}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    fn quiet() -> Output {
        Output::new(true)
    }

    #[test]
    fn help_and_usage_exit_codes() {
        assert_eq!(run(&quiet(), &words("--help")), 0);
        assert_eq!(run(&quiet(), &[]), crate::EXIT_USAGE);
        assert_eq!(run(&quiet(), &words("bogus")), crate::EXIT_USAGE);
        assert_eq!(run(&quiet(), &words("pair")), crate::EXIT_USAGE);
        assert_eq!(run(&quiet(), &words("read")), crate::EXIT_USAGE);
        assert_eq!(run(&quiet(), &words("steer abc")), crate::EXIT_USAGE);
        assert_eq!(
            run(&quiet(), &words("list --timeout soon")),
            crate::EXIT_USAGE
        );
    }

    #[test]
    fn a_malformed_invitation_is_refused_before_any_network() {
        let temp = std::env::temp_dir().join(format!("oa-session-{}", std::process::id()));
        let store = temp.display().to_string();
        assert_eq!(
            run(
                &quiet(),
                &words(&format!("pair not-an-invitation --store {store}"))
            ),
            EXIT_FAILURE
        );
        assert!(!temp.exists());
    }

    #[test]
    fn control_is_refused_without_a_grant_that_admits_it() {
        let temp = std::env::temp_dir().join(format!("oa-session-empty-{}", std::process::id()));
        let store = temp.display().to_string();
        assert_eq!(
            run(
                &quiet(),
                &words(&format!("steer chat-1 keep going --store {store}"))
            ),
            EXIT_FAILURE
        );
        assert_eq!(
            run(
                &quiet(),
                &words(&format!("interrupt chat-1 --store {store}"))
            ),
            EXIT_FAILURE
        );
        let message = control_refusal(&[], "steer", "chat-1").unwrap();
        assert!(message.contains("no saved connection"));
    }

    #[test]
    fn record_lines_project_readable_text_and_raw_sizes() {
        let readable = json!({
            "index": 3,
            "offset": 10,
            "end_offset": 30,
            "readable": { "role": "assistant", "text": "one\ntwo", "text_truncated": true }
        });
        assert_eq!(record_line(&readable), "3 assistant: one two …");
        let raw = json!({ "index": 4, "offset": 10, "end_offset": 30 });
        assert_eq!(record_line(&raw), "4 [raw 20 bytes]");
    }

    #[test]
    fn the_store_round_trips_nothing_when_empty() {
        let temp = std::env::temp_dir().join(format!("oa-session-store-{}", std::process::id()));
        let store = Store::new(Some(temp.to_str().unwrap()));
        let args = Args::parse(&[], &[]).unwrap();
        assert!(store.load_all(&args).unwrap().is_empty());
        assert!(store.clients(&args).is_err());
    }
}
