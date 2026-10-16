//! Shared Computers client for account-owned Solver jobs. The saved request,
//! command, and output cursor survive a dispatcher restart; credentials do not.

use crate::{Credentials, GoogleToken, Progress, Reader, Run, Stop, dispatch::Site};
use fs2::FileExt;
use openagents_sdk::{
    CancellationToken, Client, Deployment,
    computers::{
        CommandOptions, CommandOutcome, CommandRef, CommandRequest, ComputerRef, CreateOptions,
        CreateRequest, OutputReader, WaitFor,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

/// The work service supplies a credential limited to this saved job binding.
#[derive(Clone, Deserialize)]
pub struct Job {
    pub binding: Value,
    pub account: String,
    credential: String,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub stop_reason: Option<String>,
}
impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputerJob").finish_non_exhaustive()
    }
}
impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.account == other.account
            && self.binding == other.binding
            && self.credential == other.credential
    }
}
impl Eq for Job {}

#[derive(Serialize, Deserialize)]
struct Saved {
    schema: String,
    account: String,
    binding: Value,
    create: CreateRequest,
    computer: Option<ComputerRef>,
    command: Option<CommandRequest>,
    command_id: Option<CommandRef>,
    output: Option<OutputReader>,
    reader: Reader,
    pending: Vec<u8>,
    gap: bool,
    result: Option<Value>,
    deleted: bool,
}

struct Journal {
    path: PathBuf,
    _lock: File,
}
impl Journal {
    fn open(root: &Path, id: &str) -> Result<Self, String> {
        if !id.starts_with("wrc")
            || id.len() != 27
            || !id[3..].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("The job ID is invalid.".into());
        }
        fs::create_dir_all(root).map_err(|_| "The job folder couldn't be opened.")?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "The job folder couldn't be protected.")?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(root.join(format!("{id}.lock")))
            .map_err(|_| "The job lock couldn't be opened.")?;
        lock.try_lock_exclusive()
            .map_err(|_| "This job is already running on this dispatcher.")?;
        Ok(Self {
            path: root.join(format!("{id}.json")),
            _lock: lock,
        })
    }
    fn read(&self) -> Result<Option<Saved>, String> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| "The saved job couldn't be read.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("The saved job couldn't be opened.".into()),
        }
    }
    fn save(&self, saved: &Saved) -> Result<(), String> {
        let path = self.path.with_extension("pending");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| "The job couldn't be saved.")?;
        file.write_all(&serde_json::to_vec(saved).map_err(|_| "The job couldn't be saved.")?)
            .and_then(|_| file.sync_all())
            .map_err(|_| "The job couldn't be saved.")?;
        fs::rename(path, &self.path)
            .and_then(|_| File::open(self.path.parent().unwrap())?.sync_all())
            .map_err(|_| "The job couldn't be saved.".into())
    }
}

fn error(message: impl Into<String>) -> Value {
    json!({"engine":"solver","ok":false,"error":message.into(),"cost_usd":null})
}

