//! SSH transport for an operator-pinned Linux process owner. The helper owns
//! the durable create intent; reconnecting is observation, never another launch.
use crate::{
    Result,
    provider::{Cost, Create, Lookup, Provider, Realized, Teardown},
};
use nostr::contracts::{digest_bytes, digest_value, jcs};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use workspace_host::artifact::{Artifacts, atomic, read, token};
const BINDING: &str = "environment/ssh-native";
const LIMIT: usize = 1_048_576;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteConfig {
    pub root: PathBuf,
    pub account: String,
    pub executable: PathBuf,
    pub executable_digest: String,
    pub arguments: Vec<String>,
    pub runtime: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Identity {
    pid: u32,
    boot: String,
    start: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    generation: String,
    created_at: u64,
    identity: Option<Identity>,
    failed: bool,
    stopping: bool,
    stopped: bool,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    config: String,
    entries: BTreeMap<String, Entry>,
}

fn private(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.permissions().mode() & 0o077 != 0
    {
        return Err("not_admitted: private operator-owned path required".into());
    }
    Ok(())
}
fn hash_file(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > 512 * 1024 * 1024 {
        return Err("limit_exceeded: executable file".into());
    }
    let mut h = Sha256::new();
    let mut buf = [0; 65536];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > 512 * 1024 * 1024 {
            return Err("limit_exceeded: executable changed size".into());
        }
        h.update(&buf[..n]);
    }
    Ok(format!("sha256:{:x}", h.finalize()))
}
fn identity(pid: u32) -> Result<Identity> {
    if pid < 2 {
        return Err("not_admitted: invalid process".into());
    }
    let stat = String::from_utf8(read(&PathBuf::from(format!("/proc/{pid}/stat")), 8192)?)
        .map_err(|_| "process stat")?;
    let rest = stat.rsplit_once(')').ok_or("process stat")?.1;
    let fields = rest.split_whitespace().collect::<Vec<_>>();
    if fields.first().is_none_or(|s| *s == "Z" || *s == "X") {
        return Err("unavailable: process ended".into());
    }
    let start = fields.get(19).ok_or("process start time")?.to_string();
    let boot = String::from_utf8(read(Path::new("/proc/sys/kernel/random/boot_id"), 128)?)
        .map_err(|_| "boot identity")?
        .trim()
        .to_string();
    Ok(Identity { pid, boot, start })
}
fn pidfd(i: &Identity) -> Result<OwnedFd> {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, i.pid, 0) };
    if fd < 0 {
        return Err(format!("unavailable: {}", std::io::Error::last_os_error()));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    if identity(i.pid)? != *i {
        return Err("identity_mismatch: process generation changed".into());
    }
    Ok(fd)
}
fn ended(fd: &OwnedFd) -> Result<bool> {
    let mut p = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    if unsafe { libc::poll(&mut p, 1, 0) } < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(p.revents & libc::POLLIN != 0)
}

