//! The remote script and the launcher against a fake `ssh`.
//!
//! The fake `ssh` records its arguments, answers `ssh -G`, optionally asks
//! for a password through `SSH_ASKPASS`, and runs the remote command in a
//! local shell with `HOME` set to a temporary "remote" home. The real
//! `~/.ssh` and `~/.openagents` are never read or written. Every host a test
//! starts is a `sleep` process that the fixture kills by its recorded
//! process identifier.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_ssh::{
    Arch, Artifact, Error, Install, Launcher, Os, Ownership, Release, Removal, Runner, Secret,
    Start,
};

#[path = "support/fake_ssh.rs"]
mod fake_ssh;

const SERVE: &[&str] = &["host", "serve", "--loopback"];
const INVITE: &[&str] = &["host", "invite"];
const HOST_PORT: u16 = 47001;
const PASSWORD: &str = "correct horse battery staple";

const FAKE_CODER: &str = r#"#!/bin/sh
case "${1:-}" in
  --version) echo 'coder 0.0.0-fake'; exit 0 ;;
  host)
    case "${2:-}" in
      serve)
        d="$HOME/.openagents/host"
        mkdir -p "$d"
        printf 'schema=openagents.coder.host-runtime.v1\npid=%s\nport=%s\n' "$$" 47001 > "$d/runtime.tmp"
        mv "$d/runtime.tmp" "$d/runtime"
        sleep 600 &
        child=$!
        trap 'kill "$child" 2>/dev/null; exit 0' HUP INT TERM
        wait "$child"
        exit 0 ;;
      invite) echo 'oa-invite:abc123'; exit 0 ;;
    esac ;;
esac
exit 2
"#;

const BROKEN_CODER: &str = "#!/bin/sh\nexit 3\n";

fn local_platform() -> (Os, Arch) {
    let os = match std::env::consts::OS {
        "macos" => Os::Macos,
        "linux" => Os::Linux,
        other => panic!("no fake platform for {other}"),
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => Arch::Aarch64,
        "x86_64" => Arch::X86_64,
        other => panic!("no fake architecture for {other}"),
    };
    (os, arch)
}

fn sha256_file(path: &Path) -> String {
    fake_ssh::sha256_file(path)
}

