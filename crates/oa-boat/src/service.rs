//! What each Boat operation does on GCE.
//!
//! - A sandbox is one GCE instance, `oa-boat-<suffix>` for sandbox
//!   `bx_<suffix>`, from a template image or the newest `oa-coder-host`
//!   image, with no service account and no external address. Labels carry
//!   what must survive a restart of this service: the size, spot or on
//!   demand, the TTL deadline, the idle limit, the idempotency key's
//!   digest, the stop count, and the run seconds already accounted.
//! - "Ready" is per boot: once the VM answers SSH, the service writes the
//!   caller's `env` to `/run/oa-boat/env` (tmpfs, so a stopped disk or an
//!   image made from it never holds it), runs `setupScript` once, and
//!   touches `/run/oa-boat/ready`.
//! - A stop keeps the boot disk; a resume starts the same VM. The "latest
//!   snapshot" is the stopped disk itself, named by the stop time.
//! - A template ("named snapshot") is a GCE image of the boot disk.
//! - A fork is a disk snapshot made into a new VM.
//! - Commands run over SSH as `user`, with the env sourced: synchronous,
//!   streamed as NDJSON frames, or detached under `~/.oa-boat/proc/<pid>`.

use crate::gce::{Compute, GceError, Image, Instance};
use crate::remote::{Chunk, Remote};
use crate::sizes::{self, Provisioning, Size};
use crate::time;
use boat::Nullable;
use boat::models::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub const MANAGED: &str = "openagents-managed";
pub const MANAGED_SANDBOX: &str = "oa-boat";
pub const MANAGED_TEMPLATE: &str = "oa-boat-template";
const PREFIX: &str = "oa-boat-";
const EXIT_MARK: &[u8] = b"\x1fOA-BOAT-EXIT ";
/// The largest output a synchronous command returns per stream.
pub const OUTPUT_CAP: usize = 8 * 1024 * 1024;
/// The largest file read or written in one call.
pub const FILE_CAP: usize = 48 * 1024 * 1024;

/// A refusal in Boat's error envelope.
#[derive(Clone, Debug)]
pub struct ApiErr {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiErr {
    pub fn new(status: u16, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
    fn not_found(what: &str) -> Self {
        Self::new(404, "not_found", format!("{what} was not found."))
    }
    fn gce(e: GceError) -> Self {
        eprintln!("oa-boat: {e}");
        if e.capacity() {
            return Self::new(
                503,
                "capacity_unavailable",
                "Google Cloud has no capacity for this machine right now.",
            );
        }
        Self::new(502, "compute_error", "Compute Engine refused the request.")
    }
}

pub type Res<T> = Result<T, ApiErr>;

/// The service's settings.
#[derive(Clone, Debug)]
pub struct Config {
    pub project: String,
    pub region: String,
    /// Zones tried in order when one has no capacity.
    pub zones: Vec<String>,
    pub subnetwork: String,
    /// The network tag the firewall admits SSH to.
    pub tag: String,
    /// The image family a sandbox without `from` boots.
    pub base_family: String,
    /// `ssh-ed25519 AAAA...`: put on every VM for `user`.
    pub ssh_public_key: String,
    pub user: String,
    pub run_dir: String,
    pub default_ttl: i64,
    pub default_idle: i64,
    pub default_provisioning: Provisioning,
    pub max_active: i64,
    /// GCE stops a VM that has run this long, whatever this service does.
    pub max_run_seconds: i64,
}

impl Config {
    pub fn for_tests() -> Self {
        Self {
            project: "test".into(),
            region: "us-central1".into(),
            zones: vec!["us-central1-a".into(), "us-central1-b".into()],
            subnetwork: "default".into(),
            tag: "oa-boat-sandbox".into(),
            base_family: "oa-coder-host".into(),
            ssh_public_key: "ssh-ed25519 AAAA test".into(),
            user: "user".into(),
            run_dir: "$HOME/.oa-boat/run".into(),
            default_ttl: 3600,
            default_idle: 1800,
            default_provisioning: Provisioning::Standard,
            max_active: 50,
            max_run_seconds: 24 * 3600,
        }
    }
}

/// A sandbox whose VM is not inserted yet (or could not be).
#[derive(Clone, Debug)]
struct Pending {
    size: Size,
    provisioning: Provisioning,
    created: i64,
    error: Option<String>,
}

#[derive(Default)]
struct State {
    zones: HashMap<String, String>,
    pending: HashMap<String, Pending>,
    env: HashMap<String, BTreeMap<String, String>>,
    setup: HashMap<String, String>,
    setup_status: HashMap<String, (String, Option<String>)>,
    /// The boot (`lastStartTimestamp`) a sandbox was made ready for.
    ready: HashMap<String, String>,
    preparing: HashSet<String>,
    activity: HashMap<String, Instant>,
    idempotency: HashMap<String, String>,
    started: HashMap<String, Instant>,
}

pub struct Service<C, R> {
    pub cfg: Config,
    pub compute: C,
    pub remote: R,
    state: Mutex<State>,
}

/// Where a sandbox is.
enum Found {
    Vm(Instance),
    Pending(Pending),
}

/// A command's answer.
pub enum Reply {
    Finished(CommandResponse),
    Started(CommandStartedResponse),
    Stream(mpsc::Receiver<CommandStreamFrame>),
}

fn suffix_of(id: &str) -> Res<String> {
    let s = id
        .strip_prefix("bx_")
        .ok_or_else(|| ApiErr::not_found("The sandbox"))?;
    if s.is_empty()
        || s.len() > 40
        || !s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return Err(ApiErr::not_found("The sandbox"));
    }
    Ok(s.to_owned())
}

fn new_suffix() -> String {
    use rand::Rng;
    const A: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut r = rand::rng();
    (0..10)
        .map(|_| A[r.random_range(0..A.len())] as char)
        .collect()
}

fn digest(text: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(text.as_bytes()))[..40].to_owned()
}

fn label_i64(i: &Instance, k: &str) -> i64 {
    i.labels.get(k).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn env_file(env: &BTreeMap<String, String>, sandbox: &str) -> Res<String> {
    let mut out = format!("OA_BOAT_SANDBOX_ID={}\n", boat::shell_quote(sandbox));
    for (k, v) in env {
        let ok = !k.is_empty()
            && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !k.as_bytes()[0].is_ascii_digit();
        if !ok {
            return Err(ApiErr::new(
                400,
                "invalid_env",
                "Environment variable names must be letters, digits and underscores.",
            ));
        }
        out.push_str(&format!("{k}={}\n", boat::shell_quote(v)));
    }
    Ok(out)
}

fn extra_str<'a>(extra: &'a BTreeMap<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| extra.get(*k).and_then(Value::as_str))
}

fn extra_i64(extra: &BTreeMap<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|k| extra.get(*k).and_then(Value::as_i64))
}

