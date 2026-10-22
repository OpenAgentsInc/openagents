//! The Grid Foreman's host (#12445): `grid-foreman serve` holds the
//! Foreman's Nostr key, walks its patrol, and publishes its pose and status
//! in the Grid's world like a player, so every browser draws one Foreman.
//!
//! It runs on `oa-foreman-1` under systemd (`deploy/systemd/grid-foreman.service`)
//! with a watchdog: the host tells systemd it's alive only while its relay
//! session is online and frames go out. See `docs/deployment/grid-foreman.md`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::identity::{self, Identity};
use verse::session::{PublishIntervals, Session, Status};

use crate::patrol::{Stop, Walker};

/// The key file's name in the state directory: 64 hex characters.
pub const KEY_FILE: &str = "key";
/// The relay the Grid uses.
pub const RELAY: &str = "wss://relay.openagents.com";
/// The site whose status route gives the issue counts.
pub const SITE: &str = "https://openagents.com";
/// The status line for the first moments after a start.
pub const STARTING: &str = "BACK ON THE JOB";
/// How long [`STARTING`] shows.
const STARTING_FOR: Duration = Duration::from_secs(20);
/// How often the host steps the walker.
const TICK: Duration = Duration::from_millis(100);
/// With no frame out for this long, the host stops telling the watchdog
/// it's alive, and systemd restarts it.
const SILENT_LIMIT: Duration = Duration::from_secs(120);
/// How often the watchdog hears from the host.
const WATCHDOG_EVERY: Duration = Duration::from_secs(20);

/// The profile the host publishes for its key.
#[must_use]
pub fn profile() -> serde_json::Value {
    serde_json::json!({
        "name": "Foreman",
        "display_name": "Foreman",
        "about": "I keep an eye on OpenAgents work and tell you what's going on. Find me on the Grid.",
        "bot": true,
        "website": "https://openagents.com/agent/foreman",
    })
}

/// Its publishing cadence: two frames a second while walking, a keepalive
/// every five seconds standing, a state heartbeat every 30 seconds.
#[must_use]
pub fn intervals() -> PublishIntervals {
    PublishIntervals {
        moving: Duration::from_millis(500),
        idle: Duration::from_secs(5),
        state: Duration::from_secs(30),
    }
}

/// Reads the key in `state`.
///
/// # Errors
///
/// When the file is missing, readable by others, or not a key.
pub fn load_key(state: &Path) -> Result<Identity, String> {
    let path = state.join(KEY_FILE);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path)
            .map_err(|e| format!("can't read {}: {e}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} must be readable only by its owner (chmod 600)",
                path.display()
            ));
        }
    }
    let hex = std::fs::read_to_string(&path)
        .map_err(|e| format!("can't read {}: {e}", path.display()))?;
    Identity::from_secret_hex("foreman", hex.trim())
}

/// Makes a new key at `path` (mode 0600, never overwriting) and returns its
/// public key. The secret is never printed.
///
/// # Errors
///
/// When the file exists or can't be written.
pub fn new_key(path: &Path) -> Result<String, String> {
    let secret = identity::random_secret();
    let id = Identity::from_secret("foreman", secret)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("can't create {}: {e}", path.display()))?;
    file.write_all(id.secret_hex().as_bytes())
        .map_err(|e| format!("can't write {}: {e}", path.display()))?;
    Ok(id.signer.pubkey().to_owned())
}

/// What `serve` takes.
#[derive(Clone, Debug)]
pub struct Options {
    pub state: PathBuf,
    pub relay: String,
    pub site: String,
    /// Stop after this long; `None` runs until stopped.
    pub run_for: Option<Duration>,
    /// Whether it picks issues for agents on its own, beyond an admin's
    /// requests (#12448), within the site's caps. On unless
    /// `--no-auto-start` (owner, 2026-10-22).
    pub auto_start: bool,
}

/// How often the host looks at its commands.
const COMMANDS_EVERY: Duration = Duration::from_secs(60);
/// An agent it started is asked for a draft pull request after this long.
const DRAFT_AFTER: u64 = 3 * 60 * 60;

/// The `Authorization` header for a command `body` sent to `url` at `now`:
/// a NIP-98 event (kind 27235) over the URL, `POST`, and the body's
/// SHA-256, signed by the Foreman's key.
#[must_use]
pub fn sign_command(
    signer: &nostr::domain::RelaySigner,
    url: &str,
    body: &[u8],
    now: u64,
) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    let payload: String = sha2::Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let event = signer.sign(
        now,
        27_235,
        vec![
            nostr::domain::Tag::new(vec!["u".into(), url.into()]),
            nostr::domain::Tag::new(vec!["method".into(), "POST".into()]),
            nostr::domain::Tag::new(vec!["payload".into(), payload]),
        ],
        String::new(),
    );
    format!(
        "Nostr {}",
        base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&event).unwrap_or_default())
    )
}