fn alive(pid: u32) -> bool {
    // SAFETY: signal zero sends nothing.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn kill(pid: u32) {
    if pid > 1 {
        // A fake host stops its own `sleep` child on SIGTERM.
        // SAFETY: the identifier was recorded by this test's own fake host.
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if !wait_dead(pid) {
            // SAFETY: as above.
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }
}

fn wait_dead(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn node_name() -> String {
    let output = Command::new("uname").arg("-n").output().unwrap();
    String::from_utf8_lossy(&output.stdout)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(64)
        .collect()
}

struct Fixture {
    _temp: Option<tempfile::TempDir>,
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        Self::with_shell("sh")
    }

    fn with_shell(shell: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().canonicalize().unwrap();
        let fixture = Fixture {
            _temp: Some(temp),
            dir,
        };
        std::fs::create_dir_all(fixture.home()).unwrap();
        std::fs::create_dir_all(fixture.dir.join("bin")).unwrap();
        fixture.write_archive(FAKE_CODER, "archive.tar.gz");
        fixture.write_shim(shell);
        fixture
    }

    /// Reopens a fixture another process created, without owning cleanup.
    fn open(dir: PathBuf) -> Self {
        Fixture { _temp: None, dir }
    }

    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }

    fn write_archive(&self, coder: &str, name: &str) -> PathBuf {
        fake_ssh::archive(&self.dir, coder, name)
    }

    fn write_shim(&self, shell: &str) {
        fake_ssh::shim(&self.dir, &self.home(), shell);
    }

    fn shim(&self) -> PathBuf {
        self.dir.join("ssh")
    }

    fn fake_uname(&self, os: &str, arch: &str) {
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\n  -s) echo {os} ;;\n  -m) echo {arch} ;;\n  *) exec /usr/bin/uname \"$@\" ;;\nesac\n"
        );
        let path = self.dir.join("bin/uname");
        std::fs::write(&path, script).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn release_for(&self, archive: &Path, sha: &str) -> Release {
        let (os, arch) = local_platform();
        Release::new(vec![Artifact {
            os,
            arch,
            sha256: sha.to_string(),
            archive: archive.to_path_buf(),
        }])
        .unwrap()
    }

    fn release(&self) -> Release {
        let archive = self.dir.join("archive.tar.gz");
        let sha = sha256_file(&archive);
        self.release_for(&archive, &sha)
    }

    fn runner(serve: &[&str]) -> Runner {
        Runner::new(
            serve.iter().map(ToString::to_string).collect(),
            INVITE.iter().map(ToString::to_string).collect(),
        )
        .unwrap()
    }

    fn launcher_with(&self, release: Release, serve: &[&str]) -> Launcher {
        Launcher::new("devbox", release, Self::runner(serve))
            .unwrap()
            .program(self.shim())
            .lock_wait(5)
    }

    fn launcher(&self) -> Launcher {
        self.launcher_with(self.release(), SERVE)
    }

    fn remote(&self, path: &str) -> PathBuf {
        self.home().join(".openagents").join(path)
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
    }

    fn uploads(&self) -> usize {
        self.calls().matches("oa-ssh-upload").count()
    }

    fn recorded_pid(&self, path: &str) -> Option<u32> {
        let text = std::fs::read_to_string(self.remote(path)).ok()?;
        text.lines()
            .find_map(|line| line.strip_prefix("pid="))
            .and_then(|pid| pid.parse().ok())
    }

    /// Starts a host the way a person would, outside any launcher.
    fn start_external(&self) -> u32 {
        let archive_dir = self.dir.join("bundle-archive.tar.gz");
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg("nohup \"$1\" host serve --loopback </dev/null >/dev/null 2>&1 &")
            .arg("external")
            .arg(archive_dir.join("coder"))
            .env("HOME", self.home())
            .status()
            .unwrap();
        assert!(status.success());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(pid) = self.recorded_pid("host/runtime") {
                std::fs::write(self.dir.join("external"), pid.to_string()).unwrap();
                return pid;
            }
            assert!(Instant::now() < deadline, "the external host did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self._temp.is_none() {
            return;
        }
        for path in ["host/runtime", "ssh-host/managed"] {
            if let Some(pid) = self.recorded_pid(path) {
                kill(pid);
            }
        }
        for name in ["tunnel", "external", "client"] {
            if let Ok(text) = std::fs::read_to_string(self.dir.join(name)) {
                for word in text.split_whitespace() {
                    if let Ok(pid) = word.parse::<u32>() {
                        kill(pid);
                    }
                }
            }
        }
    }
}

#[test]
fn fresh_install_then_reuse() {
    let fixture = Fixture::new();
    let launcher = fixture.launcher();
    let first = launcher.up().unwrap();
    let (os, arch) = local_platform();
    assert_eq!((first.os, first.arch), (os, arch));
    assert_eq!(first.install, Install::Fresh);
    assert_eq!(first.start, Start::Started);
    assert_eq!(first.ownership, Ownership::Managed);
    assert_eq!(first.port, HOST_PORT);
    assert!(alive(first.pid));
    assert_eq!(fixture.uploads(), 1);

    let version = fixture.remote("ssh-host/versions").join(&first.version);
    assert!(version.join("coder").is_file());
    let manifest = std::fs::read_to_string(version.join("manifest")).unwrap();
    assert!(manifest.contains(&format!("archive_sha256={}", first.version)));
    assert!(!fixture.remote("ssh-host/lock").exists());
    assert_eq!(
        std::fs::read_dir(fixture.remote("ssh-host/uploads"))
            .unwrap()
            .count(),
        0
    );

    let second = launcher.up().unwrap();
    assert_eq!(second.install, Install::Reused);
    assert_eq!(second.start, Start::Reused);
    assert_eq!(second.pid, first.pid);
    assert_eq!(fixture.uploads(), 1, "a reused install uploads nothing");

    // Every connection Coder makes disables multiplexing.
    for line in fixture.calls().lines() {
        assert!(line.contains("ControlMaster=no"), "{line}");
        assert!(line.contains("BatchMode=yes"), "{line}");
    }
}

