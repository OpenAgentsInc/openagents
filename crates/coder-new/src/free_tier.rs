//! Coder's free answers while signed out (#11645): an anonymous OpenAgents
//! agent account (#11630, `docs/agents/self-registration.md`), made the
//! first time Coder has no sign-in and no login on this computer can
//! answer. Coder solves the sign-up's proof of work, keeps the agent's key
//! in its state folder (`free-agent.json`), and asks the agent's free tier:
//! `google/gemini-2.5-flash-lite`, 20 short requests a day. When the day's
//! free answers are used up, the turn says so with the command for more.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use openrouter::{Message, Streamed};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The one model the free tier runs.
pub const MODEL: &str = "google/gemini-2.5-flash-lite";

/// The file in Coder's state folder that keeps the agent's key.
pub const FILE: &str = "free-agent.json";

/// The variable that turns the free answers off (`1`).
pub const OFF_VAR: &str = "CODER_NO_FREE_TIER";

/// What the person reads when the day's free answers are used up.
pub const USED_UP: &str = "Today's free answers are used up. Log in with `coder login` for more.";

/// What the person reads when the sign-up was refused for too many
/// sign-ups from this network.
pub const SIGNUP_LIMIT: &str = "Coder couldn't get its free answers right now (too many sign-ups from this network today). Log in with `coder login`.";

/// What the person reads when openagents.com can't be reached.
pub const UNREACHABLE: &str = "Coder couldn't reach openagents.com for its free answers. Check the connection, or log in with `coder login`.";

/// The most input the free tier takes is 8,000 characters (counted as
/// bytes, framing included); Coder keeps well under it.
pub const INPUT_BUDGET: usize = 7_000;

/// The most output a free request may ask for.
pub const MAX_OUTPUT_TOKENS: u64 = 1_024;

/// The short instruction every free answer runs under.
pub const SYSTEM: &str = "You are Coder, a helpful coding assistant. Answer briefly and plainly.";

/// The agent account Coder made for itself.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub api_key: String,
    pub account: String,
    /// When it was made, in seconds since the epoch.
    pub registered_at: u64,
}

/// Whether the free answers may be used here: always, unless
/// [`OFF_VAR`] is `1`.
#[must_use]
pub fn available() -> bool {
    #[cfg(test)]
    if let Some(on) = TEST_AVAILABLE.with(std::cell::Cell::get) {
        return on;
    }
    std::env::var(OFF_VAR).map_or(true, |value| value.trim() != "1")
}

