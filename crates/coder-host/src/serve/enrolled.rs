//! Scoped logical computers. Physical hosts and user files are never deleted.

use super::workspace_files::WorkspaceFiles;
use crate::authority::Authority;
use coder_access::enrolled::{Action, Answer, Call, Command, MAX_OUTPUT, Progress, Root, Scope};
use coder_access::protocol::{DeviceState, random_id};
use coder_access::{Code, Right};
use coder_connect::store::Store;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;

const VERSION: &str = "openagents.host-enrolled.v1";
const MAX_COMPUTERS: usize = 128;
const MAX_COMMANDS: usize = 4096;
const MAX_LIVE: usize = 4;
const OUTPUT_BUDGET: usize = 8 * 1024 * 1024;
const SHORTENED: &[u8] = b"\nOutput was shortened.\n";

pub(super) struct Manager {
    serial: Mutex<()>,
    closing: AtomicBool,
    directory: PathBuf,
    roots: BTreeMap<String, PathBuf>,
    authority: Arc<Authority>,
    boot: String,
    live: Mutex<BTreeMap<String, watch::Sender<bool>>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    computers: BTreeMap<String, Computer>,
    commands: BTreeMap<String, Recorded>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Computer {
    device: String,
    account: String,
    epoch: u64,
    root: Root,
    attached: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recorded {
    computer: String,
    command: String,
    digest: String,
    source_digest: String,
    boot: String,
    progress: Progress,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Manager {
    pub(super) fn new(
        access: &Path,
        roots: BTreeMap<String, PathBuf>,
        authority: Arc<Authority>,
    ) -> Result<Arc<Self>, Code> {
        let directory = access.join("enrolled");
        let fresh = !directory.exists();
        let mut store = Store::open(&directory, fresh).map_err(unavailable)?;
        let book = store.load::<Book>().map_err(unavailable)?;
        match book {
            None if fresh => store
                .save(&Book {
                    v: VERSION.into(),
                    computers: BTreeMap::new(),
                    commands: BTreeMap::new(),
                })
                .map_err(unavailable)?,
            Some(book) if valid(&book) => {}
            _ => return Err(Code::Unavailable),
        }
        Ok(Arc::new(Self {
            serial: Mutex::new(()),
            closing: AtomicBool::new(false),
            directory,
            roots,
            authority,
            boot: random_id(),
            live: Mutex::new(BTreeMap::new()),
            tasks: Mutex::new(Vec::new()),
        }))
    }

    fn open(&self) -> Result<(Store, Book), Code> {
        let store = Store::open(&self.directory, false).map_err(unavailable)?;
        let book = store
            .load::<Book>()
            .map_err(unavailable)?
            .filter(valid)
            .ok_or(Code::Unavailable)?;
        Ok((store, book))
    }

    fn files(&self, root: &Root) -> Result<WorkspaceFiles, Code> {
        let configured = self.roots.get(&root.workspace).ok_or(Code::Forbidden)?;
        let resolved = configured.canonicalize().map_err(|_| Code::Unavailable)?;
        if resolved != Path::new(&root.path) {
            return Err(Code::Stale);
        }
        // A workspace must not expose the host's enrollment and command book.
        // Otherwise its command right could be used to rewrite its own grant.
        let state = self
            .directory
            .parent()
            .ok_or(Code::Unavailable)?
            .canonicalize()
            .map_err(|_| Code::Unavailable)?;
        if state.starts_with(&resolved) || resolved.starts_with(&state) {
            return Err(Code::Forbidden);
        }
        let files = WorkspaceFiles::open(Path::new(&root.path))?;
        if files.root() != Path::new(&root.path) || files.identity()? != root.identity {
            return Err(Code::Stale);
        }
        Ok(files)
    }

    pub(super) fn call(
        self: &Arc<Self>,
        device: &str,
        epoch: u64,
        scope: &Scope,
        call: &Call,
    ) -> Result<Answer, Code> {
        let _serial = self.serial.lock().unwrap();
        if self.closing.load(Ordering::SeqCst) {
            return Err(Code::Unavailable);
        }
        call.validate().map_err(|e| e.code)?;
        if call.account != scope.account {
            return Err(Code::Forbidden);
        }
        let (mut store, mut book) = self.open()?;
        let id = call.action.computer();
        if let Action::Attach { workspace, .. } = &call.action {
            let root = scope
                .roots
                .iter()
                .find(|r| r.workspace == *workspace)
                .ok_or(Code::Forbidden)?;
            self.files(root)?;
            if let Some(old) = book.computers.get(id) {
                admit(old, device, epoch, scope)?;
                if old.root != *root {
                    return Err(Code::Conflict);
                }
                return Ok(Answer::Computer {
                    computer: id.into(),
                    attached: old.attached,
                });
            }
            if book.computers.len() >= MAX_COMPUTERS {
                return Err(Code::Bounds);
            }
            book.computers.insert(
                id.into(),
                Computer {
                    device: device.into(),
                    account: scope.account.clone(),
                    epoch,
                    root: root.clone(),
                    attached: true,
                },
            );
            store.save(&book).map_err(unavailable)?;
            return Ok(Answer::Computer {
                computer: id.into(),
                attached: true,
            });
        }
        let computer = book.computers.get(id).ok_or(Code::Forbidden)?.clone();
        admit(&computer, device, epoch, scope)?;
        if let Action::Observe { .. } = &call.action {
            if computer.attached {
                self.files(&computer.root)?;
            }
            return Ok(Answer::Computer {
                computer: id.into(),
                attached: computer.attached,
            });
        }
        if let Action::Detach { .. } = &call.action {
            book.computers.get_mut(id).expect("bound computer").attached = false;
            store.save(&book).map_err(unavailable)?;
            return Ok(Answer::Computer {
                computer: id.into(),
                attached: false,
            });
        }
        if !computer.attached {
            return Err(Code::Revoked);
        }
        match &call.action {
            Action::File { file, .. } => Ok(Answer::File {
                file: self.files(&computer.root)?.answer(file)?,
            }),
            Action::Start { command, .. } => {
                let key = command_key(id, &command.id);
                if let Some(record) = book.commands.get(&key) {
                    if record.digest != command.digest {
                        return Err(Code::Conflict);
                    }
                    return Ok(answer(record, &self.boot, 0, 0, 0));
                }
                self.files(&computer.root)?;
                if book.commands.len() >= MAX_COMMANDS
                    || book
                        .commands
                        .values()
                        .filter(|r| matches!(r.progress, Progress::Running))
                        .count()
                        >= MAX_LIVE
                {
                    return Err(Code::Bounds);
                }
                let (job, boundary) = self.prepare(&computer.root, command)?;
                // This save precedes process creation. Lost or uncertain starts
                // retain their identity and cannot become a second process.
                book.commands.insert(
                    key.clone(),
                    Recorded {
                        computer: id.into(),
                        command: command.id.clone(),
                        digest: command.digest.clone(),
                        source_digest: command.source_digest.clone(),
                        boot: self.boot.clone(),
                        progress: Progress::Running,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    },
                );
                store.save(&book).map_err(unavailable)?;
                drop(store);
                let live = match job.start(supervise::Input::Null) {
                    Ok(live) => live,
                    Err(_) => {
                        if let Ok((mut store, mut book)) = self.open() {
                            if let Some(record) = book.commands.get_mut(&key) {
                                record.progress = Progress::Lost;
                            }
                            let _ = store.save(&book);
                        }
                        return Err(Code::Unavailable);
                    }
                };
                let (stop, cancel) = watch::channel(false);
                self.live.lock().unwrap().insert(key.clone(), stop);
                let manager = self.clone();
                let task_key = key.clone();
                let task = tokio::spawn(async move {
                    manager
                        .run(task_key, computer, live, cancel, boundary)
                        .await;
                });
                self.tasks.lock().unwrap().push(task);
                let (_, current) = self.open()?;
                Ok(answer(
                    current.commands.get(&key).ok_or(Code::Unavailable)?,
                    &self.boot,
                    0,
                    0,
                    0,
                ))
            }
            Action::Read {
                command,
                stdout,
                stderr,
                max_bytes,
                ..
            } => {
                self.files(&computer.root)?;
                let record = book
                    .commands
                    .get(&command_key(id, command))
                    .ok_or(Code::Unavailable)?;
                if *stdout > record.stdout.len() as u64 || *stderr > record.stderr.len() as u64 {
                    return Err(Code::Bounds);
                }
                Ok(answer(
                    record,
                    &self.boot,
                    *stdout as usize,
                    *stderr as usize,
                    *max_bytes,
                ))
            }
            Action::Stop { command, .. } => {
                let key = command_key(id, command);
                let record = book.commands.get(&key).ok_or(Code::Unavailable)?;
                if record.boot == self.boot
                    && let Some(stop) = self.live.lock().unwrap().get(&key)
                {
                    let _ = stop.send(true);
                }
                Ok(answer(record, &self.boot, 0, 0, 0))
            }
            _ => Err(Code::Malformed),
        }
    }

    fn prepare(
        &self,
        root: &Root,
        command: &Command,
    ) -> Result<(supervise::Job, coder_boundary::boundary::Held), Code> {
        command.validate().map_err(|e| e.code)?;
        let files = self.files(root)?;
        let cwd = files
            .root()
            .join(&command.cwd)
            .canonicalize()
            .map_err(|_| Code::Unavailable)?;
        if !cwd.starts_with(files.root()) || !cwd.is_dir() {
            return Err(Code::Forbidden);
        }
        let boundary = coder_boundary::boundary::Boundary::writing(files.root())
            .confining_reads()
            .offline()
            .owned_scratch_under(&self.directory)
            .build()
            .map_err(|_| Code::Unsupported)?;
        if boundary.checkout() != Some(files.root())
            || coder_access::enrolled::root_identity(files.root()).map_err(|e| e.code)?
                != root.identity
        {
            return Err(Code::Stale);
        }
        let mut process = boundary
            .command("/bin/sh", ["-c", &command.script])
            .map_err(|_| Code::Unsupported)?;
        process
            .current_dir(cwd)
            .env_clear()
            .env("HOME", files.root())
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LANG", "C");
        if let Some(scratch) = boundary.scratch() {
            process.env("TMPDIR", scratch);
        }
        process.envs(&command.env);
        let job = supervise::Job::from_command(process).bounded(
            supervise::Limits::within(Duration::from_secs(command.timeout_seconds))
                .keeping(MAX_OUTPUT)
                .memory(Some(512 * 1024 * 1024)),
        );
        Ok((job, boundary.hold()))
    }

    fn current(&self, computer: &Computer) -> bool {
        let now = crate::unix_time().unwrap_or(u64::MAX);
        self.authority.devices().is_some_and(|devices| {
            devices.iter().any(|entry| {
                entry.device == computer.device
                    && entry.epoch == computer.epoch
                    && entry.state == DeviceState::Active
                    && entry.expires_at > now
                    && entry.rights.contains(Right::Operate)
                    && entry.scope.as_ref().is_some_and(|scope| {
                        scope.account == computer.account && scope.roots.contains(&computer.root)
                    })
            })
        }) && self.files(&computer.root).is_ok()
    }

    async fn run(
        self: Arc<Self>,
        key: String,
        computer: Computer,
        live: supervise::Live,
        mut cancel: watch::Receiver<bool>,
        boundary: coder_boundary::boundary::Held,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let live = live;
        let stopped = loop {
            tokio::select! {
                _ = interval.tick() => {
                    if live.finished() { break live.wait().await; }
                    if !self.current(&computer) { break live.stop().await; }
                }
                _ = cancel.changed() => { break live.stop().await; }
            }
        };
        // Retain the filesystem boundary until the supervisor has acknowledged cleanup.
        drop(boundary);
        let progress = if stopped.group_clear {
            Progress::Exited {
                code: stopped
                    .ending
                    .code()
                    .map(i64::from)
                    .unwrap_or(if stopped.requested { 130 } else { 124 }),
                timed_out: matches!(stopped.ending, supervise::Ending::TimedOut),
            }
        } else {
            Progress::Lost
        };
        let _serial = self.serial.lock().unwrap();
        if let Ok((mut store, mut book)) = self.open() {
            let used: usize = book
                .commands
                .values()
                .map(|r| r.stdout.len() + r.stderr.len())
                .sum();
            if let Some(record) = book.commands.get_mut(&key)
                && record.boot == self.boot
            {
                let available = OUTPUT_BUDGET.saturating_sub(used);
                let out = stopped.rest.bytes;
                let err = stopped.stderr.text.into_bytes();
                let keep_out = out.len().min(MAX_OUTPUT).min(available);
                let keep_err = err
                    .len()
                    .min(MAX_OUTPUT - 64)
                    .min(available.saturating_sub(keep_out));
                record.stdout = out[..keep_out].to_vec();
                record.stderr = err[..keep_err].to_vec();
                if keep_out < out.len()
                    || keep_err < err.len()
                    || stopped.stdout_bytes > keep_out as u64
                    || stopped.stderr.bytes > keep_err as u64
                {
                    record.stderr.extend_from_slice(SHORTENED);
                }
                record.progress = progress;
                let _ = store.save(&book);
            }
        }
        self.live.lock().unwrap().remove(&key);
    }

    pub(super) async fn shutdown(&self) {
        let tasks = {
            let _serial = self.serial.lock().unwrap();
            self.closing.store(true, Ordering::SeqCst);
            for stop in self.live.lock().unwrap().values() {
                let _ = stop.send(true);
            }
            std::mem::take(&mut *self.tasks.lock().unwrap())
        };
        for task in tasks {
            let _ = task.await;
        }
    }
}

fn valid(book: &Book) -> bool {
    book.v == VERSION
        && book.computers.len() <= MAX_COMPUTERS
        && book.commands.len() <= MAX_COMMANDS
}
fn admit(computer: &Computer, device: &str, epoch: u64, scope: &Scope) -> Result<(), Code> {
    if computer.device != device
        || computer.epoch != epoch
        || computer.account != scope.account
        || !scope.roots.contains(&computer.root)
    {
        Err(Code::Forbidden)
    } else {
        Ok(())
    }
}
fn command_key(computer: &str, command: &str) -> String {
    format!("{computer}/{command}")
}
fn unavailable(_: coder_connect::Error) -> Code {
    Code::Unavailable
}
fn answer(record: &Recorded, boot: &str, stdout: usize, stderr: usize, max: usize) -> Answer {
    let progress = if record.boot != boot && matches!(record.progress, Progress::Running) {
        Progress::Lost
    } else {
        record.progress.clone()
    };
    let out = record.stdout.get(stdout..).unwrap_or_default();
    let err = record.stderr.get(stderr..).unwrap_or_default();
    let take_out = max.min(out.len());
    let take_err = max.saturating_sub(take_out).min(err.len());
    Answer::Command {
        command: record.command.clone(),
        digest: record.source_digest.clone(),
        progress,
        stdout: out[..take_out].to_vec(),
        stderr: err[..take_err].to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::client::prepare_redeem;
    use coder_access::host::{Host, Unconnected};
    use coder_access::protocol::HostInvitation;
    use coder_access::{RelayPolicy, Rights};
    use secp256k1::SecretKey;

    const RELAY: &str = "ws://127.0.0.1:9876";
    const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

    struct Fixture {
        _temp: tempfile::TempDir,
        root: PathBuf,
        access: PathBuf,
        authority: Arc<Authority>,
        scope: Scope,
        device: String,
        manager: Arc<Manager>,
        computer: String,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("project");
            std::fs::create_dir(&root).unwrap();
            let access = temp.path().join("access");
            let key = SecretKey::new(&mut secp256k1::rand::rng());
            let device = coder_reach::pubkey(&key);
            let host = Host::new(&access, POLICY);
            host.init(&coder_reach::pubkey(&SecretKey::new(
                &mut secp256k1::rand::rng(),
            )))
            .unwrap();
            let now = coder_access::unix_time().unwrap();
            let scope = Scope::local("acct_test", &[("project".into(), root.clone())]).unwrap();
            let code = host
                .invite_scoped(
                    RELAY,
                    Rights::parse_list("observe,operate").unwrap(),
                    scope.clone(),
                    now,
                    now + 3600,
                )
                .unwrap()
                .code;
            let invite = HostInvitation::parse(&code, now, POLICY).unwrap();
            let pending = prepare_redeem(&invite, &key, now, POLICY).unwrap();
            host.handle(&pending.event, RELAY, now, &mut Unconnected)
                .unwrap();
            let authority = Arc::new(Authority::open(host).unwrap());
            let manager = Manager::new(
                &access,
                BTreeMap::from([("project".into(), root.clone())]),
                authority.clone(),
            )
            .unwrap();
            let f = Self {
                _temp: temp,
                root,
                access,
                authority,
                scope,
                device,
                manager,
                computer: random_id(),
            };
            f.call(Action::Attach {
                computer: f.computer.clone(),
                workspace: "project".into(),
            })
            .unwrap();
            f
        }
        fn call(&self, action: Action) -> Result<Answer, Code> {
            self.manager.call(
                &self.device,
                0,
                &self.scope,
                &Call {
                    account: self.scope.account.clone(),
                    action,
                },
            )
        }
        fn command(&self, script: &str) -> Command {
            let mut command = Command {
                id: random_id(),
                script: script.into(),
                cwd: String::new(),
                env: BTreeMap::new(),
                timeout_seconds: 5,
                digest: String::new(),
                source_digest: random_id(),
            };
            command.digest = command.content_hash();
            command
        }
        fn start(&self, command: &Command) -> Result<Answer, Code> {
            self.call(Action::Start {
                computer: self.computer.clone(),
                command: command.clone(),
            })
        }
        async fn finished(&self, command: &Command) -> Answer {
            for _ in 0..100 {
                let answer = self
                    .call(Action::Read {
                        computer: self.computer.clone(),
                        command: command.id.clone(),
                        stdout: 0,
                        stderr: 0,
                        max_bytes: 8192,
                    })
                    .unwrap();
                if matches!(
                    answer,
                    Answer::Command {
                        progress: Progress::Running,
                        ..
                    }
                ) {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                } else {
                    return answer;
                }
            }
            panic!("command did not finish");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn retries_restart_and_detach_preserve_work_and_command_identity() {
        let f = Fixture::new();
        let command = f.command("printf x >> counter; printf done");
        f.start(&command).unwrap();
        f.start(&command).unwrap();
        let mut changed = command.clone();
        changed.script.push_str("; printf changed");
        changed.digest = changed.content_hash();
        assert_eq!(f.start(&changed), Err(Code::Conflict));
        let answer = f.finished(&command).await;
        assert!(
            matches!(answer, Answer::Command {progress:Progress::Exited {code:0,..},ref stdout,..} if stdout == b"done"),
            "{answer:?}"
        );
        assert_eq!(std::fs::read(f.root.join("counter")).unwrap(), b"x");
        let restarted = Manager::new(
            &f.access,
            BTreeMap::from([("project".into(), f.root.clone())]),
            f.authority.clone(),
        )
        .unwrap();
        let call = Call {
            account: f.scope.account.clone(),
            action: Action::Start {
                computer: f.computer.clone(),
                command: command.clone(),
            },
        };
        assert!(matches!(
            restarted.call(&f.device, 0, &f.scope, &call).unwrap(),
            Answer::Command {
                progress: Progress::Exited { .. },
                ..
            }
        ));
        assert_eq!(std::fs::read(f.root.join("counter")).unwrap(), b"x");
        f.call(Action::Detach {
            computer: f.computer.clone(),
        })
        .unwrap();
        assert!(matches!(
            f.call(Action::Attach {
                computer: f.computer.clone(),
                workspace: "project".into()
            })
            .unwrap(),
            Answer::Computer {
                attached: false,
                ..
            }
        ));
        assert_eq!(f.start(&f.command("touch forbidden")), Err(Code::Revoked));
        assert!(f.root.join("counter").exists());
        assert!(!f.root.join("forbidden").exists());
        assert_eq!(f.authority.devices().unwrap().len(), 1);
        f.manager.shutdown().await;
        restarted.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_revocation_and_boundary_keep_effects_in_the_workspace() {
        let f = Fixture::new();
        let outside = f.root.parent().unwrap().join("outside");
        std::fs::write(&outside, b"private").unwrap();
        std::os::unix::fs::symlink(&outside, f.root.join("escape")).unwrap();
        let denied = f.command("cat escape; printf bad > ../outside");
        f.start(&denied).unwrap();
        let result = f.finished(&denied).await;
        assert!(
            matches!(result,Answer::Command {ref stdout,..} if stdout.is_empty()),
            "{result:?}"
        );
        assert_eq!(std::fs::read(&outside).unwrap(), b"private");
        let command = f.command("printf started > marker; exec sleep 30");
        f.start(&command).unwrap();
        f.call(Action::Stop {
            computer: f.computer.clone(),
            command: command.id.clone(),
        })
        .unwrap();
        assert!(matches!(
            f.finished(&command).await,
            Answer::Command {
                progress: Progress::Exited { .. },
                ..
            }
        ));
        let command = f.command("exec sleep 30");
        f.start(&command).unwrap();
        f.authority.revoke(&f.device).unwrap();
        assert!(matches!(
            f.finished(&command).await,
            Answer::Command {
                progress: Progress::Exited { .. },
                ..
            }
        ));
        f.manager.shutdown().await;
    }

    #[tokio::test]
    async fn a_workspace_cannot_include_or_enter_the_host_access_store() {
        let f = Fixture::new();
        let parent = f.root.parent().unwrap().to_path_buf();
        let scope = Scope::local("acct_test", &[("project".into(), parent.clone())]).unwrap();
        let manager = Manager::new(
            &f.access,
            BTreeMap::from([("project".into(), parent)]),
            f.authority.clone(),
        )
        .unwrap();
        let call = Call {
            account: scope.account.clone(),
            action: Action::Attach {
                computer: random_id(),
                workspace: "project".into(),
            },
        };
        assert_eq!(
            manager.call(&f.device, 0, &scope, &call),
            Err(Code::Forbidden)
        );
        f.manager.shutdown().await;
        manager.shutdown().await;
    }

    #[tokio::test]
    async fn stale_roots_wrong_principals_and_interrupted_starts_fail_closed() {
        let f = Fixture::new();
        let op = Call {
            account: f.scope.account.clone(),
            action: Action::Observe {
                computer: f.computer.clone(),
            },
        };
        assert_eq!(
            f.manager.call(&random_id(), 0, &f.scope, &op),
            Err(Code::Forbidden)
        );
        assert_eq!(
            f.manager.call(&f.device, 1, &f.scope, &op),
            Err(Code::Forbidden)
        );
        let command = f.command("touch duplicate");
        let (mut store, mut book) = f.manager.open().unwrap();
        book.commands.insert(
            command_key(&f.computer, &command.id),
            Recorded {
                computer: f.computer.clone(),
                command: command.id.clone(),
                digest: command.digest.clone(),
                source_digest: command.source_digest.clone(),
                boot: random_id(),
                progress: Progress::Running,
                stdout: vec![],
                stderr: vec![],
            },
        );
        store.save(&book).unwrap();
        drop(store);
        assert!(matches!(
            f.start(&command).unwrap(),
            Answer::Command {
                progress: Progress::Lost,
                ..
            }
        ));
        assert!(!f.root.join("duplicate").exists());
        std::fs::rename(&f.root, f.root.with_extension("old")).unwrap();
        std::fs::create_dir(&f.root).unwrap();
        assert_eq!(f.call(op.action), Err(Code::Stale));
        assert_eq!(f.start(&f.command("touch escape")), Err(Code::Stale));
        f.manager.shutdown().await;
    }
}