#[test]
fn dash_runs_the_same_script() {
    if !Path::new("/bin/dash").exists() {
        eprintln!("skipped: /bin/dash is not installed");
        return;
    }
    let fixture = Fixture::with_shell("/bin/dash");
    let launcher = fixture.launcher();
    let first = launcher.up().unwrap();
    assert_eq!(first.install, Install::Fresh);
    let second = launcher.up().unwrap();
    assert_eq!(second.start, Start::Reused);
    assert_eq!(second.pid, first.pid);
    let changed =
        fixture.launcher_with(fixture.release(), &["host", "serve", "--loopback", "--two"]);
    let third = changed.up().unwrap();
    assert_eq!(third.start, Start::Relaunched);
    assert!(wait_dead(first.pid));
    assert_eq!(
        changed.remove().unwrap(),
        Removal::Stopped { pid: third.pid }
    );
    assert!(wait_dead(third.pid));
}

#[test]
fn checksum_mismatch_installs_nothing() {
    let fixture = Fixture::new();
    std::fs::write(fixture.dir.join("corrupt"), "").unwrap();
    let error = fixture.launcher().up().unwrap_err();
    assert!(matches!(error, Error::ChecksumMismatch), "{error}");
    let versions = fixture.remote("ssh-host/versions");
    let entries: Vec<_> = std::fs::read_dir(&versions)
        .map(|dir| dir.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default();
    assert!(entries.is_empty(), "{entries:?}");
    assert!(!fixture.remote("host/runtime").exists());
    assert!(!fixture.remote("ssh-host/lock").exists());
}

#[test]
fn a_local_archive_that_does_not_match_its_pin_is_not_sent() {
    let fixture = Fixture::new();
    let archive = fixture.dir.join("archive.tar.gz");
    let release = fixture.release_for(&archive, &"0".repeat(64));
    let error = fixture.launcher_with(release, SERVE).up().unwrap_err();
    assert!(matches!(error, Error::LocalChecksumMismatch(_)), "{error}");
    assert_eq!(fixture.uploads(), 0);
}

#[test]
fn unsupported_platform_is_named() {
    let fixture = Fixture::new();
    fixture.fake_uname("Plan9", "mips");
    match fixture.launcher().up().unwrap_err() {
        Error::Unsupported { os, arch } => {
            assert_eq!(os, "Plan9");
            assert_eq!(arch, "mips");
        }
        other => panic!("unexpected {other}"),
    }
    assert_eq!(fixture.uploads(), 0);
}

#[test]
fn a_release_without_this_platform_is_refused() {
    let fixture = Fixture::new();
    let (os, _) = local_platform();
    let other = match os {
        Os::Macos => Os::Linux,
        Os::Linux => Os::Macos,
    };
    let archive = fixture.dir.join("archive.tar.gz");
    let release = Release::new(vec![Artifact {
        os: other,
        arch: Arch::X86_64,
        sha256: sha256_file(&archive),
        archive,
    }])
    .unwrap();
    let error = fixture.launcher_with(release, SERVE).up().unwrap_err();
    assert!(matches!(error, Error::NoArtifact { .. }), "{error}");
}

#[test]
fn a_binary_that_does_not_run_is_not_installed() {
    let fixture = Fixture::new();
    let archive = fixture.write_archive(BROKEN_CODER, "broken.tar.gz");
    let sha = sha256_file(&archive);
    let release = fixture.release_for(&archive, &sha);
    let error = fixture.launcher_with(release, SERVE).up().unwrap_err();
    assert!(matches!(error, Error::BinaryRejected), "{error}");
    assert!(!fixture.remote("ssh-host/versions").join(&sha).exists());
}

#[test]
fn a_lock_left_by_a_dead_owner_is_reclaimed() {
    let fixture = Fixture::new();
    let mut dead = Command::new("true").spawn().unwrap();
    let pid = dead.id();
    dead.wait().unwrap();
    let lock = fixture.remote("ssh-host/lock");
    std::fs::create_dir_all(&lock).unwrap();
    std::fs::write(lock.join("owner"), format!("{pid} {}\n", node_name())).unwrap();
    let host = fixture.launcher().up().unwrap();
    assert!(host.reclaimed_lock);
    assert_eq!(host.start, Start::Started);
    assert!(!lock.exists());
}

#[test]
fn a_lock_held_by_a_live_owner_is_waited_for_and_then_refused() {
    let fixture = Fixture::new();
    let lock = fixture.remote("ssh-host/lock");
    std::fs::create_dir_all(&lock).unwrap();
    let owner = format!("{} {}\n", std::process::id(), node_name());
    std::fs::write(lock.join("owner"), &owner).unwrap();
    let error = fixture.launcher().lock_wait(1).up().unwrap_err();
    assert!(matches!(error, Error::Busy), "{error}");
    assert_eq!(std::fs::read_to_string(lock.join("owner")).unwrap(), owner);
}

#[test]
fn a_running_host_is_adopted_as_external_and_never_stopped() {
    let fixture = Fixture::new();
    // Install first, so the external host runs the same binary a person
    // would have installed; then stop the managed host this created.
    let launcher = fixture.launcher();
    let managed = launcher.up().unwrap();
    assert_eq!(
        launcher.remove().unwrap(),
        Removal::Stopped { pid: managed.pid }
    );
    assert!(wait_dead(managed.pid));

    let external = fixture.start_external();
    let host = launcher.up().unwrap();
    assert_eq!(host.start, Start::Adopted);
    assert_eq!(host.ownership, Ownership::External);
    assert_eq!(host.pid, external);

    // A runner change does not relaunch a host that no launcher started.
    let changed = fixture.launcher_with(fixture.release(), &["host", "serve", "--other"]);
    assert_eq!(changed.up().unwrap().start, Start::Adopted);

    assert_eq!(
        launcher.remove().unwrap(),
        Removal::Detached { pid: external }
    );
    assert!(alive(external), "remove must not stop an external host");
    assert_eq!(launcher.up().unwrap().pid, external);
}

#[test]
fn a_runner_change_relaunches_a_managed_host() {
    let fixture = Fixture::new();
    let first = fixture.launcher().up().unwrap();
    let changed =
        fixture.launcher_with(fixture.release(), &["host", "serve", "--loopback", "--two"]);
    let second = changed.up().unwrap();
    assert_eq!(second.start, Start::Relaunched);
    assert_eq!(second.ownership, Ownership::Managed);
    assert_ne!(second.pid, first.pid);
    assert!(wait_dead(first.pid));
    assert!(alive(second.pid));
    assert_eq!(fixture.uploads(), 1);
}

#[test]
fn a_reused_process_identifier_is_never_stopped() {
    let fixture = Fixture::new();
    let launcher = fixture.launcher();
    let host = launcher.up().unwrap();
    assert_eq!(
        launcher.remove().unwrap(),
        Removal::Stopped { pid: host.pid }
    );

    // An unrelated process now holds the identifier both records name.
    let mut unrelated = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = unrelated.id();
    let runtime = format!("schema=openagents.coder.host-runtime.v1\npid={pid}\nport={HOST_PORT}\n");
    std::fs::write(fixture.remote("host/runtime"), runtime).unwrap();
    let managed = format!(
        "schema=openagents.coder.ssh-managed.v1\npid={pid}\nrunner={}\nversion={}\n",
        Fixture::runner(SERVE).fingerprint(),
        host.version
    );
    std::fs::write(fixture.remote("ssh-host/managed"), managed).unwrap();

    let fresh = launcher.up().unwrap();
    assert_eq!(fresh.start, Start::Started);
    assert_ne!(fresh.pid, pid);
    assert!(matches!(unrelated.try_wait(), Ok(None)));
    assert_eq!(
        launcher.remove().unwrap(),
        Removal::Stopped { pid: fresh.pid }
    );

    std::fs::write(
        fixture.remote("host/runtime"),
        format!("schema=openagents.coder.host-runtime.v1\npid={pid}\nport={HOST_PORT}\n"),
    )
    .unwrap();
    std::fs::write(
        fixture.remote("ssh-host/managed"),
        format!(
            "schema=openagents.coder.ssh-managed.v1\npid={pid}\nrunner={}\nversion={}\n",
            Fixture::runner(SERVE).fingerprint(),
            host.version
        ),
    )
    .unwrap();
    assert_eq!(launcher.remove().unwrap(), Removal::Absent);
    assert!(matches!(unrelated.try_wait(), Ok(None)));
    let _ = unrelated.kill();
    let _ = unrelated.wait();
}

#[test]
fn tunnel_death_leaves_the_host_running() {
    let fixture = Fixture::new();
    let launcher = fixture.launcher();
    let host = launcher.up().unwrap();
    let mut tunnel = launcher.connect(&host).unwrap();
    assert_eq!(tunnel.remote_port(), HOST_PORT);

    let deadline = Instant::now() + Duration::from_secs(5);
    let recorded = loop {
        if let Ok(text) = std::fs::read_to_string(fixture.dir.join("tunnel")) {
            break text;
        }
        assert!(Instant::now() < deadline, "the tunnel did not start");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(recorded.contains(&format!(
        "127.0.0.1:{}:127.0.0.1:{HOST_PORT}",
        tunnel.local_port()
    )));

    // The forwarded port: stand in for ssh's listener to check readiness.
    let listener = std::net::TcpListener::bind(("127.0.0.1", tunnel.local_port())).unwrap();
    tunnel.ready(Duration::from_secs(5)).unwrap();
    drop(listener);

    // The tunnel dies abruptly, as on a lost network.
    // SAFETY: the identifier is this test's own tunnel child.
    unsafe { libc::kill(tunnel.pid() as i32, libc::SIGKILL) };
    let deadline = Instant::now() + Duration::from_secs(5);
    while tunnel.alive() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(alive(host.pid), "a dead tunnel must not stop the host");
    drop(tunnel);

    let again = launcher.up().unwrap();
    assert_eq!(again.start, Start::Reused);
    assert_eq!(again.pid, host.pid);

    // Closing a live tunnel and dropping every client value also leaves it.
    let tunnel = launcher.connect(&again).unwrap();
    tunnel.close();
    drop(again);
    drop(launcher);
    assert!(alive(host.pid));
}

#[test]
#[ignore = "helper process for client_exit_leaves_the_host_running"]
fn client_process() {
    let Ok(dir) = std::env::var("CODER_SSH_TEST_CLIENT") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let fixture = Fixture::open(dir.clone());
    let launcher = fixture.launcher();
    let host = launcher.up().unwrap();
    let tunnel = launcher.connect(&host).unwrap();
    std::fs::write(dir.join("client"), format!("{} {}", host.pid, tunnel.pid())).unwrap();
    // Exit the way a crashed client does: no destructor runs.
    std::process::exit(0);
}

#[test]
fn client_exit_leaves_the_host_running() {
    let fixture = Fixture::new();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "client_process", "--ignored", "--nocapture"])
        .env("CODER_SSH_TEST_CLIENT", &fixture.dir)
        .status()
        .unwrap();
    assert!(status.success());
    let text = std::fs::read_to_string(fixture.dir.join("client")).unwrap();
    let pids: Vec<u32> = text
        .split_whitespace()
        .map(|word| word.parse().unwrap())
        .collect();
    let (host, tunnel) = (pids[0], pids[1]);
    assert!(alive(host), "client exit must not stop the host");
    // The orphaned tunnel dies too; the host still runs.
    kill(tunnel);
    assert!(wait_dead(tunnel));
    assert!(alive(host));
    let again = fixture.launcher().up().unwrap();
    assert_eq!(again.start, Start::Reused);
    assert_eq!(again.pid, host);
    assert_eq!(
        fixture.launcher().remove().unwrap(),
        Removal::Stopped { pid: host }
    );
    assert!(wait_dead(host));
    assert_eq!(fixture.launcher().remove().unwrap(), Removal::Absent);
}

