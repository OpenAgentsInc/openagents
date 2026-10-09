//! The granted GCE pool, its hosts, slot activity, and SSH/IAP transport.
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The placement name of the pool.
pub const COMPUTER: &str = "gce";
/// The image family the hosts boot from (`docs/deployment/coder-host-image.md`).
pub const IMAGE_FAMILY: &str = "oa-coder-host";
/// The pool host shape: 8 vCPU, 32 GB.
pub const DEFAULT_MACHINE: &str = "c3-standard-8";
/// Coder runs per host: one slot of 4 vCPU and 16 GB each.
pub const SLOTS_PER_HOST: u64 = 2;
const DISK_GB: u64 = 200;
pub const DEFAULT_IDLE_MINUTES: u64 = 10;
pub const DEFAULT_MAX_HOSTS: u64 = 8;
/// The backstop for a host whose agent never deletes it.
const MAX_RUN_DURATION: &str = "12h";
/// The label every pool host carries, and the one naming its pool.
const MANAGED_LABEL: &str = "openagents-managed=coder-pool";
const POOL_LABEL: &str = "openagents-pool";
const HOST_ACCOUNT: &str = "oa-coder-host";
/// The role the host's own account gets on that one instance, so its agent
/// can delete it when idle.
const SELF_DELETE_ROLE: &str = "roles/compute.instanceAdmin.v1";
/// pd-balanced at $0.10 a GB-month, per hour.
const DISK_USD_PER_GB_HOUR: f64 = 0.10 / 730.0;

pub fn project() -> String {
    std::env::var("OA_PROJECT")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "openagentsgemini".into())
}

fn zones() -> Vec<String> {
    std::env::var("OA_ZONES")
        .ok()
        .filter(|z| !z.trim().is_empty())
        .unwrap_or_else(|| "us-central1-a us-central1-b us-central1-c us-central1-f".into())
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_CLOUD_HOME") {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/cloud")
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A short random-enough hex word from the clock and a counter.
fn word(len: usize) -> String {
    use std::hash::{BuildHasher, Hasher};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    hasher.write_u64(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    format!("{:016x}", hasher.finish())[..len].to_owned()
}

// ---------------------------------------------------------------------------
// The grant: the pool record.

/// The pool this computer granted itself: one computer, `gce`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    pub schema: String,
    pub computer: String,
    /// The pool's name, on every host's `openagents-pool` label.
    pub pool: String,
    pub project: String,
    /// `gce:<pool>`.
    pub grant: String,
    /// The revocation epoch: each new grant of the pool is one more.
    pub epoch: u64,
    /// Unix seconds; `None` while the grant holds.
    #[serde(default)]
    pub revoked_at: Option<u64>,
    pub granted_at: u64,
    pub machine: String,
    pub spot: bool,
    pub max_hosts: u64,
    pub idle_minutes: u64,
    pub slots_per_host: u64,
}

impl Pool {
    fn path() -> PathBuf {
        home().join("pool.json")
    }