/// Split the exit trailer off stderr: `(stderr, code)`.
fn take_exit(stderr: &[u8]) -> (Vec<u8>, Option<i64>) {
    let Some(at) = stderr
        .windows(EXIT_MARK.len())
        .rposition(|w| w == EXIT_MARK)
    else {
        return (stderr.to_vec(), None);
    };
    let code = std::str::from_utf8(&stderr[at + EXIT_MARK.len()..])
        .ok()
        .and_then(|s| s.trim().parse().ok());
    (stderr[..at].to_vec(), code)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl<C: Compute, R: Remote> Service<C, R> {
    pub fn new(cfg: Config, compute: C, remote: R) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            compute,
            remote,
            state: Mutex::new(State::default()),
        })
    }

    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("state")
    }

    pub fn touch(&self, suffix: &str) {
        self.st().activity.insert(suffix.into(), Instant::now());
    }

    // ------------------------------------------------------------------
    // Finding and describing sandboxes.

    async fn find(&self, suffix: &str) -> Res<Found> {
        let name = format!("{PREFIX}{suffix}");
        let known = self.st().zones.get(suffix).cloned();
        if let Some(zone) = known {
            match self.compute.get_instance(&zone, &name).await {
                Ok(Some(i)) => return Ok(Found::Vm(i)),
                Ok(None) => {}
                Err(e) => return Err(ApiErr::gce(e)),
            }
        }
        if let Some(p) = self.st().pending.get(suffix).cloned() {
            return Ok(Found::Pending(p));
        }
        let all = self
            .compute
            .list_instances("oa-boat-id", suffix)
            .await
            .map_err(ApiErr::gce)?;
        match all.into_iter().find(|i| i.name == name) {
            Some(i) => {
                self.st().zones.insert(suffix.into(), i.zone.clone());
                Ok(Found::Vm(i))
            }
            None => Err(ApiErr::not_found("The sandbox")),
        }
    }

    async fn vm(&self, id: &str) -> Res<(String, Instance)> {
        let suffix = suffix_of(id)?;
        self.touch(&suffix);
        match self.find(&suffix).await? {
            Found::Vm(i) => Ok((suffix, i)),
            Found::Pending(p) if p.error.is_some() => Err(ApiErr::new(
                409,
                "sandbox_error",
                p.error.unwrap_or_default(),
            )),
            Found::Pending(_) => Err(ApiErr::new(
                409,
                "boat_starting",
                "The sandbox is still starting.",
            )),
        }
    }

    fn ready_for(&self, suffix: &str, i: &Instance) -> bool {
        i.status == "RUNNING"
            && self
                .st()
                .ready
                .get(suffix)
                .is_some_and(|b| Some(b) == i.last_start.as_ref())
    }

    fn state_of(&self, suffix: &str, i: &Instance) -> &'static str {
        match i.status.as_str() {
            "RUNNING" if self.ready_for(suffix, i) => "ready",
            "RUNNING" | "PROVISIONING" | "STAGING" | "REPAIRING" => "provisioned",
            "STOPPING" | "SUSPENDING" => "stopping",
            _ => "stopped",
        }
    }

    fn size_of(i: &Instance) -> Size {
        i.labels
            .get("oa-boat-type")
            .and_then(|t| sizes::size(t))
            .or_else(|| sizes::by_machine(&i.machine))
            .unwrap_or(sizes::SIZES[1])
    }

    fn provisioning_of(i: &Instance) -> Provisioning {
        if i.spot {
            Provisioning::Spot
        } else {
            Provisioning::Standard
        }
    }

    fn stop_op(suffix: &str, i: &Instance) -> Nullable<StopOperation> {
        let n = label_i64(i, "oa-boat-stopn");
        let stopped = matches!(i.status.as_str(), "TERMINATED" | "SUSPENDED" | "STOPPED");
        if n == 0 && !stopped {
            return Nullable::Null;
        }
        let requested = label_i64(i, "oa-boat-stopat");
        Nullable::Value(StopOperation {
            id: if n == 0 {
                format!("stop_{suffix}_auto")
            } else {
                format!("stop_{suffix}_{n}")
            },
            status: if stopped {
                "completed".into()
            } else if matches!(i.status.as_str(), "STOPPING" | "SUSPENDING") {
                "running".into()
            } else {
                // A later start superseded this stop.
                "completed".into()
            },
            requested_at: (requested > 0).then(|| time::format(requested)),
            last_attempt_at: (requested > 0).then(|| time::format(requested)),
            error: None,
            ended_at: if stopped {
                i.last_stop
                    .as_deref()
                    .and_then(time::parse)
                    .map(time::format)
            } else {
                None
            },
            ..Default::default()
        })
    }

    fn view(&self, suffix: &str, i: &Instance) -> Sandbox {
        let size = Self::size_of(i);
        let prov = Self::provisioning_of(i);
        let state = self.state_of(suffix, i);
        let disk = i.disk_gb.unwrap_or(size.disk_gb);
        let setup = self.st().setup_status.get(suffix).cloned();
        let mut extra = BTreeMap::new();
        extra.insert("provisioning".into(), json!(prov.label()));
        extra.insert("machineType".into(), json!(i.machine));
        extra.insert("zone".into(), json!(i.zone));
        extra.insert("diskGb".into(), json!(disk));
        extra.insert("dollarsPerHour".into(), json!(size.hourly(prov)));
        extra.insert(
            "diskDollarsPerHour".into(),
            json!(disk as f64 * sizes::DISK_HOURLY_PER_GB),
        );
        extra.insert("backend".into(), json!("openagents-gce"));
        let deadline = label_i64(i, "oa-boat-deadline");
        Sandbox {
            id: format!("bx_{suffix}"),
            name: format!("bx_{suffix}"),
            state: state.into(),
            hydrated: Some(state == "ready"),
            type_: Some(size.name.into()),
            vcpu: Some(size.vcpu),
            memory_gb: Some(size.memory_gb),
            billing_multiplier: Some(1.0),
            machine_provider: Nullable::Value("gce".into()),
            created_at: i
                .creation
                .as_deref()
                .and_then(time::parse)
                .map(|t| Nullable::Value(time::format(t)))
                .unwrap_or(Nullable::Null),
            updated_at: Nullable::Value(time::format(time::now())),
            archive_after: if deadline > 0 {
                Nullable::Value(time::format(deadline))
            } else {
                Nullable::Null
            },
            desktop_available: false,
            snapshots: Some(true),
            snapshot_available: state == "stopped",
            holds_creator_logins: Some(false),
            wipe_pending_until_restart: Some(false),
            access: Some("owner".into()),
            setup_status: match &setup {
                Some((s, _)) => Nullable::Value(s.clone()),
                None => Nullable::Null,
            },
            setup_error: match setup {
                Some((_, Some(e))) => Nullable::Value(e),
                _ => Nullable::Null,
            },
            environment: Nullable::Null,
            stop: Self::stop_op(suffix, i),
            extra,
            ..Default::default()
        }
    }

    fn pending_view(suffix: &str, p: &Pending) -> Sandbox {
        let mut extra = BTreeMap::new();
        extra.insert("provisioning".into(), json!(p.provisioning.label()));
        extra.insert("machineType".into(), json!(p.size.machine));
        extra.insert(
            "dollarsPerHour".into(),
            json!(p.size.hourly(p.provisioning)),
        );
        extra.insert("backend".into(), json!("openagents-gce"));
        Sandbox {
            id: format!("bx_{suffix}"),
            name: format!("bx_{suffix}"),
            state: if p.error.is_some() {
                "error".into()
            } else {
                "provisioned".into()
            },
            error: match &p.error {
                Some(e) => Nullable::Value(e.clone()),
                None => Nullable::Null,
            },
            hydrated: Some(false),
            type_: Some(p.size.name.into()),
            vcpu: Some(p.size.vcpu),
            memory_gb: Some(p.size.memory_gb),
            machine_provider: Nullable::Value("gce".into()),
            created_at: Nullable::Value(time::format(p.created)),
            desktop_available: false,
            snapshot_available: false,
            holds_creator_logins: Some(false),
            stop: Nullable::Null,
            extra,
            ..Default::default()
        }
    }

    /// After a restart of this service, a running VM it has not seen yet
    /// may already be ready for this boot: ask it (one short SSH call)
    /// before calling it "starting".
    async fn confirm_ready(&self, suffix: &str, i: &Instance) -> bool {
        if self.ready_for(suffix, i) {
            return true;
        }
        if i.status != "RUNNING" || self.st().preparing.contains(suffix) {
            return false;
        }
        let (Some(ip), Some(boot)) = (&i.ip, &i.last_start) else {
            return false;
        };
        let o = self
            .remote
            .run(
                ip,
                &format!("test -f \"{}/ready\"", self.cfg.run_dir),
                vec![],
                Duration::from_secs(15),
                1024,
            )
            .await;
        if o.code == Some(0) {
            self.st().ready.insert(suffix.into(), boot.clone());
            return true;
        }
        false
    }

    /// A sandbox's view; a running VM of unknown readiness (this service
    /// restarted) is checked and, when needed, prepared again.
    async fn describe(self: &Arc<Self>, suffix: &str) -> Res<Sandbox> {
        match self.find(suffix).await? {
            Found::Pending(p) => Ok(Self::pending_view(suffix, &p)),
            Found::Vm(i) => {
                if i.status == "RUNNING"
                    && !self.confirm_ready(suffix, &i).await
                    && !self.st().preparing.contains(suffix)
                {
                    self.spawn_prepare(suffix.to_owned(), i.zone.clone(), true);
                }
                Ok(self.view(suffix, &i))
            }
        }
    }

    // ------------------------------------------------------------------
    // Readiness.

    fn prepare_script(&self, recover: bool) -> String {
        let r = &self.cfg.run_dir;
        let recover = if recover {
            format!("[ -f \"{r}/ready\" ] && exit 0\n")
        } else {
            String::new()
        };
        format!(
            r#"set -e
{recover}R="{r}"
if ! mkdir -p "$R" 2>/dev/null || [ ! -w "$R" ]; then
  sudo -n install -d -m 0755 -o "$(id -u)" -g "$(id -g)" "$R"
fi
umask 077
cat > "$R/env.tmp"
mv "$R/env.tmp" "$R/env"
mkdir -p "$HOME/.oa-boat/proc"
if ! command -v coder-cloud-runtime >/dev/null 2>&1 && [ -x /home/coder/.local/bin/coder-cloud-runtime ]; then
  sudo -n install -m 0755 /home/coder/.local/bin/coder-cloud-runtime /usr/local/bin/coder-cloud-runtime 2>/dev/null || true
fi
"#
        )
    }

    fn prelude(&self) -> String {
        let r = &self.cfg.run_dir;
        format!("if [ -r \"{r}/env\" ]; then set -a; . \"{r}/env\"; set +a; fi\n")
    }

    /// Make the VM ready for its current boot, in the background.
    pub fn spawn_prepare(self: &Arc<Self>, suffix: String, zone: String, recover: bool) {
        if !self.st().preparing.insert(suffix.clone()) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let result = this.prepare(&suffix, &zone, recover).await;
            let mut st = this.st();
            st.preparing.remove(&suffix);
            if let Err(e) = result {
                eprintln!("oa-boat: bx_{suffix} not ready: {e}");
            }
        });
    }

    async fn prepare(&self, suffix: &str, zone: &str, recover: bool) -> Result<(), String> {
        let name = format!("{PREFIX}{suffix}");
        let start = Instant::now();
        let limit = Duration::from_secs(900);
        let (ip, boot) = loop {
            if start.elapsed() > limit {
                return Err("the VM did not start".into());
            }
            match self.compute.get_instance(zone, &name).await {
                Ok(Some(i)) if i.status == "RUNNING" => {
                    if let (Some(ip), Some(boot)) = (i.ip.clone(), i.last_start.clone()) {
                        break (ip, boot);
                    }
                }
                Ok(Some(i)) if matches!(i.status.as_str(), "TERMINATED" | "STOPPING") => {
                    return Err("the VM stopped before it was ready".into());
                }
                Ok(None) => return Err("the VM is gone".into()),
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        };
        self.remote.forget(&ip).await;
        loop {
            if start.elapsed() > limit {
                return Err("the VM never answered SSH".into());
            }
            let o = self
                .remote
                .run(&ip, "true", vec![], Duration::from_secs(20), 1024)
                .await;
            if o.code == Some(0) {
                break;
            }
            self.remote.forget(&ip).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let env = self.st().env.remove(suffix);
        let recovering = recover && env.is_none();
        let file =
            env_file(&env.unwrap_or_default(), &format!("bx_{suffix}")).map_err(|e| e.message)?;
        let o = self
            .remote
            .run(
                &ip,
                &self.prepare_script(recovering),
                file.into_bytes(),
                Duration::from_secs(120),
                64 * 1024,
            )
            .await;
        if o.code != Some(0) {
            return Err(format!("prepare failed: {}", text(&o.stderr)));
        }
        let setup = self.st().setup.remove(suffix);
        if let Some(script) = setup {
            self.st()
                .setup_status
                .insert(suffix.into(), ("running".into(), None));
            let o = self
                .remote
                .run(
                    &ip,
                    &format!("{}cd \"$HOME\"\n{script}", self.prelude()),
                    vec![],
                    Duration::from_secs(1800),
                    64 * 1024,
                )
                .await;
            let status = if o.code == Some(0) {
                ("completed".to_owned(), None)
            } else {
                (
                    "failed".to_owned(),
                    Some(format!("setup exited {:?}", o.code)),
                )
            };
            self.st().setup_status.insert(suffix.into(), status);
        }
        let r = &self.cfg.run_dir;
        let o = self
            .remote
            .run(
                &ip,
                &format!("touch \"{r}/ready\""),
                vec![],
                Duration::from_secs(30),
                1024,
            )
            .await;
        if o.code != Some(0) {
            return Err("cannot mark the VM ready".into());
        }
        let since = self.st().started.remove(suffix);
        self.st().ready.insert(suffix.into(), boot);
        self.touch(suffix);
        eprintln!(
            "oa-boat: bx_{suffix} ready in {:.1} s (prepare {:.1} s)",
            since.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0),
            start.elapsed().as_secs_f64()
        );
        Ok(())
    }

    // ------------------------------------------------------------------
    // Lifecycle.

    async fn image_for(&self, from: Option<&str>) -> Res<Image> {
        match from {
            Some(name) if !name.is_empty() => {
                let image = self
                    .compute
                    .get_image(name)
                    .await
                    .map_err(ApiErr::gce)?
                    .ok_or_else(|| ApiErr::not_found("The named snapshot"))?;
                if image.status != "READY" {
                    return Err(ApiErr::new(
                        409,
                        "named_snapshot_not_ready",
                        "The named snapshot is not ready.",
                    ));
                }
                Ok(image)
            }
            _ => self
                .compute
                .image_from_family(&self.cfg.base_family)
                .await
                .map_err(ApiErr::gce)?
                .ok_or_else(|| ApiErr::new(503, "no_base_image", "No base image is available.")),
        }
    }

    fn deadline(&self, ttl: &Nullable<i64>) -> i64 {
        match ttl {
            Nullable::Unset => time::now() + self.cfg.default_ttl,
            Nullable::Null => 0,
            Nullable::Value(t) if *t <= 0 => 0,
            Nullable::Value(t) => time::now() + t,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn instance_body(
        &self,
        suffix: &str,
        zone: &str,
        size: Size,
        prov: Provisioning,
        source: Value,
        disk_gb: i64,
        labels: &BTreeMap<String, String>,
    ) -> Value {
        let mut init = json!({
            "diskSizeGb": disk_gb.to_string(),
            "diskType": format!("zones/{zone}/diskTypes/pd-balanced"),
        });
        if let (Some(o), Some(s)) = (init.as_object_mut(), source.as_object()) {
            for (k, v) in s {
                o.insert(k.clone(), v.clone());
            }
        }
        let scheduling = match prov {
            Provisioning::Spot => json!({
                "provisioningModel": "SPOT",
                "instanceTerminationAction": "STOP",
                "automaticRestart": false,
                "onHostMaintenance": "TERMINATE",
                "maxRunDuration": {"seconds": self.cfg.max_run_seconds.to_string()},
            }),
            Provisioning::Standard => json!({
                "provisioningModel": "STANDARD",
                "instanceTerminationAction": "STOP",
                "automaticRestart": true,
                "onHostMaintenance": "MIGRATE",
                "maxRunDuration": {"seconds": self.cfg.max_run_seconds.to_string()},
            }),
        };
        json!({
            "name": format!("{PREFIX}{suffix}"),
            "machineType": format!("zones/{zone}/machineTypes/{}", size.machine),
            "labels": labels,
            "tags": {"items": [self.cfg.tag]},
            "metadata": {"items": [
                {"key": "ssh-keys", "value": format!("{}:{} oa-boat", self.cfg.user, self.cfg.ssh_public_key.trim())},
                {"key": "block-project-ssh-keys", "value": "TRUE"},
                {"key": "enable-oslogin", "value": "FALSE"},
            ]},
            "disks": [{"boot": true, "autoDelete": true, "initializeParams": init}],
            "networkInterfaces": [{
                "subnetwork": format!("projects/{}/regions/{}/subnetworks/{}", self.cfg.project, self.cfg.region, self.cfg.subnetwork),
            }],
            "scheduling": scheduling,
            "shieldedInstanceConfig": {"enableSecureBoot": true, "enableVtpm": true, "enableIntegrityMonitoring": true},
            "serviceAccounts": [],
            "description": format!("oa-boat sandbox bx_{suffix}"),
        })
    }

    /// Insert in the first zone with room, in the background.
    fn spawn_insert(
        self: &Arc<Self>,
        suffix: String,
        bodies: impl Fn(&str) -> Value + Send + 'static,
        after: Option<String>,
    ) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut last = None;
            for zone in this.cfg.zones.clone() {
                let mut inserted = this.compute.insert_instance(&zone, bodies(&zone)).await;
                // A failure while waiting can still have made the VM: never
                // leave one running that no record holds.
                if let Err(e) = &inserted
                    && !e.capacity()
                    && let Ok(Some(_)) = this
                        .compute
                        .get_instance(&zone, &format!("{PREFIX}{suffix}"))
                        .await
                {
                    eprintln!("oa-boat: bx_{suffix}: {e}, but the VM exists; using it");
                    inserted = Ok(());
                }
                match inserted {
                    Ok(()) => {
                        {
                            let mut st = this.st();
                            st.zones.insert(suffix.clone(), zone.clone());
                            st.pending.remove(&suffix);
                        }
                        if let Some(snapshot) = &after {
                            let _ = this.compute.delete_snapshot(snapshot).await;
                        }
                        eprintln!("oa-boat: bx_{suffix} inserted in {zone}");
                        this.spawn_prepare(suffix, zone, false);
                        return;
                    }
                    Err(e) if e.capacity() => {
                        eprintln!("oa-boat: bx_{suffix}: no capacity in {zone}");
                        last = Some(e);
                    }
                    Err(e) => {
                        last = Some(e);
                        break;
                    }
                }
            }
            if let Some(snapshot) = &after {
                let _ = this.compute.delete_snapshot(snapshot).await;
            }
            let why = last
                .map(|e| {
                    eprintln!("oa-boat: bx_{suffix} failed: {e}");
                    if e.capacity() {
                        "No zone had capacity for this machine.".to_owned()
                    } else {
                        "Compute Engine refused the machine.".to_owned()
                    }
                })
                .unwrap_or_default();
            if let Some(p) = this.st().pending.get_mut(&suffix) {
                p.error = Some(why);
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn labels(
        &self,
        size: Size,
        prov: Provisioning,
        deadline: i64,
        idle: i64,
        suffix: &str,
        idem: Option<&str>,
        from: Option<&str>,
    ) -> BTreeMap<String, String> {
        let mut l = BTreeMap::from([
            (MANAGED.to_owned(), MANAGED_SANDBOX.to_owned()),
            ("oa-boat-id".to_owned(), suffix.to_owned()),
            ("oa-boat-type".to_owned(), size.name.to_owned()),
            ("oa-boat-prov".to_owned(), prov.label().to_owned()),
            ("oa-boat-deadline".to_owned(), deadline.to_string()),
            ("oa-boat-idle".to_owned(), idle.to_string()),
            ("oa-boat-acc".to_owned(), "0".to_owned()),
            ("oa-boat-accu".to_owned(), "0".to_owned()),
            ("oa-boat-stopn".to_owned(), "0".to_owned()),
        ]);
        if let Some(k) = idem {
            l.insert("oa-boat-idem".into(), digest(k));
        }
        if let Some(f) = from {
            let f: String = f
                .to_ascii_lowercase()
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .take(63)
                .collect();
            l.insert("oa-boat-from".into(), f);
        }
        l
    }

    async fn by_idempotency(&self, key: &str) -> Res<Option<String>> {
        if let Some(s) = self.st().idempotency.get(key).cloned() {
            return Ok(Some(s));
        }
        let found = self
            .compute
            .list_instances("oa-boat-idem", &digest(key))
            .await
            .map_err(ApiErr::gce)?;
        Ok(found
            .first()
            .and_then(|i| i.labels.get("oa-boat-id").cloned()))
    }

    fn provisioning(
        &self,
        extra: &BTreeMap<String, Value>,
        header: Option<&str>,
    ) -> Res<Provisioning> {
        match header.or_else(|| extra_str(extra, &["provisioning", "oaProvisioning"])) {
            Some(p) => Provisioning::parse(p).ok_or_else(|| {
                ApiErr::new(
                    400,
                    "invalid_provisioning",
                    "provisioning is spot or standard.",
                )
            }),
            None => Ok(self.cfg.default_provisioning),
        }
    }

    pub async fn create(
        self: &Arc<Self>,
        req: CreateSandboxRequest,
        idempotency: Option<String>,
        provisioning: Option<&str>,
    ) -> Res<CreateSandboxResponse> {
        if let Some(key) = &idempotency
            && let Some(existing) = self.by_idempotency(key).await?
        {
            let sandbox = self.describe(&existing).await?;
            return Ok(CreateSandboxResponse {
                ok: true,
                type_: "sandbox".into(),
                status: sandbox.state.clone(),
                ttl_seconds: None,
                sandbox,
                ..Default::default()
            });
        }
        let size_name = req.type_.as_deref().unwrap_or("default");
        let size = sizes::size(size_name)
            .ok_or_else(|| ApiErr::new(400, "invalid_type", "Unknown sandbox type."))?;
        let prov = self.provisioning(&req.extra, provisioning)?;
        let active = self.active().await?;
        if active >= self.cfg.max_active {
            return Err(ApiErr::new(
                429,
                "sandbox_limit",
                "Too many sandboxes are running.",
            ));
        }
        let image = self.image_for(req.from_.as_deref()).await?;
        let deadline = self.deadline(&req.ttl_seconds);
        let idle = extra_i64(&req.extra, &["idleStopSeconds"]).unwrap_or(self.cfg.default_idle);
        let suffix = new_suffix();
        let env = req.env.clone().unwrap_or_default();
        env_file(&env, "check")?;
        let labels = self.labels(
            size,
            prov,
            deadline,
            idle,
            &suffix,
            idempotency.as_deref(),
            req.from_.as_deref(),
        );
        let disk = size.disk_gb.max(image.disk_gb);
        let pending = Pending {
            size,
            provisioning: prov,
            created: time::now(),
            error: None,
        };
        {
            let mut st = self.st();
            st.pending.insert(suffix.clone(), pending.clone());
            st.env.insert(suffix.clone(), env);
            if let Some(s) = req.setup_script.clone().filter(|s| !s.trim().is_empty()) {
                st.setup.insert(suffix.clone(), s);
                st.setup_status
                    .insert(suffix.clone(), ("pending".into(), None));
            }
            if let Some(k) = &idempotency {
                st.idempotency.insert(k.clone(), suffix.clone());
            }
            st.started.insert(suffix.clone(), Instant::now());
            st.activity.insert(suffix.clone(), Instant::now());
        }
        let this = self.clone();
        let s2 = suffix.clone();
        let link = image.self_link.clone();
        self.spawn_insert(
            suffix.clone(),
            move |zone| {
                this.instance_body(
                    &s2,
                    zone,
                    size,
                    prov,
                    json!({"sourceImage": link}),
                    disk,
                    &labels,
                )
            },
            None,
        );
        eprintln!(
            "oa-boat: create bx_{suffix} {} {} from {}",
            size.name,
            prov.label(),
            image.name
        );
        Ok(CreateSandboxResponse {
            ok: true,
            type_: "sandbox".into(),
            status: "provisioned".into(),
            ttl_seconds: (deadline > 0).then(|| deadline - time::now()),
            sandbox: Self::pending_view(&suffix, &pending),
            ..Default::default()
        })
    }

    pub async fn get(self: &Arc<Self>, id: &str) -> Res<SandboxInfoResponse> {
        let suffix = suffix_of(id)?;
        self.touch(&suffix);
        Ok(SandboxInfoResponse {
            ok: true,
            type_: "sandbox".into(),
            sandbox: self.describe(&suffix).await?,
            ..Default::default()
        })
    }

    async fn active(&self) -> Res<i64> {
        let all = self
            .compute
            .list_instances(MANAGED, MANAGED_SANDBOX)
            .await
            .map_err(ApiErr::gce)?;
        let pending = self
            .st()
            .pending
            .values()
            .filter(|p| p.error.is_none())
            .count();
        Ok(all
            .iter()
            .filter(|i| !matches!(i.status.as_str(), "TERMINATED" | "SUSPENDED"))
            .count() as i64
            + pending as i64)
    }

    pub async fn list(&self, state: Option<&str>, limit: Option<i64>) -> Res<SandboxListResponse> {
        let all = self
            .compute
            .list_instances(MANAGED, MANAGED_SANDBOX)
            .await
            .map_err(ApiErr::gce)?;
        let mut out: Vec<Sandbox> = all
            .iter()
            .filter_map(|i| {
                let suffix = i.labels.get("oa-boat-id")?;
                Some(self.view(suffix, i))
            })
            .collect();
        let known: HashSet<String> = out.iter().map(|s| s.id.clone()).collect();
        for (suffix, p) in self.st().pending.iter() {
            if !known.contains(&format!("bx_{suffix}")) {
                out.push(Self::pending_view(suffix, p));
            }
        }
        if let Some(s) = state {
            out.retain(|x| x.state == s);
        }
        out.sort_by(|a, b| {
            let k = |s: &Sandbox| match &s.created_at {
                Nullable::Value(v) => v.clone(),
                _ => String::new(),
            };
            k(b).cmp(&k(a))
        });
        if let Some(l) = limit.filter(|l| *l > 0) {
            out.truncate(l as usize);
        }
        Ok(SandboxListResponse {
            ok: true,
            type_: "sandboxes".into(),
            sandboxes: out,
            ..Default::default()
        })
    }

    /// Read-modify-write the labels, retrying on a stale fingerprint.
    async fn relabel(
        &self,
        zone: &str,
        name: &str,
        change: impl Fn(&mut BTreeMap<String, String>),
    ) -> Res<Instance> {
        for _ in 0..4 {
            let i = self
                .compute
                .get_instance(zone, name)
                .await
                .map_err(ApiErr::gce)?
                .ok_or_else(|| ApiErr::not_found("The sandbox"))?;
            let mut labels = i.labels.clone();
            change(&mut labels);
            if labels == i.labels {
                return Ok(i);
            }
            match self
                .compute
                .set_labels(zone, name, &labels, &i.label_fingerprint)
                .await
            {
                Ok(()) => {
                    let mut i = i;
                    i.labels = labels;
                    return Ok(i);
                }
                Err(e) if e.status == 412 || e.code == "conditionNotMet" => continue,
                Err(e) => return Err(ApiErr::gce(e)),
            }
        }
        Err(ApiErr::new(
            409,
            "conflict",
            "The sandbox changed; try again.",
        ))
    }

    pub async fn update(
        self: &Arc<Self>,
        id: &str,
        req: UpdateSandboxRequest,
    ) -> Res<SandboxInfoResponse> {
        let (suffix, i) = self.vm(id).await?;
        if !req.ttl_seconds.is_unset() {
            let deadline = self.deadline(&req.ttl_seconds);
            self.relabel(&i.zone, &i.name, |l| {
                l.insert("oa-boat-deadline".into(), deadline.to_string());
            })
            .await?;
        }
        Ok(SandboxInfoResponse {
            ok: true,
            type_: "sandbox".into(),
            sandbox: self.describe(&suffix).await?,
            ..Default::default()
        })
    }

    /// Count a finished run into `oa-boat-acc` once.
    fn account(labels: &mut BTreeMap<String, String>, i: &Instance) {
        let (Some(start), Some(stop)) = (
            i.last_start.as_deref().and_then(time::parse),
            i.last_stop.as_deref().and_then(time::parse),
        ) else {
            return;
        };
        let accu: i64 = labels
            .get("oa-boat-accu")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if stop <= accu || stop < start {
            return;
        }
        let acc: i64 = labels
            .get("oa-boat-acc")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        labels.insert("oa-boat-acc".into(), (acc + stop - start).to_string());
        labels.insert("oa-boat-accu".into(), stop.to_string());
    }

    pub async fn stop(self: &Arc<Self>, id: &str) -> Res<SandboxActionResponse> {
        let (suffix, i) = self.vm(id).await?;
        let mut current = i.clone();
        if matches!(
            i.status.as_str(),
            "RUNNING" | "PROVISIONING" | "STAGING" | "REPAIRING"
        ) {
            current = self
                .relabel(&i.zone, &i.name, |l| {
                    let n: i64 = l
                        .get("oa-boat-stopn")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    l.insert("oa-boat-stopn".into(), (n + 1).to_string());
                    l.insert("oa-boat-stopat".into(), time::now().to_string());
                })
                .await?;
            self.compute
                .instance_action(&i.zone, &i.name, "stop")
                .await
                .map_err(ApiErr::gce)?;
            current.status = "STOPPING".into();
            if let Some(ip) = &i.ip {
                self.remote.forget(ip).await;
            }
            self.st().ready.remove(&suffix);
            eprintln!("oa-boat: stop bx_{suffix} (requested)");
        }
        Ok(SandboxActionResponse {
            ok: true,
            type_: "sandbox".into(),
            id: format!("bx_{suffix}"),
            status: "stopping".into(),
            sandbox: Nullable::Value(self.view(&suffix, &current)),
            ..Default::default()
        })
    }

    pub async fn resume(
        self: &Arc<Self>,
        id: &str,
        req: ResumeRequest,
    ) -> Res<SandboxActionResponse> {
        let (suffix, i) = self.vm(id).await?;
        match i.status.as_str() {
            "TERMINATED" | "SUSPENDED" => {}
            "STOPPING" | "SUSPENDING" => {
                return Err(ApiErr::new(
                    409,
                    "sandbox_stopping",
                    "The sandbox is still stopping.",
                ));
            }
            _ => {
                return Ok(SandboxActionResponse {
                    ok: true,
                    type_: "sandbox".into(),
                    id: format!("bx_{suffix}"),
                    status: self.state_of(&suffix, &i).into(),
                    sandbox: Nullable::Value(self.view(&suffix, &i)),
                    ..Default::default()
                });
            }
        }
        let env = req.env.clone().unwrap_or_default();
        env_file(&env, "check")?;
        let deadline = self.deadline(&req.ttl_seconds);
        let snapshot = i.clone();
        self.relabel(&i.zone, &i.name, |l| {
            Self::account(l, &snapshot);
            l.insert("oa-boat-deadline".into(), deadline.to_string());
        })
        .await?;
        {
            let mut st = self.st();
            st.env.insert(suffix.clone(), env);
            st.ready.remove(&suffix);
            st.started.insert(suffix.clone(), Instant::now());
        }
        self.compute
            .instance_action(&i.zone, &i.name, "start")
            .await
            .map_err(ApiErr::gce)?;
        eprintln!("oa-boat: resume bx_{suffix}");
        self.spawn_prepare(suffix.clone(), i.zone.clone(), false);
        let mut current = i;
        current.status = "STAGING".into();
        Ok(SandboxActionResponse {
            ok: true,
            type_: "sandbox".into(),
            id: format!("bx_{suffix}"),
            status: "provisioned".into(),
            sandbox: Nullable::Value(self.view(&suffix, &current)),
            ..Default::default()
        })
    }

    pub async fn fork(
        self: &Arc<Self>,
        id: &str,
        req: ForkParamsBody,
        idempotency: Option<String>,
        provisioning: Option<&str>,
    ) -> Res<SandboxActionResponse> {
        if let Some(key) = &idempotency
            && let Some(existing) = self.by_idempotency(key).await?
        {
            let sandbox = self.describe(&existing).await?;
            return Ok(SandboxActionResponse {
                ok: true,
                type_: "sandbox".into(),
                id: sandbox.id.clone(),
                status: sandbox.state.clone(),
                sandbox: Nullable::Value(sandbox),
                ..Default::default()
            });
        }
        let (source_suffix, source) = self.vm(id).await?;
        let disk = source
            .disk
            .clone()
            .ok_or_else(|| ApiErr::new(409, "no_disk", "The sandbox has no disk."))?;
        let size = match req.type_.as_deref() {
            Some(t) => sizes::size(t)
                .ok_or_else(|| ApiErr::new(400, "invalid_type", "Unknown sandbox type."))?,
            None => Self::size_of(&source),
        };
        let prov = self.provisioning(&req.extra, provisioning)?;
        let deadline = self.deadline(&req.ttl_seconds);
        let idle = extra_i64(&req.extra, &["idleStopSeconds"]).unwrap_or(self.cfg.default_idle);
        let env = req.env.clone().unwrap_or_default();
        env_file(&env, "check")?;
        let suffix = new_suffix();
        let labels = self.labels(
            size,
            prov,
            deadline,
            idle,
            &suffix,
            idempotency.as_deref(),
            Some(&format!("fork-{source_suffix}")),
        );
        let disk_gb = source.disk_gb.unwrap_or(size.disk_gb).max(size.disk_gb);
        let pending = Pending {
            size,
            provisioning: prov,
            created: time::now(),
            error: None,
        };
        {
            let mut st = self.st();
            st.pending.insert(suffix.clone(), pending.clone());
            st.env.insert(suffix.clone(), env);
            if let Some(k) = &idempotency {
                st.idempotency.insert(k.clone(), suffix.clone());
            }
            st.started.insert(suffix.clone(), Instant::now());
            st.activity.insert(suffix.clone(), Instant::now());
        }
        let snapshot = format!("oa-boat-fork-{suffix}");
        let this = self.clone();
        let s2 = suffix.clone();
        tokio::spawn(async move {
            if let Err(e) = this
                .compute
                .snapshot_disk(&source.zone, &disk, &snapshot)
                .await
            {
                eprintln!("oa-boat: fork bx_{s2}: {e}");
                if let Some(p) = this.st().pending.get_mut(&s2) {
                    p.error = Some("The source disk could not be copied.".into());
                }
                return;
            }
            let body_this = this.clone();
            let s3 = s2.clone();
            let link = format!("projects/{}/global/snapshots/{snapshot}", this.cfg.project);
            this.spawn_insert(
                s2.clone(),
                move |zone| {
                    body_this.instance_body(
                        &s3,
                        zone,
                        size,
                        prov,
                        json!({"sourceSnapshot": link}),
                        disk_gb,
                        &labels,
                    )
                },
                Some(snapshot),
            );
        });
        eprintln!("oa-boat: fork bx_{source_suffix} -> bx_{suffix}");
        Ok(SandboxActionResponse {
            ok: true,
            type_: "sandbox".into(),
            id: format!("bx_{suffix}"),
            status: "provisioned".into(),
            sandbox: Nullable::Value(Self::pending_view(&suffix, &pending)),
            ..Default::default()
        })
    }

    pub async fn delete(
        self: &Arc<Self>,
        id: &str,
        confirm: Option<&str>,
    ) -> Res<DeletionOperationResponse> {
        if confirm != Some(id) {
            return Err(ApiErr::new(
                400,
                "confirmation_required",
                "X-Ascii-Confirm-Delete must name the sandbox.",
            ));
        }
        let suffix = suffix_of(id)?;
        let found = self.find(&suffix).await?;
        match found {
            Found::Pending(p) if p.error.is_none() => {
                return Err(ApiErr::new(
                    409,
                    "boat_starting",
                    "The sandbox is still starting.",
                ));
            }
            Found::Pending(_) => {
                self.st().pending.remove(&suffix);
            }
            Found::Vm(i) => {
                if let Some(ip) = &i.ip {
                    self.remote.forget(ip).await;
                }
                match self
                    .compute
                    .instance_action(&i.zone, &i.name, "delete")
                    .await
                {
                    Ok(()) => {}
                    Err(e) if e.not_found() => {}
                    Err(e) => return Err(ApiErr::gce(e)),
                }
                eprintln!("oa-boat: delete bx_{suffix}");
            }
        }
        {
            let mut st = self.st();
            st.ready.remove(&suffix);
            st.activity.remove(&suffix);
            st.env.remove(&suffix);
            st.setup_status.remove(&suffix);
        }
        Ok(DeletionOperationResponse {
            ok: true,
            type_: "deletionOperation".into(),
            operation: DeletionOperation {
                id: format!("del_{suffix}"),
                kind: "sandbox".into(),
                target_id: format!("bx_{suffix}"),
                reason: "requested".into(),
                status: "running".into(),
                attempt_count: 1,
                requested_at: time::format(time::now()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    pub async fn deletion(&self, op: &str) -> Res<DeletionOperationResponse> {
        let suffix = op
            .strip_prefix("del_")
            .ok_or_else(|| ApiErr::not_found("The operation"))?;
        let done = match self.find(suffix).await {
            Err(e) if e.status == 404 => true,
            Err(e) => return Err(e),
            Ok(_) => false,
        };
        Ok(DeletionOperationResponse {
            ok: true,
            type_: "deletionOperation".into(),
            operation: DeletionOperation {
                id: op.into(),
                kind: "sandbox".into(),
                target_id: format!("bx_{suffix}"),
                reason: "requested".into(),
                status: if done { "completed" } else { "running" }.into(),
                attempt_count: 1,
                requested_at: time::format(time::now()),
                completed_at: done.then(|| time::format(time::now())),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    // ------------------------------------------------------------------
    // Commands and files.

    async fn ready_vm(self: &Arc<Self>, id: &str) -> Res<(String, String)> {
        let (suffix, i) = self.vm(id).await?;
        if !self.confirm_ready(&suffix, &i).await {
            if i.status == "RUNNING" && !self.st().preparing.contains(&suffix) {
                self.spawn_prepare(suffix.clone(), i.zone.clone(), true);
            }
            return Err(match i.status.as_str() {
                "TERMINATED" | "SUSPENDED" | "STOPPING" => ApiErr::new(
                    409,
                    "sandbox_stopped",
                    "The sandbox is stopped; resume it first.",
                ),
                _ => ApiErr::new(409, "boat_starting", "The sandbox is still starting."),
            });
        }
        let ip = i
            .ip
            .ok_or_else(|| ApiErr::new(409, "boat_starting", "The sandbox has no address yet."))?;
        Ok((suffix, ip))
    }

    fn cd(cwd: Option<&str>) -> String {
        match cwd.filter(|c| !c.is_empty()) {
            Some(c) => format!("cd {} || exit 2\n", boat::shell_quote(c)),
            None => "cd \"$HOME\"\n".into(),
        }
    }

    fn sync_script(&self, req: &CommandRequest) -> String {
        let t = req.timeout_seconds.unwrap_or(600).clamp(1, 3600);
        let q = boat::shell_quote(&req.command);
        format!(
            "{}{}if command -v timeout >/dev/null 2>&1; then timeout -k 5 {t} bash -c {q}; else bash -c {q}; fi\nc=$?\nprintf '\\037OA-BOAT-EXIT %d\\n' \"$c\" >&2\nexit 0\n",
            self.prelude(),
            Self::cd(req.cwd.as_deref()),
        )
    }

    fn detached_script(&self, req: &CommandRequest) -> String {
        let q = boat::shell_quote(&req.command);
        let wrapper = boat::shell_quote(
            r#"p=$$; d="$HOME/.oa-boat/proc/$p"; mkdir -p "$d"; date -u +%s > "$d/start"; cat /proc/sys/kernel/random/boot_id > "$d/boot" 2>/dev/null; printf %s "$PWD" > "$d/cwd"; bash -c "$1" > "$d/out" 2> "$d/err" < /dev/null; c=$?; date -u +%s > "$d/end"; echo "$c" > "$d/exit.tmp"; mv "$d/exit.tmp" "$d/exit""#,
        );
        format!(
            "{}mkdir -p \"$HOME/.oa-boat/proc\"\n{}if command -v setsid >/dev/null 2>&1; then s=setsid; else s=; fi\n$s nohup bash -c {wrapper} oa-boat {q} >/dev/null 2>&1 </dev/null &\np=$!\ni=0; while [ ! -d \"$HOME/.oa-boat/proc/$p\" ] && [ $i -lt 100 ]; do sleep 0.05; i=$((i+1)); done\necho \"$p\"\necho \"$HOME\"\n",
            self.prelude(),
            Self::cd(req.cwd.as_deref()),
        )
    }

    pub async fn command(self: &Arc<Self>, id: &str, req: CommandRequest) -> Res<Reply> {
        if req.command.trim().is_empty() {
            return Err(ApiErr::new(400, "invalid_command", "The command is empty."));
        }
        let (suffix, ip) = self.ready_vm(id).await?;
        let started = time::format(time::now());
        if req.detached == Some(true) {
            let o = self
                .remote
                .run(
                    &ip,
                    &self.detached_script(&req),
                    vec![],
                    Duration::from_secs(60),
                    4096,
                )
                .await;
            let out = text(&o.stdout);
            let mut lines = out.lines();
            let pid: i64 = lines
                .next()
                .and_then(|l| l.trim().parse().ok())
                .ok_or_else(|| {
                    ApiErr::new(
                        502,
                        "boat_direct_failed",
                        "The command may not have started.",
                    )
                })?;
            let home = lines.next().unwrap_or("/home/user").trim().to_owned();
            self.touch(&suffix);
            return Ok(Reply::Started(CommandStartedResponse {
                ok: true,
                type_: "command".into(),
                success: true,
                process_id: pid,
                pid,
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                started_at: started,
                log_path: Some(format!("{home}/.oa-boat/proc/{pid}/out")),
                err_log_path: Some(format!("{home}/.oa-boat/proc/{pid}/err")),
                ..Default::default()
            }));
        }
        let t = req.timeout_seconds.unwrap_or(600).clamp(1, 3600) as u64;
        let limit = Duration::from_secs(t + 30);
        if req.stream == Some(true) {
            let mut rx = self
                .remote
                .stream(&ip, &self.sync_script(&req), limit)
                .await;
            let (tx, out) = mpsc::channel(64);
            let this = self.clone();
            tokio::spawn(async move {
                let frame = |t: &str| CommandStreamFrame {
                    type_: t.into(),
                    ..Default::default()
                };
                let _ = tx.send(frame("started")).await;
                // Hold back the tail of stderr until the end: it carries
                // the exit trailer.
                let mut held: Vec<u8> = Vec::new();
                let keep = EXIT_MARK.len() + 16;
                let mut code = None;
                let mut failed = None;
                while let Some(chunk) = rx.recv().await {
                    this.touch(&suffix);
                    match chunk {
                        Chunk::Stdout(b) => {
                            let mut f = frame("stdout");
                            f.data = Some(text(&b));
                            if tx.send(f).await.is_err() {
                                return;
                            }
                        }
                        Chunk::Stderr(b) => {
                            held.extend_from_slice(&b);
                            if held.len() > keep {
                                let cut = held.len() - keep;
                                let mut f = frame("stderr");
                                f.data = Some(text(&held[..cut]));
                                held.drain(..cut);
                                if tx.send(f).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Chunk::Exit(c) => code = Some(c),
                        Chunk::Failed(m) => failed = Some(m),
                    }
                }
                let (rest, exit) = take_exit(&held);
                if !rest.is_empty() {
                    let mut f = frame("stderr");
                    f.data = Some(text(&rest));
                    let _ = tx.send(f).await;
                }
                match (exit, failed, code) {
                    (Some(c), _, _) => {
                        let mut f = frame("exit");
                        f.exit_code = Nullable::Value(c);
                        f.success = Some(c == 0);
                        f.timed_out = Some(c == 124);
                        let _ = tx.send(f).await;
                    }
                    _ => {
                        let mut f = frame("error");
                        f.error = Some("boat_direct_failed".into());
                        f.message = Some("The connection to the sandbox failed; the command may still be running.".into());
                        f.retryable = Some(false);
                        let _ = tx.send(f).await;
                    }
                }
            });
            return Ok(Reply::Stream(out));
        }
        let o = self
            .remote
            .run(&ip, &self.sync_script(&req), vec![], limit, OUTPUT_CAP)
            .await;
        self.touch(&suffix);
        let (stderr, exit) = take_exit(&o.stderr_with_tail());
        let Some(code) = exit else {
            return Err(ApiErr::new(
                502,
                "boat_direct_failed",
                "The connection to the sandbox failed; the command may still be running.",
            ));
        };
        Ok(Reply::Finished(CommandResponse {
            ok: true,
            type_: "command".into(),
            success: code == 0,
            exit_code: Some(code),
            signal: Nullable::Null,
            stdout: text(&o.stdout),
            stderr: text(&stderr),
            stdout_truncated: Some(o.stdout_truncated),
            stderr_truncated: Some(o.stderr_truncated),
            timed_out: code == 124,
            cwd: req.cwd.clone(),
            started_at: Some(started),
            finished_at: Some(time::format(time::now())),
            ..Default::default()
        }))
    }

    pub async fn command_status(
        self: &Arc<Self>,
        id: &str,
        pid: i64,
        tail: Option<i64>,
    ) -> Res<CommandStatusResponse> {
        let (_, ip) = self.ready_vm(id).await?;
        let n = tail.unwrap_or(256 * 1024).clamp(0, OUTPUT_CAP as i64);
        let script = format!(
            r#"d="$HOME/.oa-boat/proc/{pid}"
[ -d "$d" ] || {{ echo none; exit 0; }}
b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); ob=$(cat "$d/boot" 2>/dev/null)
if [ -f "$d/exit" ]; then echo "exit $(cat "$d/exit")"
elif [ "$b" = "$ob" ] && kill -0 {pid} 2>/dev/null; then echo running
else echo lost; fi
echo "$HOME"
cat "$d/start" 2>/dev/null || echo 0
cat "$d/end" 2>/dev/null || echo 0
tail -c {n} "$d/out" 2>/dev/null | base64 | tr -d '\n'; echo
tail -c {n} "$d/err" 2>/dev/null | base64 | tr -d '\n'; echo
wc -c < "$d/out" 2>/dev/null || echo 0
wc -c < "$d/err" 2>/dev/null || echo 0
"#
        );
        let o = self
            .remote
            .run(
                &ip,
                &script,
                vec![],
                Duration::from_secs(60),
                OUTPUT_CAP * 2,
            )
            .await;
        if o.code != Some(0) {
            return Err(ApiErr::new(
                502,
                "boat_direct_failed",
                "The sandbox did not answer.",
            ));
        }
        let out = text(&o.stdout);
        let lines: Vec<&str> = out.lines().collect();
        let first = lines.first().copied().unwrap_or("none");
        if first == "none" {
            return Err(ApiErr::not_found("The process"));
        }
        use base64::Engine;
        let b64 = |s: Option<&&str>| {
            base64::engine::general_purpose::STANDARD
                .decode(s.map(|s| s.trim()).unwrap_or(""))
                .map(|b| text(&b))
                .unwrap_or_default()
        };
        let num = |s: Option<&&str>| s.and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
        let home = lines.get(1).copied().unwrap_or("/home/user");
        let (status, running, exit_code) = if let Some(c) = first.strip_prefix("exit ") {
            ("exited", false, c.trim().parse().ok())
        } else if first == "running" {
            ("running", true, None)
        } else {
            ("lost", false, None)
        };
        let (start, end) = (num(lines.get(2)), num(lines.get(3)));
        let (out_len, err_len) = (num(lines.get(6)), num(lines.get(7)));
        Ok(CommandStatusResponse {
            ok: true,
            type_: "commandStatus".into(),
            success: exit_code == Some(0),
            process_id: pid,
            pid: Some(pid),
            status: status.into(),
            known: Some(true),
            running,
            exit_code,
            signal: Nullable::Null,
            started_at: if start > 0 {
                Nullable::Value(time::format(start))
            } else {
                Nullable::Null
            },
            finished_at: if end > 0 {
                Nullable::Value(time::format(end))
            } else {
                Nullable::Null
            },
            stdout: b64(lines.get(4)),
            stderr: b64(lines.get(5)),
            stdout_truncated: Some(out_len > n),
            stderr_truncated: Some(err_len > n),
            log_path: Some(format!("{home}/.oa-boat/proc/{pid}/out")),
            err_log_path: Some(format!("{home}/.oa-boat/proc/{pid}/err")),
            ..Default::default()
        })
    }

    pub async fn read_file(
        self: &Arc<Self>,
        id: &str,
        path: &str,
        encoding: Option<&str>,
    ) -> Res<FileReadResponse> {
        let (_, ip) = self.ready_vm(id).await?;
        let script = format!(
            "f={}\n[ -f \"$f\" ] || exit 44\nexec cat -- \"$f\"\n",
            boat::shell_quote(path)
        );
        let o = self
            .remote
            .run(&ip, &script, vec![], Duration::from_secs(300), FILE_CAP + 1)
            .await;
        match o.code {
            Some(0) => {}
            Some(44) => return Err(ApiErr::not_found("The file")),
            _ => {
                return Err(ApiErr::new(
                    502,
                    "boat_direct_failed",
                    "The file could not be read.",
                ));
            }
        }
        if o.stdout_truncated {
            return Err(ApiErr::new(
                413,
                "file_too_large",
                "The file is too large to read in one call.",
            ));
        }
        let size = o.stdout.len() as i64;
        let (encoding, content) = match encoding.unwrap_or("utf8") {
            "base64" => {
                use base64::Engine;
                (
                    "base64",
                    base64::engine::general_purpose::STANDARD.encode(&o.stdout),
                )
            }
            _ => ("utf8", text(&o.stdout)),
        };
        Ok(FileReadResponse {
            ok: true,
            type_: "file".into(),
            success: true,
            path: path.into(),
            encoding: encoding.into(),
            size,
            content,
            ..Default::default()
        })
    }

    pub async fn write_file(
        self: &Arc<Self>,
        id: &str,
        req: FileWriteRequest,
    ) -> Res<FileWriteResponse> {
        let (_, ip) = self.ready_vm(id).await?;
        let bytes = match req.encoding.as_deref().unwrap_or("utf8") {
            "base64" => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(req.content.trim())
                    .map_err(|_| {
                        ApiErr::new(400, "invalid_content", "The content is not base64.")
                    })?
            }
            _ => req.content.clone().into_bytes(),
        };
        if bytes.len() > FILE_CAP {
            return Err(ApiErr::new(
                413,
                "file_too_large",
                "The file is too large to write in one call.",
            ));
        }
        let size = bytes.len() as i64;
        let script = format!(
            "f={}\nmkdir -p -- \"$(dirname -- \"$f\")\" && cat > \"$f.oa-boat-tmp\" && mv -f -- \"$f.oa-boat-tmp\" \"$f\"\n",
            boat::shell_quote(&req.path)
        );
        let o = self
            .remote
            .run(&ip, &script, bytes, Duration::from_secs(300), 64 * 1024)
            .await;
        if o.code != Some(0) {
            return Err(ApiErr::new(
                502,
                "boat_direct_failed",
                "The file could not be written.",
            ));
        }
        Ok(FileWriteResponse {
            ok: true,
            type_: "file".into(),
            success: true,
            path: req.path,
            encoding: req.encoding.unwrap_or_else(|| "utf8".into()),
            size,
            ..Default::default()
        })
    }

    // ------------------------------------------------------------------
    // Usage, snapshots, limits.

    /// Run seconds so far: the accounted runs plus the current one.
    pub fn run_seconds(i: &Instance, now: i64) -> i64 {
        let acc = label_i64(i, "oa-boat-acc");
        let accu = label_i64(i, "oa-boat-accu");
        let start = i.last_start.as_deref().and_then(time::parse);
        let stop = i.last_stop.as_deref().and_then(time::parse);
        let current = match (i.status.as_str(), start, stop) {
            ("RUNNING", Some(s), _) => now - s,
            ("TERMINATED" | "SUSPENDED", Some(s), Some(e)) if e > accu && e >= s => e - s,
            (_, Some(s), _) if s > accu => now - s,
            _ => 0,
        };
        acc + current.max(0)
    }

    pub async fn usage(&self, id: &str) -> Res<SandboxUsageResponse> {
        let (suffix, i) = self.vm(id).await?;
        let size = Self::size_of(&i);
        let prov = Self::provisioning_of(&i);
        let now = time::now();
        let seconds = Self::run_seconds(&i, now);
        let rate = size.hourly(prov);
        let disk = i.disk_gb.unwrap_or(size.disk_gb);
        let disk_rate = disk as f64 * sizes::DISK_HOURLY_PER_GB;
        let mut extra = BTreeMap::new();
        extra.insert("dollarsPerHour".into(), json!(rate));
        extra.insert("diskDollarsPerHour".into(), json!(disk_rate));
        extra.insert("totalDollarsPerHour".into(), json!(rate + disk_rate));
        extra.insert("machineType".into(), json!(i.machine));
        extra.insert("provisioning".into(), json!(prov.label()));
        extra.insert("diskGb".into(), json!(disk));
        extra.insert("backend".into(), json!("openagents-gce"));
        let since = i.creation.as_deref().and_then(time::parse).unwrap_or(now);
        Ok(SandboxUsageResponse {
            ok: true,
            type_: "usage".into(),
            sandbox_id: format!("bx_{suffix}"),
            sandbox_type: size.name.into(),
            billing_multiplier: 1.0,
            since: time::format(since),
            until: time::format(now),
            seconds,
            dollars: (seconds as f64 / 3600.0 * rate * 1_000_000.0).round() / 1_000_000.0,
            seconds_per_dollar: (3600.0 / rate).round() as i64,
            running: i.status == "RUNNING",
            extra,
        })
    }

    pub async fn latest_snapshot(&self, id: &str) -> Res<SnapshotLatestResponse> {
        let (suffix, i) = self.vm(id).await?;
        let snapshot = i
            .last_stop
            .as_deref()
            .and_then(time::parse)
            .filter(|_| i.status != "STOPPING")
            .map(|stop| SnapshotSummary {
                id: format!("snap_{suffix}_{stop}"),
                sandbox_id: format!("bx_{suffix}"),
                status: "completed".into(),
                kind: Nullable::Value("disk".into()),
                generation: Some(label_i64(&i, "oa-boat-stopn")),
                created_at: time::format(stop),
                completed_at: Nullable::Value(time::format(stop)),
                size_bytes: i.disk_gb.map(|g| g * 1024 * 1024 * 1024),
                ..Default::default()
            });
        Ok(SnapshotLatestResponse {
            ok: true,
            type_: "snapshot".into(),
            snapshot,
            ..Default::default()
        })
    }

    pub async fn limits(&self) -> Res<LimitsResponse> {
        let active = self.active().await?;
        Ok(LimitsResponse {
            ok: true,
            type_: "limits".into(),
            can_start: active < self.cfg.max_active,
            active_sandboxes: active,
            max_active_sandboxes: self.cfg.max_active,
            billing_status: "active".into(),
            plan: Nullable::Value("openagents-gce".into()),
            plan_name: Nullable::Value("OpenAgents on Google Cloud".into()),
            unlimited: Some(true),
            service_account: Some(true),
            ..Default::default()
        })
    }

    fn named(image: &Image) -> NamedSnapshot {
        let status = match image.status.as_str() {
            "READY" => "ready",
            "PENDING" => "saving",
            "FAILED" => "failed",
            "DELETING" => "deleting",
            _ => "saving",
        };
        let mut extra = BTreeMap::new();
        extra.insert("image".into(), json!(image.self_link));
        extra.insert("diskGb".into(), json!(image.disk_gb));
        NamedSnapshot {
            name: image.name.clone(),
            status: status.into(),
            error: (status == "failed").then(|| "The image could not be made.".into()),
            source_sandbox_id: image
                .labels
                .get("oa-boat-src")
                .map(|s| format!("bx_{s}"))
                .unwrap_or_default(),
            snapshot_id: (status == "ready").then(|| image.id.clone()),
            type_: image.labels.get("oa-boat-type").cloned(),
            size_bytes: image
                .archive_bytes
                .or(Some(image.disk_gb * 1024 * 1024 * 1024)),
            created_at: image
                .creation
                .as_deref()
                .and_then(time::parse)
                .map(time::format)
                .unwrap_or_default(),
            extra,
        }
    }

    fn ours(image: &Image) -> bool {
        image.labels.get(MANAGED).map(String::as_str) == Some(MANAGED_TEMPLATE)
    }

    pub async fn list_named(&self) -> Res<NamedSnapshotListResponse> {
        let mut images = self
            .compute
            .list_images(MANAGED, MANAGED_TEMPLATE)
            .await
            .map_err(ApiErr::gce)?;
        images.sort_by(|a, b| b.creation.cmp(&a.creation));
        Ok(NamedSnapshotListResponse {
            ok: true,
            type_: "namedSnapshots".into(),
            snapshots: images.iter().map(Self::named).collect(),
            ..Default::default()
        })
    }

    pub async fn get_named(&self, name: &str) -> Res<NamedSnapshotInfoResponse> {
        let image = self
            .compute
            .get_image(name)
            .await
            .map_err(ApiErr::gce)?
            .filter(Self::ours)
            .ok_or_else(|| ApiErr::not_found("The named snapshot"))?;
        Ok(NamedSnapshotInfoResponse {
            ok: true,
            type_: "namedSnapshot".into(),
            snapshot: Self::named(&image),
            ..Default::default()
        })
    }

    pub async fn save_named(
        self: &Arc<Self>,
        req: NamedSnapshotSaveRequest,
    ) -> Res<NamedSnapshotSavingResponse> {
        let name = req.name.trim();
        let valid = !name.is_empty()
            && name.len() <= 62
            && name.as_bytes()[0].is_ascii_lowercase()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid {
            return Err(ApiErr::new(
                400,
                "invalid_name",
                "A template name is lowercase letters, digits and hyphens, starting with a letter.",
            ));
        }
        if self
            .compute
            .get_image(name)
            .await
            .map_err(ApiErr::gce)?
            .is_some()
        {
            return Err(ApiErr::new(
                409,
                "named_snapshot_exists",
                "That name is taken.",
            ));
        }
        let (suffix, i) = self.vm(&req.sandbox_id).await?;
        let disk = i
            .disk
            .clone()
            .ok_or_else(|| ApiErr::new(409, "no_disk", "The sandbox has no disk."))?;
        let size = Self::size_of(&i);
        let body = json!({
            "name": name,
            "sourceDisk": format!("projects/{}/zones/{}/disks/{disk}", self.cfg.project, i.zone),
            "labels": {
                MANAGED: MANAGED_TEMPLATE,
                "oa-boat-src": suffix,
                "oa-boat-type": size.name,
            },
            "description": format!("oa-boat template {name} from bx_{suffix}"),
            "storageLocations": [self.cfg.region],
        });
        self.compute.insert_image(body).await.map_err(ApiErr::gce)?;
        eprintln!("oa-boat: save template {name} from bx_{suffix}");
        Ok(NamedSnapshotSavingResponse {
            ok: true,
            type_: "namedSnapshot".into(),
            status: "saving".into(),
            snapshot: NamedSnapshot {
                name: name.into(),
                status: "saving".into(),
                source_sandbox_id: format!("bx_{suffix}"),
                type_: Some(size.name.into()),
                created_at: time::format(time::now()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    pub async fn delete_named(&self, name: &str) -> Res<NamedSnapshotDeletedResponse> {
        self.get_named(name).await?;
        self.compute.delete_image(name).await.map_err(ApiErr::gce)?;
        eprintln!("oa-boat: delete template {name}");
        Ok(NamedSnapshotDeletedResponse {
            ok: true,
            type_: "namedSnapshot".into(),
            name: name.into(),
            status: "deleted".into(),
            ..Default::default()
        })
    }

    // ------------------------------------------------------------------
    // The reaper: accounting, TTL, idle stop.

    async fn busy(&self, ip: &str) -> bool {
        let script = r#"for d in "$HOME"/.oa-boat/proc/*/; do [ -d "$d" ] || continue; p=$(basename "$d"); [ -f "$d/exit" ] && continue; kill -0 "$p" 2>/dev/null && { echo busy; exit 0; }; done; echo idle"#;
        let o = self
            .remote
            .run(ip, script, vec![], Duration::from_secs(30), 1024)
            .await;
        // When the VM cannot say, it counts as busy: never stop on doubt.
        o.code != Some(0) || text(&o.stdout).trim() != "idle"
    }

    /// One pass. Returns what it stopped, for tests and the log.
    pub async fn reap(self: &Arc<Self>) -> Vec<(String, &'static str)> {
        let mut stopped = Vec::new();
        let Ok(all) = self.compute.list_instances(MANAGED, MANAGED_SANDBOX).await else {
            return stopped;
        };
        let now = time::now();
        for i in all {
            let Some(suffix) = i.labels.get("oa-boat-id").cloned() else {
                continue;
            };
            self.st().zones.insert(suffix.clone(), i.zone.clone());
            match i.status.as_str() {
                "TERMINATED" | "SUSPENDED" => {
                    let accu = label_i64(&i, "oa-boat-accu");
                    let stop = i.last_stop.as_deref().and_then(time::parse).unwrap_or(0);
                    if stop > accu {
                        let snapshot = i.clone();
                        let _ = self
                            .relabel(&i.zone, &i.name, |l| Self::account(l, &snapshot))
                            .await;
                    }
                    self.st().ready.remove(&suffix);
                }
                "RUNNING" => {
                    // A VM this process never made ready (it restarted, or
                    // a start failed half way) is made ready again, so its
                    // idle stop applies.
                    if !self.ready_for(&suffix, &i) && !self.st().preparing.contains(&suffix) {
                        self.spawn_prepare(suffix.clone(), i.zone.clone(), true);
                    }
                    let deadline = label_i64(&i, "oa-boat-deadline");
                    let idle = label_i64(&i, "oa-boat-idle");
                    let reason = if deadline > 0 && now > deadline {
                        Some("ttl")
                    } else if idle > 0 {
                        let quiet = {
                            let mut st = self.st();
                            let at = *st
                                .activity
                                .entry(suffix.clone())
                                .or_insert_with(Instant::now);
                            at.elapsed().as_secs() as i64
                        };
                        let ready = self.ready_for(&suffix, &i);
                        match (&i.ip, quiet >= idle && ready) {
                            (Some(ip), true) if !self.busy(ip).await => Some("idle"),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        let r = self
                            .relabel(&i.zone, &i.name, |l| {
                                let n: i64 = l
                                    .get("oa-boat-stopn")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(0);
                                l.insert("oa-boat-stopn".into(), (n + 1).to_string());
                                l.insert("oa-boat-stopat".into(), now.to_string());
                            })
                            .await;
                        if r.is_ok()
                            && self
                                .compute
                                .instance_action(&i.zone, &i.name, "stop")
                                .await
                                .is_ok()
                        {
                            if let Some(ip) = &i.ip {
                                self.remote.forget(ip).await;
                            }
                            self.st().ready.remove(&suffix);
                            eprintln!("oa-boat: stop bx_{suffix} ({reason})");
                            stopped.push((format!("bx_{suffix}"), reason));
                        }
                    }
                }
                _ => {}
            }
        }
        stopped
    }
}

impl crate::remote::Output {
    /// stderr with its kept tail, so the exit trailer survives the cap.
    pub fn stderr_with_tail(&self) -> Vec<u8> {
        if !self.stderr_truncated {
            return self.stderr.clone();
        }
        let mut v = self.stderr.clone();
        v.extend_from_slice(b"\n[... output truncated ...]\n");
        v.extend_from_slice(&self.stderr_tail);
        v
    }
}