#[test]
fn a_password_goes_through_a_one_shot_askpass_pipe() {
    let fixture = Fixture::new();
    std::fs::write(fixture.dir.join("password"), PASSWORD).unwrap();

    // Without a prompter, ssh runs in batch mode and cannot prompt.
    let error = fixture.launcher().up().unwrap_err();
    assert!(
        matches!(
            error,
            Error::Ssh {
                code: Some(255),
                ..
            }
        ),
        "{error}"
    );

    let prompts = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = Arc::clone(&prompts);
    let launcher = fixture.launcher().prompter(Arc::new(move |prompt: &str| {
        seen.lock().unwrap().push(prompt.to_string());
        Some(Secret::new(PASSWORD))
    }));
    let host = launcher.up().unwrap();
    assert_eq!(host.start, Start::Started);
    let asked = prompts.lock().unwrap().clone();
    assert!(!asked.is_empty());
    assert!(asked.iter().all(|prompt| prompt.contains("fake.example")));

    // Each invocation had its own helper, and every helper is gone.
    let paths = std::fs::read_to_string(fixture.dir.join("askpass-paths")).unwrap();
    let paths: Vec<&str> = paths.lines().collect();
    assert_eq!(paths.len(), asked.len());
    for path in &paths {
        assert!(!Path::new(path).exists(), "{path} was not removed");
        assert!(!Path::new(path).parent().unwrap().exists());
    }
    for line in fixture.calls().lines() {
        assert!(!line.contains(PASSWORD));
    }

    // A wrong answer or a refusal fails authentication.
    let wrong = fixture
        .launcher()
        .prompter(Arc::new(|_: &str| Some(Secret::new("wrong"))));
    assert!(matches!(wrong.remove().unwrap_err(), Error::Ssh { .. }));
    let refused = fixture.launcher().prompter(Arc::new(|_: &str| None));
    assert!(matches!(refused.remove().unwrap_err(), Error::Ssh { .. }));
    assert!(alive(host.pid));
}