    /// The record on this computer, live or revoked.
    pub fn load() -> Option<Self> {
        let text = std::fs::read_to_string(Self::path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The live grant, or why there is none.
    pub fn granted() -> Result<Self, String> {
        match Self::load() {
            Some(pool) if pool.revoked_at.is_none() => Ok(pool),
            Some(pool) => Err(format!(
                "the GCE pool {} was revoked by `openagents cloud down`; `openagents cloud up` \
                 grants it again",
                pool.pool
            )),
            None => Err(
                "this computer has not granted a GCE pool; `openagents cloud up --hosts N` \
                 grants one and starts its hosts"
                    .into(),
            ),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A new grant: the same pool name and one more epoch when a record
    /// exists, else a new pool.
    pub fn grant(
        previous: Option<&Self>,
        machine: &str,
        spot: bool,
        max_hosts: u64,
        idle: u64,
    ) -> Self {
        let (pool, epoch) = match previous {
            Some(p) if p.revoked_at.is_some() => (p.pool.clone(), p.epoch + 1),
            Some(p) => (p.pool.clone(), p.epoch),
            None => (format!("p{}", word(6)), 1),
        };
        Self {
            schema: "openagents.cloud.pool.v1".into(),
            computer: COMPUTER.into(),
            grant: format!("{COMPUTER}:{pool}"),
            pool,
            project: project(),
            epoch,
            revoked_at: None,
            granted_at: previous
                .filter(|p| p.revoked_at.is_none())
                .map_or_else(now, |p| p.granted_at),
            machine: machine.into(),
            spot,
            max_hosts,
            idle_minutes: idle,
            slots_per_host: SLOTS_PER_HOST,
        }
    }

    fn host_account(&self) -> String {
        format!("{HOST_ACCOUNT}@{}.iam.gserviceaccount.com", self.project)
    }
}

// ---------------------------------------------------------------------------
// gcloud.

/// Runs `gcloud ARGS --project P --quiet`; stdout, or stderr's last lines.
fn gcloud(project: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("gcloud")
        .args(args)
        .args(["--project", project, "--quiet"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("gcloud did not start: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = error.lines().rev().take(4).collect();
        Err(tail.into_iter().rev().collect::<Vec<_>>().join(" "))
    }
}

/// One pool host as GCE lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub name: String,
    pub zone: String,
    pub status: String,
    pub machine: String,
    pub spot: bool,
    /// RFC 3339.
    pub created: String,
    pub address: Option<String>,
}

fn basename(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_owned()
}

fn hosts_from(listing: &Value) -> Vec<Host> {
    let mut hosts: Vec<Host> = listing
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|vm| Host {
            name: vm["name"].as_str().unwrap_or_default().to_owned(),
            zone: basename(vm["zone"].as_str().unwrap_or_default()),
            status: vm["status"].as_str().unwrap_or_default().to_owned(),
            machine: basename(vm["machineType"].as_str().unwrap_or_default()),
            spot: vm["scheduling"]["provisioningModel"].as_str() == Some("SPOT"),
            created: vm["creationTimestamp"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            address: vm["networkInterfaces"][0]["networkIP"]
                .as_str()
                .map(str::to_owned),
        })
        .filter(|h| !h.name.is_empty())
        .collect();
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    hosts
}

/// Every host of `pool` (any state), or of every pool when `None`.
pub fn list_hosts(project: &str, pool: Option<&str>) -> Result<Vec<Host>, String> {
    let filter = match pool {
        Some(pool) => format!("labels.{POOL_LABEL}={pool}"),
        None => "labels.openagents-managed=coder-pool".to_owned(),
    };
    let text = gcloud(
        project,
        &[
            "compute",
            "instances",
            "list",
            &format!("--filter={filter}"),
            "--format=json",
        ],
    )?;
    let listing: Value = serde_json::from_str(&text).map_err(|e| format!("gcloud listing: {e}"))?;
    Ok(hosts_from(&listing))
}

/// A GCE refusal that means "no capacity here; try elsewhere".
fn out_of_capacity(error: &str) -> bool {
    [
        "ZONE_RESOURCE_POOL_EXHAUSTED",
        "stockout",
        "does not have enough resources",
        "QUOTA",
        "Quota",
        "UNSUPPORTED_OPERATION",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

/// Hourly list-price estimate of one host: `OA_POOL_HOURLY_USD`, else
/// about $0.0175 (spot) or $0.0525 (on demand) per vCPU of a C3 shape, plus
/// the 200 GB disk.
pub fn hourly_usd(machine: &str, spot: bool) -> f64 {
    if let Some(price) = std::env::var("OA_POOL_HOURLY_USD")
        .ok()
        .and_then(|p| p.parse::<f64>().ok())
    {
        return price;
    }
    let vcpus = machine
        .rsplit('-')
        .next()
        .and_then(|n| n.parse::<f64>().ok())
        .unwrap_or(8.0);
    let per_vcpu = if spot { 0.0175 } else { 0.0525 };
    vcpus * per_vcpu + DISK_GB as f64 * DISK_USD_PER_GB_HOUR
}

// ---------------------------------------------------------------------------
// ssh.

fn key_path() -> PathBuf {
    home().join("pool_ed25519")
}

/// The pool's ssh key on this computer, made once. Returns the public key.
pub fn ensure_key() -> Result<String, String> {
    let key = key_path();
    if !key.exists() {
        if let Some(dir) = key.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let made = Command::new("ssh-keygen")
            .args([
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-C",
                "openagents-cloud-pool",
                "-f",
            ])
            .arg(&key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|e| format!("ssh-keygen did not start: {e}"))?;
        if !made.success() {
            return Err("ssh-keygen could not make the pool key".into());
        }
    }
    let public = std::fs::read_to_string(key.with_extension("pub"))
        .map_err(|e| format!("the pool key's public half: {e}"))?;
    Ok(public.trim().to_owned())
}

/// An `ssh` to `host` as `coder` running `remote`: through an IAP tunnel, or
/// to the internal address under `OA_POOL_SSH=internal`.
pub fn ssh(project: &str, host: &Host, remote: &str) -> Command {
    let mut command = Command::new("ssh");
    command.arg("-i").arg(key_path()).args([
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
        "-o",
        "LogLevel=ERROR",
        "-o",
        "BatchMode=yes",
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "ConnectTimeout=40",
        "-o",
        "ServerAliveInterval=20",
        "-o",
        "ServerAliveCountMax=9",
    ]);
    let internal = std::env::var("OA_POOL_SSH").is_ok_and(|mode| mode == "internal");
    let target = match (&host.address, internal) {
        (Some(address), true) => address.clone(),
        _ => {
            command.arg("-o").arg(format!(
                "ProxyCommand=gcloud compute start-iap-tunnel {} 22 --listen-on-stdin \
                 --project {project} --zone {} --verbosity=warning",
                host.name, host.zone
            ));
            host.name.clone()
        }
    };
    command.arg(format!("coder@{target}")).arg(remote);
    command
}

/// Runs `remote` on `host` and returns its stdout.
pub fn ssh_output(project: &str, host: &Host, remote: &str) -> Result<String, String> {
    let output = ssh(project, host, remote)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ssh did not start: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "ssh to {} failed ({}): {}",
            host.name,
            output.status,
            error.lines().last().unwrap_or_default()
        ))
    }
}

// ---------------------------------------------------------------------------
// The host side.

/// The host agent: each pool VM's startup script, run as root on every
/// boot. It installs a one-minute timer that keeps `~/.oa-pool/busy` fresh
/// while a run is alive and deletes the VM after `oa-pool-idle-minutes`
/// without one, through the role on this instance alone. When the delete is
/// refused, it powers the VM off (a stopped VM bills only its disk;
/// `openagents cloud down` deletes it).
pub const HOST_AGENT: &str = r#"#!/bin/bash
# openagents cloud pool host agent (#10225). Runs as root on every boot.
set -u
install -d -o coder -g coder /home/coder/.oa-pool /home/coder/.oa-pool/runs /home/coder/.oa-pool/bin
touch /home/coder/.oa-pool/busy && chown coder:coder /home/coder/.oa-pool/busy
# A pool host: stale Coder worktree pruning ships on (#10292). Images baked
# before that lack the marker the setup script now writes.
install -d -m 0755 /etc/openagents && touch /etc/openagents/pool-host
cat >/usr/local/bin/oa-pool-idle <<'IDLE'
#!/bin/bash
# Deletes this VM once no pool run has been alive for oa-pool-idle-minutes.
set -u
md() { curl -sf -H Metadata-Flavor:Google "http://metadata.google.internal/computeMetadata/v1/$1"; }
d=/home/coder/.oa-pool
for p in "$d"/runs/*/pid; do
  [ -f "$p" ] || continue
  if kill -0 "$(cat "$p" 2>/dev/null)" 2>/dev/null; then touch "$d/busy"; exit 0; fi
done
idle=$(md instance/attributes/oa-pool-idle-minutes || echo 10)
case "$idle" in ''|*[!0-9]*) idle=10 ;; esac
age=$(( $(date +%s) - $(stat -c %Y "$d/busy" 2>/dev/null || date +%s) ))
[ "$age" -ge $(( idle * 60 )) ] || exit 0
name=$(md instance/name); zone=$(md instance/zone); zone=${zone##*/}; project=$(md project/project-id)
token=$(md instance/service-accounts/default/token | jq -r .access_token)
echo "OA_POOL_IDLE_DELETE idle_seconds=$age" >/dev/ttyS0
code=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE -H "Authorization: Bearer $token" \
  "https://compute.googleapis.com/compute/v1/projects/$project/zones/$zone/instances/$name")
case "$code" in
  200) sleep 600 ;;
  *) echo "OA_POOL_IDLE_POWEROFF delete_http=$code" >/dev/ttyS0; systemctl poweroff ;;