/// No account credential or provider identifier is written to this command.
fn command(run: &Run, id: &str, revision: &str) -> String {
    let q = boat::shell_quote;
    let entry = include_str!("../../../deploy/solver-box/solver-box-run");
    let script = include_str!("box-run.sh").replace(
        "-v /run/oa-solver:/run/oa-solver \"$IMAGE\"",
        "-v /run/oa-solver:/run/oa-solver --entrypoint bash \"$IMAGE\" /run/oa-solver/entrypoint",
    );
    let script = script.replace(
        "docker run --rm --privileged",
        &format!("docker run --rm --name oa-solver-{id} --privileged"),
    );
    format!(
        r#"set -eu
umask 077
sudo -n install -d -m 700 -o "$(id -u)" -g "$(id -g)" /run/oa-solver /run/oa-solver-host
printf '%s' "$OA_SOLVER_ENV_BODY" > /run/oa-solver/env
printf '%s' "$OA_SOLVER_GOOGLE_TOKEN" > /run/oa-solver/vertex-token
printf '%s' "$OA_SOLVER_REGISTRY_TOKEN" > /run/oa-solver-host/registry-token
printf '\nOA_SOURCE_REVISION=%s\n' {revision} >> /run/oa-solver/env
printf '%s' {entry} > /run/oa-solver/entrypoint
printf '%s' {script} > /run/oa-solver-host/run.sh
(
  while :; do
    sleep 60
    if curl --fail --silent --show-error --connect-timeout 5 --max-time 15 \
      -H "Authorization: Bearer $OA_SOLVER_REFRESH_CREDENTIAL" \
      "$OA_SOLVER_REFRESH_URL" -o /run/oa-solver/vertex-token.next 2>/dev/null; then
      mv /run/oa-solver/vertex-token.next /run/oa-solver/vertex-token
    else
      rm -f /run/oa-solver/vertex-token.next
    fi
  done
) &
refresher=$!
unset OA_SOLVER_ENV_BODY OA_SOLVER_GOOGLE_TOKEN OA_SOLVER_REGISTRY_TOKEN
unset OA_SOLVER_REFRESH_URL OA_SOLVER_REFRESH_CREDENTIAL
d=$(mktemp -d)
trap 'kill "$refresher" 2>/dev/null || true; sudo -n docker rm -f oa-solver-{id} >/dev/null 2>&1 || true; rm -rf "$d" /run/oa-solver /run/oa-solver-host' EXIT
mkfifo "$d/events"
LC_ALL=C awk '{{if(length($0)){{last=$0;long=(length(last)>196608);if(long)last="";}}if(sent<32768){{text=substr($0,1,32768-sent);print text;sent+=length(text)+1;fflush();}}}} END{{if(long)exit 1;print last;fflush();}}' < "$d/events" &
reader=$!
set +e
bash /run/oa-solver-host/run.sh {image} > "$d/events"
code=$?
wait "$reader" || code=1
exit "$code"
"#,
        revision = q(revision),
        entry = q(entry),
        script = q(&script),
        image = q(&run.image)
    )
}

pub async fn run_until(
    site: &Site,
    id: &str,
    job: &Job,
    run: &Run,
    credentials: &Credentials,
    google: &GoogleToken,
    stop: &Stop,
    on: impl Fn(Progress),
) -> Value {
    let Some(root) = std::env::var_os("OA_SOLVER_STATE").map(PathBuf::from) else {
        return error("The dispatcher needs a persistent job folder.");
    };
    match execute(&root, site, id, job, run, credentials, google, stop, &on).await {
        Ok(value) => value,
        Err(why) => {
            let mut result = error(why);
            result["computer_cleanup_pending"] = json!(true);
            result
        }
    }
}