#[test]
fn an_invitation_is_returned_to_the_caller() {
    let fixture = Fixture::new();
    let launcher = fixture.launcher();
    let host = launcher.up().unwrap();
    let invitation = launcher.invite(&host).unwrap();
    assert_eq!(invitation.expose(), "oa-invite:abc123");
    assert!(!format!("{invitation:?}").contains("abc123"));
    assert!(!fixture.remote("ssh-host/lock").exists());
}

#[test]
fn a_destination_resolves_through_ssh_g() {
    let fixture = Fixture::new();
    let resolved = fixture.launcher().resolve().unwrap();
    assert_eq!(resolved.hostname, "fake.example");
    assert_eq!(resolved.user, "fake");
    assert_eq!(resolved.port, 2222);
    assert!(fixture.calls().contains("-G -- devbox"));
}

/// The real-`sshd` run's settings, from the environment.
struct Real {
    destination: String,
    ssh: PathBuf,
    coder: PathBuf,
    os: Os,
    arch: Arch,
}

impl Real {
    fn from_env() -> Option<Self> {
        let destination = std::env::var("CODER_SSH_REAL_DESTINATION").ok()?;
        let coder = std::env::var_os("CODER_SSH_REAL_CODER")
            .map(PathBuf::from)
            .expect("CODER_SSH_REAL_CODER names the coder binary to install");
        let ssh =
            std::env::var_os("CODER_SSH_REAL_SSH").map_or_else(|| "ssh".into(), PathBuf::from);
        let (os, arch) = match std::env::var("CODER_SSH_REAL_PLATFORM").as_deref() {
            Ok("linux/x86_64") => (Os::Linux, Arch::X86_64),
            Ok("linux/aarch64") => (Os::Linux, Arch::Aarch64),
            Ok("macos/x86_64") => (Os::Macos, Arch::X86_64),
            Ok("macos/aarch64") => (Os::Macos, Arch::Aarch64),
            Ok(other) => panic!("CODER_SSH_REAL_PLATFORM is OS/ARCH, not {other}"),
            Err(_) => local_platform(),
        };
        Some(Real {
            destination,
            ssh,
            coder,
            os,
            arch,
        })
    }