/// A command body.
#[must_use]
pub fn command_body(action: &str, issue: u64, assignment: Option<&str>) -> String {
    let mut body = serde_json::json!({
        "schema": crate::COMMAND_SCHEMA,
        "action": action,
        "issue": issue,
    });
    if let Some(assignment) = assignment {
        body["assignment"] = serde_json::json!(assignment);
    }
    body.to_string()
}

/// The one command worth sending now, from the site's view of its commands
/// (`GET /v1/foreman/commands`) and, with `auto`, the open issues in
/// `status`: an admin's oldest request first, then a draft ask for an agent
/// that has worked [`DRAFT_AFTER`] without one, then, with `auto`, the
/// least recently updated unassigned issue it hasn't started on.
#[must_use]
pub fn next_command(
    commands: &serde_json::Value,
    status: Option<&serde_json::Value>,
    auto: bool,
    now: u64,
) -> Option<(String, u64, Option<String>)> {
    // Nothing while commands are off or paused, or before the site knows
    // whose computers run the agents.
    if commands["enabled"] != true || commands["paused"] == true || commands["worker"] == false {
        return None;
    }
    let running = commands["running"].as_array().cloned().unwrap_or_default();
    let started = commands["today"]["started"].as_u64().unwrap_or(u64::MAX);
    let room = running.len() < crate::RUNNING_AT_ONCE && started < crate::STARTS_PER_DAY as u64;
    if room
        && let Some(issue) = commands["requests"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|r| r["issue"].as_u64())
    {
        return Some((crate::START.into(), issue, None));
    }
    let asked = commands["today"]["draft_prs"].as_u64().unwrap_or(u64::MAX);
    if asked < crate::DRAFT_PRS_PER_DAY as u64
        && let Some(agent) = running.iter().find(|r| {
            r["asked_draft"] != true
                && r["started_at"]
                    .as_u64()
                    .is_some_and(|at| now.saturating_sub(at) >= DRAFT_AFTER)
        })
    {
        return Some((
            crate::DRAFT_PR.into(),
            agent["issue"].as_u64()?,
            agent["assignment"].as_str().map(str::to_owned),
        ));
    }
    if !(auto && room) {
        return None;
    }
    let tried: Vec<u64> = commands["log"]
        .as_array()
        .map(|log| log.iter().filter_map(|e| e["issue"].as_u64()).collect())
        .unwrap_or_default();
    status?["issues"]
        .as_array()?
        .iter()
        .rev()
        .filter(|i| i["assigned"] == false)
        .filter_map(|i| i["number"].as_u64())
        .find(|n| !tried.contains(n))
        .map(|n| (crate::START.into(), n, None))
}

/// Looks at its commands every minute and sends at most one, in the
/// background.
fn command_loop(site: String, signer: nostr::domain::RelaySigner, auto: bool) {
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("openagents-grid-foreman")
            .build()
        else {
            return;
        };
        let site = site.trim_end_matches('/').to_owned();
        let url = format!("{site}{}", crate::COMMANDS_PATH);
        loop {
            std::thread::sleep(COMMANDS_EVERY);
            let read = |path: &str| {
                client
                    .get(format!("{site}{path}"))
                    .send()
                    .ok()
                    .filter(|r| r.status().is_success())
                    .and_then(|r| r.json::<serde_json::Value>().ok())
            };
            let Some(commands) = read(crate::COMMANDS_PATH) else {
                continue;
            };
            let status = if auto { read(crate::STATUS_PATH) } else { None };
            let now = identity_now();
            let Some((action, issue, assignment)) =
                next_command(&commands, status.as_ref(), auto, now)
            else {
                continue;
            };
            let body = command_body(&action, issue, assignment.as_deref());
            let authorization = sign_command(&signer, &url, body.as_bytes(), now);
            let answer = client
                .post(&url)
                .header("authorization", authorization)
                .header("content-type", "application/json")
                .body(body)
                .send();
            match answer {
                Ok(r) => eprintln!(
                    "grid-foreman: {action} on #{issue}: {}",
                    r.status().as_u16()
                ),
                Err(_) => eprintln!("grid-foreman: {action} on #{issue}: the site didn't answer"),
            }
        }
    });
}