esac
IDLE
chmod 755 /usr/local/bin/oa-pool-idle
cat >/etc/systemd/system/oa-pool-idle.service <<'UNIT'
[Unit]
Description=openagents cloud pool: delete this host when idle
[Service]
Type=oneshot
ExecStart=/usr/local/bin/oa-pool-idle
UNIT
cat >/etc/systemd/system/oa-pool-idle.timer <<'UNIT'
[Unit]
Description=openagents cloud pool: idle check every minute
[Timer]
OnBootSec=60
OnUnitActiveSec=60
[Install]
WantedBy=timers.target
UNIT
systemctl daemon-reload
systemctl enable --now oa-pool-idle.timer
# Coder's run boundary on Linux is bubblewrap; images baked before
# 2026-10-03 lack it (and the bake dropped the apt lists). Right after boot
# apt can be locked or offline, which left a host without it (#10275): retry.
for attempt in 1 2 3 4 5 6; do
  command -v bwrap >/dev/null && break
  apt-get update -q >/dev/null 2>&1 && DEBIAN_FRONTEND=noninteractive apt-get install -y -q bubblewrap >/dev/null 2>&1 && break
  sleep 10
done
command -v bwrap >/dev/null || echo "OA_POOL_AGENT bubblewrap could not be installed" >/dev/ttyS0
touch /home/coder/.oa-pool/agent-ready
echo "OA_POOL_AGENT_READY" >/dev/ttyS0
"#;

