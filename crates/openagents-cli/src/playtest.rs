//! `openagents playtest`: the triage inbox for playtest reports
//! (`docs/game/playtest-triage.md`).
//!
//! `inbox` reads the NIP-17 gift wraps addressed to the triage key from the
//! OpenAgents relay, opens them with `playtest::report::open`, drops
//! deliveries the triage log already holds, and writes one issue draft per
//! new report for a person to edit and approve. `file` creates the GitHub
//! issue only with `--approve` and records the acceptance; `decide`,
//! `verify`, and `session` record the other outcomes. The triage log
//! (`log.jsonl`) is append-only. The triage key's secret is read from a
//! file only its owner can read and is never printed.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use nostr::domain::{Event, RelaySigner};
use playtest::report::{self, Opened};
use playtest::triage::{
    self, Contribution, Decision, Draft, Entry, Format, Log, Severity, Verified,
};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents playtest COMMAND [OPTIONS]
  keygen --out PATH             Create a triage key file (0600); print only its npub.
  inbox --triage-key PATH       Read new reports and draft an issue for each.
        [--relay URL] [--since UNIX] [--timeout SECONDS]
  file CODE --contribution feedback|bug|design [--severity p0|p1|p2|p3]
        [--triager KEY] [--repo OWNER/REPO] [--approve | --issue N]
                                Without --approve, show what would be filed.
                                --approve runs `gh issue create`; --issue records
                                an issue a person already filed or kept.
  decide CODE --decision duplicate|not-reproducible|design|idea|declined
        --reason TEXT [--issue N]
  verify --issue N --fix-build \"1.0.0 (16)\" --verified yes|no|unverified
  session --tester KEY --script NAME --format unmoderated|moderated|group|diary
        --build \"1.0.0 (15)\" [--code PT-…] [--moderator KEY]
  log [--acceptances | --pending]
  award (CODE | --issue N [--verified] | --script NAME --tester KEY)
        --quest ID@VERSION [--session ID] [--triager KEY] [--commit SHA]
        [--referee-key PATH] [--relay URL] [--repo OWNER/REPO] [--publish]
                                Sign the NIP-XP playtest award for an accepted
                                contribution with the playtest referee key;
                                without --publish, show it. Off until the
                                playtest referee key exists.
  testflight [--asc-env FILE] [--app ID] [--since 2026-09-28]
                                Read TestFlight feedback (screenshots and
                                crashes) from App Store Connect and draft each
                                new one. The key comes from FILE or the
                                ASC_API_KEY_ID, ASC_API_ISSUER_ID, and
                                ASC_API_PRIVATE_KEY_PATH variables.
Files live in ~/.openagents/playtest (OPENAGENTS_PLAYTEST_HOME overrides):
log.jsonl, and drafts/CODE.md (edit it before filing), .json, .report.json,
and .jpg (private; never published).";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("keygen", Effect::LocalWrite),
    Declared::computer("inbox", Effect::LocalWrite),
    Declared::computer("file", Effect::Publishes),
    Declared::computer("decide", Effect::LocalWrite),
    Declared::computer("verify", Effect::LocalWrite),
    Declared::computer("session", Effect::LocalWrite),
    Declared::computer("log", Effect::ReadOnly),
    Declared::computer("award", Effect::Publishes),
    Declared::computer("testflight", Effect::LocalWrite),
];

const DEFAULT_REPO: &str = "OpenAgentsInc/openagents";
/// Gift wraps backdate their timestamps by up to two days.
const WRAP_JITTER: u64 = 2 * 24 * 60 * 60;

pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_PLAYTEST_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    home.join(".openagents").join("playtest")
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("playtest", "a command is required", USAGE);
    };
    let args = match Args::parse(
        rest,
        &[
            "approve",
            "acceptances",
            "pending",
            "help",
            "verified",
            "publish",
        ],
    ) {
        Ok(args) => args,
        Err(message) => return output.usage("playtest", &message, USAGE),
    };
    let home = home();
    let result = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return 0;
        }
        "keygen" => keygen(&args),
        "inbox" => inbox(&home, &args),
        "file" => file(&home, &args),
        "decide" => decide(&home, &args),
        "verify" => verify(&home, &args),
        "session" => session(&home, &args),
        "log" => log(&home, &args),
        "testflight" => testflight::run(&home, &args),
        "award" => award::run(&home, &args),
        other => return output.usage("playtest", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(Failure::Usage(message)) => output.usage("playtest", &message, USAGE),
        Err(Failure::Failed(message)) => output.fail("playtest", &message),
    }
}