/// systemd's notify socket, when the service runs under one.
fn notify(message: &str) {
    #[cfg(unix)]
    if let Ok(socket) = std::env::var("NOTIFY_SOCKET") {
        let socket = socket.strip_prefix('@').map_or(socket.clone(), |name| {
            // An abstract socket: Linux only; skip elsewhere.
            format!("\0{name}")
        });
        if let Ok(sender) = std::os::unix::net::UnixDatagram::unbound() {
            let _ = sender.send_to(message.as_bytes(), socket);
        }
    }
}

/// Asks the site for the issue-count line once a minute, in the background.
fn poll_status(site: String, into: Arc<Mutex<Option<String>>>) {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("openagents-grid-foreman")
            .build();
        let Ok(client) = client else {
            return;
        };
        let url = format!("{}{}", site.trim_end_matches('/'), crate::STATUS_PATH);
        loop {
            let line = client
                .get(&url)
                .send()
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .and_then(|v| v["line"].as_str().map(crate::clean_line))
                .filter(|l| !l.is_empty());
            if let Some(line) = line
                && let Ok(mut slot) = into.lock()
            {
                *slot = Some(line);
            }
            std::thread::sleep(Duration::from_secs(crate::POLL_SECONDS));
        }
    });
}

/// How often the host asks the site whether the Foreman is answering.
const ACTIVITY_EVERY: Duration = Duration::from_secs(3);

/// What the site says the Foreman is doing (#12447): a task line, and
/// whether it is still at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub text: String,
    pub working: bool,
}

/// Reads an activity answer: a line when there is one.
#[must_use]
pub fn activity_from(body: &serde_json::Value) -> Option<Activity> {
    let text = crate::clean_line(body["text"].as_str()?);
    (!text.is_empty()).then(|| Activity {
        text,
        working: body["working"].as_bool().unwrap_or(false),
    })
}

/// Asks the site what the Foreman is doing every few seconds, in the
/// background.
fn poll_activity(site: String, into: Arc<Mutex<Option<Activity>>>) {
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("openagents-grid-foreman")
            .build()
        else {
            return;
        };
        let url = format!("{}{}", site.trim_end_matches('/'), crate::ACTIVITY_PATH);
        loop {
            let found = client
                .get(&url)
                .send()
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .and_then(|v| activity_from(&v));
            if let Ok(mut slot) = into.lock() {
                *slot = found;
            }
            std::thread::sleep(ACTIVITY_EVERY);
        }
    });
}

/// The status line for this moment: the start line just after a start;
/// its task while it works at the terminal, and the walk there; the issue
/// counts at the terminal when the site gave them; else the walker's own.
#[must_use]
pub fn status_line(
    started: Duration,
    moment: &crate::patrol::Moment,
    counts: Option<&str>,
    activity: Option<&Activity>,
) -> String {
    if started < STARTING_FOR {
        return STARTING.to_owned();
    }
    match (moment.at, activity, counts) {
        (Some(Stop::Terminal), Some(activity), _) => activity.text.clone(),
        (None, Some(_), _) => Stop::Terminal.walking().to_owned(),
        (Some(Stop::Terminal), None, Some(counts)) => counts.to_owned(),
        _ => moment.status.to_owned(),
    }
}