/// The shell function every pool script uses to build `origin/main`'s
/// `openagents` and `microcoder` into `~/.oa-pool/bin` on the image's warm
/// target, once per commit and one package per Cargo invocation (a combined
/// build unifies features and misses the warm target; see the image
/// runbook). Builds on one host take turns.
pub const BUILD: &str = r#"export PATH="$HOME/.cargo/bin:$HOME/.grok/bin:$HOME/.local/bin:/usr/local/bin:$PATH" CARGO_INCREMENTAL=0
oa_install() {
  local warm=$1 rev=$2 installed="$HOME/.oa-pool/versions/$2" staged="$HOME/.oa-pool/versions/$2.writing"
  if [ ! -d "$installed" ]; then
    mkdir -p "$staged"
    rm -f "$staged/openagents" "$staged/microcoder" "$staged/coder-cloud-runtime" "$staged/runtime.json" "$staged/rev"
    for b in openagents microcoder coder-cloud-runtime; do
      if ! command -v strip >/dev/null 2>&1 || ! strip --strip-all -o "$staged/$b" "$warm/debug/$b" 2>/dev/null; then
        cp "$warm/debug/$b" "$staged/$b" || return 3
      fi
      chmod 555 "$staged/$b"
      "$staged/$b" --version >/dev/null || return 3
    done
    "$staged/coder-cloud-runtime" --runtime-manifest > "$staged/runtime.json" || return 3
    jq -e --arg rev "$rev" '.revision == $rev and .tree == "clean"' "$staged/runtime.json" >/dev/null || return 3
    echo "$rev" > "$staged/rev"
    chmod 444 "$staged/runtime.json" "$staged/rev"
    mv "$staged" "$installed" || return 3
  fi
  mkdir -p "$HOME/.oa-pool/bin"
  for b in openagents microcoder coder-cloud-runtime runtime.json rev; do
    ln -sfn "$installed/$b" "$HOME/.oa-pool/bin/$b.next" && mv -f "$HOME/.oa-pool/bin/$b.next" "$HOME/.oa-pool/bin/$b" || return 3
  done
}
oa_build() {
  exec 8>"$HOME/.oa-pool/build.lock"; flock 8
  cd "$HOME/openagents" || { echo "pool: no clone at ~/openagents" >&2; return 2; }
  git fetch -q origin main || { echo "pool: git fetch failed" >&2; return 2; }
  local rev; rev=$(git rev-parse origin/main)
  if [ "$(cat "$HOME/.oa-pool/bin/rev" 2>/dev/null)" = "$rev" ] && [ -x "$HOME/.oa-pool/bin/microcoder" ] && [ -L "$HOME/.oa-pool/bin/coder-cloud-runtime" ]; then
    flock -u 8; exec 8>&-; return 0
  fi
  git checkout -q --detach "$rev" || return 2
  local warm; warm=$(jq -r .warm_target.slot "$HOME/.openagents/coder-host.json" 2>/dev/null)
  [ -n "$warm" ] && [ "$warm" != null ] || warm=$HOME/.openagents/targets/pool-runtime
  echo "pool: building openagents, microcoder, and Coder runtime at ${rev:0:10} on the warm target" >&2
  local lease; lease=$(command -v openagents || true)
  if [ -z "$lease" ] || ! "$lease" lease --help >/dev/null 2>&1; then
    CARGO_TARGET_DIR="$warm" cargo build --locked -q -p openagents-cli --bin openagents >/tmp/oa-pool-build.log 2>&1 || return 3
    lease=$warm/debug/openagents
  fi
  for p in "openagents-cli --bin openagents" "microcoder --bin microcoder" "coder-new --bin coder-cloud-runtime"; do
    CARGO_TARGET_DIR="$warm" "$lease" lease build --keep-target-dir -- cargo build --locked -q -p $p >/tmp/oa-pool-build.log 2>&1 \
      || { tail -n 40 /tmp/oa-pool-build.log >&2; flock -u 8; exec 8>&-; return 3; }
  done
  oa_install "$warm" "$rev" || return 3
  flock -u 8; exec 8>&-
}
"#;

/// The run directory of `run` on a host.
fn run_dir(run: &str) -> String {
    format!("$HOME/.oa-pool/runs/{run}")
}

/// The remote command that saves stdin as `run`'s script, starts it in a
/// session of its own (so it outlives this connection), and follows it.
pub fn start_remote(run: &str) -> String {
    let dir = run_dir(run);
    format!(
        "set -e; d={dir}; mkdir -p \"$d\"; umask 077; cat >\"$d/run.sh\"; cd \"$HOME\"; \
         setsid nohup bash \"$d/run.sh\" >\"$d/out\" 2>\"$d/err\" </dev/null & echo $! >\"$d/pid\"; \
         {}",
        follow_remote(run, 1)
    )
}

/// The remote command that prints `run`'s stdout from line `from` until the
/// run ends, then `OA_POOL_END <exit>` and its stderr's last lines, each
/// marked `OA_POOL_ERR `.
pub fn follow_remote(run: &str, from: u64) -> String {
    let dir = run_dir(run);
    format!(
        "d={dir}; tail -n +{from} -F --pid=\"$(cat \"$d/pid\")\" \"$d/out\" 2>/dev/null; \
         echo \"OA_POOL_END $(cat \"$d/exit\" 2>/dev/null || echo none)\"; \
         tail -n 40 \"$d/err\" 2>/dev/null | sed 's/^/OA_POOL_ERR /'"
    )
}

