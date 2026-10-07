//! Headless Coder jobs on the existing granted GCE pool.
use crate::{
    Backend, Mode, Observation, Record, Result, Task,
    pool::{self, Host, Pool},
    runtime::{self, Credentials},
};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;

#[allow(async_fn_in_trait)]
pub trait Transport {
    fn granted(&self) -> Result<Pool>;
    fn hosts(&self, pool: &Pool) -> Result<Vec<Host>>;
    fn start(&self, pool: &Pool) -> Result<Host>;
    async fn prepare_runtime(&self, _pool: &Pool, _host: &Host) -> Result<()> {
        Ok(())
    }
    async fn execute(
        &self,
        pool: &Pool,
        host: &Host,
        script: &str,
        input: Option<&[u8]>,
    ) -> Result<String>;
}
pub struct System;
impl Transport for System {
    fn granted(&self) -> Result<Pool> {
        Pool::granted()
    }
    fn hosts(&self, p: &Pool) -> Result<Vec<Host>> {
        pool::list_hosts(&p.project, Some(&p.pool))
    }
    fn start(&self, p: &Pool) -> Result<Host> {
        pool::grow_host(p)
    }
    async fn prepare_runtime(&self, p: &Pool, h: &Host) -> Result<()> {
        self.execute(
            p,
            h,
            &format!(
                "bash -c {}",
                boat::shell_quote(&runtime_preparation(pool::BUILD))
            ),
            None,
        )
        .await?;
        Ok(())
    }
    async fn execute(
        &self,
        p: &Pool,
        h: &Host,
        script: &str,
        input: Option<&[u8]>,
    ) -> Result<String> {
        let remote = format!("timeout 610 sh -c {}", boat::shell_quote(script));
        let command = pool::ssh(&p.project, h, &remote);
        let mut child = tokio::process::Command::from(command)
            .kill_on_drop(true)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Cannot start GCE SSH.")?;
        if let Some(bytes) = input {
            let mut stdin = child.stdin.take().ok_or("Missing SSH input.")?;
            tokio::time::timeout(Duration::from_secs(120), stdin.write_all(bytes))
                .await
                .map_err(|_| "GCE upload timed out.")?
                .map_err(|_| "GCE upload disconnected.")?;
        }
        let output = tokio::time::timeout(Duration::from_secs(650), child.wait_with_output())
            .await
            .map_err(|_| "GCE SSH timed out.")?
            .map_err(|_| "GCE SSH disconnected.")?;
        if !output.status.success() {
            return Err("The GCE command failed or disconnected.".into());
        }
        if output.stdout.len() > 32 * 1024 * 1024 {
            return Err("The GCE response exceeds its limit.".into());
        }
        String::from_utf8(output.stdout).map_err(|_| "Invalid GCE response encoding.".into())
    }
}
pub struct Gce<T = System> {
    pub transport: T,
    pub credentials: Credentials,
}
impl Gce {
    pub fn from_env(names: &[String]) -> Result<Self> {
        Ok(Self {
            transport: System,
            credentials: Credentials::from_names(names, |n| std::env::var(n).ok())?,
        })
    }
}
pub fn directory(r: &Record) -> String {
    format!("/home/coder/.oa-pool/runs/{}", r.id)
}
fn binding(r: &Record) -> Result<(Pool, Host)> {
    let p: Pool = serde_json::from_value(r.binding["pool"].clone())
        .map_err(|_| "Invalid retained GCE grant.")?;
    let h: Host = serde_json::from_value(r.binding["host"].clone())
        .map_err(|_| "Invalid retained GCE host.")?;
    if r.resource.as_deref() != Some(h.name.as_str()) {
        return Err("GCE resource identity mismatch.".into());
    }
    Ok((p, h))
}
impl<T: Transport> Gce<T> {
    fn admitted(&self, r: &Record) -> Result<(Pool, Host)> {
        let (p, h) = binding(r)?;
        let live = self.transport.granted()?;
        if live.project != p.project || live.grant != p.grant || live.epoch != p.epoch {
            return Err("The retained GCE grant no longer holds. Cancel this job before starting under the new grant.".into());
        }
        Ok((p, h))
    }
    pub async fn command(&self, r: &Record, script: &str, input: Option<&[u8]>) -> Result<String> {
        let (p, h) = self.admitted(r)?;
        self.transport.execute(&p, &h, script, input).await
    }
    async fn put(&self, r: &Record, path: &str, bytes: &[u8]) -> Result<()> {
        self.command(
            r,
            &format!("umask 077; cat > {}", boat::shell_quote(path)),
            Some(bytes),
        )
        .await?;
        Ok(())
    }
}
impl<T: Transport> Backend for Gce<T> {
    async fn provision(&self, r: &mut Record) -> Result<String> {
        if r.spec.mode != Mode::Coder {
            return Err("GCE requires the Coder runtime.".into());
        }
        let p = self.transport.granted()?;
        // Host selection is read-only. A retained binding survives a lost response.
        if !r.binding.is_null() {
            let retained: Pool = serde_json::from_value(r.binding["pool"].clone())
                .map_err(|_| "Invalid retained GCE grant.")?;
            if retained.grant != p.grant
                || retained.epoch != p.epoch
                || retained.project != p.project
            {
                return Err("The GCE grant changed during provisioning.".into());
            }
            return r.binding["host"]["name"]
                .as_str()
                .map(str::to_owned)
                .ok_or("Invalid retained GCE host.".into());
        }
        let hosts = self.transport.hosts(&p)?;
        let mut selected = None;
        for h in hosts.iter().filter(|h| h.status == "RUNNING") {
            let script = "n=0; for p in ~/.oa-pool/runs/*/pid; do [ ! -f \"$p\" ] || ! kill -0 \"$(cat \"$p\")\" 2>/dev/null || n=$((n+1)); done; echo $n";
            if let Ok(n) = self.transport.execute(&p, h, script, None).await {
                if n.trim().parse::<u64>().unwrap_or(u64::MAX) < p.slots_per_host {
                    selected = Some(h.clone());
                    break;
                }
            }
        }
        let h = match selected {
            Some(h) => h,
            None if (hosts.len() as u64) < p.max_hosts => self.transport.start(&p)?,
            None => {
                return Err(
                    "All GCE pool slots are occupied and the grant's host limit is reached.".into(),
                );
            }
        };
        r.binding = json!({"pool":p,"host":h});
        Ok(h.name)
    }
    async fn prepare(&self, r: &Record) -> Result<()> {
        let (p, h) = self.admitted(r)?;
        self.transport.prepare_runtime(&p, &h).await?;
        let dir = directory(r);
        self.command(r, &runtime::claim_script(r, &dir), None)
            .await?;
        self.command(
            r,
            &format!(
                "umask 077; mkdir -p {}/workspace; touch ~/.oa-pool/busy",
                boat::shell_quote(&dir)
            ),
            None,
        )
        .await?;
        self.put(r, &(dir.clone() + "/task"), r.spec.task.as_bytes())
            .await?;
        self.put(
            r,
            &format!("/tmp/oa-coder-{}.env", r.id),
            self.credentials.shell().as_bytes(),
        )
        .await?;
        self.command(r, &runtime::prepare_script(r, &dir), None)
            .await?;
        if let Some(snapshot) = &r.workspace {
            let marker = self
                .command(
                    r,
                    &format!(
                        "cat {}/workspace-input 2>/dev/null || true",
                        boat::shell_quote(&dir)
                    ),
                    None,
                )
                .await?;
            if marker != snapshot.input_digest {
                if !marker.is_empty() {
                    return Err("The remote workspace identity differs from this job.".into());
                }
                self.put(r, &(dir.clone() + "/input.json"), &snapshot.input()?)
                    .await?;
                self.command(r, &crate::workspace::restore_script(r, &dir)?, None)
                    .await?;
            }
        }
        let (p, _) = self.admitted(r)?;
        let mut script = runtime::launch_script(r, &dir);
        let slots = format!(
            r#"# Hold one pool slot for the complete process lifetime.
slot=''
for i in $(seq 0 {last}); do
 exec 8>"$HOME/.oa-pool/slot-$i.lock"
 if flock -n 8; then slot=$i; break; fi
done
[ -n "$slot" ] || {{ printf '75' > "$d/exit"; exit 75; }}
touch "$HOME/.oa-pool/busy"
"#,
            last = p.slots_per_host.saturating_sub(1)
        );
        script = script.replace(
            "printf '%s' \"$$\" > \"$d/pid\"",
            &(slots + "printf '%s' \"$$\" > \"$d/pid\""),
        );
        self.put(r, &(dir + "/run.sh"), script.as_bytes()).await
    }
    async fn dispatch(&self, r: &Record) -> Result<Task> {
        self.command(r,&format!("d={}; setsid nohup sh \"$d/run.sh\" >\"$d/launcher.out\" 2>\"$d/launcher.err\" </dev/null &",boat::shell_quote(&directory(r))),None).await?;
        Ok(Task {
            id: r.id.clone(),
            conversation: None,
        })
    }
    async fn recover(&self, r: &Record) -> Result<Option<Task>> {
        let text = self
            .command(r, &runtime::poll_script(&directory(r), 0), None)
            .await?;
        let v: Value = serde_json::from_str(&text).map_err(|_| "Invalid GCE process evidence.")?;
        Ok(
            (v["started"] == true || !v["exit"].is_null()).then(|| Task {
                id: r.id.clone(),
                conversation: None,
            }),
        )
    }
    async fn poll(&self, r: &Record) -> Result<Observation> {
        let offset = r
            .cursor
            .as_deref()
            .unwrap_or("0")
            .parse()
            .map_err(|_| "Invalid GCE cursor.")?;
        match self
            .command(r, &runtime::poll_script(&directory(r), offset), None)
            .await
        {
            Ok(text) => {
                let mut observed = runtime::parse_poll(r, &text)?;
                for e in &mut observed.events {
                    self.credentials.redact(e);
                }
                if let Some(Ok(v)) = &mut observed.end {
                    self.credentials.redact(v);
                }
                Ok(observed)
            }
            Err(error) => {
                let (p, h) = binding(r)?;
                if !self.transport.hosts(&p)?.iter().any(|live| {
                    live.name == h.name && live.zone == h.zone && live.status == "RUNNING"
                }) {
                    Ok(Observation {
                        events: vec![json!({"event":"host_lost","host":h.name,"confirmed":true})],
                        cursor: r.cursor.clone(),
                        end: Some(Err(
                            "GCE host loss confirmed. This job was not replayed on another host."
                                .into(),
                        )),
                    })
                } else {
                    Err(error)
                }
            }
        }
    }
    async fn cancel(&self, r: &Record) -> Result<()> {
        let (p, h) = binding(r)?;
        if !self
            .transport
            .hosts(&p)?
            .iter()
            .any(|live| live.name == h.name && live.zone == h.zone && live.status == "RUNNING")
        {
            return Ok(());
        }
        self.transport
            .execute(&p, &h, &cancel_script(&directory(r)), None)
            .await?;
        Ok(())
    }
    async fn collect(&self, r: &Record) -> Result<Option<Value>> {
        let (p, h) = binding(r)?;
        if !self
            .transport
            .hosts(&p)?
            .iter()
            .any(|live| live.name == h.name && live.zone == h.zone && live.status == "RUNNING")
        {
            return Ok(None);
        }
        let text = self
            .command(r, &crate::workspace::collect_script(r, &directory(r)), None)
            .await?;
        let mut v: Value =
            serde_json::from_str(&text).map_err(|_| "Invalid remote artifact manifest.")?;
        self.credentials.sanitize_artifacts(&mut v)?;
        Ok(Some(v))
    }
    async fn restart(&self, r: &Record) -> Result<()> {
        self.cancel(r).await?;
        self.command(
            r,
            &format!(
                "d={}; rm -f \"$d/started\" \"$d/pid\" \"$d/exit\" \"$d/out\"; true",
                boat::shell_quote(&directory(r))
            ),
            None,
        )
        .await?;
        Ok(())
    }
    async fn cleanup(&self, r: &Record) -> Result<Option<Value>> {
        self.cancel(r).await?;
        let (p, h) = binding(r)?;
        let seconds = crate::now_ms().saturating_sub(r.created_ms) as f64 / 1000.;
        let hourly = pool::hourly_usd(&h.machine, h.spot);
        Ok(Some(
            json!({"basis":"estimated_shared_host_list_price","seconds":seconds,"dollars":seconds/3600.*hourly/p.slots_per_host.max(1) as f64,"host_hourly_usd":hourly,"slots":p.slots_per_host,"host":h.name,"host_retained_for_pool":true}),
        ))
    }
}
pub fn cancel_script(dir: &str) -> String {
    runtime::cancel_script(dir)
}

fn runtime_preparation(build: &str) -> String {
    format!(
        r#"set -eu
d="$HOME/.oa-pool/runs/runtime-prepare-$$"
mkdir -p "$d"
printf '%s' "$$" > "$d/pid"
touch "$HOME/.oa-pool/busy"
trap 'rm -f "$d/pid"; rmdir "$d" 2>/dev/null || true; touch "$HOME/.oa-pool/busy"' EXIT
{build}
oa_build
"#
    )
}

#[cfg(test)]
mod preparation_tests {
    #[test]
    fn preparation_counts_as_pool_activity_and_releases_its_marker_on_failure() {
        for code in [0, 7] {
            let home = tempfile::tempdir().unwrap();
            let script = super::runtime_preparation(&format!(
                "oa_build() {{ test -f \"$d/pid\" && kill -0 \"$(cat \"$d/pid\")\" || exit 9; return {code}; }}"
            ));
            let output = std::process::Command::new("bash")
                .args(["-c", &script])
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap())
                .env("HOME", home.path())
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(code));
            assert!(home.path().join(".oa-pool/busy").is_file());
            assert_eq!(
                std::fs::read_dir(home.path().join(".oa-pool/runs"))
                    .unwrap()
                    .count(),
                0
            );
        }
    }
}
