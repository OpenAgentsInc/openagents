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
/// A host disk: room for the 200 GB image the bake writes plus a second
/// slot's target.
const DISK_GB: u64 = 300;
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

/// Where the pool record, its key, the budget and the spend ledger live.
pub(crate) fn cloud_home() -> PathBuf {
    home()
}

fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_CLOUD_HOME") {
        return PathBuf::from(dir);
    }
    user_home::home_os()
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
    /// The gcloud configuration (`CLOUDSDK_CONFIG`) the grant was made
    /// with. Every later gcloud call of the pool uses it when the
    /// environment names none, so a process started without it (an agent's
    /// tool call) still runs as the same account, never a person's login
    /// that expires (#12058).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gcloud_config: Option<String>,
    /// The gcloud account that made the grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
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
            gcloud_config: std::env::var("CLOUDSDK_CONFIG")
                .ok()
                .filter(|c| !c.trim().is_empty())
                .or_else(|| previous.and_then(|p| p.gcloud_config.clone())),
            account: previous.and_then(|p| p.account.clone()),
        }
    }

    fn host_account(&self) -> String {
        format!("{HOST_ACCOUNT}@{}.iam.gserviceaccount.com", self.project)
    }
}

// ---------------------------------------------------------------------------
// gcloud.

/// The longest one gcloud call of the pool may take (a create or a delete
/// of several hosts included).
const GCLOUD_LIMIT: Duration = Duration::from_secs(600);

/// `CLOUDSDK_CONFIG` for the pool's gcloud calls: none when the
/// environment names one, else the configuration the grant was made with.
fn pinned_config() -> Option<String> {
    if std::env::var_os("CLOUDSDK_CONFIG").is_some() {
        return None;
    }
    Pool::load().and_then(|pool| pool.gcloud_config)
}

/// `gcloud`, under the pool's pinned configuration.
pub fn gcloud_command() -> Command {
    let mut command = Command::new("gcloud");
    if let Some(config) = pinned_config() {
        command.env("CLOUDSDK_CONFIG", config);
    }
    command
}

/// The gcloud account the pool's calls run as (`gcloud config get-value
/// account`), if gcloud answers.
pub fn active_account() -> Option<String> {
    let mut command = gcloud_command();
    command.args(["config", "get-value", "account"]);
    run_within(command, Duration::from_secs(30))
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|account| !account.is_empty() && !account.contains(' '))
}

/// A person's login rather than a service account: such logins expire and
/// need an interactive sign-in, so the pool should not depend on one.
pub fn is_person(account: &str) -> bool {
    !account.ends_with(".gserviceaccount.com")
}

/// Runs `command` with a deadline; stdout, or stderr's last lines.
fn run_within(mut command: Command, limit: Duration) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("gcloud did not start: {e}"))?;
    let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let stdout = read(child.stdout.take().map(|p| Box::new(p) as _));
    let stderr = read(child.stderr.take().map(|p| Box::new(p) as _));
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("gcloud took longer than {} s", limit.as_secs()));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    let (out, error) = (
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default(),
    );
    if status.success() {
        Ok(out)
    } else {
        let tail: Vec<&str> = error.lines().rev().take(4).collect();
        Err(tail.into_iter().rev().collect::<Vec<_>>().join(" "))
    }
}

/// Runs `gcloud ARGS --project P --quiet` within [`GCLOUD_LIMIT`]; stdout,
/// or stderr's last lines.
fn gcloud(project: &str, args: &[&str]) -> Result<String, String> {
    gcloud_in(project, args, GCLOUD_LIMIT)
}