async fn execute(
    root: &Path,
    site: &Site,
    id: &str,
    job: &Job,
    run: &Run,
    credentials: &Credentials,
    google: &GoogleToken,
    stop: &Stop,
    on: &impl Fn(Progress),
) -> Result<Value, String> {
    let journal = Journal::open(root, id)?;
    let deployment = if site.base == "https://openagents.com" {
        Deployment::production()
    } else {
        Deployment::new(&format!("{}/v1", site.base), &site.base).map_err(|e| e.to_string())?
    };
    let client = Client::new(deployment).map_err(|e| e.to_string())?;
    let session = client
        .authenticate_computer_job(&job.credential)
        .await
        .map_err(|e| e.to_string())?;
    if session.account().id != job.account {
        return Err("The job belongs to another account.".into());
    }
    let api = client.computers(&session).map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    let mut saved = match journal.read()? {
        Some(saved) => {
            if saved.schema != "openagents.solver.computer-run.v1"
                || saved.account != job.account
                || saved.binding["create"] != job.binding["create"]
                || saved.binding["environment"] != job.binding["environment"]
                || saved.binding["payer"] != job.binding["payer"]
            {
                return Err("The saved computer belongs to a different job.".into());
            }
            saved
        }
        None => {
            let options: CreateOptions = serde_json::from_value(job.binding["create"].clone())
                .map_err(|_| "The computer settings are invalid.")?;
            let saved = Saved {
                schema: "openagents.solver.computer-run.v1".into(),
                account: job.account.clone(),
                binding: job.binding.clone(),
                create: api.create_request(options).map_err(|e| e.to_string())?,
                computer: None,
                command: None,
                command_id: None,
                output: None,
                reader: Reader::default(),
                pending: Vec::new(),
                gap: false,
                result: None,
                deleted: false,
            };
            journal.save(&saved)?;
            saved
        }
    };
    if saved.deleted {
        site.computer_cleaned(id).await?;
        return Ok(saved
            .result
            .unwrap_or_else(|| error("The job was stopped.")));
    }
    let started = Instant::now();
    // A lost create reply is recovered with the same saved request.
    if saved.computer.is_none() {
        if let Some(id) = job.binding["computer"].as_str() {
            saved.computer = Some(api.reference(id).map_err(|e| e.to_string())?);
            journal.save(&saved)?;
        } else if stop.load(Ordering::Relaxed) {
            site.computer_cleaned(id).await?;
            saved.deleted = true;
            saved.result = Some(error(
                job.stop_reason
                    .as_deref()
                    .unwrap_or("The job was cancelled."),
            ));
            journal.save(&saved)?;
            return Ok(saved.result.unwrap());
        }
    }
    if saved.computer.is_none() {
        let computer = api
            .create(&saved.create, &cancel)
            .await
            .map_err(|e| e.to_string())?;
        saved.computer = Some(computer.reference().clone());
        journal.save(&saved)?;
    }
    let computer = saved.computer.as_ref().unwrap().clone();
    if saved.command_id.is_none() {
        let key = format!("solver-command-{id}");
        if let Some(body) = job.binding["commands"].get(&key) {
            let options: CommandOptions = serde_json::from_value(body.clone())
                .map_err(|_| "The saved command couldn't be read.")?;
            saved.command = Some(
                api.command_request(&computer, options)
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(id) = job.binding["command_ids"]
            .as_array()
            .and_then(|ids| ids.first())
            .and_then(Value::as_str)
        {
            let reference = api
                .command_reference(&computer, id)
                .map_err(|e| e.to_string())?;
            saved.output = Some(api.output(&reference).map_err(|e| e.to_string())?);
            saved.command_id = Some(reference);
        }
        journal.save(&saved)?;
    }
    let mut refreshed = Instant::now();
    let mut refresh_after = Duration::from_secs(60);
    let work: Result<(), String> = async {
        if saved.result.is_some() {
            return Ok(());
        }
        if stop.load(Ordering::Relaxed) {
            return Err(job.stop_reason.as_deref().unwrap_or("The job was cancelled.").into());
        }
        if saved.command_id.is_none() {
            api.wake(&computer, &cancel).await.map_err(|e| e.to_string())?;
            api.wait(&computer, WaitFor::Awake, Duration::from_secs(600), &cancel)
                .await.map_err(|e| e.to_string())?;
            if stop.load(Ordering::Relaxed) {
                return Err("The job was cancelled.".into());
            }
            if saved.command.is_none() {
                let revision = job.binding["environment"]["source"]["revision"]
                    .as_str().ok_or("The source commit is missing.")?;
                let options = CommandOptions {
                    command: command(run, id, revision),
                    timeout_seconds: Some(3600),
                    idempotency_key: Some(format!("solver-command-{id}")),
                    credential_ref: Some(format!("cred_solver_{id}")),
                    cwd: None,
                    env: None,
                    extra: Default::default(),
                };
                saved.command = Some(api.command_request(&computer, options).map_err(|e| e.to_string())?);
                journal.save(&saved)?;
            }
            let request = saved.command.as_ref().unwrap();
            let token = google.get().await?;
            let preparation = json!({
                "computer": computer.id(),
                "command": request.options(),
                "values": {
                    "OA_SOLVER_ENV_BODY": crate::env_file(run, credentials),
                    "OA_SOLVER_GOOGLE_TOKEN": token,
                    "OA_SOLVER_REGISTRY_TOKEN": token,
                },
            });
            site.prepare_computer(id, &preparation).await?;
            let command = api.execute(request, &cancel).await.map_err(|e| e.to_string())?;
            saved.command_id = Some(command.reference().clone());
            saved.output = Some(api.output(command.reference()).map_err(|e| e.to_string())?);
            journal.save(&saved)?;
        }
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err("The job was cancelled.".into());
            }
            let page = saved.output.as_mut().ok_or("The output position is missing.")?
                .next_page(&api, &cancel).await.map_err(|e| e.to_string())?;
            let Some(page) = page else {
                return Err("The job ended without a result.".into());
            };
            saved.gap |= page.stdout_gap || page.stderr_gap;
            saved.pending.extend(page.stdout);
            while let Some(end) = saved.pending.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = saved.pending.drain(..=end).collect();
                for progress in saved.reader.line(&String::from_utf8_lossy(&line)) {
                    on(progress);
                }
            }
            if saved.pending.len() > 196608 {
                return Err("The task answer is too long to read.".into());
            }
            if page.terminal && !page.has_more {
                if saved.gap {
                    return Err("Some task output is missing. Review the job before trying again.".into());
                }
                let summary = saved.reader.summary.as_ref().ok_or("The task ended without a Solver result.")?;
                let already_done = matches!(page.command.outcome(), Some(CommandOutcome::Failed))
                    && page.command.data().exit_code == Some(1)
                    && summary["started"] == true && summary["ok"] == false
                    && summary["pr"].is_null() && summary["outcome"]["status"] == "already_done";
                if !matches!(page.command.outcome(), Some(CommandOutcome::Succeeded)) && !already_done {
                    return Err(match page.command.outcome() {
                        Some(CommandOutcome::Failed) => "The task failed.",
                        Some(CommandOutcome::Cancelled) => "The task was cancelled.",
                        Some(CommandOutcome::TimedOut) => "The task ran out of time.",
                        _ => "I couldn't tell whether the task finished. Review its changes before starting another.",
                    }.into());
                }
                let elapsed = summary["secs"].as_f64().filter(|s| s.is_finite() && *s >= 0.0)
                    .unwrap_or_else(|| started.elapsed().as_secs_f64());
                saved.result = Some(crate::result(Some(summary), None, &crate::BoxUsage {
                    sandbox: computer.id().into(), usd: None, secs: None,
                }, elapsed));
                journal.save(&saved)?;
                return Ok(());
            }
            journal.save(&saved)?;
            if refreshed.elapsed() >= refresh_after {
                let pulse = match google.get().await {
                    Ok(token) => site.refresh_computer_token(id, &token).await,
                    Err(error) => Err(error),
                };
                refreshed = Instant::now();
                match pulse {
                    Ok(true) => refresh_after = Duration::from_secs(60),
                    Ok(false) => return Err("Repository access ended. Reconnect it before starting another job.".into()),
                    Err(_) => refresh_after = Duration::from_secs(10),
                }
            }
        }
    }.await;
    if let Err(why) = work {
        saved.result = Some(error(why));
        journal.save(&saved)?;
    }
    // Cancellation, stop intent, and observed deletion are separate actions.
    if let Some(command) = &saved.command_id {
        let _ = api.cancel_command(command, &cancel).await;
    }
    let _ = api.stop(&computer, &cancel).await;
    api.delete(&computer, &cancel)
        .await
        .map_err(|_| "The job ended; its computer still needs cleanup.")?;
    api.wait(
        &computer,
        WaitFor::Deleted,
        Duration::from_secs(600),
        &cancel,
    )
    .await
    .map_err(|_| "The computer's deletion hasn't been confirmed yet.")?;
    saved.deleted = true;
    journal.save(&saved)?;
    site.computer_cleaned(id).await?;
    Ok(saved
        .result
        .unwrap_or_else(|| error("The job ended without a result.")))
}