/// The remote command that stops `run` (its whole session).
pub fn kill_remote(run: &str) -> String {
    let dir = run_dir(run);
    format!("d={dir}; kill -TERM -- -\"$(cat \"$d/pid\")\" 2>/dev/null; true")
}

/// The script that prepares a new host: builds the CLI and engine once, so
/// the first run starts at once. Runs as a pool run so the idle check
/// counts it.
fn prepare_script() -> String {
    format!(
        "set -uo pipefail\nrm -f \"$0\"\n{BUILD}main() {{ oa_build; }}\nmain; rc=$?\n\
         echo $rc >\"$(dirname \"$0\")/exit\"; touch \"$HOME/.oa-pool/busy\"; exit $rc\n"
    )
}

// ---------------------------------------------------------------------------
// Starting hosts.

/// How long each step of starting one host took.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Started {
    pub host: Option<Host>,
    pub create_seconds: f64,
    pub ready_seconds: f64,
    pub ssh_seconds: f64,
    pub prepared_seconds: f64,
    pub error: Option<String>,
}

/// Creates one host of `pool` in the first zone with capacity, spot first.
fn create(pool: &Pool, public_key: &str) -> Result<Host, String> {
    create_with(pool, public_key, &zones(), gcloud)
}

fn create_with(
    pool: &Pool,
    public_key: &str,
    zones: &[String],
    mut call: impl FnMut(&str, &[&str]) -> Result<String, String>,
) -> Result<Host, String> {
    let name = format!("oa-pool-{}-{}", pool.pool, word(4));
    let dir = std::env::temp_dir().join(format!("oa-pool-{}", word(8)));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let agent = dir.join("agent.sh");
    let keys = dir.join("ssh-keys");
    std::fs::write(&agent, HOST_AGENT).map_err(|e| e.to_string())?;
    std::fs::write(&keys, format!("coder:{public_key}\n")).map_err(|e| e.to_string())?;
    let models: &[&str] = if pool.spot {
        &["SPOT", "STANDARD"]
    } else {
        &["STANDARD"]
    };
    let mut last = String::new();
    let result = 'found: {
        for model in models {
            for zone in zones {
                let mut args: Vec<String> = [
                    "compute",
                    "instances",
                    "create",
                    &name,
                    "--zone",
                    &zone,
                    "--machine-type",
                    &pool.machine,
                    "--image-family",
                    IMAGE_FAMILY,
                    "--image-project",
                    &pool.project,
                    "--boot-disk-type",
                    "pd-balanced",
                    &format!("--boot-disk-size={DISK_GB}GB"),
                    "--no-address",
                    "--service-account",
                    &pool.host_account(),
                    "--scopes",
                    "cloud-platform",
                    "--shielded-secure-boot",
                    "--shielded-vtpm",
                    "--shielded-integrity-monitoring",
                    &format!("--labels={MANAGED_LABEL},{POOL_LABEL}={}", pool.pool),
                    &format!(
                        "--metadata=block-project-ssh-keys=TRUE,enable-oslogin=FALSE,\
                         oa-pool-idle-minutes={}",
                        pool.idle_minutes
                    ),
                    &format!(
                        "--metadata-from-file=startup-script={},ssh-keys={}",
                        agent.display(),
                        keys.display()
                    ),
                    &format!("--max-run-duration={MAX_RUN_DURATION}"),
                    "--instance-termination-action=DELETE",
                    "--format=json",
                ]
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
                if *model == "SPOT" {
                    args.push("--provisioning-model=SPOT".into());
                }
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                match call(&pool.project, &refs) {
                    Ok(text) => {
                        let listing: Value =
                            serde_json::from_str(&text).unwrap_or(Value::Array(Vec::new()));
                        let hosts = hosts_from(&listing);
                        break 'found hosts.into_iter().next().ok_or_else(|| {
                            format!("gcloud created {name} but did not describe it")
                        });
                    }
                    Err(error) if out_of_capacity(&error) => {
                        eprintln!("cloud: no {model} {} in {zone}; trying on", pool.machine);
                        last = error;
                    }
                    Err(error) => break 'found Err(format!("could not create {name}: {error}")),
                }
            }
        }
        Err(format!(
            "no zone in [{}] had capacity for {}: {last}",
            zones.join(" "),
            pool.machine
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    let host = result?;
    // The host's own account may delete this instance and nothing else.
    if let Err(error) = call(
        &pool.project,
        &[
            "compute",
            "instances",
            "add-iam-policy-binding",
            &host.name,
            "--zone",
            &host.zone,
            &format!("--member=serviceAccount:{}", pool.host_account()),
            &format!("--role={SELF_DELETE_ROLE}"),
            "--format=none",
        ],
    ) {
        eprintln!(
            "cloud: {} cannot delete itself ({error}); when idle it powers off instead",
            host.name
        );
    }
    Ok(host)
}

/// Waits until `host` prints `OA_CODER_HOST_READY` on its serial console.
fn wait_ready(project: &str, host: &Host, deadline: Instant) -> Result<(), String> {
    let mut start = 0_u64;
    while Instant::now() < deadline {
        let text = gcloud(
            project,
            &[
                "compute",
                "instances",
                "get-serial-port-output",
                &host.name,
                "--zone",
                &host.zone,
                &format!("--start={start}"),
                "--format=json",
            ],
        );
        match text {
            Ok(text) => {
                let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if let Some(next) = value["next"]
                    .as_str()
                    .and_then(|n| n.parse().ok())
                    .or_else(|| value["next"].as_u64())
                {
                    start = next;
                }
                if value["contents"]
                    .as_str()
                    .is_some_and(|c| c.contains("OA_CODER_HOST_READY"))
                {
                    return Ok(());
                }
            }
            Err(error) if error.contains("was not found") => {
                return Err(format!("{} is gone (preempted or deleted)", host.name));
            }
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    Err(format!("{} was not ready in time", host.name))
}

/// Waits until `host` takes an ssh command and its agent has finished.
fn wait_ssh(project: &str, host: &Host, deadline: Instant) -> Result<(), String> {
    loop {
        match ssh_output(project, host, "test -e ~/.oa-pool/agent-ready") {
            Ok(_) => return Ok(()),
            Err(error) if Instant::now() > deadline => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_secs(4)),
        }
    }
}

/// Runs a script as a pool run on `host`, printing its stderr lines marked
/// with the host; true when it exited 0.
fn run_prepare(project: &str, host: &Host, json_output: bool) -> Result<(), String> {
    let run = format!("prepare-{}", word(6));
    let mut child = ssh(project, host, &start_remote(&run))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ssh did not start: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prepare_script().as_bytes())
            .map_err(|e| format!("sending the prepare script: {e}"))?;
    }
    let mut end = None;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(code) = line.strip_prefix("OA_POOL_END ") {
                end = Some(code.trim().to_owned());
            } else if let Some(error) = line.strip_prefix("OA_POOL_ERR ") {
                if !json_output {
                    eprintln!("{}: {error}", host.name);
                }
            }
        }
    }
    let _ = child.wait();
    match end.as_deref() {
        Some("0") => Ok(()),
        other => Err(format!(
            "{} could not build openagents and microcoder (exit {})",
            host.name,
            other.unwrap_or("unknown")
        )),
    }
}

