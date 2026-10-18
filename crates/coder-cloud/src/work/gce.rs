//! Issue flows use the existing durable GCE owner and the same prepared CLI
//! command as `chat work`. No local CLI process dispatches the work.
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use base64::Engine as _;
use openagents_sdk::work::{self as api, Error, LandMode, Placement, PoolStatus, WorkRequest};
use serde_json::{Value, json};

use crate::{Observation, Record, Store, budget, gce_backend, pool, runtime};
use gce_backend::Transport as _;

use super::{Runner, refused};

pub(super) struct System;

fn admission(agent: &str) -> Result<(), Error> {
    budget::admit_run_for(Some(agent)).map_err(|message| {
        let owner_command = message.split('`').nth(1).map(str::to_owned);
        let mut error = refused(message);
        error.owner_command = owner_command;
        error
    })
}

impl Runner for System {
    fn admit(&self, request: &WorkRequest) -> Result<(), Error> {
        request.validate()?;
        if request.on.as_ref().is_some_and(|on| *on != Placement::Gce) {
            return Err(refused(
                "This connection owns the GCE pool. Connect to the requested computer to start work there.",
            ));
        }
        issue_command(request)?;
        admission(&request.agent)?;
        pool::Pool::granted().map_err(|message| {
            let mut error = refused(message);
            error.owner_command = Some(format!(
                "openagents cloud up --hosts 1 --agent {}",
                request.agent
            ));
            error
        })?;
        Ok(())
    }

    fn execute(&self, store: &Store, id: &str) -> Result<(), Error> {
        let lease = store.lease(id).map_err(refused)?;
        let mut record = lease.read(id).map_err(refused)?;
        let request: WorkRequest = serde_json::from_str(&record.spec.task)
            .map_err(|_| refused("This work request could not be read."))?;
        if record.state.terminal() && record.cleanup_complete {
            return Ok(());
        }
        if lease.cancelled()
            && matches!(
                record.state,
                crate::State::Created | crate::State::Provisioning
            )
        {
            record.cancel_requested = true;
            record.state = crate::State::Cancelled;
            record.cleanup_complete = true;
            lease.save(&record).map_err(refused)?;
            return Ok(());
        }
        if record.state == crate::State::Created {
            self.admit(&request)?;
        }
        if record.state == crate::State::Running {
            if let Err(error) = budget::admit_run_for(Some(&request.agent)) {
                record.cancel_requested = true;
                record.error = Some(error);
                lease.save(&record).map_err(refused)?;
            }
        }
        let credentials = if record.state.terminal() || record.cancel_requested || lease.cancelled()
        {
            runtime::Credentials::default()
        } else {
            // Keep credential fragments for output redaction on recovery.
            // A running command needs no refreshed login to read its progress.
            credentials(
                request.engine.as_deref().unwrap_or("briefed:codex"),
                record.state != crate::State::Running,
            )
            .map_err(refused)?
        };
        let backend = Issue {
            inner: gce_backend::Gce {
                transport: Actor {
                    agent: request.agent.clone(),
                },
                credentials,
            },
            request,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| refused("The work runner could not start."))?;
        runtime
            .block_on(crate::drive(
                &backend,
                &lease,
                &mut record,
                &AtomicBool::new(false),
                Duration::from_secs(2),
                &mut |_| {},
            ))
            .map_err(refused)
    }