    /// Runs one command on the remote machine, outside the launcher.
    fn remote(&self, script: &str) -> (bool, String) {
        let output = Command::new(&self.ssh)
            .args(["-o", "BatchMode=yes", "-o", "ControlMaster=no"])
            .args(["-o", "ControlPath=none", "-T", "--"])
            .arg(&self.destination)
            .arg(script)
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        )
    }

    fn remote_alive(&self, pid: u32) -> bool {
        self.remote(&format!("kill -0 {pid}")).0
    }

    fn remote_dead(&self, pid: u32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if !self.remote_alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }
}

/// A relay that nothing serves. The host waits for it, then serves direct
/// channels anyway.
const REAL_RELAY: &str = "ws://127.0.0.1:9/";
const REAL_OWNER: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

/// Install, start (detached with `setsid` where the remote has it), reuse,
/// tunnel, invite, tunnel death, adoption of an external host, and explicit
/// remove, against a real `sshd` and a real `coder host serve`.
///
/// Skipped unless `CODER_SSH_REAL_DESTINATION` names a disposable account
/// and `CODER_SSH_REAL_CODER` names a `coder` binary for the remote
/// platform. `CODER_SSH_REAL_SSH` replaces the `ssh` program, for example
/// with a wrapper that adds `-F` and a throwaway configuration, and
/// `CODER_SSH_REAL_PLATFORM` (`linux/x86_64`) names the remote platform when
/// it differs from this one. The run writes only under the remote
/// account's `~/.openagents`, so use an account or `HOME` you can discard.
#[test]
fn real_sshd_lifecycle() {
    let Some(real) = Real::from_env() else {
        eprintln!(
            "skipped: set CODER_SSH_REAL_DESTINATION and CODER_SSH_REAL_CODER to run against a real sshd"
        );
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let bundle = temp.path().join("bundle");
    std::fs::create_dir(&bundle).unwrap();
    std::fs::copy(&real.coder, bundle.join("coder")).unwrap();
    let archive = temp.path().join("coder.tar.gz");
    // macOS tar otherwise adds an AppleDouble `._coder` entry for a file
    // with extended attributes, and it lands in the version directory.
    let status = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&bundle)
        .arg("coder")
        .status()
        .unwrap();
    assert!(status.success());
    let sha = sha256_file(&archive);
    let release = Release::new(vec![Artifact {
        os: real.os,
        arch: real.arch,
        sha256: sha.clone(),
        archive: archive.clone(),
    }])
    .unwrap();
    let serve: Vec<String> = [
        "host",
        "serve",
        "--loopback",
        "--loopback-test",
        "--no-telemetry",
        "--owner",
        REAL_OWNER,
        "--relay",
        REAL_RELAY,
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let invite: Vec<String> = ["host", "invite", "--loopback-test", "--relay", REAL_RELAY]
        .iter()
        .map(ToString::to_string)
        .collect();
    let launcher = Launcher::new(
        &real.destination,
        release,
        Runner::new(serve.clone(), invite).unwrap(),
    )
    .unwrap()
    .program(&real.ssh);

    let resolved = launcher.resolve().unwrap();
    eprintln!(
        "resolved: {}@{}:{}",
        resolved.user, resolved.hostname, resolved.port
    );
    assert_eq!(launcher.remove().unwrap(), Removal::Absent);

    // Fresh install and start. The ssh session that started the host has
    // ended when `up` returns.
    let host = launcher.up().unwrap();
    eprintln!(
        "up: {:?} {:?} {:?} pid={} port={} os={} arch={}",
        host.install,
        host.start,
        host.ownership,
        host.pid,
        host.port,
        host.os.as_str(),
        host.arch.as_str()
    );
    assert_eq!(
        (host.install, host.start, host.ownership),
        (Install::Fresh, Start::Started, Ownership::Managed)
    );
    assert_eq!(host.version, sha);
    let (_, process) = real.remote(&format!(
        "ps -o pid=,ppid=,pgid=,sid=,args= -p {}",
        host.pid
    ));
    eprintln!("remote process: {process}");
    let fields: Vec<&str> = process.split_whitespace().collect();
    let (_, has_setsid) = real.remote("command -v setsid >/dev/null && echo yes || echo no");
    if has_setsid == "yes" {
        // setsid made the host lead its own session and process group.
        assert_eq!(fields[2], fields[0], "the host leads its process group");
        assert_eq!(fields[3], fields[0], "the host leads its session");
    } else {
        assert_eq!(fields[2], fields[0], "the host leads its process group");
    }
    assert!(real.remote_alive(host.pid));

    let again = launcher.up().unwrap();
    eprintln!("up again: {:?} {:?}", again.install, again.start);
    assert_eq!(
        (again.install, again.start),
        (Install::Reused, Start::Reused)
    );
    assert_eq!(again.pid, host.pid);

    // A tunnel reaches the host's direct listener.
    let mut tunnel = launcher.connect(&host).unwrap();
    tunnel.ready(Duration::from_secs(20)).unwrap();
    eprintln!(
        "tunnel: 127.0.0.1:{} -> remote 127.0.0.1:{} (ssh pid {})",
        tunnel.local_port(),
        tunnel.remote_port(),
        tunnel.pid()
    );
    let invitation = launcher.invite(&host).unwrap();
    assert!(invitation.expose().starts_with("coder-host:"));
    eprintln!("invite: one line, {invitation:?}");

    // The tunnel dies abruptly; the host keeps running.
    // SAFETY: the identifier is this test's own tunnel child.
    unsafe { libc::kill(tunnel.pid() as i32, libc::SIGKILL) };
    let deadline = Instant::now() + Duration::from_secs(5);
    while tunnel.alive() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(tunnel);
    assert!(
        real.remote_alive(host.pid),
        "a dead tunnel stopped the host"
    );
    let after = launcher.up().unwrap();
    eprintln!("after tunnel death: {:?} pid={}", after.start, after.pid);
    assert_eq!((after.start, after.pid), (Start::Reused, host.pid));

    // Explicit remove stops the managed host.
    assert_eq!(
        launcher.remove().unwrap(),
        Removal::Stopped { pid: host.pid }
    );
    assert!(real.remote_dead(host.pid));
    eprintln!("remove: stopped {}", host.pid);

    // A host started outside any launcher is adopted and never stopped.
    let binary = format!("\"$HOME/.openagents/ssh-host/versions/{sha}/coder\"");
    let (started, _) = real.remote(&format!(
        "nohup {binary} {} </dev/null >/dev/null 2>&1 & echo $!",
        serve.join(" ")
    ));
    assert!(started);
    let deadline = Instant::now() + Duration::from_secs(20);
    let external = loop {
        let (ok, pid) = real.remote("sed -n 's/^pid=//p' \"$HOME/.openagents/host/runtime\"");
        if ok && let Ok(pid) = pid.parse::<u32>() {
            break pid;
        }
        assert!(Instant::now() < deadline, "the external host did not start");
        std::thread::sleep(Duration::from_millis(200));
    };
    let adopted = launcher.up().unwrap();
    eprintln!(
        "external pid={external}: {:?} {:?}",
        adopted.start, adopted.ownership
    );
    assert_eq!(
        (adopted.start, adopted.ownership, adopted.pid),
        (Start::Adopted, Ownership::External, external)
    );
    assert_eq!(
        launcher.remove().unwrap(),
        Removal::Detached { pid: external }
    );
    assert!(
        real.remote_alive(external),
        "remove stopped an external host"
    );
    eprintln!("remove: detached from {external}; it still runs");
    // This test started the external host, so it stops it.
    assert!(real.remote(&format!("kill {external}")).0);
    assert!(real.remote_dead(external));
    let (_, left) = real.remote("find \"$HOME/.openagents\" | sort");
    eprintln!("remote files after the run:\n{left}");
}