/// Starts `count` hosts of `pool` at once and waits until each is ready and
/// prepared. Hosts that fail are deleted.
pub fn start_hosts(pool: &Pool, count: u64, json_output: bool) -> Result<Vec<Started>, String> {
    let public_key = ensure_key()?;
    let handles: Vec<_> = (0..count)
        .map(|_| {
            let (pool, public_key) = (pool.clone(), public_key.clone());
            std::thread::spawn(move || start_one(&pool, &public_key, json_output))
        })
        .collect();
    Ok(handles
        .into_iter()
        .map(|h| {
            h.join().unwrap_or_else(|_| Started {
                error: Some("the start thread panicked".into()),
                ..Default::default()
            })
        })
        .collect())
}

/// Grow once under a local lock, with the current grant's host limit.
pub fn grow_host(pool: &Pool) -> Result<Host, String> {
    std::fs::create_dir_all(home()).map_err(|_| "Cannot create the GCE pool directory.")?;
    let path = home().join("growth.lock");
    crate::regular_or_missing(&path)?;
    let lock = crate::private_options()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| "Cannot open the GCE pool growth lock.")?;
    lock.try_lock()
        .map_err(|_| "Another process is growing the GCE pool. Follow this job to retry.")?;
    let current = Pool::granted()?;
    if current.grant != pool.grant || current.epoch != pool.epoch || current.project != pool.project
    {
        return Err("The GCE grant changed before pool growth.".into());
    }
    if list_hosts(&pool.project, Some(&pool.pool))?.len() as u64 >= pool.max_hosts {
        return Err("The GCE pool reached the grant's host limit.".into());
    }
    let first = start_hosts(pool, 1, false)?
        .into_iter()
        .next()
        .ok_or("No GCE host was started.")?;
    if let Some(error) = first.error {
        return Err(error);
    }
    first.host.ok_or("The started GCE host is unknown.".into())
}