    fn pool(&self, method: &str, agent: &str, hosts: u32) -> Result<PoolStatus, Error> {
        let mut current = pool::Pool::load();
        if method == api::POOL_UP {
            admission(agent)?;
        } else if method == api::POOL_DOWN && !budget::Budget::load().agents.contains_key(agent) {
            return Err(Error {
                code: "refused".into(),
                message: "This saved agent has no compute grant.".into(),
                owner_command: Some(format!(
                    "openagents cloud budget --agent {agent} --daily 300"
                )),
            });
        }
        if method == api::POOL_UP {
            // Starting or growing an existing grant is allowed; creating a
            // grant remains the owner's CLI action.
            let candidate = pool::Pool::granted().map_err(|message| {
                let mut error = refused(message);
                error.owner_command = Some(format!(
                    "openagents cloud up --hosts {hosts} --agent {agent}"
                ));
                error
            })?;
            let live =
                pool::list_hosts(&candidate.project, Some(&candidate.pool)).map_err(refused)?;
            let running = live.iter().filter(|host| host.status == "RUNNING").count() as u64;
            let adding = u64::from(hosts).saturating_sub(running);
            budget::admit_for(&candidate, &live, adding, Some(agent)).map_err(refused)?;
            for _ in 0..adding {
                // Growth rechecks the live grant under the existing pool lock.
                // Never save this snapshot over an owner's concurrent revoke.
                pool::grow_host_for(&candidate, Some(agent)).map_err(refused)?;
            }
            current = Some(candidate);
        } else if method == api::POOL_DOWN {
            if let Some(mut current) = current {
                let hosts =
                    pool::list_hosts(&current.project, Some(&current.pool)).map_err(refused)?;
                pool::delete_hosts(&current.project, &hosts).map_err(refused)?;
                current.revoked_at = Some(pool::now());
                current.save().map_err(refused)?;
            }
            return Ok(PoolStatus::default());
        }
        let budget = budget::Budget::load();
        let hosts = match &current {
            Some(pool) => pool::list_hosts(&pool.project, Some(&pool.pool)).map_err(refused)?,
            None => vec![],
        };
        let ledger = budget::Ledger::load();
        let activity = match &current {
            Some(pool) => pool::activity(&pool.project, &hosts),
            None => vec![],
        };
        Ok(PoolStatus {
            pool: current.as_ref().map(|pool| pool.pool.clone()),
            granted: current
                .as_ref()
                .is_some_and(|pool| pool.revoked_at.is_none()),
            slots: hosts.iter().filter(|host| host.status == "RUNNING").count() as u64
                * current.as_ref().map_or(0, |pool| pool.slots_per_host),
            hosts: hosts.into_iter().map(|host| json!(host)).collect(),
            activity: activity
                .into_iter()
                .map(|activity| json!(activity))
                .collect(),
            spent_usd: ledger.spent(pool::now(), None),
            daily_usd: budget.daily_usd,
            agent_spent_usd: Some(ledger.spent(pool::now(), Some(agent))),
            agent_daily_usd: budget.agents.get(agent).map(|grant| grant.daily_usd),
        })
    }
}

#[derive(Clone)]
struct Actor {
    agent: String,
}
impl gce_backend::Transport for Actor {
    fn granted(&self) -> crate::Result<pool::Pool> {
        pool::Pool::granted()
    }
    fn hosts(&self, pool: &pool::Pool) -> crate::Result<Vec<pool::Host>> {
        pool::list_hosts(&pool.project, Some(&pool.pool))
    }
    fn start(&self, pool: &pool::Pool) -> crate::Result<pool::Host> {
        pool::grow_host_for(pool, Some(&self.agent))
    }
    async fn prepare_runtime(&self, pool: &pool::Pool, host: &pool::Host) -> crate::Result<()> {
        gce_backend::System.prepare_runtime(pool, host).await
    }
    async fn execute(
        &self,
        pool: &pool::Pool,
        host: &pool::Host,
        script: &str,
        input: Option<&[u8]>,
    ) -> crate::Result<String> {
        gce_backend::System.execute(pool, host, script, input).await
    }
}

struct Issue {
    inner: gce_backend::Gce<Actor>,
    request: WorkRequest,
}
impl crate::Backend for Issue {
    async fn provision(&self, record: &mut Record) -> crate::Result<String> {
        budget::admit_run_for(Some(&self.request.agent))?;
        let pool = self.inner.transport.granted()?;
        let hosts = self.inner.transport.hosts(&pool)?;
        budget::admit_for(&pool, &hosts, 0, Some(&self.request.agent))?;
        self.inner.provision(record).await
    }
    async fn prepare(&self, record: &Record) -> crate::Result<()> {
        budget::admit_run_for(Some(&self.request.agent))?;
        self.inner.prepare(record).await?;
        let host: pool::Host = serde_json::from_value(record.binding["host"].clone())
            .map_err(|_| "The work host could not be read.")?;
        let pool: pool::Pool = serde_json::from_value(record.binding["pool"].clone())
            .map_err(|_| "The pool grant could not be read.")?;
        let cap = budget::Budget::load().run_limit(&host, pool.slots_per_host.max(1));
        let dir = gce_backend::directory(record);
        let body = issue_body(record, &self.request, pool.slots_per_host, cap)?;
        let script = runtime::wrap_launch(&dir, &body);
        self.inner
            .command(
                record,
                &format!("umask 077; cat > {}", boat::shell_quote(&(dir + "/run.sh"))),
                Some(script.as_bytes()),
            )
            .await?;
        Ok(())
    }
    async fn dispatch(&self, record: &Record) -> crate::Result<crate::Task> {
        budget::admit_run_for(Some(&self.request.agent))?;
        self.inner.dispatch(record).await
    }
    async fn recover(&self, record: &Record) -> crate::Result<Option<crate::Task>> {
        self.inner.recover(record).await
    }
    async fn poll(&self, record: &Record) -> crate::Result<Observation> {
        if let Err(error) = budget::admit_run_for(Some(&self.request.agent)) {
            return Ok(Observation {
                events: vec![],
                cursor: record.cursor.clone(),
                end: Some(Err(error)),
            });
        }
        self.inner.poll(record).await
    }
    async fn cancel(&self, record: &Record) -> crate::Result<()> {
        self.inner.cancel(record).await
    }
    async fn cleanup(&self, record: &Record) -> crate::Result<Option<Value>> {
        self.inner.cleanup(record).await
    }
}

