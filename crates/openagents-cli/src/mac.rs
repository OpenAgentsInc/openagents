//! `openagents mac`: Mac-only steps sent to a Mac linked to the account
//! (#11223), from a cloud environment, an agent, or any terminal.
//!
//! - `mac run RECIPE --ref REF [-- ARGS]` sends a job and follows it: its
//!   log, the owner's approval when it needs one, its result, and (with
//!   `--out DIR`) its files.
//! - `mac jobs`, `mac show ID [--follow]`, `mac fetch ID --out DIR`,
//!   `mac cancel ID`, and `mac macs` read and stop jobs and list the Macs.
//! - `mac serve` runs on the Mac: it takes the account's jobs and runs
//!   them ([`crate::mac_serve`]). `mac capabilities` shows what it reports.
//!
//! Every call goes to the website under the account's own sign-in: Coder's
//! (`coder login`, kept in `~/.openagents/coder-new`), another folder's
//! (`--account-dir`), or `OPENAGENTS_APP_TOKEN` with `OPENAGENTS_ORIGIN` in
//! an environment. The token is never printed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use coder::cli_route::tree::{Declared, Effect};
use mac_jobs::Recipe;
use openagents_login::Saved;
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents mac COMMAND
  run RECIPE --ref REF [--repo OWNER/NAME] [--computer NAME] [--out DIR] [--no-wait] [-- ARGS]
              Send a job to a Mac linked to your account and follow it: its log,
              the owner's approval when it needs one, its result, and with --out
              its files. RECIPE is ios-release-gate (--simulator NAME),
              ios-testflight (--validate-only, --build N; waits for the owner),
              desktop-capture (--kept), or xcodebuild (allowlisted arguments).
  jobs        Your Mac jobs, newest first.
  show ID [--follow]
              One job and its log; --follow waits for it to end.
  fetch ID --out DIR
              Save the files a job made.
  cancel ID   Stop a job.
  macs        Your linked Macs and what each can do.
  capabilities [--root DIR]
              What this Mac reports: macOS, Xcode, signing identities by name,
              simulators, and whether an App Store Connect key is here.
  serve [--computer NAME] [--root DIR] [--repo OWNER/NAME[=PATH]]... [--min-free-gb N] [--once]
              Run on the Mac: take your account's jobs and run each in its own
              worktree and build folder. Signing and App Store keys stay here.
Signs in as Coder does (coder login), or with --account-dir DIR, or with
OPENAGENTS_APP_TOKEN and OPENAGENTS_ORIGIN. The repository defaults to
OpenAgentsInc/openagents. See docs/cloud/linked-mac.md.";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("run", Effect::Publishes),
    Declared::computer("jobs", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("fetch", Effect::LocalWrite),
    Declared::computer("cancel", Effect::Publishes),
    Declared::computer("macs", Effect::ReadOnly),
    Declared::computer("capabilities", Effect::ReadOnly),
    Declared::computer("serve", Effect::LongRunning),
];

const DEFAULT_REPO: &str = "OpenAgentsInc/openagents";
/// The longest a single read waits on the website.
const READ_WAIT: u64 = 20;

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, &["no-wait", "follow", "once"]) {
        Ok(args) => args,
        Err(message) => return output.usage("mac", &message, USAGE),
    };
    let positional = args.positional();
    let command = positional.first().map(String::as_str).unwrap_or_default();
    let known: &[&str] = match command {
        "run" => &["ref", "repo", "computer", "out", "account-dir"],
        "jobs" | "cancel" | "macs" => &["account-dir"],
        "show" => &["account-dir"],
        "fetch" => &["out", "account-dir"],
        "capabilities" => &["root"],
        "serve" => &["computer", "root", "repo", "min-free-gb", "account-dir"],
        _ => return output.usage("mac", &format!("unknown command `{command}`"), USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !known.contains(name))
    {
        return output.usage("mac", &format!("unknown option `--{name}`"), USAGE);
    }
    let result = match command {
        "capabilities" => {
            let root = args.option("root").map_or_else(default_root, PathBuf::from);
            let caps = crate::mac_serve::capabilities(&root);
            Ok(json!(caps))
        }
        "serve" => serve(&args),
        _ => match account(args.option("account-dir")) {
            Ok(site) => match command {
                "run" => submit(output, &site, &args),
                "jobs" => site.get("/v1/mac-jobs"),
                "macs" => site.get("/v1/mac-jobs/macs"),
                "show" => match positional.get(1) {
                    Some(id) if args.switch("follow") => follow(output, &site, id, None),
                    Some(id) => site.get(&format!("/v1/mac-jobs/{}", segment(id))),
                    None => return output.usage("mac", "show takes a job id", USAGE),
                },
                "fetch" => match (positional.get(1), args.option("out")) {
                    (Some(id), Some(out)) => fetch(&site, id, Path::new(out)),
                    _ => return output.usage("mac", "fetch takes a job id and --out DIR", USAGE),
                },
                "cancel" => match positional.get(1) {
                    Some(id) => {
                        site.post(&format!("/v1/mac-jobs/{}/cancel", segment(id)), &json!({}))
                    }
                    None => return output.usage("mac", "cancel takes a job id", USAGE),
                },
                _ => unreachable!("checked above"),
            },
            Err(why) => Err(why),
        },
    };
    match result {
        Ok(value) => {
            let failed = value["state"]
                .as_str()
                .is_some_and(|state| matches!(state, "failed" | "cancelled"));
            output.emit(&value, |value| render(command, value));
            if failed { crate::EXIT_FAILURE } else { 0 }
        }
        Err(why) => output.fail(&format!("mac {command}"), &why),
    }
}