/// [`gcloud`] with its own deadline, for callers outside the pool (the run
/// credentials' Secret Manager reads) that should run as the same account.
pub fn gcloud_in(project: &str, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut command = gcloud_command();
    command.args(args).args(["--project", project, "--quiet"]);
    run_within(command, limit)
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

pub(crate) fn hosts_from(listing: &Value) -> Vec<Host> {
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
    let hosts = hosts_from(&listing);
    if let Some(pool) = pool {
        crate::budget::observe(pool, &hosts);
    }
    Ok(hosts)
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
/// the 300 GB disk. Archil sandboxes (`archil-{vcpus}x{mib}`) are priced
/// at Archil's rates.
pub fn hourly_usd(machine: &str, spot: bool) -> f64 {
    // An Archil sandbox (`chat work --on archil`) at Archil's list price.
    if let Some(price) = crate::archil_backend::hourly_usd(machine) {
        return price;
    }
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
            private_dir(dir)?;
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

/// Makes `dir` (and its parents) and leaves it readable by its owner alone.
/// Off Unix the directory keeps the account's default access.
fn private_dir(dir: &std::path::Path) -> Result<(), String> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Whether `name` is a GCE instance name: lowercase letters, digits and
/// hyphens, starting with a letter, at most 63 characters.
fn gce_name(name: &str) -> bool {
    (1..=63).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Whether `project` is a GCP project id (domain-scoped ids carry `:` and
/// `.`).
fn gcp_project(project: &str) -> bool {
    (1..=100).contains(&project.len())
        && project
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | ':' | '.'))
}

/// Refuses identifiers that would be interpolated into a shell string
/// (the IAP `ProxyCommand`) unless they are plain GCE names.
fn check_identifiers(project: &str, host: &Host) -> Result<(), String> {
    if !gcp_project(project) {
        return Err(format!("{project:?} is not a GCP project id"));
    }
    if !gce_name(&host.name) {
        return Err(format!("{:?} is not a GCE instance name", host.name));
    }
    if !gce_name(&host.zone) {
        return Err(format!("{:?} is not a GCE zone", host.zone));
    }
    if let Some(address) = &host.address
        && address.parse::<std::net::IpAddr>().is_err()
    {
        return Err(format!("{address:?} is not an IP address"));
    }
    Ok(())
}

/// The host key types a pin may carry.
const HOST_KEY_TYPES: &[&str] = &[
    "ssh-ed25519",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "ssh-rsa",
];

/// `known_hosts` lines for `alias` from the `hostkeys/` guest attributes GCE
/// publishes for an instance (`gcloud compute instances
/// get-guest-attributes --query-path=hostkeys/ --format=json`). Entries of
/// another type or with anything but base64 in the key are dropped; no
/// usable key is an error.
pub fn known_hosts_from(alias: &str, attributes: &Value) -> Result<String, String> {
    let mut lines = String::new();
    for entry in attributes.as_array().map(Vec::as_slice).unwrap_or_default() {
        let kind = entry["key"].as_str().unwrap_or_default();
        let mut value = entry["value"].as_str().unwrap_or_default().trim();
        // Some agents publish "TYPE BASE64"; keep the key alone.
        if let Some((prefix, rest)) = value.split_once(' ')
            && prefix == kind
        {
            value = rest.split_whitespace().next().unwrap_or_default();
        }
        let base64 = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
        if HOST_KEY_TYPES.contains(&kind) && base64 {
            lines.push_str(&format!("{alias} {kind} {value}\n"));
        }
    }
    if lines.is_empty() {
        Err(format!("{alias} has published no ssh host key yet"))
    } else {
        Ok(lines)
    }
}

/// Where `host`'s pinned host keys live.
fn known_hosts_path(host: &Host) -> PathBuf {
    home()
        .join("known_hosts.d")
        .join(format!("{}.{}", host.zone, host.name))
}

/// Runs `gcloud` with a deadline, so a hung call cannot hold an ssh.
fn gcloud_within(project: &str, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut child = gcloud_command()
        .args(args)
        .args(["--project", project, "--quiet"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("gcloud did not start: {e}"))?;
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    std::io::Read::read_to_string(&mut stdout, &mut out)
                        .map_err(|e| e.to_string())?;
                }
                return if status.success() {
                    Ok(out)
                } else {
                    Err(format!("gcloud exited {status}"))
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("gcloud took too long".into());
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Pins `host`'s ssh host keys: reads what GCE publishes for the instance
/// in its `hostkeys/` guest attributes (through our own authenticated
/// gcloud, never through the ssh channel) and writes them to the host's own
/// `known_hosts` under the private cloud home. GCE is asked on every call,
/// so an instance recreated under the same name gets its new keys; when GCE
/// cannot be asked, the last pin stands. No pin at all leaves the file
/// empty, and the strict ssh that reads it refuses the host.
fn pin_host_keys(project: &str, host: &Host) -> Result<PathBuf, String> {
    let path = known_hosts_path(host);
    if let Some(dir) = path.parent() {
        private_dir(dir)?;
    }
    let fetched = gcloud_within(
        project,
        &[
            "compute",
            "instances",
            "get-guest-attributes",
            &host.name,
            "--zone",
            &host.zone,
            "--query-path=hostkeys/",
            "--format=json",
        ],
        Duration::from_secs(30),
    )
    .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
    .and_then(|value| known_hosts_from(&host.name, &value));
    let lines = match fetched {
        Ok(lines) => lines,
        Err(_) if path.exists() => return Ok(path),
        Err(_) => String::new(),
    };
    write_private(&path, &lines)?;
    Ok(path)
}

/// Writes `text` to `path` readable by its owner alone, atomically.
fn write_private(path: &std::path::Path, text: &str) -> Result<(), String> {
    let temp = path.with_extension(format!("writing-{}", word(6)));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(&temp)
        .map_err(|e| format!("{}: {e}", temp.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The ssh arguments, before the target, for a host whose pinned keys are
/// in `known_hosts`: the host key must match the pin (no first-use trust,
/// no other known_hosts file), checked under the instance name whichever
/// way the connection goes.
fn ssh_args(
    project: &str,
    host: &Host,
    known_hosts: &std::path::Path,
    internal: bool,
) -> (Vec<String>, String) {
    let mut args: Vec<String> = vec!["-i".into(), key_path().display().to_string()];
    for option in [
        "StrictHostKeyChecking=yes".to_owned(),
        format!("UserKnownHostsFile={}", known_hosts.display()),
        "GlobalKnownHostsFile=/dev/null".into(),
        format!("HostKeyAlias={}", host.name),
        "CheckHostIP=no".into(),
        "UpdateHostKeys=no".into(),
        "LogLevel=ERROR".into(),
        "BatchMode=yes".into(),
        "IdentitiesOnly=yes".into(),
        "ConnectTimeout=40".into(),
        "ServerAliveInterval=20".into(),
        "ServerAliveCountMax=9".into(),
    ] {
        args.push("-o".into());
        args.push(option);
    }
    let target = match (&host.address, internal) {
        (Some(address), true) => address.clone(),
        _ => {
            args.push("-o".into());
            args.push(format!(
                "ProxyCommand=gcloud compute start-iap-tunnel {} 22 --listen-on-stdin \
                 --project {project} --zone {} --verbosity=warning",
                host.name, host.zone
            ));
            host.name.clone()
        }
    };
    (args, target)
}

/// An `ssh` to `host` as `coder` running `remote`: through an IAP tunnel, or
/// to the internal address under `OA_POOL_SSH=internal`. The host key is
/// checked against the keys GCE publishes for the instance
/// ([#11281](https://github.com/OpenAgentsInc/openagents/issues/11281)).
/// When the host cannot be checked, the command refuses: it says why on
/// stderr and exits 255, as ssh does when it cannot connect.
pub fn ssh(project: &str, host: &Host, remote: &str) -> Command {
    try_ssh(project, host, remote).unwrap_or_else(|reason| refused(&reason))
}

/// A command that exits 255 after printing `reason`.
fn refused(reason: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "printf 'ssh refused: %s\\n' \"$1\" >&2; exit 255",
            "oa-pool-ssh",
        ])
        .arg(reason);
    command
}

/// [`ssh`], or why the host cannot be checked.
///
/// # Errors
///
/// When an identifier is not a plain GCE name or the pin cannot be written.
pub fn try_ssh(project: &str, host: &Host, remote: &str) -> Result<Command, String> {
    check_identifiers(project, host)?;
    let known_hosts = pin_host_keys(project, host)?;
    let internal = std::env::var("OA_POOL_SSH").is_ok_and(|mode| mode == "internal");
    let (args, target) = ssh_args(project, host, &known_hosts, internal);
    let mut command = Command::new("ssh");
    // The IAP tunnel's gcloud runs as the pool's account too.
    if let Some(config) = pinned_config() {
        command.env("CLOUDSDK_CONFIG", config);
    }
    command
        .args(args)
        .arg(format!("coder@{target}"))
        .arg(remote);
    Ok(command)
}

/// Runs `remote` on `host` and returns its stdout.
pub fn ssh_output(project: &str, host: &Host, remote: &str) -> Result<String, String> {
    let output = try_ssh(project, host, remote)?
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
# The briefed agent's context finder needs numpy (#12058); without it the
# briefing falls back to its lite finder.
for attempt in 1 2 3; do
  python3 -c 'import numpy' 2>/dev/null && break
  apt-get update -q >/dev/null 2>&1 && DEBIAN_FRONTEND=noninteractive apt-get install -y -q python3-numpy >/dev/null 2>&1 && break
  sleep 10
done
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
    # The briefed agent, the default engine of `chat work` (#12058); it has
    # no --version, so it only has to be there.
    if [ -x "$warm/debug/briefed-agent" ]; then
      strip --strip-all -o "$staged/briefed-agent" "$warm/debug/briefed-agent" 2>/dev/null || cp "$warm/debug/briefed-agent" "$staged/briefed-agent" || return 3
      chmod 555 "$staged/briefed-agent"
    fi
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
  if [ -x "$installed/briefed-agent" ]; then
    ln -sfn "$installed/briefed-agent" "$HOME/.oa-pool/bin/briefed-agent.next" && mv -f "$HOME/.oa-pool/bin/briefed-agent.next" "$HOME/.oa-pool/bin/briefed-agent" || return 3
  fi
}
# Every toolchain `$1` pins (the root and apps/openagents pin different
# ones), with rustfmt and clippy, installed once under the build lock. Two
# runs that each let rustup install a missing pin at once leave it without
# rustfmt or clippy, and every check then fails as a checker error.
oa_toolchains() {
  local rev=$1 file channel log="$HOME/.oa-pool/toolchain.log"
  mkdir -p "$HOME/.oa-pool" || return 3
  for file in rust-toolchain.toml apps/openagents/rust-toolchain.toml; do
    channel=$(git show "$rev:$file" 2>/dev/null | sed -n 's/^channel *= *"\([^"]*\)".*/\1/p' | head -n 1)
    [ -n "$channel" ] || continue
    rustup run "$channel" rustfmt --version >/dev/null 2>&1 && rustup run "$channel" cargo clippy --version >/dev/null 2>&1 && continue
    echo "pool: installing Rust $channel with rustfmt and clippy" >&2
    rustup toolchain uninstall "$channel" >/dev/null 2>&1
    rustup toolchain install "$channel" --profile minimal -c rustfmt -c clippy >"$log" 2>&1 \
      || { tail -n 20 "$log" >&2; return 3; }
  done
}
oa_build() {
  exec 8>"$HOME/.oa-pool/build.lock"; flock 8
  cd "$HOME/openagents" || { echo "pool: no clone at ~/openagents" >&2; return 2; }
  git fetch -q origin main || { echo "pool: git fetch failed" >&2; return 2; }
  local rev; rev=$(git rev-parse origin/main)
  oa_toolchains "$rev" || { echo "pool: installing the pinned Rust toolchains failed" >&2; flock -u 8; exec 8>&-; return 3; }
  if [ "$(cat "$HOME/.oa-pool/bin/rev" 2>/dev/null)" = "$rev" ] && [ -x "$HOME/.oa-pool/bin/microcoder" ] && [ -L "$HOME/.oa-pool/bin/coder-cloud-runtime" ] && [ -x "$HOME/.oa-pool/bin/briefed-agent" ]; then
    flock -u 8; exec 8>&-; return 0
  fi
  git checkout -q --detach "$rev" || return 2
  # The runs' checks build --offline; apps/openagents is its own workspace
  # with git dependencies (gpui) the root build never fetches.
  if [ -f apps/openagents/Cargo.toml ]; then
    (cd apps/openagents && cargo fetch --locked -q) >/tmp/oa-pool-fetch.log 2>&1 \
      || { tail -n 20 /tmp/oa-pool-fetch.log >&2; flock -u 8; exec 8>&-; return 3; }
  fi
  # The build scripts stamp the commit and tree only from the environment
  # (d765f537cb), and the install below requires a clean stamp of $rev.
  local dirty=0
  [ -z "$(git status --porcelain --untracked-files=no)" ] || dirty=1
  export CODER_BUILD_COMMIT="$rev" CODER_BUILD_DIRTY="$dirty"
  local warm; warm=$(jq -r .warm_target.slot "$HOME/.openagents/coder-host.json" 2>/dev/null)
  [ -n "$warm" ] && [ "$warm" != null ] || warm=$HOME/.openagents/targets/pool-runtime
  echo "pool: building openagents, microcoder, and Coder runtime at ${rev:0:10} on the warm target" >&2
  local lease; lease=$(command -v openagents || true)
  if [ -z "$lease" ] || ! "$lease" lease --help >/dev/null 2>&1; then
    CARGO_TARGET_DIR="$warm" cargo build --locked -q -p openagents-cli --bin openagents >/tmp/oa-pool-build.log 2>&1 \
      || { tail -n 40 /tmp/oa-pool-build.log >&2; flock -u 8; exec 8>&-; return 3; }
    lease=$warm/debug/openagents
  fi
  for p in "openagents-cli --bin openagents" "microcoder --bin microcoder" "coder-new --bin coder-cloud-runtime" "briefed-agent --bin briefed-agent"; do
    CARGO_TARGET_DIR="$warm" "$lease" lease build --keep-target-dir -- cargo build --locked -q -p $p >/tmp/oa-pool-build.log 2>&1 \
      || { tail -n 40 /tmp/oa-pool-build.log >&2; flock -u 8; exec 8>&-; return 3; }
  done
  oa_install "$warm" "$rev" || { echo "pool: installing the build failed" >&2; flock -u 8; exec 8>&-; return 3; }
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

/// The issue-flow command shared by CLI and SDK pool dispatch. The CLI and
/// controller always come from the same prepared pool build.
pub fn issue_command(issue: u64, land: Option<&str>, engine: &str) -> Result<String, String> {
    let land = match land {
        None => String::new(),
        Some(word @ ("main" | "queue" | "pr" | "none")) => format!(" --land {word}"),
        Some(_) => return Err("Choose queue, pr, or none for landing.".into()),
    };
    Ok(format!(
        r#"OA_CLI=$HOME/.oa-pool/bin/openagents OPENAGENTS_CODER_CONTROLLER=$HOME/.oa-pool/bin/microcoder OPENAGENTS_CODER_PLACEMENT=gce \
    "$HOME/.oa-pool/bin/openagents" chat work --local --json --issues {issue} --parallel 1 --engine {}{land}"#,
        boat::shell_quote(engine)
    ))
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
                         enable-guest-attributes=TRUE,oa-pool-idle-minutes={}",
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
    let mut child = try_ssh(project, host, &start_remote(&run))?
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
    start_hosts_for(pool, count, json_output, crate::budget::agent().as_deref())
}

/// Starts hosts under one explicit agent's admission and spend attribution.
pub fn start_hosts_for(
    pool: &Pool,
    count: u64,
    json_output: bool,
    agent: Option<&str>,
) -> Result<Vec<Started>, String> {
    // Every path that starts a host stays inside the compute budget.
    let hosts = list_hosts(&pool.project, Some(&pool.pool))?;
    crate::budget::admit_for(pool, &hosts, count, agent)?;
    let public_key = ensure_key()?;
    let handles: Vec<_> = (0..count)
        .map(|_| {
            let (pool, public_key, agent) =
                (pool.clone(), public_key.clone(), agent.map(str::to_owned));
            std::thread::spawn(move || start_one(&pool, &public_key, json_output, agent.as_deref()))
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
    grow_host_for(pool, crate::budget::agent().as_deref())
}

/// Grows a pool for one agent on a host serving several independent agents.
pub fn grow_host_for(pool: &Pool, agent: Option<&str>) -> Result<Host, String> {
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
    let first = start_hosts_for(pool, 1, false, agent)?
        .into_iter()
        .next()
        .ok_or("No GCE host was started.")?;
    if let Some(error) = first.error {
        return Err(error);
    }
    first.host.ok_or("The started GCE host is unknown.".into())
}

fn start_one(pool: &Pool, public_key: &str, json_output: bool, agent: Option<&str>) -> Started {
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
    crate::budget::started_for(pool, &host, agent);
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

    fn a_host() -> Host {
        Host {
            name: "oa-pool-a-wxyz".into(),
            zone: "us-central1-a".into(),
            status: "RUNNING".into(),
            machine: DEFAULT_MACHINE.into(),
            spot: false,
            created: String::new(),
            address: Some("10.0.0.7".into()),
        }
    }

    #[test]
    fn pool_ssh_checks_the_host_key_against_the_pin_alone() {
        let pin = std::path::Path::new("/pins/us-central1-a.oa-pool-a-wxyz");
        for internal in [false, true] {
            let (args, target) = ssh_args("openagentsgemini", &a_host(), pin, internal);
            let joined = args.join(" ");
            assert!(joined.contains("StrictHostKeyChecking=yes"), "{joined}");
            assert!(!joined.contains("StrictHostKeyChecking=no"), "{joined}");
            assert!(!joined.contains("accept-new"), "{joined}");
            assert!(!joined.contains("UserKnownHostsFile=/dev/null"), "{joined}");
            assert!(joined.contains(&format!("UserKnownHostsFile={}", pin.display())));
            assert!(joined.contains("GlobalKnownHostsFile=/dev/null"));
            assert!(joined.contains("HostKeyAlias=oa-pool-a-wxyz"));
            let expected = if internal {
                "10.0.0.7"
            } else {
                "oa-pool-a-wxyz"
            };
            assert_eq!(target, expected);
            assert_eq!(joined.contains("ProxyCommand="), !internal);
        }
    }

    #[test]
    fn host_keys_are_pinned_from_guest_attributes_only_when_well_formed() {
        let attributes = json!([
            {"namespace": "hostkeys", "key": "ssh-ed25519", "value": "AAAAC3NzaC1lZDI1NTE5AAAAIabc+/="},
            {"namespace": "hostkeys", "key": "ecdsa-sha2-nistp256", "value": "ecdsa-sha2-nistp256 AAAAE2Vj"},
            {"namespace": "hostkeys", "key": "ssh-ed25519", "value": "AAAA\n* ssh-rsa EVIL"},
            {"namespace": "hostkeys", "key": "evil", "value": "AAAA"}
        ]);
        let lines = known_hosts_from("oa-pool-a-wxyz", &attributes).unwrap();
        assert_eq!(
            lines,
            "oa-pool-a-wxyz ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIabc+/=\n\
             oa-pool-a-wxyz ecdsa-sha2-nistp256 AAAAE2Vj\n"
        );
        assert!(known_hosts_from("oa-pool-a-wxyz", &json!([])).is_err());
        assert!(
            known_hosts_from("oa-pool-a-wxyz", &json!([{"key": "evil", "value": "AAAA"}])).is_err()
        );
    }

    #[test]
    fn identifiers_that_reach_a_shell_string_must_be_plain_gce_names() {
        assert!(check_identifiers("openagentsgemini", &a_host()).is_ok());
        assert!(check_identifiers("example.com:my-project", &a_host()).is_ok());
        assert!(check_identifiers("p; rm -rf ~", &a_host()).is_err());
        for name in ["", "a;b", "A-host", "-x", "x-", "oa pool", "$(id)"] {
            let host = Host {
                name: name.into(),
                ..a_host()
            };
            assert!(check_identifiers("p", &host).is_err(), "{name}");
        }
        let zone = Host {
            zone: "us-central1-a --flag".into(),
            ..a_host()
        };
        assert!(check_identifiers("p", &zone).is_err());
        let address = Host {
            address: Some("10.0.0.7 -oProxyCommand=x".into()),
            ..a_host()
        };
        assert!(check_identifiers("p", &address).is_err());
    }

    #[test]
    fn a_host_that_cannot_be_checked_is_refused_not_reached() {
        let host = Host {
            name: "$(id)".into(),
            ..a_host()
        };
        let output = ssh("openagentsgemini", &host, "true").output().unwrap();
        assert_eq!(output.status.code(), Some(255));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("ssh refused: "), "{stderr}");
        assert!(stderr.contains("not a GCE instance name"), "{stderr}");
    }

    #[test]
    fn the_pin_directory_is_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let pins = dir.path().join("cloud/known_hosts.d");
        private_dir(&pins).unwrap();
        let mode = std::fs::metadata(&pins).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let file = pins.join("pin");
        write_private(&file, "x\n").unwrap();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

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
    fn the_build_installs_each_pinned_toolchain_once_with_rustfmt_and_clippy() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let repo = home.path().join("repo");
        std::fs::create_dir_all(repo.join("apps/openagents")).unwrap();
        let pin = |channel: &str| format!("[toolchain]\nchannel = \"{channel}\"\n");
        std::fs::write(repo.join("rust-toolchain.toml"), pin("1.97.1")).unwrap();
        std::fs::write(
            repo.join("apps/openagents/rust-toolchain.toml"),
            pin("1.98.1"),
        )
        .unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(&repo)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "pins"]);
        // A rustup that has 1.97.1 whole and 1.98.1 missing its components.
        // BUILD puts $HOME/.cargo/bin first on PATH, ahead of a real rustup.
        let bin = home.path().join(".cargo/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(home.path().join("whole-1.97.1"), "").unwrap();
        let rustup = bin.join("rustup");
        std::fs::write(
            &rustup,
            "#!/bin/sh\necho \"$*\" >> \"$HOME/rustup.log\"\ncase \"$1 $2\" in\n\
             \"run \"*) test -f \"$HOME/whole-$2\";;\n\
             \"toolchain install\") touch \"$HOME/whole-$3\";;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
        let toolchains = BUILD.split("oa_build()").next().unwrap();
        let run = || {
            let output = std::process::Command::new("bash")
                .args(["-c", &format!("{toolchains}\noa_toolchains HEAD")])
                .current_dir(&repo)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .env("HOME", home.path())
                .env(
                    "PATH",
                    format!(
                        "{}:{}",
                        bin.display(),
                        std::env::var("PATH")
                            .unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".into())
                    ),
                )
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run();
        run();
        let log = std::fs::read_to_string(home.path().join("rustup.log")).unwrap();
        let installs: Vec<&str> = log
            .lines()
            .filter(|l| l.starts_with("toolchain install"))
            .collect();
        assert_eq!(
            installs,
            ["toolchain install 1.98.1 --profile minimal -c rustfmt -c clippy"]
        );
        assert!(BUILD.contains("oa_toolchains \"$rev\" ||"));
        assert!(BUILD.contains("(cd apps/openagents && cargo fetch --locked -q)"));
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

    #[test]
    fn a_host_disk_holds_the_baked_image_and_a_second_slot() {
        let script = include_str!("../../../scripts/cloud/build-coder-host-image.sh");
        let bake: u64 = script
            .lines()
            .find_map(|l| l.strip_prefix("DISK_GB=\"${OA_DISK_GB:-"))
            .and_then(|rest| rest.strip_suffix("}\""))
            .and_then(|n| n.parse().ok())
            .expect("the bake script's default disk size");
        assert_eq!(bake, 200);
        assert_eq!(DISK_GB, 300);
        assert!(DISK_GB >= bake + 100, "pool {DISK_GB} GB, image {bake} GB");
    }
}

#[cfg(test)]
mod issue_command_tests {
    #[test]
    fn issue_commands_keep_the_selected_engine_and_prepared_cli() {
        let command = super::issue_command(12062, Some("queue"), "briefed:codex").unwrap();
        assert!(command.contains("OA_CLI=$HOME/.oa-pool/bin/openagents"));
        assert!(command.contains("OPENAGENTS_CODER_CONTROLLER=$HOME/.oa-pool/bin/microcoder"));
        assert!(
            command.contains("--issues 12062 --parallel 1 --engine 'briefed:codex' --land queue")
        );
        assert!(super::issue_command(12, Some("invalid"), "bare").is_err());
        assert!(
            !super::issue_command(12, None, "bare")
                .unwrap()
                .contains("--land")
        );
    }
}