fn issue_command(request: &WorkRequest) -> Result<String, Error> {
    let engine = request.engine.as_deref().unwrap_or("briefed:codex");
    let land = match request.land {
        LandMode::Queue => "queue",
        LandMode::Pr => "pr",
        LandMode::None if engine.starts_with("briefed") => "none",
        LandMode::None => {
            return Err(Error::invalid(
                "Choose queue or pr for this engine; it cannot leave work unlanded.",
            ));
        }
    };
    pool::issue_command(request.issue, Some(land), engine).map_err(refused)
}

fn issue_body(
    record: &Record,
    request: &WorkRequest,
    slots: u64,
    cap: Duration,
) -> crate::Result<String> {
    let command = issue_command(request).map_err(|error| error.to_string())?;
    let dir = boat::shell_quote(&gce_backend::directory(record));
    let url = boat::shell_quote(&format!("https://github.com/{}.git", request.repo));
    let command = boat::shell_quote(&command);
    Ok(format!(
        r#"set -u
d={dir}
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/bin:$PATH"
export CODEX_HOME="/tmp/oa-coder-{id}/codex"
unset OA_CODEX_AUTH
finish() {{ rc=$?; printf '%s' "$rc" > "$d/exit.writing"; mv "$d/exit.writing" "$d/exit"; touch "$HOME/.oa-pool/busy"; }}
trap finish EXIT
expires=$(( $(date +%s) + {seconds} ))
slot=''
while [ -z "$slot" ]; do
 for i in $(seq 0 {last}); do
  exec 8>"$HOME/.oa-pool/slot-$i.lock"
  if flock -n 8; then slot=$i; break; fi
 done
 [ -n "$slot" ] && break
 [ "$(date +%s)" -lt "$expires" ] || exit 124
 sleep 1
done
touch "$HOME/.oa-pool/busy"
gh auth setup-git >/dev/null 2>&1 || true
if [ ! -d "$d/workspace/repo/.git" ]; then
 clone_limit=$(( expires - $(date +%s) ))
 [ "$clone_limit" -gt 0 ] || exit 124
 [ "$clone_limit" -le 120 ] || clone_limit=120
 timeout --kill-after=10s "$clone_limit" git clone -- {url} "$d/workspace/repo" >"$d/clone.out" 2>"$d/err" || exit $?
fi
cd "$d/workspace/repo" || exit 2
remaining=$(( expires - $(date +%s) ))
[ "$remaining" -gt 0 ] || exit 124
timeout --kill-after=10s "$remaining" sh -c {command} >"$d/out" 2>"$d/err"
rc=$?
python3 - "$d/out" "$rc" <<'PY'
import json,sys
path,code=sys.argv[1],int(sys.argv[2]);result=None
with open(path) as stream:
 for line in stream:
  try: item=json.loads(line)
  except ValueError: continue
  if item.get('event')=='issue':result=item
if result is not None:
 result['event']='finished';result['ok']=code==0
 with open(path,'a') as stream:stream.write(json.dumps(result)+'\n')
PY
exit "$rc"
"#,
        id = record.id,
        last = slots.saturating_sub(1),
        seconds = cap.as_secs().max(1).min(12 * 3600)
    ))
}