enum Failure {
    Usage(String),
    Failed(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

fn usage(message: impl Into<String>) -> Failure {
    Failure::Usage(message.into())
}

fn render(value: &Value) -> String {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return text.to_owned();
    }
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn now() -> u64 {
    crate::relay::unix_now()
}

fn required<'a>(args: &'a Args, name: &str) -> Result<&'a str, Failure> {
    args.option(name)
        .ok_or_else(|| usage(format!("--{name} is required")))
}

fn code_arg(args: &Args) -> Result<String, Failure> {
    match args.positional() {
        [code] => Ok(code.to_ascii_uppercase()),
        _ => Err(usage("give one report code, such as PT-1A2B3C4D")),
    }
}

fn parse_enum<T: serde::de::DeserializeOwned>(name: &str, value: &str) -> Result<T, Failure> {
    serde_json::from_value(Value::String(value.to_ascii_lowercase()))
        .map_err(|_| usage(format!("--{name} doesn't take `{value}`")))
}

/// A public key given as an npub or 64 hex characters, as hex.
fn public_key(value: &str) -> Result<String, Failure> {
    let bytes = if value.starts_with("npub1") {
        nostr::nip19::decode_npub(value).map_err(|_| usage(format!("`{value}` isn't an npub")))?
    } else {
        let parsed: secp256k1::XOnlyPublicKey = value
            .parse()
            .map_err(|_| usage(format!("`{value}` isn't a public key")))?;
        parsed.serialize()
    };
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn log_path(home: &Path) -> PathBuf {
    home.join("log.jsonl")
}

fn drafts(home: &Path) -> PathBuf {
    home.join("drafts")
}

fn load(home: &Path) -> Result<Log, String> {
    match std::fs::read_to_string(log_path(home)) {
        Ok(text) => Log::parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Log::default()),
        Err(error) => Err(format!("reading the triage log: {error}")),
    }
}

fn private_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("creating {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Checks `entry` against the log and appends it as one line.
fn append(home: &Path, log: &mut Log, entry: Entry) -> Result<(), String> {
    log.admit(&entry)?;
    private_dir(home)?;
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(log_path(home))
        .and_then(|mut file| file.write_all(Log::line(&entry).as_bytes()))
        .map_err(|e| format!("appending to the triage log: {e}"))?;
    log.entries.push(entry);
    Ok(())
}

/// Reads a triage key file: an nsec or 64 hex characters, readable only by
/// its owner.
pub fn read_key(path: &Path) -> Result<SecretKey, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} is readable by others; run chmod 600 on it",
                path.display()
            ));
        }
    }
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let text = text.trim();
    let bytes = if text.starts_with("nsec1") {
        nostr::nip19::decode_nsec(text).map_err(|_| "the key file's nsec doesn't decode")?
    } else {
        let parsed: SecretKey = text
            .parse()
            .map_err(|_| "the key file isn't an nsec or hex key")?;
        parsed.secret_bytes()
    };
    SecretKey::from_byte_array(bytes).map_err(|_| "the key file's key isn't valid".into())
}

fn npub_of(secret: &SecretKey) -> (String, String) {
    let (key, _) = secret.x_only_public_key(&Secp256k1::new());
    (key.to_string(), nostr::nip19::encode_npub(&key.serialize()))
}

fn keygen(args: &Args) -> Result<Value, Failure> {
    let out = PathBuf::from(required(args, "out")?);
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(&out)
        .and_then(|mut file| {
            file.write_all(nostr::nip19::encode_nsec(&secret.secret_bytes()).as_bytes())
        })
        .map_err(|e| format!("creating {} (it must not exist yet): {e}", out.display()))?;
    let (hex, npub) = npub_of(&secret);
    Ok(json!({
        "path": out.display().to_string(),
        "npub": npub,
        "pubkey": hex,
        "text": format!("Created {}.\nnpub: {npub}\nhex: {hex}\nThe secret stays in the file; it is never printed.", out.display()),
    }))
}

/// What one inbox run did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Ingested {
    /// New report codes, in arrival order.
    pub new: Vec<String>,
    /// Each new **Give feedback** report (#10127), one line: its code, the
    /// selected text, and the comment, for the operator's terminal only.
    pub feedback: Vec<String>,
    /// Deliveries the log already held.
    pub repeats: usize,
    /// Wraps that weren't playtest reports for this key.
    pub refused: usize,
}