fn start_one(pool: &Pool, public_key: &str, json_output: bool) -> Started {
    let began = Instant::now();
    let mut started = Started::default();
    let say = |what: &str, host: &str| {
        let value = json!({"event": "cloud_host", "host": host, "state": what,
            "seconds": began.elapsed().as_secs_f64()});
        if json_output {
            println!("{value}");
        } else {
            eprintln!("cloud: {host} {what} ({} s)", began.elapsed().as_secs());
        }
    };
    let host = match create(pool, public_key) {
        Ok(host) => host,
        Err(error) => {
            started.error = Some(error);
            return started;
        }
    };
    started.create_seconds = began.elapsed().as_secs_f64();
    say(
        &format!(
            "created in {} ({})",
            host.zone,
            if host.spot { "spot" } else { "on demand" }
        ),
        &host.name,
    );
    let steps = (|| {
        wait_ready(&pool.project, &host, began + Duration::from_secs(900))?;
        started.ready_seconds = began.elapsed().as_secs_f64();
        say("ready", &host.name);
        wait_ssh(
            &pool.project,
            &host,
            Instant::now() + Duration::from_secs(180),
        )?;
        started.ssh_seconds = began.elapsed().as_secs_f64();
        say("reachable over ssh", &host.name);
        run_prepare(&pool.project, &host, json_output)?;
        started.prepared_seconds = began.elapsed().as_secs_f64();
        say("built origin/main's openagents and microcoder", &host.name);
        Ok::<(), String>(())
    })();
    if let Err(error) = steps {
        started.error = Some(error);
        let _ = delete_hosts(&pool.project, std::slice::from_ref(&host));
        say("deleted after a failed start", &host.name);
    }
    started.host = Some(host);
    started
}