/// A job id or name as one path segment.
/// Random bytes for a request key.
fn rand_bytes() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        bytes = (nanos ^ u128::from(std::process::id())).to_le_bytes();
    }
    bytes
}

fn segment(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("mac-jobs")
}

/// The account's sign-in: `OPENAGENTS_APP_TOKEN` and `OPENAGENTS_ORIGIN`,
/// else the folder named, else Coder's.
pub(crate) fn saved(dir: Option<&str>) -> Result<Saved, String> {
    if dir.is_none()
        && let Ok(token) = std::env::var("OPENAGENTS_APP_TOKEN")
        && !token.trim().is_empty()
    {
        let origin = openagents_login::origin_from(|name| std::env::var(name).ok());
        return serde_json::from_value(json!({
            "origin": origin,
            "account": "",
            "label": "this account",
            "expires_at": u64::MAX,
            "token": token.trim(),
        }))
        .map_err(|_| "OPENAGENTS_APP_TOKEN couldn't be read.".to_owned());
    }
    let dir = dir.map_or_else(
        || {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".openagents")
                .join("coder-new")
        },
        PathBuf::from,
    );
    let saved = Saved::load(&dir).ok_or_else(|| {
        format!(
            "Not signed in here ({}). Sign in with: coder login",
            dir.display()
        )
    })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    if saved.expired(now) {
        return Err("This sign-in has expired. Sign in again with: coder login".into());
    }
    Ok(saved)
}

/// The website, as the signed-in account.
struct Site {
    saved: Saved,
    http: reqwest::blocking::Client,
}

fn account(dir: Option<&str>) -> Result<Site, String> {
    let saved = saved(dir)?;
    let http = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(READ_WAIT + 30))
        .build()
        .map_err(|error| format!("Couldn't start the web client: {error}"))?;
    Ok(Site { saved, http })
}

impl Site {
    fn answer(response: reqwest::Result<reqwest::blocking::Response>) -> Result<Value, String> {
        let response = response.map_err(|_| "The website couldn't be reached.".to_owned())?;
        let status = response.status();
        let body: Value = response.json().unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(body);
        }
        if status.as_u16() == 401 {
            return Err("This sign-in stopped working. Sign in again with: coder login".into());
        }
        Err(body["error"]["message"]
            .as_str()
            .map_or_else(|| format!("The website answered {status}."), str::to_owned))
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        Self::answer(
            self.http
                .get(format!("{}{path}", self.saved.origin))
                .bearer_auth(self.saved.token())
                .send(),
        )
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        Self::answer(
            self.http
                .post(format!("{}{path}", self.saved.origin))
                .bearer_auth(self.saved.token())
                .json(body)
                .send(),
        )
    }

    /// `POST` that may be sent again: it carries one `Idempotency-Key`, and
    /// a request that got no answer, or a busy answer, is tried again with
    /// the same key, so the website makes one job however many times it
    /// arrives (#11253).
    fn post_once(&self, path: &str, body: &Value) -> Result<Value, String> {
        let bytes: [u8; 16] = rand_bytes();
        let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let mut last = Err("The website couldn't be reached.".to_owned());
        for attempt in 0..4u32 {
            if attempt > 0 {
                std::thread::sleep(Duration::from_secs(2u64.pow(attempt)));
            }
            let sent = self
                .http
                .post(format!("{}{path}", self.saved.origin))
                .bearer_auth(self.saved.token())
                .header("idempotency-key", &key)
                .json(body)
                .send();
            let again = match &sent {
                Err(_) => true,
                Ok(response) => {
                    response.status().is_server_error() || response.status().as_u16() == 429
                }
            };
            last = Self::answer(sent);
            if !again {
                break;
            }
        }
        last
    }

    fn download(&self, path: &str, to: &Path) -> Result<u64, String> {
        let mut response = self
            .http
            .get(format!("{}{path}", self.saved.origin))
            .bearer_auth(self.saved.token())
            .timeout(Duration::from_secs(1800))
            .send()
            .map_err(|_| "The website couldn't be reached.".to_owned())?;
        if !response.status().is_success() {
            return Err(format!("The website answered {}.", response.status()));
        }
        let mut file =
            std::fs::File::create(to).map_err(|error| format!("{}: {error}", to.display()))?;
        response
            .copy_to(&mut file)
            .map_err(|error| format!("{}: {error}", to.display()))
    }
}