/// Opens `events` with the triage key, keeps the reports the log hasn't
/// seen, appends a `received` entry for each, and writes its draft files.
pub fn ingest(
    home: &Path,
    events: &[Event],
    triage: &SecretKey,
    at: u64,
) -> Result<Ingested, String> {
    let mut log = load(home)?;
    let mut result = Ingested::default();
    let mut opened: Vec<Opened> = Vec::new();
    for event in events {
        match report::open(event, triage) {
            Ok(report) => opened.push(report),
            Err(_) => result.refused += 1,
        }
    }
    let total = opened.len();
    let fresh = triage::fresh(opened, &log);
    result.repeats = total - fresh.len();
    let dir = drafts(home);
    private_dir(&dir)?;
    for report in fresh {
        write_draft(&dir, &report)?;
        append(home, &mut log, Entry::received(&report, at))?;
        if let Some(selection) = &report.report.selection {
            result.feedback.push(feedback_line(
                &report.code,
                selection,
                &report.report.happened,
            ));
        }
        result.new.push(report.code);
    }
    Ok(result)
}

/// `PT-1A2B3C4D on “the selected text” (turn 3, model gpt-5.4): the comment`.
fn feedback_line(code: &str, selection: &playtest::report::Selection, comment: &str) -> String {
    let short = |text: &str, max: usize| {
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if flat.chars().count() > max {
            format!("{}…", flat.chars().take(max).collect::<String>())
        } else {
            flat
        }
    };
    let mut from = vec![];
    if let Some(turn) = selection.turn {
        from.push(format!("turn {turn}"));
    }
    if let Some(model) = &selection.model {
        from.push(format!("model {model}"));
    }
    let from = if from.is_empty() {
        String::new()
    } else {
        format!(" ({})", from.join(", "))
    };
    format!(
        "{code} on “{}”{from}: {}",
        short(&selection.text, 120),
        short(comment, 240)
    )
}

fn write_draft(dir: &Path, opened: &Opened) -> Result<(), String> {
    let draft = triage::draft(opened);
    let code = &opened.code;
    private_file(&dir.join(format!("{code}.md")), draft.markdown().as_bytes())?;
    let meta = json!({"code": code, "labels": draft.labels, "tester": opened.tester, "digest": opened.digest});
    private_file(
        &dir.join(format!("{code}.json")),
        meta.to_string().as_bytes(),
    )?;
    let mut private = opened.report.clone();
    if let Some(shot) = private.screenshot.take() {
        let jpeg = base64::engine::general_purpose::STANDARD
            .decode(shot.jpeg_base64.as_bytes())
            .map_err(|_| format!("{code}'s screenshot isn't base64"))?;
        private_file(&dir.join(format!("{code}.jpg")), &jpeg)?;
    }
    let text = serde_json::to_string_pretty(&private).unwrap_or_default();
    private_file(&dir.join(format!("{code}.report.json")), text.as_bytes())
}

fn inbox(home: &Path, args: &Args) -> Result<Value, Failure> {
    let triage = read_key(Path::new(required(args, "triage-key")?))?;
    let relay = args.option("relay").unwrap_or(playtest::RELAY).to_owned();
    let timeout = Duration::from_secs(args.number("timeout", 20u64).map_err(usage)?);
    let (hex, npub) = npub_of(&triage);
    let mut filter = json!({"kinds": [1059], "#p": [hex]});
    if let Some(since) = args.option("since") {
        let since: u64 = since
            .parse()
            .map_err(|_| usage("--since takes Unix seconds"))?;
        filter["since"] = json!(since.saturating_sub(WRAP_JITTER));
    }
    let signer = RelaySigner::from_secret_hex(&triage.display_secret().to_string())
        .map_err(|_| "the triage key can't sign".to_owned())?;
    let mut client = crate::relay::Client::connect(&relay, signer);
    let mut events = Vec::new();
    let mut read = |client: &mut crate::relay::Client| {
        client.subscribe(vec![filter.clone()], false, timeout, |event| {
            events.push(event.clone());
        })
    };
    // The relay serves gift wraps only to the authenticated reader.
    if let Err(error) = read(&mut client) {
        if !error.contains("auth-required")
            || !client.authenticate(std::time::Instant::now() + timeout)
        {
            client.close();
            return Err(Failure::Failed(error));
        }
        read(&mut client).map_err(Failure::Failed)?;
    }
    client.close();
    let result = ingest(home, &events, &triage, now())?;
    Ok(json!({
        "triage_npub": npub,
        "relay": relay,
        "read": events.len(),
        "new": result.new,
        "feedback": result.feedback,
        "repeats": result.repeats,
        "refused": result.refused,
        "drafts": drafts(home).display().to_string(),
        "text": format!(
            "{} wraps read from {relay}: {} new, {} already in the log, {} not reports.{}{}",
            events.len(), result.new.len(), result.repeats, result.refused,
            if result.new.is_empty() { String::new() } else {
                format!("\nEdit the drafts in {} and file each with `openagents playtest file CODE --contribution … --approve`:\n  {}", drafts(home).display(), result.new.join("\n  "))
            },
            if result.feedback.is_empty() { String::new() } else {
                format!("\nFeedback on selected text:\n  {}", result.feedback.join("\n  "))
            }
        ),
    }))
}