/// Deletes `hosts`, each in its own zone.
pub fn delete_hosts(project: &str, hosts: &[Host]) -> Result<(), String> {
    let mut by_zone: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for host in hosts {
        by_zone.entry(&host.zone).or_default().push(&host.name);
    }
    let mut errors = Vec::new();
    for (zone, names) in by_zone {
        let mut args = vec!["compute", "instances", "delete"];
        args.extend(names);
        args.extend(["--zone", zone, "--delete-disks=all"]);
        if let Err(error) = gcloud(project, &args) {
            if !error.contains("was not found") {
                errors.push(error);
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

// ---------------------------------------------------------------------------
/// What one host says about its runs.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Activity {
    pub host: String,
    pub runs: u64,
    pub idle_seconds: Option<u64>,
    pub error: Option<String>,
}

const ACTIVITY: &str = "n=0; for p in ~/.oa-pool/runs/*/pid; do [ -f \"$p\" ] && \
    kill -0 \"$(cat \"$p\")\" 2>/dev/null && n=$((n+1)); done; \
    echo \"$n $(( $(date +%s) - $(stat -c %Y ~/.oa-pool/busy 2>/dev/null || date +%s) ))\"";

pub fn activity(project: &str, hosts: &[Host]) -> Vec<Activity> {
    let handles: Vec<_> = hosts
        .iter()
        .filter(|h| h.status == "RUNNING")
        .cloned()
        .map(|host| {
            let project = project.to_owned();
            std::thread::spawn(move || match ssh_output(&project, &host, ACTIVITY) {
                Ok(text) => {
                    let mut words = text.split_whitespace();
                    Activity {
                        host: host.name.clone(),
                        runs: words.next().and_then(|w| w.parse().ok()).unwrap_or(0),
                        idle_seconds: words.next().and_then(|w| w.parse().ok()),
                        error: None,
                    }
                }
                Err(error) => Activity {
                    host: host.name.clone(),
                    error: Some(error),
                    ..Default::default()
                },
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_grant_after_a_revoke_is_the_next_epoch_of_the_same_pool() {
        let first = Pool::grant(None, DEFAULT_MACHINE, true, 8, 10);
        assert_eq!(first.epoch, 1);
        assert!(first.pool.starts_with('p') && first.pool.len() == 7);
        assert_eq!(first.grant, format!("gce:{}", first.pool));
        let same = Pool::grant(Some(&first), DEFAULT_MACHINE, true, 8, 10);
        assert_eq!((same.pool.as_str(), same.epoch), (first.pool.as_str(), 1));
        let mut revoked = first.clone();
        revoked.revoked_at = Some(1);
        let again = Pool::grant(Some(&revoked), DEFAULT_MACHINE, false, 4, 5);
        assert_eq!((again.pool.as_str(), again.epoch), (first.pool.as_str(), 2));
        assert!(again.revoked_at.is_none() && !again.spot);
    }

    #[test]
    fn hosts_are_read_from_the_gce_listing() {
        let listing = json!([{
            "name": "oa-pool-pabc123-0f0f",
            "zone": "https://www.googleapis.com/compute/v1/projects/p/zones/us-central1-b",
            "status": "RUNNING",
            "machineType": "https://www.googleapis.com/compute/v1/projects/p/zones/us-central1-b/machineTypes/c3-standard-8",
            "scheduling": {"provisioningModel": "SPOT"},
            "creationTimestamp": "2026-10-02T10:00:00.000-07:00",
            "networkInterfaces": [{"networkIP": "10.128.0.9"}]
        }]);
        let hosts = hosts_from(&listing);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].zone, "us-central1-b");
        assert_eq!(hosts[0].machine, "c3-standard-8");
        assert!(hosts[0].spot);
        assert_eq!(hosts[0].address.as_deref(), Some("10.128.0.9"));
    }

    #[test]
    fn exhausted_spot_zones_fall_back_to_on_demand() {
        let pool = Pool::grant(None, "c3-standard-8", true, 1, 10);
        let mut calls = Vec::new();
        let zones = vec!["us-central1-a".into(), "us-central1-b".into()];
        let host = create_with(&pool, "ssh-ed25519 fake", &zones, |_, args| {
            if args.get(2) == Some(&"add-iam-policy-binding") {
                return Ok(String::new());
            }
            let zone = args[args.iter().position(|a| *a == "--zone").unwrap() + 1];
            let spot = args.contains(&"--provisioning-model=SPOT");
            calls.push((zone.to_owned(), spot));
            if spot {
                return Err("ZONE_RESOURCE_POOL_EXHAUSTED".into());
            }
            Ok(json!([{"name": args[3], "zone": zone, "status": "RUNNING",
                "machineType": "c3-standard-8", "scheduling": {"provisioningModel": "STANDARD"}}])
            .to_string())
        })
        .unwrap();
        assert!(!host.spot);
        assert_eq!(
            calls,
            [
                ("us-central1-a".into(), true),
                ("us-central1-b".into(), true),
                ("us-central1-a".into(), false)
            ]
        );
    }

    #[test]
    fn capacity_refusals_try_the_next_zone_and_others_stop() {
        assert!(out_of_capacity(
            "ERROR: (gcloud.compute.instances.create) ZONE_RESOURCE_POOL_EXHAUSTED"
        ));
        assert!(out_of_capacity("Quota 'C3_CPUS' exceeded"));
        assert!(!out_of_capacity("Permission denied on resource"));
    }

    #[test]
    fn the_host_scripts_follow_runs_and_never_carry_a_credential() {
        let start = start_remote("r1");
        assert!(start.contains("setsid nohup bash"));
        assert!(start.contains("umask 077"));
        assert!(start.contains("tail -n +1 -F --pid="));
        assert!(follow_remote("r1", 42).contains("tail -n +42 "));
        assert!(kill_remote("r1").contains("kill -TERM -- -"));
        for script in [HOST_AGENT, BUILD, start.as_str()] {
            for word in ["GH_TOKEN", "XAI_API_KEY", "ghp_", "gho_"] {
                assert!(!script.contains(word), "{word}");
            }
        }
        assert!(HOST_AGENT.contains("oa-pool-idle-minutes"));
        assert!(HOST_AGENT.contains("bubblewrap") && HOST_AGENT.contains("agent-ready"));
        assert!(HOST_AGENT.contains("compute.googleapis.com"));
        assert!(HOST_AGENT.contains("/etc/openagents/pool-host"));
        assert!(prepare_script().contains("oa_build"));
    }

    #[test]
    fn runtime_updates_keep_the_previous_bundle_and_its_cli_companion() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let warm = home.path().join("warm");
        std::fs::create_dir_all(warm.join("debug")).unwrap();
        let publisher = BUILD.split("oa_build()").next().unwrap();
        let publish = |rev: &str| {
            for bin in ["openagents", "microcoder", "coder-cloud-runtime"] {
                let path = warm.join("debug").join(bin);
                std::fs::write(&path, format!("#!/bin/sh\ncase \"$1\" in --runtime-manifest) echo '{{\"revision\":\"{rev}\",\"tree\":\"clean\"}}';; *) echo {rev};; esac\n")).unwrap();
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            let output = std::process::Command::new("bash")
                .args([
                    "-c",
                    &format!(
                        "{publisher}\noa_install {} {rev}",
                        boat::shell_quote(&warm.to_string_lossy())
                    ),
                ])
                .env_clear()
                .env("HOME", home.path())
                .env("PATH", std::env::var_os("PATH").unwrap())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        let current = home.path().join(".oa-pool/bin/coder-cloud-runtime");
        publish("v1");
        let pinned = std::fs::canonicalize(&current).unwrap();
        publish("v2");
        let version = |p: &std::path::Path| {
            std::process::Command::new(p)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        };
        assert_eq!(version(&current), b"v2\n");
        assert_eq!(version(&pinned), b"v1\n");
        assert_eq!(
            version(&pinned.parent().unwrap().join("openagents")),
            b"v1\n"
        );
    }

    #[test]
    fn the_hourly_estimate_scales_with_the_shape() {
        let spot = hourly_usd("c3-standard-8", true);
        let demand = hourly_usd("c3-standard-8", false);
        assert!(spot > 0.1 && spot < 0.2, "{spot}");
        assert!(demand > 0.4 && demand < 0.5, "{demand}");
        assert!(hourly_usd("c3-standard-22", true) > spot);
    }
}