/// The job `mac run` sends.
pub(crate) fn job_body(args: &Args) -> Result<Value, String> {
    let positional = args.positional();
    let recipe = positional.get(1).ok_or(
        "run takes a recipe: ios-release-gate, ios-testflight, desktop-capture, or xcodebuild",
    )?;
    let recipe = Recipe::parse(recipe).ok_or_else(|| {
        format!("{recipe} isn't a recipe: ios-release-gate, ios-testflight, desktop-capture, or xcodebuild")
    })?;
    let git_ref = args.option("ref").ok_or("run takes --ref REF")?;
    let spec = mac_jobs::Spec {
        repo: args.option("repo").unwrap_or(DEFAULT_REPO).to_owned(),
        git_ref: git_ref.to_owned(),
        recipe,
        args: positional[2..].to_vec(),
    };
    spec.check()?;
    let mut body = json!({
        "repo": spec.repo,
        "ref": spec.git_ref,
        "recipe": spec.recipe,
        "args": spec.args,
    });
    if let Some(computer) = args.option("computer") {
        body["computer"] = json!(computer);
    }
    Ok(body)
}

fn submit(output: &Output, site: &Site, args: &Args) -> Result<Value, String> {
    let body = job_body(args)?;
    let queued = site.post_once("/v1/mac-jobs", &body)?;
    let id = queued["id"].as_str().unwrap_or_default().to_owned();
    if args.switch("no-wait") {
        return Ok(queued);
    }
    if !output.json() {
        eprintln!(
            "Sent to {}{}.",
            queued["computer"].as_str().unwrap_or("your Mac"),
            if queued["online"] == true {
                ""
            } else {
                " (offline now; it runs when the Mac is back)"
            }
        );
    }
    let job = follow(output, site, &id, args.option("out").map(Path::new))?;
    Ok(job)
}

/// Follow job `id` to its end, printing its log, then save its files to
/// `out`.
fn follow(output: &Output, site: &Site, id: &str, out: Option<&Path>) -> Result<Value, String> {
    let mut after = 0u64;
    let mut asked: Option<String> = None;
    let job = loop {
        let job = site.get(&format!(
            "/v1/mac-jobs/{}?after={after}&wait={READ_WAIT}",
            segment(id)
        ))?;
        for line in job["lines"].as_array().into_iter().flatten() {
            if let Some(line) = line.as_str() {
                if output.json() {
                    output.line(&json!({"line": line}), |_| String::new());
                } else {
                    eprintln!("{line}");
                }
            }
        }
        after = job["next"].as_u64().unwrap_or(after);
        if job["state"] == "asking"
            && let Some(question) = job["question"]["id"].as_str()
            && asked.as_deref() != Some(question)
        {
            asked = Some(question.to_owned());
            eprintln!(
                "Waiting for the owner's approval, on the phone or at {}/settings/mac-jobs/{}:\n{}",
                site.saved.origin,
                id,
                job["question"]["text"].as_str().unwrap_or_default()
            );
        }
        if matches!(job["state"].as_str(), Some("done" | "failed" | "cancelled")) {
            break job;
        }
    };
    if let Some(out) = out {
        fetch(site, id, out)?;
    }
    Ok(job)
}