/// Runs on the SSH account, with configuration supplied by the operator rather
/// than by an environment request. Every public operation locks the same book.
pub struct RemoteOwner {
    path: PathBuf,
    config: RemoteConfig,
    fingerprint: String,
    store: Artifacts,
}
impl RemoteOwner {
    pub fn open(path: PathBuf) -> Result<Self> {
        private(&path)?;
        let config: RemoteConfig = serde_json::from_slice(&read(&path, 65536)?)
            .map_err(|_| "malformed: SSH helper configuration")?;
        if !config.root.is_absolute()
            || !config.executable.is_absolute()
            || !config.runtime.is_absolute()
            || config.arguments.len() > 64
            || config
                .arguments
                .iter()
                .any(|a| a.len() > 4096 || a.contains('\0'))
            || config.account.is_empty()
            || config.account.len() > 255
            || config.executable_digest.len() != 71
        {
            return Err("malformed: SSH helper configuration".into());
        }
        let store = Artifacts::new(config.root.clone())?;
        private(&config.root)?;
        let fingerprint = digest_value(&serde_json::to_value(&config).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            config,
            fingerprint,
            store,
        })
    }
    fn component(&self) -> Value {
        json!({"v":"openagents.environment-native-executable.v1","requires":[],"executable":self.config.executable_digest,"configuration":self.fingerprint})
    }
    pub fn profile(&self) -> Value {
        let component = self.component();
        let bytes = jcs(&component).expect("fixed executable descriptor");
        json!({"configuration":self.fingerprint,"provider":format!("{BINDING}/{}",&self.fingerprint[7..]),"component":component,"artifact":workspace_host::artifact::reference(&bytes,"application/json",Some("openagents.environment-native-executable.v1"))})
    }
    fn lock(&self) -> Result<workspace_host::artifact::Lock> {
        let start = Instant::now();
        loop {
            match self.store.lock() {
                Ok(lock) => return Ok(lock),
                Err(e) => {
                    if start.elapsed() > Duration::from_secs(3) {
                        return Err(e);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
    fn book(&self) -> Result<Book> {
        let p = self.config.root.join("processes.json");
        if !p.exists() {
            return Ok(Book {
                config: self.fingerprint.clone(),
                ..Default::default()
            });
        }
        let b: Book =
            serde_json::from_slice(&read(&p, LIMIT)?).map_err(|_| "malformed: SSH process book")?;
        if b.config != self.fingerprint {
            return Err("identity_mismatch: SSH operator configuration changed".into());
        }
        Ok(b)
    }
    fn save(&self, b: &Book) -> Result<()> {
        let bytes = serde_json::to_vec(b).map_err(|e| e.to_string())?;
        if bytes.len() > LIMIT {
            return Err("limit_exceeded: SSH process book".into());
        }
        atomic(&self.config.root.join("processes.json"), &bytes)
    }
    fn verify_binary(&self) -> Result<()> {
        if hash_file(&self.config.executable)? != self.config.executable_digest {
            return Err("identity_mismatch: installed executable changed".into());
        }
        Ok(())
    }
    fn runtime(&self) -> Result<Option<Identity>> {
        let bytes = match read(&self.config.runtime, 4096) {
            Ok(b) => b,
            Err(e) => {
                if !self.config.runtime.exists() {
                    return Ok(None);
                }
                return Err(e);
            }
        };
        let text = std::str::from_utf8(&bytes).map_err(|_| "malformed: host runtime")?;
        let mut r = json!({});
        for line in text.lines() {
            let (k, v) = line.split_once('=').ok_or("malformed: host runtime")?;
            if r.get(k).is_some() {
                return Err("malformed: duplicate runtime field".into());
            }
            r[k] = if k == "pid" || k == "port" {
                json!(v.parse::<u64>().map_err(|_| "runtime number")?)
            } else {
                json!(v)
            };
        }
        if r["port"].as_u64().is_none_or(|n| n == 0 || n > 65535) {
            return Err("malformed: runtime port".into());
        }
        if r["schema"] != "openagents.coder.host-runtime.v1"
            && r["v"] != "openagents.coder.host-runtime.v1"
        {
            return Err("identity_mismatch: host runtime schema".into());
        }
        let pid = u32::try_from(r["pid"].as_u64().ok_or("runtime process")?)
            .map_err(|_| "runtime process")?;
        match identity(pid) {
            Ok(i) => {
                let fd = pidfd(&i)?;
                if hash_file(&PathBuf::from(format!("/proc/{pid}/exe")))?
                    != self.config.executable_digest
                    || ended(&fd)?
                {
                    return Err("identity_mismatch: runtime executable changed".into());
                }
                Ok(Some(i))
            }
            Err(_) => Ok(None),
        }
    }
    fn resource(&self, key: &str, e: &Entry) -> Result<Realized> {
        let i = e
            .identity
            .as_ref()
            .ok_or("unknown: process identity not recorded")?;
        let resource = format!("pid:{}", i.pid);
        let ownership=jcs(&json!({"v":"openagents.environment-ownership.v1","requires":[],"provider":format!("{BINDING}/{}",&self.fingerprint[7..]),"account":self.config.account,"resource":resource,"generation":e.generation,"key":key,"observed_at":e.created_at,"meta":{"process":i,"configuration":self.fingerprint,"executable":self.config.executable_digest}})).map_err(|e|e.to_string())?;
        Ok(Realized {
            resource,
            generation: e.generation.clone(),
            account: self.config.account.clone(),
            ownership,
        })
    }
    fn observe(&self, key: &str, b: &Book) -> Result<Value> {
        let Some(e) = b.entries.get(key) else {
            return Ok(
                json!({"status":"unknown","reason":"original create intent is not recorded"}),
            );
        };
        if e.failed || e.stopped {
            return Ok(json!({"status":"absent"}));
        }
        let Some(i) = &e.identity else {
            return Ok(
                json!({"status":"unknown","reason":"original process identity is not recorded"}),
            );
        };
        let fd = match pidfd(i) {
            Ok(fd) => fd,
            Err(_) => {
                return Ok(
                    json!({"status":"unknown","reason":"original process is no longer observable"}),
                );
            }
        };
        if self.runtime()?.as_ref() != Some(i) || ended(&fd)? {
            return Ok(json!({"status":"unknown","reason":"original process is not ready"}));
        }
        Ok(json!({"status":"found","resource":self.resource(key,e)?}))
    }
    fn create(&self, key: &str) -> Result<Value> {
        let guard = self.lock()?;
        let mut b = self.book()?;
        if b.entries.contains_key(key) {
            return self.observe(key, &b);
        }
        self.verify_binary()?;
        // Refuse before launch on kernels without process descriptors.
        let _capability = pidfd(&identity(std::process::id())?)?;
        if self.runtime()?.is_some() || b.entries.values().any(|e| !e.failed && !e.stopped) {
            return Ok(
                json!({"status":"refused","reason":"a host or unresolved allocation already exists"}),
            );
        }
        if b.entries.len() >= 256 {
            return Err("limit_exceeded: SSH process retention".into());
        }
        b.entries.insert(
            key.into(),
            Entry {
                generation: token(),
                created_at: crate::now(),
                identity: None,
                failed: false,
                stopping: false,
                stopped: false,
            },
        );
        self.save(&b)?;
        let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        command
            .args(["ssh-child", "--config"])
            .arg(&self.path)
            .args(["--request", key])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Detach only the owned child. Dropping SSH never stops the resource.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn();
        if child.is_err() {
            b.entries.get_mut(key).ok_or("create entry")?.failed = true;
            self.save(&b)?;
            return Ok(json!({"status":"refused","reason":"host process could not start"}));
        }
        drop(child);
        drop(guard);
        let started = Instant::now();
        loop {
            let observed = {
                let _g = self.lock()?;
                self.observe(key, &self.book()?)?
            };
            if observed["status"] != "unknown" || started.elapsed() > Duration::from_secs(3) {
                return Ok(observed);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    pub fn child(&self, key: &str) -> Result<()> {
        let guard = self.lock()?;
        let mut b = self.book()?;
        let entry = b
            .entries
            .get_mut(key)
            .ok_or("not_admitted: no original create intent")?;
        if entry.identity.is_some() || entry.failed || entry.stopping || entry.stopped {
            return Err("conflict: original launch already attempted".into());
        }
        // Keep the exact verified executable open through exec; replacing its
        // pathname after this point cannot substitute another executable.
        let executable = File::open(&self.config.executable).map_err(|e| e.to_string())?;
        let executable_path = PathBuf::from(format!("/proc/self/fd/{}", executable.as_raw_fd()));
        if hash_file(&executable_path)? != self.config.executable_digest {
            return Err("identity_mismatch: installed executable changed".into());
        }
        entry.identity = Some(identity(std::process::id())?);
        self.save(&b)?;
        drop(guard);
        let error = Command::new(&executable_path)
            .args(&self.config.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .exec();
        let _g = self.lock()?;
        let mut b = self.book()?;
        b.entries.get_mut(key).ok_or("create entry")?.failed = true;
        self.save(&b)?;
        Err(format!("unavailable: {error}"))
    }
    fn exact<'a>(&self, resource: &Realized, b: &'a Book) -> Result<(&'a str, &'a Entry)> {
        if resource.account != self.config.account {
            return Err("not_admitted: SSH account differs".into());
        }
        for (key, e) in &b.entries {
            if e.identity.is_some() && self.resource(key, e)? == *resource {
                return Ok((key, e));
            }
        }
        Err("identity_mismatch: exact owned SSH generation required".into())
    }
    fn external(&self, i: &Identity) -> Result<Realized> {
        let key = digest_value(&json!({"process":i,"configuration":self.fingerprint}))
            .map_err(|e| e.to_string())?[7..]
            .to_string();
        self.resource(
            &key,
            &Entry {
                generation: key.clone(),
                created_at: 0,
                identity: Some(i.clone()),
                failed: false,
                stopping: false,
                stopped: false,
            },
        )
    }
    fn stop(&self, resource: &Realized, recovery: bool) -> Result<Value> {
        let _g = self.lock()?;
        let mut b = self.book()?;
        let (key, e) = self.exact(resource, &b)?;
        let key = key.to_string();
        let i = e.identity.clone().ok_or("process")?;
        if e.stopped {
            return Ok(json!({"status":"stopped"}));
        }
        let fd = match pidfd(&i) {
            Ok(fd) => Some(fd),
            Err(_) => None,
        };
        if recovery || e.stopping {
            if !e.stopping {
                return Ok(json!({"status":"unknown","reason":"no original stop intent"}));
            }
            // A different boot/start proves the original process ended. It says
            // nothing about the replacement, whose runtime and files stay intact.
            let gone = match &fd {
                Some(fd) => ended(fd)?,
                None => identity(i.pid)
                    .map(|other| other != i)
                    .unwrap_or_else(|_| !PathBuf::from(format!("/proc/{}", i.pid)).exists()),
            };
            if gone {
                b.entries.get_mut(&key).ok_or("entry")?.stopped = true;
                self.save(&b)?;
                return Ok(json!({"status":"stopped"}));
            }
            return Ok(json!({"status":"unknown","reason":"original stop remains unconfirmed"}));
        }
        let fd = fd.ok_or("unknown: original process cannot be addressed")?;
        if self.runtime()?.as_ref() != Some(&i) {
            return Err("identity_mismatch: current host differs from lease".into());
        }
        b.entries.get_mut(&key).ok_or("entry")?.stopping = true;
        self.save(&b)?;
        if unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                libc::SIGTERM,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        } < 0
        {
            return Err("unknown: process stop response was lost".into());
        }
        let start = Instant::now();
        while !ended(&fd)? && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(20));
        }
        if !ended(&fd)? {
            return Ok(json!({"status":"unknown","reason":"original stop remains unconfirmed"}));
        }
        b.entries.get_mut(&key).ok_or("entry")?.stopped = true;
        self.save(&b)?;
        Ok(json!({"status":"stopped"}))
    }
    pub fn handle(&self, input: &Value) -> Result<Value> {
        if input["configuration"] != self.fingerprint {
            return Err("identity_mismatch: pinned SSH configuration differs".into());
        }
        if input["account"] != self.config.account {
            return Err("not_admitted: SSH account differs".into());
        }
        match input["operation"].as_str().ok_or("operation")? {
            "create" | "lookup" => {
                let key = input["key"].as_str().ok_or("create key")?;
                if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("malformed: create key".into());
                }
                if input["operation"] == "create" {
                    self.create(key)
                } else {
                    let _g = self.lock()?;
                    self.observe(key, &self.book()?)
                }
            }
            "observe" => {
                let _g = self.lock()?;
                let b = self.book()?;
                let Some(i) = self.runtime()? else {
                    return Ok(json!({"status":"unknown"}));
                };
                for (key, e) in &b.entries {
                    if e.identity.as_ref() == Some(&i) && !e.stopped && !e.failed {
                        return Ok(
                            json!({"status":"found","resource":self.resource(key,e)?,"ownership":"managed","profile":self.profile()}),
                        );
                    }
                }
                Ok(
                    json!({"status":"found","resource":self.external(&i)?,"ownership":"external","profile":self.profile()}),
                )
            }
            "destroy" | "reconcile" => {
                let resource: Realized =
                    serde_json::from_value(input["resource"].clone()).map_err(|_| "resource")?;
                self.stop(&resource, input["operation"] == "reconcile")
            }
            "adopt" | "materialize" | "collect" => {
                let _g = self.lock()?;
                let b = self.book()?;
                let resource: Realized =
                    serde_json::from_value(input["resource"].clone()).map_err(|_| "resource")?;
                let i = self.runtime()?.ok_or("unavailable: host not ready")?;
                let external = self.external(&i)?;
                let owned = self.exact(&resource, &b).is_ok_and(|(_, e)| {
                    e.identity.as_ref() == Some(&i) && !e.stopping && !e.stopped && !e.failed
                });
                if !owned && resource != external {
                    return Err("identity_mismatch: observed host differs".into());
                }
                if input["operation"] == "collect" {
                    let path = self.config.root.join("outputs");
                    let mut files = Vec::new();
                    let mut remaining = 131072;
                    if path.exists() {
                        private(&path)?;
                        for file in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                            let file = file.map_err(|e| e.to_string())?;
                            if files.len() >= 64
                                || !file.file_type().map_err(|e| e.to_string())?.is_file()
                            {
                                return Err("limit_exceeded: retained SSH outputs".into());
                            }
                            let name = file.file_name().into_string().map_err(|_| "output name")?;
                            if name.is_empty()
                                || name.len() > 64
                                || !name
                                    .bytes()
                                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                            {
                                return Err("malformed: output name".into());
                            }
                            let bytes = read(&file.path(), remaining)?;
                            remaining -= bytes.len();
                            files.push((name, bytes));
                        }
                    }
                    files.sort_by(|a, b| a.0.cmp(&b.0));
                    return Ok(json!({"files":files}));
                }
                Ok(
                    json!({"status":"found","resource":resource,"executable":self.config.executable_digest,"component":self.component()}),
                )
            }
            _ => Err("unsupported_feature: SSH operation".into()),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    pub destination: String,
    pub helper: PathBuf,
    pub configuration: PathBuf,
    pub configuration_digest: String,
}
/// Native SSH provider. The remote helper and its private configuration are
/// installed by the operator; caller input cannot replace either one.
pub struct NativeSsh {
    config: ClientConfig,
    ssh: PathBuf,
}
impl NativeSsh {
    pub fn new(config: ClientConfig) -> Result<Self> {
        coder_ssh::Destination::parse(&config.destination).map_err(|e| e.to_string())?;
        for p in [&config.helper, &config.configuration] {
            let s = p.to_str().ok_or("SSH path")?;
            if !p.is_absolute()
                || s.len() > 1024
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
            {
                return Err("malformed: absolute SSH helper path required".into());
            }
        }
        if !config.configuration_digest.starts_with("sha256:")
            || config.configuration_digest.len() != 71
            || !config.configuration_digest[7..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err("malformed: SSH configuration digest".into());
        }
        Ok(Self {
            config,
            ssh: "ssh".into(),
        })
    }
    /// Select an operator-owned SSH executable, for an isolated transport or a
    /// system installation that does not place SSH on PATH.
    pub fn program(mut self, path: PathBuf) -> Self {
        self.ssh = path;
        self
    }
    fn call(&self, mut input: Value) -> Result<Value> {
        input["account"] = json!(self.config.destination);
        input["configuration"] = json!(self.config.configuration_digest);
        let bytes = jcs(&input).map_err(|e| e.to_string())?;
        if bytes.len() > 2 * LIMIT {
            return Err("limit_exceeded: SSH request".into());
        }
        let remote = format!(
            "exec {} ssh-remote --config {}",
            self.config.helper.display(),
            self.config.configuration.display()
        );
        let mut c = Command::new(&self.ssh)
            .args([
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "ControlMaster=no",
                "-o",
                "ControlPath=none",
                "-o",
                "ForwardAgent=no",
                "-o",
                "ForwardX11=no",
                "-o",
                "ClearAllForwardings=yes",
                "-o",
                "ConnectTimeout=10",
                "--",
                &self.config.destination,
                &remote,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("unknown: {e}"))?;
        let mut stdout = c.stdout.take().ok_or("SSH output")?;
        let reader = std::thread::spawn(move || {
            let mut b = Vec::new();
            stdout
                .by_ref()
                .take((LIMIT + 1) as u64)
                .read_to_end(&mut b)
                .map(|_| b)
        });
        let mut stdin = c.stdin.take().ok_or("SSH input")?;
        let writer = std::thread::spawn(move || stdin.write_all(&bytes));
        let start = Instant::now();
        let status = loop {
            match c.try_wait().map_err(|e| e.to_string())? {
                Some(s) => break s,
                None => {
                    if start.elapsed() > Duration::from_secs(15) {
                        let _ = c.kill();
                        let _ = c.wait();
                        return Err("unknown: SSH reply was not received".into());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        };
        writer
            .join()
            .map_err(|_| "SSH input")?
            .map_err(|_| "unknown: SSH request was interrupted")?;
        let bytes = reader
            .join()
            .map_err(|_| "SSH output")?
            .map_err(|_| "unknown: SSH output was interrupted")?;
        if (!status.success() && status.code() != Some(1)) || bytes.len() > LIMIT {
            return Err("unknown: SSH helper did not return a bounded result".into());
        }
        let value: Value = nostr::contracts::parse_strict(&bytes)
            .map_err(|_| "unknown: invalid SSH helper reply")?;
        if value["error"].is_string() {
            return Err(value["error"].as_str().ok_or("SSH error")?.into());
        }
        Ok(value)
    }
    pub fn observe(&self) -> Result<Value> {
        self.call(json!({"operation":"observe"}))
    }
    fn resource(value: &Value) -> Result<Realized> {
        serde_json::from_value(value["resource"].clone())
            .map_err(|_| "unknown: SSH ownership response differs".into())
    }
    fn receipt(&self, resource: &Realized) -> Vec<u8> {
        jcs(&json!({"provider":BINDING,"account":self.config.destination,"resource":resource.resource,"generation":resource.generation,"observed":"stopped"})).unwrap_or_default()
    }
}
impl Provider for NativeSsh {
    fn binding(&self) -> String {
        format!("{BINDING}/{}", &self.config.configuration_digest[7..])
    }
    fn create(&mut self, account: &str, key: &str) -> Create {
        if account != self.config.destination {
            return Create::Refused("not_admitted: SSH account differs".into());
        }
        match self.call(json!({"operation":"create","key":key})) {
            Ok(v) if v["status"] == "found" => Self::resource(&v)
                .map(|r| Create::Created(r, Cost::Unknown))
                .unwrap_or_else(Create::Unknown),
            Ok(v) if v["status"] == "refused" => {
                Create::Refused(v["reason"].as_str().unwrap_or("host unavailable").into())
            }
            Ok(_) => Create::Unknown("original SSH launch remains unconfirmed".into()),
            Err(e) => Create::Unknown(e),
        }
    }
    fn lookup(&mut self, account: &str, key: &str) -> Lookup {
        if account != self.config.destination {
            return Lookup::Unknown("SSH account differs".into());
        }
        match self.call(json!({"operation":"lookup","key":key})) {
            Ok(v) if v["status"] == "found" => Self::resource(&v)
                .map(Lookup::Found)
                .unwrap_or_else(Lookup::Unknown),
            Ok(v) if v["status"] == "absent" => Lookup::Absent,
            _ => Lookup::Unknown("original SSH launch remains unconfirmed".into()),
        }
    }
    fn adopt(
        &mut self,
        account: &str,
        resource: &str,
        generation: &str,
        ownership: &[u8],
    ) -> Result<Realized> {
        if account != self.config.destination {
            return Err("not_admitted: SSH account differs".into());
        }
        let r = Realized {
            resource: resource.into(),
            generation: generation.into(),
            account: account.into(),
            ownership: ownership.to_vec(),
        };
        let v = self.call(json!({"operation":"adopt","resource":r}))?;
        let observed = Self::resource(&v)?;
        if observed != r {
            return Err("identity_mismatch: SSH adoption differs".into());
        }
        Ok(r)
    }
    fn materialize(
        &mut self,
        resource: &Realized,
        components: &[(String, Vec<u8>)],
    ) -> Result<Vec<(String, String)>> {
        let observed = self.call(json!({"operation":"materialize","resource":resource}))?;
        let descriptor = jcs(&observed["component"]).map_err(|e| e.to_string())?;
        Ok(components
            .iter()
            .filter(|(_, b)| *b == descriptor)
            .map(|(id, b)| (id.clone(), digest_bytes(b)))
            .collect())
    }

    fn collect(&mut self, resource: &Realized) -> Result<Vec<(String, Vec<u8>)>> {
        let observed = self.call(json!({"operation":"collect","resource":resource}))?;
        serde_json::from_value(observed["files"].clone())
            .map_err(|_| "unknown: retained SSH output response differs".into())
    }
    fn destroy(&mut self, resource: &Realized) -> Teardown {
        match self.call(json!({"operation":"destroy","resource":resource})) {
            Ok(v) if v["status"] == "stopped" => {
                Teardown::Confirmed(self.receipt(resource), Cost::Unknown)
            }
            Ok(_) => Teardown::Unknown("original SSH stop remains unconfirmed".into()),
            Err(e) => Teardown::Unknown(e),
        }
    }
    fn reconcile_teardown(&mut self, resource: &Realized, action: &str) -> Teardown {
        if action == "detach" {
            return self.detach(resource);
        }
        match self.call(json!({"operation":"reconcile","resource":resource})) {
            Ok(v) if v["status"] == "stopped" => {
                Teardown::Confirmed(self.receipt(resource), Cost::Unknown)
            }
            _ => Teardown::Unknown("original SSH stop remains unconfirmed".into()),
        }
    }
    fn detach(&mut self, resource: &Realized) -> Teardown {
        if resource.account != self.config.destination {
            return Teardown::Failed("SSH account differs".into());
        }
        Teardown::Confirmed(jcs(&json!({"provider":BINDING,"resource":resource.resource,"generation":resource.generation,"observed":"detached"})).unwrap_or_default(),Cost::Measured(0))
    }
}