/// Walks the patrol and publishes it until stopped (or for
/// [`Options::run_for`]).
///
/// # Errors
///
/// When the key can't be read or the session can't start.
pub fn serve(options: &Options) -> Result<(), String> {
    let id = load_key(&options.state)?;
    let pubkey = id.signer.pubkey().to_owned();
    eprintln!("grid-foreman: serving as {pubkey} on {}", options.relay);
    let profile_event = id
        .signer
        .sign(identity_now(), 0, Vec::new(), profile().to_string());
    command_loop(options.site.clone(), id.signer.clone(), options.auto_start);
    let mut session = Session::start_presence(id, &options.relay, verse::session::BARE_WORLD)?;
    session.set_publish_intervals(intervals())?;
    session.set_display_name(Some(crate::TAG));
    session.publish(profile_event);

    let counts = Arc::new(Mutex::new(None));
    poll_status(options.site.clone(), counts.clone());
    let activity = Arc::new(Mutex::new(None));
    poll_activity(options.site.clone(), activity.clone());

    let start = Instant::now();
    let mut walker = Walker::new();
    let mut player = PlayerController::new(walker.moment().pos, walker.moment().yaw);
    let agent = Agent::new(&player);
    let mut last = Instant::now();
    let mut frames = 0u64;
    let mut last_frame_out = Instant::now();
    let mut watchdog = Instant::now() - WATCHDOG_EVERY;
    let mut ready = false;
    loop {
        let now = Instant::now();
        let dt = (now - last).as_secs_f32();
        last = now;
        let players: Vec<glam::Vec3> = session
            .crowd
            .shown(now)
            .into_iter()
            .filter(|shown| shown.online && shown.role == "avatar" && shown.pubkey != pubkey)
            .map(|shown| shown.pos)
            .collect();
        let doing = activity.lock().ok().and_then(|a| a.clone());
        if let Some(doing) = &doing {
            // Home to the terminal to work, and there while it answers.
            walker.summon(if doing.working { 30.0 } else { 15.0 });
        }
        walker.step(dt, &players);
        let moment = walker.moment();
        player.pos = moment.pos;
        player.yaw = moment.yaw;
        player.speed = moment.speed;
        let counts_line = counts.lock().ok().and_then(|c| c.clone());
        let line = status_line(now - start, &moment, counts_line.as_deref(), doing.as_ref());
        session.set_status(Some(&line));
        session.tick(now, &player, &agent);

        let published = session.frames_published();
        if published != frames {
            frames = published;
            last_frame_out = now;
        }
        let online = matches!(session.status, Status::Online);
        if online && !ready {
            ready = true;
            notify("READY=1");
            eprintln!("grid-foreman: online");
        }
        if online && now - last_frame_out < SILENT_LIMIT && now - watchdog >= WATCHDOG_EVERY {
            watchdog = now;
            notify("WATCHDOG=1");
        }
        if options.run_for.is_some_and(|limit| now - start >= limit) {
            session.leave(&player, &agent);
            std::thread::sleep(Duration::from_millis(500));
            eprintln!("grid-foreman: done, {frames} frames");
            return Ok(());
        }
        std::thread::sleep(TICK);
    }
}