fn credentials(engine: &str, fresh: bool) -> crate::Result<runtime::Credentials> {
    let mut values = BTreeMap::new();
    let github = std::env::var("OA_BOAT_GH_TOKEN")
        .ok()
        .or_else(|| std::env::var("GH_TOKEN").ok())
        .or_else(|| {
            pool::gcloud_in(
                &pool::project(),
                &[
                    "secrets",
                    "versions",
                    "access",
                    "latest",
                    "--secret",
                    "coder-pool-git-token",
                ],
                Duration::from_secs(60),
            )
            .ok()
            .map(|value| value.trim().to_owned())
        })
        .filter(|value| !value.is_empty())
        .ok_or("Connect GitHub before starting issue work on the pool.")?;
    values.insert("GH_TOKEN".into(), github);
    if engine == "briefed" || engine == "briefed:codex" {
        let home = std::env::var_os("CODEX_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| user_home::home().map(|home| home.join(".codex")))
            .ok_or(user_home::NO_HOME)?;
        let path = std::env::var_os("OA_CODER_CODEX_AUTH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home.join("auth.json"));
        let text = std::fs::read_to_string(path).map_err(
            |_| "Sign Codex in on this computer before starting pool work (`codex login`).",
        )?;
        let login = access_only_login(&text, pool::now(), fresh)?;
        values.insert("OA_CODEX_AUTH".into(), login);
    } else {
        for key in ["CLAUDE_CODE_OAUTH_TOKEN", "XAI_API_KEY"] {
            if let Ok(value) = std::env::var(key) {
                values.insert(key.into(), value);
            }
        }
    }
    for key in ["OPENAGENTS_TOKEN", "OPENAGENTS_SITE", "OPENAGENTS_API_BASE"] {
        if let Ok(value) = std::env::var(key) {
            values.insert(key.into(), value);
        }
    }
    runtime::Credentials::from_names(&values.keys().cloned().collect::<Vec<_>>(), |key| {
        values.get(key).cloned()
    })
}

fn access_only_login(text: &str, now: u64, fresh: bool) -> crate::Result<String> {
    let auth: Value =
        serde_json::from_str(text).map_err(|_| "The Codex login could not be read.")?;
    let access = auth["tokens"]["access_token"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("Sign Codex in with ChatGPT before starting pool work.")?;
    let payload = access
        .split('.')
        .nth(1)
        .ok_or("The Codex login has no expiry.")?;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| "The Codex login could not be read.")?;
    let payload: Value =
        serde_json::from_slice(&payload).map_err(|_| "The Codex login could not be read.")?;
    if fresh && payload["exp"].as_u64().unwrap_or(0).saturating_sub(now) < 7200 {
        return Err("Refresh Codex on this computer before starting pool work; its login needs two hours left.".into());
    }
    Ok(json!({"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"id_token":auth["tokens"]["id_token"],"access_token":access,"refresh_token":"","account_id":auth["tokens"]["account_id"]},"last_refresh":auth["last_refresh"]}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> WorkRequest {
        WorkRequest {
            repo: "example/project".into(),
            issue: 12062,
            land: LandMode::Queue,
            engine: Some("briefed:codex".into()),
            on: Some(Placement::Gce),
            agent: "agt_builder".into(),
        }
    }
    #[test]
    fn issue_launch_keeps_engine_cli_identity_slots_and_a_remote_budget_deadline() {
        let request = request();
        let record = Record::new(
            "work-fixture",
            crate::Spec {
                placement: crate::Placement::Gce,
                mode: crate::Mode::Coder,
                agent: request.agent.clone(),
                task: serde_json::to_string(&request).unwrap(),
                model: None,
                reasoning: None,
                cwd: "/fixture".into(),
                timeout_seconds: 3600,
                size: "default".into(),
                template: None,
                credential_names: vec![],
            },
        )
        .unwrap();
        let body = issue_body(&record, &request, 2, Duration::from_secs(60)).unwrap();
        assert!(
            body.contains("--engine")
                && body.contains("briefed:codex")
                && body.contains("--land queue")
        );
        assert!(body.contains("OA_CLI=$HOME/.oa-pool/bin/openagents"));
        assert!(body.contains("slot-$i.lock") && body.contains("expires - $(date +%s)"));
        assert!(body.contains("timeout --kill-after=10s \"$remaining\" sh -c"));
        assert!(body.contains("clone_limit=120") && body.contains("\"$clone_limit\" git clone"));
        let script = runtime::wrap_launch(&gce_backend::directory(&record), &body);
        assert!(script.contains("owner.lock") && script.contains("pid-start"));
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-n")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    }
    #[test]
    fn an_unlanded_request_never_falls_back_to_repository_policy() {
        let mut request = request();
        request.land = LandMode::None;
        assert!(issue_command(&request).unwrap().ends_with("--land none"));
        request.engine = Some("bare".into());
        assert!(issue_command(&request).is_err());
        assert_eq!(System.admit(&request).unwrap_err().code, "invalid");
    }

    #[test]
    fn a_remote_codex_login_never_carries_the_refresh_token() {
        let access = format!(
            "header.{}.signature",
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(json!({"exp":10000}).to_string())
        );
        let login=json!({"tokens":{"access_token":access,"refresh_token":"fixture-refresh-must-stay-local","id_token":"fixture-id","account_id":"fixture-account"}}).to_string();
        let remote = access_only_login(&login, 1000, true).unwrap();
        assert!(!remote.contains("fixture-refresh-must-stay-local"));
        let remote: Value = serde_json::from_str(&remote).unwrap();
        assert_eq!(remote["tokens"]["refresh_token"], "");
        assert!(access_only_login(&login, 9000, true).is_err());
        assert!(
            access_only_login(&login, 11000, false).is_ok(),
            "expired fragments still redact a running job's output"
        );
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    #[test]
    fn a_gce_connection_refuses_other_placements_before_reading_credentials() {
        let request = WorkRequest {
            repo: "example/project".into(),
            issue: 12,
            land: LandMode::Queue,
            engine: None,
            on: Some(Placement::Boat),
            agent: "agt_reviewer".into(),
        };
        let error = System.admit(&request).unwrap_err();
        assert!(error.message.contains("requested computer"));
    }
}