/// Save the files job `id` made under `out`.
fn fetch(site: &Site, id: &str, out: &Path) -> Result<Value, String> {
    let job = site.get(&format!("/v1/mac-jobs/{}", segment(id)))?;
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    let mut saved = Vec::new();
    for artifact in job["artifacts"].as_array().into_iter().flatten() {
        let (Some(name), Some(url)) = (artifact["name"].as_str(), artifact["url"].as_str()) else {
            continue;
        };
        // Names come from the website's own check: no folders.
        if name.contains('/') || name.starts_with('.') {
            continue;
        }
        let to = out.join(name);
        let bytes = site.download(url, &to)?;
        saved.push(json!({"name": name, "size": bytes, "path": to}));
    }
    let mut job = job;
    job["saved"] = json!(saved);
    Ok(job)
}

fn serve(args: &Args) -> Result<Value, String> {
    let saved = saved(args.option("account-dir"))?;
    let here = std::env::current_dir().ok();
    let repos = crate::mac_serve::repos(&args.options("repo"), here.as_deref())?;
    let settings = crate::mac_serve::Settings {
        computer: args
            .option("computer")
            .map_or_else(openagents_login::computer_name, str::to_owned),
        root: args.option("root").map_or_else(default_root, PathBuf::from),
        repos,
        min_free_gb: args.number("min-free-gb", 40u64)?,
        once: args.switch("once"),
        approvals: coder_new::risk_policy::approvals_path(),
    };
    crate::mac_serve::serve(&saved, &settings)?;
    Ok(json!({"served": true}))
}

fn render(command: &str, value: &Value) -> String {
    match command {
        "jobs" => {
            let mut rows = vec![vec![
                "ID".to_owned(),
                "STATE".to_owned(),
                "MAC".to_owned(),
                "JOB".to_owned(),
            ]];
            for job in value["jobs"].as_array().into_iter().flatten() {
                rows.push(vec![
                    job["id"].as_str().unwrap_or_default().to_owned(),
                    job["state"].as_str().unwrap_or_default().to_owned(),
                    job["computer"].as_str().unwrap_or_default().to_owned(),
                    job["title"].as_str().unwrap_or_default().to_owned(),
                ]);
            }
            if rows.len() == 1 {
                return "No Mac jobs yet.".into();
            }
            crate::out::table(&rows)
        }
        "macs" => {
            let macs = value["macs"].as_array().cloned().unwrap_or_default();
            if macs.is_empty() {
                return "No Mac is linked yet. On the Mac: openagents mac serve".into();
            }
            macs.iter()
                .map(|mac| {
                    let caps = &mac["capabilities"];
                    format!(
                        "{} ({}): macOS {}, Xcode {}, {} signing identities, {} simulators, App Store Connect key {}",
                        mac["name"].as_str().unwrap_or_default(),
                        if mac["online"] == true { "online" } else { "offline" },
                        caps["macos"].as_str().unwrap_or("unknown"),
                        caps["xcode"].as_str().unwrap_or("none"),
                        caps["signing_identities"].as_array().map_or(0, Vec::len),
                        caps["simulators"].as_array().map_or(0, Vec::len),
                        if caps["asc_key"] == true { "present" } else { "missing" },
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "capabilities" | "serve" => serde_json::to_string_pretty(value).unwrap_or_default(),
        _ => {
            if value["id"].is_string() && value["state"].is_string() {
                let mut text = format!(
                    "{} {}: {}",
                    value["id"].as_str().unwrap_or_default(),
                    value["title"].as_str().unwrap_or_default(),
                    value["state"].as_str().unwrap_or_default()
                );
                for key in ["summary", "why"] {
                    if let Some(said) = value[key].as_str() {
                        text.push_str(&format!("\n{said}"));
                    }
                }
                for artifact in value["artifacts"].as_array().into_iter().flatten() {
                    text.push_str(&format!(
                        "\nfile {} ({} bytes)",
                        artifact["name"].as_str().unwrap_or_default(),
                        artifact["size"]
                    ));
                }
                for saved in value["saved"].as_array().into_iter().flatten() {
                    text.push_str(&format!(
                        "\nsaved {}",
                        saved["path"].as_str().unwrap_or_default()
                    ));
                }
                text
            } else if let Some(id) = value["id"].as_str() {
                format!(
                    "Sent job {id} to {}. Follow it with: openagents mac show {id} --follow",
                    value["computer"].as_str().unwrap_or("your Mac")
                )
            } else {
                value.to_string()
            }
        }
    }
}