fn file(home: &Path, args: &Args) -> Result<Value, Failure> {
    let code = code_arg(args)?;
    let mut log = load(home)?;
    if !log.pending().contains(&code.as_str()) {
        return Err(Failure::Failed(format!(
            "{code} isn't waiting for triage (see `openagents playtest log --pending`)"
        )));
    }
    let contribution: Contribution = parse_enum("contribution", required(args, "contribution")?)?;
    let severity: Option<Severity> = args
        .option("severity")
        .map(|s| parse_enum("severity", s))
        .transpose()?;
    if contribution == Contribution::Bug && severity.is_none() {
        return Err(usage("a bug needs --severity p0, p1, p2, or p3"));
    }
    let triager = args.option("triager").map(public_key).transpose()?;
    let repo = args.option("repo").unwrap_or(DEFAULT_REPO);
    let dir = drafts(home);
    let meta: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(format!("{code}.json")))
            .map_err(|e| format!("reading {code}'s draft: {e}"))?,
    )
    .map_err(|e| format!("reading {code}'s draft: {e}"))?;
    let labels: Vec<String> = serde_json::from_value(meta["labels"].clone()).unwrap_or_default();
    let body_path = dir.join(format!("{code}.md"));
    let draft = Draft::from_markdown(
        &code,
        labels,
        &std::fs::read_to_string(&body_path).map_err(|e| format!("reading the draft: {e}"))?,
    )?;
    let record = |issue: u64| Entry::Filed {
        at: now(),
        code: code.clone(),
        issue,
        contribution,
        severity,
        triager: triager.clone(),
    };
    if let Some(issue) = args.option("issue") {
        let issue: u64 = issue
            .trim_start_matches('#')
            .parse()
            .map_err(|_| usage("--issue takes a number"))?;
        append(home, &mut log, record(issue))?;
        return Ok(
            json!({"code": code, "issue": issue, "text": format!("Recorded {code} as accepted on #{issue}.")}),
        );
    }
    if draft.body.contains(triage::PARAPHRASE) || draft.title.ends_with("(write a title)") {
        return Err(Failure::Failed(format!(
            "{} still has placeholders: the tester didn't allow quoting, so write the title and text in your own words first",
            body_path.display()
        )));
    }
    let mut command = vec![
        "issue".to_owned(),
        "create".into(),
        "--repo".into(),
        repo.into(),
        "--title".into(),
        draft.title.clone(),
        "--body-file".into(),
        "-".into(),
    ];
    for label in &draft.labels {
        command.extend(["--label".into(), label.clone()]);
    }
    if !args.switch("approve") {
        return Ok(json!({
            "code": code,
            "title": draft.title,
            "labels": draft.labels,
            "dry_run": true,
            "text": format!(
                "Would file on {repo}:\n  {}\n  labels: {}\nNothing was filed. Re-run with --approve to create it.",
                draft.title, draft.labels.join(", ")
            ),
        }));
    }
    for label in draft.labels.iter().filter(|l| *l != triage::LABEL) {
        let _ = std::process::Command::new("gh")
            .args(["label", "create", label, "--repo", repo, "--force"])
            .output();
    }
    let mut child = std::process::Command::new("gh")
        .args(&command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("running gh: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(draft.body.as_bytes())
            .map_err(|e| format!("writing to gh: {e}"))?;
    }
    let done = child
        .wait_with_output()
        .map_err(|e| format!("running gh: {e}"))?;
    if !done.status.success() {
        return Err(Failure::Failed("gh issue create failed".into()));
    }
    let url = String::from_utf8_lossy(&done.stdout).trim().to_owned();
    let issue: u64 = url
        .rsplit('/')
        .next()
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("gh printed no issue URL: {url}"))?;
    append(home, &mut log, record(issue))?;
    Ok(json!({"code": code, "issue": issue, "url": url, "text": format!("Filed {code} as {url}.")}))
}