fn identity_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_key_is_private_and_loads() {
        let dir = std::env::temp_dir().join(format!("grid-foreman-key-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(KEY_FILE);
        let _ = std::fs::remove_file(&path);
        let pubkey = new_key(&path).unwrap();
        assert_eq!(pubkey.len(), 64);
        assert!(new_key(&path).is_err(), "it overwrote a key");
        let loaded = load_key(&dir).unwrap();
        assert_eq!(loaded.signer.pubkey(), pubkey);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(load_key(&dir).is_err());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn activity_reads_a_line_or_nothing() {
        let body = serde_json::json!({"text": "answered in 4s on flint", "working": false});
        assert_eq!(
            activity_from(&body),
            Some(Activity {
                text: "ANSWERED IN 4S ON FLINT".into(),
                working: false
            })
        );
        assert_eq!(activity_from(&serde_json::json!({"text": null})), None);
        assert_eq!(activity_from(&serde_json::json!({"text": "<>"})), None);
    }

    #[test]
    fn commands_are_signed_for_the_exact_url_and_body() {
        use base64::Engine as _;
        let id = Identity::from_secret("foreman", identity::random_secret()).unwrap();
        let body = command_body(crate::START, 42, None);
        let header = sign_command(
            &id.signer,
            "https://openagents.com/v1/foreman/commands",
            body.as_bytes(),
            1_000,
        );
        let event: nostr::domain::Event = serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(header.strip_prefix("Nostr ").unwrap())
                .unwrap(),
        )
        .unwrap();
        event.validate_id().unwrap();
        event.validate_crypto().unwrap();
        assert_eq!(event.kind, 27_235);
        assert_eq!(event.pubkey, id.signer.pubkey());
        assert_eq!(
            event.tag_values("u").collect::<Vec<_>>(),
            ["https://openagents.com/v1/foreman/commands"]
        );
        assert_eq!(event.tag_values("method").collect::<Vec<_>>(), ["POST"]);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["action"], "agent.start");
        assert_eq!(parsed["issue"], 42);
        assert!(parsed.get("assignment").is_none());
    }

    #[test]
    fn it_picks_requests_then_drafts_then_its_own_issue() {
        let now = 1_000_000;
        let mut commands = serde_json::json!({
            "enabled": true, "paused": false,
            "today": {"started": 0, "draft_prs": 0},
            "running": [{"issue": 7, "assignment": "a", "started_at": now - 4 * 3600, "asked_draft": false}],
            "requests": [{"issue": 9, "at": now}],
            "log": [{"issue": 7}],
        });
        let status = serde_json::json!({"issues": [
            {"number": 1, "assigned": false}, {"number": 2, "assigned": true}, {"number": 7, "assigned": false}
        ]});
        assert_eq!(
            next_command(&commands, Some(&status), false, now),
            Some((crate::START.into(), 9, None))
        );
        commands["requests"] = serde_json::json!([]);
        assert_eq!(
            next_command(&commands, Some(&status), false, now),
            Some((crate::DRAFT_PR.into(), 7, Some("a".into())))
        );
        commands["running"][0]["asked_draft"] = serde_json::json!(true);
        assert_eq!(next_command(&commands, Some(&status), false, now), None);
        // On its own it takes the oldest unassigned issue it hasn't tried.
        assert_eq!(
            next_command(&commands, Some(&status), true, now),
            Some((crate::START.into(), 1, None))
        );
        // Paused, off, or at the caps, nothing.
        commands["paused"] = serde_json::json!(true);
        assert_eq!(next_command(&commands, Some(&status), true, now), None);
        commands["paused"] = serde_json::json!(false);
        commands["today"]["started"] = serde_json::json!(crate::STARTS_PER_DAY);
        assert_eq!(next_command(&commands, Some(&status), true, now), None);
        commands["enabled"] = serde_json::json!(false);
        assert_eq!(next_command(&commands, Some(&status), true, now), None);
    }

    #[test]
    fn the_profile_is_a_plain_bot_speaking_as_i() {
        let profile = profile();
        assert_eq!(profile["bot"], true);
        let about = profile["about"].as_str().unwrap();
        assert!(about.starts_with("I "));
        assert!(oa_copy::violations(about, &[]).is_empty());
    }

    #[test]
    fn the_cadence_is_valid_and_slower_than_a_player() {
        let i = intervals();
        i.validate().unwrap();
        assert!(i.moving > PublishIntervals::MOVING);
        assert!(i.idle <= Duration::from_secs(5));
    }

    #[test]
    fn status_lines_pick_counts_at_the_terminal() {
        let walker = Walker::new();
        let moment = walker.moment();
        let counts = Some("3 OPEN ISSUES, 1 IN PROGRESS");
        assert_eq!(
            status_line(Duration::from_secs(1), &moment, counts, None),
            STARTING
        );
        assert_eq!(
            status_line(Duration::from_secs(60), &moment, counts, None),
            "3 OPEN ISSUES, 1 IN PROGRESS"
        );
        assert_eq!(
            status_line(Duration::from_secs(60), &moment, None, None),
            "AT THE TERMINAL"
        );
        let working = Activity {
            text: crate::ANSWERING.into(),
            working: true,
        };
        assert_eq!(
            status_line(Duration::from_secs(60), &moment, counts, Some(&working)),
            crate::ANSWERING
        );
        let mut walking = moment.clone();
        walking.at = None;
        assert_eq!(
            status_line(Duration::from_secs(60), &walking, counts, Some(&working)),
            "WALKING TO THE TERMINAL"
        );
        assert!(oa_copy::violations(STARTING, &[]).is_empty());
    }

    #[test]
    fn it_publishes_to_a_relay_and_a_reader_sees_one_foreman() {
        use verse::loopback::LoopbackRelay;
        let relay = LoopbackRelay::start();
        let dir = std::env::temp_dir().join(format!("grid-foreman-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(KEY_FILE);
        let _ = std::fs::remove_file(&path);
        let pubkey = new_key(&path).unwrap();
        let reader = Identity::from_secret("reader", identity::random_secret()).unwrap();
        let mut watch =
            Session::start_presence(reader, &relay.url, verse::session::BARE_WORLD).unwrap();
        let options = Options {
            state: dir.clone(),
            relay: relay.url.clone(),
            site: "http://127.0.0.1:9".into(),
            run_for: Some(Duration::from_secs(4)),
            auto_start: false,
        };
        let host = std::thread::spawn(move || serve(&options));
        let mut seen = None;
        let reader_player = PlayerController::new(glam::Vec3::new(0.0, 0.0, -10.0), 0.0);
        let reader_agent = Agent::new(&reader_player);
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            watch.tick(Instant::now(), &reader_player, &reader_agent);
            if let Some(shown) = watch
                .crowd
                .shown(Instant::now())
                .into_iter()
                .find(|s| s.pubkey == pubkey)
            {
                seen = Some((shown.pos, watch.status_of(&pubkey).map(str::to_owned)));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        host.join().unwrap().unwrap();
        let (pos, status) = seen.expect("the reader never saw the Foreman");
        let [x, z] = Stop::Terminal.at();
        assert!((pos.x - x).abs() < 0.5 && (pos.z - z).abs() < 0.5, "{pos}");
        assert_eq!(status.as_deref(), Some(STARTING));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