#[cfg(test)]
thread_local! {
    static TEST_AVAILABLE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static TEST_ORIGIN: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static TEST_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Points this test thread's free answers at `origin` (a mock server) and
/// keeps the agent's key under `dir`, never the real state folder.
#[cfg(test)]
pub(crate) fn use_for_test(origin: Option<&str>, dir: Option<&std::path::Path>) {
    TEST_ORIGIN.with(|cell| *cell.borrow_mut() = origin.map(str::to_owned));
    TEST_DIR.with(|cell| *cell.borrow_mut() = dir.map(std::path::Path::to_path_buf));
}

/// Turns the free answers on or off for this test thread.
#[cfg(test)]
pub(crate) fn set_available_for_test(on: Option<bool>) {
    TEST_AVAILABLE.with(|cell| cell.set(on));
}

/// The site: `OPENAGENTS_ORIGIN`, else openagents.com.
fn origin() -> String {
    #[cfg(test)]
    if let Some(origin) = TEST_ORIGIN.with(|cell| cell.borrow().clone()) {
        return origin;
    }
    openagents_login::origin_from(|name| std::env::var(name).ok())
}

/// The file the agent's key is kept in.
fn file() -> Option<PathBuf> {
    #[cfg(test)]
    {
        // A test never reads or writes the real state folder.
        return TEST_DIR.with(|cell| cell.borrow().clone().map(|dir| dir.join(FILE)));
    }
    #[cfg(not(test))]
    crate::home::data_path(FILE)
}

/// The agent kept on this computer, if any.
#[must_use]
pub fn saved() -> Option<Agent> {
    let text = std::fs::read_to_string(file()?).ok()?;
    serde_json::from_str::<Agent>(&text)
        .ok()
        .filter(|agent| agent.api_key.starts_with("oak_"))
}

fn save(agent: &Agent) -> Result<(), String> {
    let path = file().ok_or("Coder's settings folder is unavailable.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|_| "Coder couldn't save its free answers' key.")?;
    }
    let text = serde_json::to_string_pretty(agent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut out = options
            .open(&temporary)
            .map_err(|_| "Coder couldn't save its free answers' key.")?;
        out.write_all(text.as_bytes())
            .map_err(|_| "Coder couldn't save its free answers' key.")?;
    }
    std::fs::rename(&temporary, &path).map_err(|_| "Coder couldn't save its free answers' key.")?;
    Ok(())
}

fn forget() {
    if let Some(path) = file() {
        let _ = std::fs::remove_file(path);
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Whether SHA-256 of `challenge:solution` starts with `bits` zero bits.
#[must_use]
pub fn solves(challenge: &str, solution: &str, bits: u32) -> bool {
    let digest = Sha256::new()
        .chain_update(challenge.as_bytes())
        .chain_update(b":")
        .chain_update(solution.as_bytes())
        .finalize();
    leading_zero_bits(&digest) >= bits
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut bits = 0;
    for byte in bytes {
        if *byte == 0 {
            bits += 8;
        } else {
            return bits + byte.leading_zeros();
        }
    }
    bits
}

/// A decimal counter whose SHA-256 with `challenge` has `bits` leading zero
/// bits, searched on every core; `None` when `cancel` is set first.
#[must_use]
pub fn solve(challenge: &str, bits: u32, cancel: &AtomicBool) -> Option<String> {
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get) as u64;
    let found = AtomicBool::new(false);
    let answer = AtomicU64::new(u64::MAX);
    std::thread::scope(|scope| {
        for start in 0..threads {
            let (found, answer) = (&found, &answer);
            scope.spawn(move || {
                // The prefix hashed once; each try adds only its counter.
                let prefix = Sha256::new()
                    .chain_update(challenge.as_bytes())
                    .chain_update(b":");
                let mut counter = start;
                let mut buffer = itoa_buffer();
                loop {
                    if counter % 4096 == start % 4096
                        && (found.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed))
                    {
                        return;
                    }
                    let text = decimal(counter, &mut buffer);
                    let digest = prefix.clone().chain_update(text).finalize();
                    if leading_zero_bits(&digest) >= bits {
                        answer.fetch_min(counter, Ordering::Relaxed);
                        found.store(true, Ordering::Relaxed);
                        return;
                    }
                    counter += threads;
                }
            });
        }
    });
    let answer = answer.load(Ordering::Relaxed);
    (answer != u64::MAX).then(|| answer.to_string())
}

fn itoa_buffer() -> [u8; 20] {
    [0; 20]
}

fn decimal(mut value: u64, buffer: &mut [u8; 20]) -> &[u8] {
    let mut at = buffer.len();
    loop {
        at -= 1;
        buffer[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return &buffer[at..];
        }
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| UNREACHABLE.to_owned())
}

/// Serializes sign-ups, so two turns at once make one agent.
fn signup_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// The kept agent, or a new one: a challenge, its proof of work, the
/// sign-up, and the key saved.
///
/// # Errors
/// openagents.com is out of reach, refused the sign-up, or the turn was
/// stopped.
pub async fn agent(cancel: &Arc<AtomicBool>) -> Result<Agent, String> {
    if let Some(agent) = saved() {
        return Ok(agent);
    }
    let _held = signup_lock().lock().await;
    if let Some(agent) = saved() {
        return Ok(agent);
    }
    let agent = register(cancel).await?;
    save(&agent)?;
    Ok(agent)
}

async fn register(cancel: &Arc<AtomicBool>) -> Result<Agent, String> {
    let origin = origin();
    let http = client()?;
    let challenge: Value = http
        .get(format!("{origin}/v1/agents/challenge"))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| UNREACHABLE.to_owned())?
        .json()
        .await
        .map_err(|_| UNREACHABLE.to_owned())?;
    let text = challenge["challenge"]
        .as_str()
        .ok_or(UNREACHABLE)?
        .to_owned();
    let bits = challenge["difficulty_bits"]
        .as_u64()
        .and_then(|bits| u32::try_from(bits).ok())
        .filter(|bits| *bits <= 40)
        .ok_or(UNREACHABLE)?;
    let worker = {
        let (text, cancel) = (text.clone(), cancel.clone());
        tokio::task::spawn_blocking(move || solve(&text, bits, &cancel))
    };
    let solution = worker.await.ok().flatten().ok_or("The turn was stopped.")?;
    let mut tag = [0u8; 4];
    let _ = getrandom::fill(&mut tag);
    let name = format!(
        "coder-{}",
        tag.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let response = http
        .post(format!("{origin}/v1/agents/register"))
        .timeout(std::time::Duration::from_secs(30))
        .json(&json!({"name": name, "challenge": text, "solution": solution}))
        .send()
        .await
        .map_err(|_| UNREACHABLE.to_owned())?;
    let status = response.status().as_u16();
    if status == 429 {
        return Err(SIGNUP_LIMIT.to_owned());
    }
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status != 201 && status != 200 {
        let why = body["error"]["message"]
            .as_str()
            .unwrap_or("the sign-up was refused");
        return Err(format!(
            "Coder couldn't get its free answers: {}. Log in with `coder login`.",
            why.trim().trim_end_matches('.')
        ));
    }
    let api_key = body["api_key"].as_str().ok_or(UNREACHABLE)?.to_owned();
    let account = body["account"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    Ok(Agent {
        api_key,
        account,
        registered_at: now(),
    })
}

/// The conversation as the free tier takes it: a short system line, then
/// the newest person and assistant messages that fit in
/// [`INPUT_BUDGET`] bytes, the newest always kept (cut to fit when it alone
/// is too long).
#[must_use]
pub fn trimmed(messages: &[Message]) -> Vec<Message> {
    let mut budget = INPUT_BUDGET.saturating_sub(SYSTEM.len());
    let mut kept: Vec<Message> = Vec::new();
    for message in messages.iter().rev() {
        if !matches!(message.role.as_str(), "user" | "assistant") || message.content.is_empty() {
            continue;
        }
        if message.content.len() <= budget {
            budget -= message.content.len();
            kept.push(message.clone());
        } else if kept.is_empty() {
            // The newest message alone is too long: keep its end.
            let mut start = message.content.len() - budget;
            while !message.content.is_char_boundary(start) {
                start += 1;
            }
            kept.push(Message {
                role: message.role.clone(),
                content: message.content[start..].to_owned(),
            });
            break;
        } else {
            break;
        }
    }
    // A chat starts with the person.
    while kept.last().is_some_and(|message| message.role != "user") {
        kept.pop();
    }
    kept.push(Message::system(SYSTEM));
    kept.reverse();
    kept
}

/// One free answer to `messages`, streamed through `text`; `model` hears
/// the model's name. A revoked key is dropped and a new agent made once.
///
/// # Errors
/// [`USED_UP`] when the day's free answers are gone, [`SIGNUP_LIMIT`] or
/// [`UNREACHABLE`] when no agent could be made, or the model's failure.
pub async fn chat(
    messages: &[Message],
    cancel: &Arc<AtomicBool>,
    text: &mut (dyn FnMut(&str) + Send),
    model: &mut (dyn FnMut(&str) + Send),
) -> Result<Streamed, String> {
    let mut retried = false;
    loop {
        let agent = agent(cancel).await?;
        match ask(&agent, messages, cancel, text, model).await {
            Err(Asked::Revoked) if !retried => {
                retried = true;
                forget();
            }
            Err(Asked::Revoked) => {
                forget();
                return Err(
                    "Coder's free answers key was turned off. Log in with `coder login`.".into(),
                );
            }
            Err(Asked::Failed(why)) => return Err(why),
            Ok(reply) => return Ok(reply),
        }
    }
}

enum Asked {
    Revoked,
    Failed(String),
}

async fn ask(
    agent: &Agent,
    messages: &[Message],
    cancel: &Arc<AtomicBool>,
    text: &mut (dyn FnMut(&str) + Send),
    model: &mut (dyn FnMut(&str) + Send),
) -> Result<Streamed, Asked> {
    let started = std::time::Instant::now();
    let http = client().map_err(Asked::Failed)?;
    let body = json!({
        "model": MODEL,
        "messages": trimmed(messages),
        "stream": true,
        "stream_options": {"include_usage": true},
        "max_completion_tokens": MAX_OUTPUT_TOKENS,
    });
    let mut response = http
        .post(format!("{}/v1/chat/completions", origin()))
        .bearer_auth(&agent.api_key)
        .timeout(std::time::Duration::from_secs(180))
        .json(&body)
        .send()
        .await
        .map_err(|_| Asked::Failed(UNREACHABLE.to_owned()))?;
    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err(Asked::Revoked);
    }
    if status == 402 {
        return Err(Asked::Failed(USED_UP.to_owned()));
    }
    if status == 429 {
        return Err(Asked::Failed(
            "Coder's free answers are busy right now. Try again in a minute, or log in with `coder login`.".to_owned(),
        ));
    }
    if !(200..300).contains(&status) {
        let body: Value = response.json().await.unwrap_or(Value::Null);
        let why = body["error"]["message"]
            .as_str()
            .map_or_else(|| format!("HTTP {status}"), str::to_owned);
        return Err(Asked::Failed(format!(
            "Coder's free answer failed: {}.",
            why.trim().trim_end_matches('.')
        )));
    }
    let mut reply = Streamed {
        model: MODEL.to_owned(),
        ..Streamed::default()
    };
    let mut told_model = false;
    let mut pending = String::new();
    let mut done = false;
    while !done {
        if cancel.load(Ordering::Relaxed) {
            return Err(Asked::Failed("The turn was stopped.".to_owned()));
        }
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) if !reply.text.is_empty() => break,
            Err(_) => return Err(Asked::Failed(UNREACHABLE.to_owned())),
        };
        pending.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = pending.find('\n') {
            let line = pending[..end].trim().to_owned();
            pending.drain(..=end);
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                done = true;
                break;
            }
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(message) = event["error"]["message"].as_str() {
                return Err(Asked::Failed(format!(
                    "Coder's free answer failed: {}.",
                    message.trim().trim_end_matches('.')
                )));
            }
            if !told_model {
                if let Some(name) = event["model"].as_str() {
                    reply.model = name.to_owned();
                }
                model(&reply.model);
                told_model = true;
            }
            if let Some(delta) = event["choices"][0]["delta"]["content"].as_str()
                && !delta.is_empty()
            {
                if reply.first_text_ms.is_none() {
                    reply.first_text_ms = Some(started.elapsed().as_millis() as u64);
                }
                reply.text.push_str(delta);
                text(delta);
            }
            if let Some(reason) = event["choices"][0]["finish_reason"].as_str() {
                reply.finish_reason = Some(reason.to_owned());
            }
            if let Ok(usage) = serde_json::from_value::<openrouter::Usage>(event["usage"].clone())
                && usage.total_tokens + usage.prompt_tokens > 0
            {
                reply.usage = usage;
            }
        }
    }
    // The free tier is never a charge to the person.
    reply.usage.cost = None;
    reply.milliseconds = started.elapsed().as_millis() as u64;
    if reply.text.trim().is_empty() {
        return Err(Asked::Failed(
            "Coder's free answer came back empty.".to_owned(),
        ));
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    /// A server that answers each request in turn with `(status, body)`,
    /// and returns each request's path and JSON body.
    fn server(answers: Vec<(u16, String)>) -> (String, JoinHandle<Vec<(String, Value)>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut seen = vec![];
            for (status, body) in answers {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = vec![];
                let mut bytes = [0; 4096];
                let end = loop {
                    let count = socket.read(&mut bytes).unwrap();
                    assert!(count > 0, "closed early");
                    request.extend_from_slice(&bytes[..count]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&request[..end]).to_string();
                        let length = header
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break end;
                        }
                    }
                };
                let header = String::from_utf8_lossy(&request[..end]).to_string();
                let path = header
                    .lines()
                    .next()
                    .unwrap()
                    .split(' ')
                    .nth(1)
                    .unwrap()
                    .to_owned();
                let body_json = serde_json::from_slice(&request[end + 4..]).unwrap_or(Value::Null);
                seen.push((path, body_json));
                let kind = if body.starts_with("data:") {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                write!(
                    socket,
                    "HTTP/1.1 {status} Fixture\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            seen
        });
        (format!("http://{address}"), handle)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn challenge() -> (u16, String) {
        (
            200,
            json!({"challenge": "v1.8.fixture", "algorithm": "sha256", "difficulty_bits": 8})
                .to_string(),
        )
    }

    fn registered(key: &str) -> (u16, String) {
        (
            201,
            json!({"api_key": key, "account": {"id": "acct_fixture", "kind": "agent"}}).to_string(),
        )
    }

    fn answer(text: &str) -> (u16, String) {
        let chunk = json!({"model": MODEL, "choices": [{"delta": {"content": text}, "finish_reason": "stop"}]});
        let usage = json!({"model": MODEL, "choices": [], "usage": {"prompt_tokens": 20, "completion_tokens": 5, "total_tokens": 25, "cost": 0.0001}});
        (
            200,
            format!("data: {chunk}\n\ndata: {usage}\n\ndata: [DONE]\n\n"),
        )
    }

    fn ask_once(messages: &[Message]) -> Result<Streamed, String> {
        let mut said = String::new();
        let result = runtime().block_on(chat(
            messages,
            &Arc::new(AtomicBool::new(false)),
            &mut |delta| said.push_str(delta),
            &mut |_| {},
        ));
        if let Ok(reply) = &result {
            assert_eq!(said, reply.text);
        }
        result
    }

    #[test]
    fn the_proof_of_work_finds_a_solution_the_check_accepts() {
        let cancel = AtomicBool::new(false);
        for bits in [0, 4, 12, 16] {
            let solution = solve("v1.fixture.challenge", bits, &cancel).unwrap();
            assert!(solves("v1.fixture.challenge", &solution, bits), "{bits}");
            assert!(solution.bytes().all(|b| b.is_ascii_digit()));
        }
        assert_eq!(leading_zero_bits(&[0, 0b0001_0000]), 11);
        let stopped = AtomicBool::new(true);
        assert_eq!(solve("v1.fixture", 64, &stopped), None);
    }

    #[test]
    fn a_new_agent_is_registered_saved_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let (origin, server) = server(vec![
            challenge(),
            registered("oak_fixture.secret"),
            answer("Hello there."),
            answer("Again."),
        ]);
        use_for_test(Some(&origin), Some(dir.path()));
        let reply = ask_once(&[Message::user("hi")]).unwrap();
        assert_eq!(reply.text, "Hello there.");
        assert_eq!(reply.usage.cost, None);
        assert_eq!(reply.usage.total_tokens, 25);
        let saved = saved().unwrap();
        assert_eq!(saved.api_key, "oak_fixture.secret");
        assert_eq!(saved.account, "acct_fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // The second turn reuses the key: no new challenge.
        assert_eq!(ask_once(&[Message::user("again")]).unwrap().text, "Again.");
        let seen = server.join().unwrap();
        let paths: Vec<&str> = seen.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/v1/agents/challenge",
                "/v1/agents/register",
                "/v1/chat/completions",
                "/v1/chat/completions"
            ]
        );
        let register = &seen[1].1;
        assert!(solves(
            "v1.8.fixture",
            register["solution"].as_str().unwrap(),
            8
        ));
        assert!(register["name"].as_str().unwrap().starts_with("coder-"));
        let ask = &seen[2].1;
        assert_eq!(ask["model"], MODEL);
        assert_eq!(ask["max_completion_tokens"], MAX_OUTPUT_TOKENS);
        assert!(ask.get("tools").is_none());
        use_for_test(None, None);
    }

    #[test]
    fn a_402_says_the_free_answers_are_used_up_with_the_command() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), "oak_fixture.used");
        let (origin, server) = server(vec![(
            402,
            json!({"error": {"type": "insufficient_balance", "code": "agent_free_requests_used", "message": "used"}}).to_string(),
        )]);
        use_for_test(Some(&origin), Some(dir.path()));
        let error = ask_once(&[Message::user("hi")]).unwrap_err();
        assert_eq!(error, USED_UP);
        assert!(error.contains("coder login"));
        server.join().unwrap();
        // The key is kept: it works again tomorrow.
        assert!(saved().is_some());
        use_for_test(None, None);
    }

    #[test]
    fn a_revoked_key_is_dropped_and_a_new_agent_made_once() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), "oak_fixture.revoked");
        let (origin, server) = server(vec![
            (401, json!({"error": {"message": "revoked"}}).to_string()),
            challenge(),
            registered("oak_fixture.fresh"),
            answer("Fresh."),
        ]);
        use_for_test(Some(&origin), Some(dir.path()));
        assert_eq!(ask_once(&[Message::user("hi")]).unwrap().text, "Fresh.");
        assert_eq!(saved().unwrap().api_key, "oak_fixture.fresh");
        server.join().unwrap();
        use_for_test(None, None);
    }

    #[test]
    fn a_refused_signup_and_no_network_say_how_to_sign_in() {
        let dir = tempfile::tempdir().unwrap();
        let (origin, server) = server(vec![
            challenge(),
            (429, json!({"error": {"message": "limit"}}).to_string()),
        ]);
        use_for_test(Some(&origin), Some(dir.path()));
        assert_eq!(ask_once(&[Message::user("hi")]).unwrap_err(), SIGNUP_LIMIT);
        server.join().unwrap();
        assert!(saved().is_none());
        // Nothing listens on port 1.
        use_for_test(Some("http://127.0.0.1:1"), Some(dir.path()));
        assert_eq!(ask_once(&[Message::user("hi")]).unwrap_err(), UNREACHABLE);
        use_for_test(None, None);
    }

    #[test]
    fn trimming_keeps_the_newest_and_stays_under_the_cap() {
        let long = "x".repeat(3_000);
        let mut messages = vec![Message::system("Coder's long system prompt")];
        for index in 0..6 {
            messages.push(Message::user(format!("{index}{long}")));
            messages.push(Message {
                role: "assistant".into(),
                content: format!("a{index}"),
            });
        }
        messages.push(Message::user("newest"));
        let kept = trimmed(&messages);
        let total: usize = kept.iter().map(|m| m.content.len()).sum();
        assert!(total <= INPUT_BUDGET, "{total}");
        assert_eq!(kept[0].role, "system");
        assert_eq!(kept[0].content, SYSTEM);
        assert_eq!(kept.last().unwrap().content, "newest");
        assert_eq!(kept[1].role, "user");
        assert!(
            kept.iter()
                .all(|m| m.content != "Coder's long system prompt")
        );
        // One message too long alone keeps its end.
        let huge = format!("start{}end", "é".repeat(10_000));
        let kept = trimmed(&[Message::user(huge)]);
        assert_eq!(kept.len(), 2);
        assert!(kept[1].content.ends_with("end"));
        assert!(kept[1].content.len() + SYSTEM.len() <= INPUT_BUDGET);
    }

    #[test]
    fn the_free_answers_can_be_turned_off() {
        set_available_for_test(Some(false));
        assert!(!available());
        set_available_for_test(None);
    }

    fn save_in(dir: &std::path::Path, key: &str) {
        use_for_test(None, Some(dir));
        save(&Agent {
            api_key: key.into(),
            account: "acct_fixture".into(),
            registered_at: 1,
        })
        .unwrap();
    }

    /// The live path, run by hand once: a real sign-up at openagents.com
    /// (one of the address's five a day) and one free answer.
    #[test]
    #[ignore = "registers a real agent at openagents.com"]
    fn live_free_answer() {
        let dir = tempfile::tempdir().unwrap();
        use_for_test(Some("https://openagents.com"), Some(dir.path()));
        let started = std::time::Instant::now();
        let reply = ask_once(&[Message::user("Say hello in five words.")]).unwrap();
        let agent = saved().unwrap();
        println!(
            "live: account {} key {}… model {} in {:?}: {}",
            agent.account,
            &agent.api_key[..8],
            reply.model,
            started.elapsed(),
            reply.text
        );
        assert!(!reply.text.trim().is_empty());
        use_for_test(None, None);
    }
}