fn decide(home: &Path, args: &Args) -> Result<Value, Failure> {
    let code = code_arg(args)?;
    let decision: Decision = parse_enum("decision", required(args, "decision")?)?;
    let reason = required(args, "reason")?.trim().to_owned();
    if reason.is_empty() {
        return Err(usage("--reason can't be empty"));
    }
    let issue = args
        .option("issue")
        .map(|i| {
            i.trim_start_matches('#')
                .parse::<u64>()
                .map_err(|_| usage("--issue takes a number"))
        })
        .transpose()?;
    let mut log = load(home)?;
    append(
        home,
        &mut log,
        Entry::Decided {
            at: now(),
            code: code.clone(),
            decision,
            reason,
            issue,
        },
    )?;
    Ok(
        json!({"code": code, "decision": decision, "text": format!("Recorded {code}: {}.", playtest::session::name(&decision))}),
    )
}

fn verify(home: &Path, args: &Args) -> Result<Value, Failure> {
    let issue: u64 = required(args, "issue")?
        .trim_start_matches('#')
        .parse()
        .map_err(|_| usage("--issue takes a number"))?;
    let fix_build = required(args, "fix-build")?.to_owned();
    let verified: Verified = parse_enum("verified", required(args, "verified")?)?;
    let mut log = load(home)?;
    append(
        home,
        &mut log,
        Entry::Verified {
            at: now(),
            issue,
            fix_build: fix_build.clone(),
            verified,
        },
    )?;
    Ok(
        json!({"issue": issue, "fix_build": fix_build, "verified": verified,
        "text": format!("Recorded #{issue} on {fix_build}: {}.", playtest::session::name(&verified))}),
    )
}

fn session(home: &Path, args: &Args) -> Result<Value, Failure> {
    let tester = public_key(required(args, "tester")?)?;
    let script = required(args, "script")?.to_owned();
    if script.is_empty()
        || script.len() > 64
        || !script
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(usage("--script is a slug such as session-2"));
    }
    let format: Format = parse_enum("format", required(args, "format")?)?;
    let build = required(args, "build")?.to_owned();
    let moderator = args.option("moderator").map(public_key).transpose()?;
    let code = args.option("code").map(str::to_ascii_uppercase);
    let mut log = load(home)?;
    append(
        home,
        &mut log,
        Entry::Session {
            at: now(),
            tester,
            script: script.clone(),
            format,
            build,
            code,
            moderator,
        },
    )?;
    Ok(json!({"script": script, "text": format!("Recorded {script} for the tester.")}))
}

fn log(home: &Path, args: &Args) -> Result<Value, Failure> {
    let log = load(home)?;
    if args.switch("pending") {
        let pending = log.pending();
        return Ok(
            json!({"pending": pending, "text": if pending.is_empty() { "Nothing waiting.".to_owned() } else { pending.join("\n") }}),
        );
    }
    if args.switch("acceptances") {
        let accepted = log.acceptances();
        let text = accepted
            .iter()
            .map(|a| {
                format!(
                    "{} {} {} {}{}",
                    a.accepted_at,
                    a.contribution,
                    &a.tester[..16.min(a.tester.len())],
                    a.issue
                        .map(|i| format!("#{i}"))
                        .or_else(|| a.script.clone())
                        .unwrap_or_default(),
                    a.severity
                        .map(|s| format!(" {}", playtest::session::name(&s)))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(
            json!({"acceptances": accepted, "text": if text.is_empty() { "No accepted contributions yet.".into() } else { text }}),
        );
    }
    let entries: Vec<Value> = log
        .entries
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or_default())
        .collect();
    let text = entries
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(json!({"entries": entries, "text": text}))
}

mod award;
mod testflight;

#[cfg(test)]
mod tests;
