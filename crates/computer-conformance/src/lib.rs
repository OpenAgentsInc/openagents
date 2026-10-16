//! Black-box conformance for computer drivers (#11624).
//!
//! [`run`] takes any [`Driver`], reads its declared [`Capabilities`], and
//! runs every case of every profile it declares. For each optional profile
//! it does *not* declare, it checks the driver refuses that operation with
//! a definite "unsupported" failure and changes nothing.
//!
//! The cases only use the contract: create, observe, exec, files, meter,
//! delete, suspend/resume, snapshot/restore, fork. Every computer and
//! snapshot a run makes is deleted at the end, pass or fail.
//!
//! - **Core**: the declaration is valid; create is idempotent under one
//!   operation; a new computer reaches Running; exec returns output and the
//!   exit code; per-command secrets reach that command only and never a
//!   file in the computer; files round-trip; the meter answers; delete is
//!   final and idempotent, and a deleted computer runs nothing.
//! - **Suspend**: suspend reaches Stopped and refuses commands; resume
//!   reaches Running with the files kept, and resuming a running computer
//!   again is done, not failed.
//! - **Snapshot**: a saved snapshot becomes ready and a computer created
//!   from it holds the saved files; deleting the snapshot is idempotent.
//! - **Fork**: a fork holds the source's files, is idempotent under one
//!   operation, and changes in it do not reach the source.

use computer_driver::*;
use std::collections::BTreeSet;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Options {
    /// Between observations while waiting for a state.
    pub poll: Duration,
    /// How long a state may take to be reached.
    pub deadline: Duration,
    /// The size class to create; `None` takes the driver's default.
    pub size: Option<String>,
    /// Unique per run: names operations and snapshots.
    pub prefix: String,
}
impl Options {
    pub fn fast(prefix: &str) -> Self {
        Self {
            poll: Duration::from_millis(50),
            deadline: Duration::from_secs(60),
            size: None,
            prefix: prefix.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    pub profile: Profile,
    pub name: &'static str,
    pub result: Result<(), String>,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub driver: String,
    pub declared: BTreeSet<Profile>,
    pub cases: Vec<Case>,
    /// Computers or snapshots the run could not delete.
    pub leftovers: Vec<String>,
}
impl Report {
    pub fn passed(&self) -> bool {
        self.leftovers.is_empty() && self.cases.iter().all(|c| c.result.is_ok())
    }
    pub fn summary(&self) -> String {
        let mut out = format!(
            "driver {} declares {}\n",
            self.driver,
            self.declared
                .iter()
                .map(|p| p.name())
                .collect::<Vec<_>>()
                .join(", ")
        );
        for c in &self.cases {
            match &c.result {
                Ok(()) => out.push_str(&format!("  ok    {:<8} {}\n", c.profile.name(), c.name)),
                Err(e) => out.push_str(&format!(
                    "  FAIL  {:<8} {}: {e}\n",
                    c.profile.name(),
                    c.name
                )),
            }
        }
        for l in &self.leftovers {
            out.push_str(&format!("  LEFT  {l}\n"));
        }
        let passed = self.cases.iter().filter(|c| c.result.is_ok()).count();
        out.push_str(&format!("{passed}/{} cases passed\n", self.cases.len()));
        out
    }
}

type R = Result<(), String>;

fn done<T>(what: &str, o: Outcome<T>) -> Result<T, String> {
    match o {
        Outcome::Done { value } => Ok(value),
        Outcome::Failed { reason } => Err(format!("{what} failed: {reason}")),
        Outcome::Unknown { reason } => Err(format!("{what} unknown: {reason}")),
    }
}

fn check(ok: bool, what: impl Into<String>) -> R {
    if ok { Ok(()) } else { Err(what.into()) }
}

struct Run<'a, D> {
    d: &'a D,
    o: &'a Options,
    computers: Vec<String>,
    snapshots: Vec<String>,
    n: u32,
}

impl<D: Driver> Run<'_, D> {
    fn op(&mut self, what: &str) -> String {
        self.n += 1;
        format!("{}-{what}-{}", self.o.prefix, self.n)
    }

    fn request(&self, operation: String, image: Option<String>) -> CreateRequest {
        CreateRequest {
            operation,
            size: self.o.size.clone(),
            image,
            ttl_seconds: Some(3600),
            labels: [("purpose".to_owned(), "conformance".to_owned())].into(),
        }
    }

    async fn wait(&self, id: &str, want: State) -> R {
        let until = tokio::time::Instant::now() + self.o.deadline;
        let mut last = None;
        loop {
            if let Outcome::Done { value } = self.d.observe(id).await {
                if value.state == want {
                    return Ok(());
                }
                if value.state == State::Failed {
                    return Err(format!("{id} failed while waiting for {want:?}"));
                }
                last = Some(value.state);
            }
            if tokio::time::Instant::now() >= until {
                return Err(format!("{id} did not reach {want:?} (last {last:?})"));
            }
            tokio::time::sleep(self.o.poll).await;
        }
    }

    async fn new_running(&mut self, image: Option<String>) -> Result<String, String> {
        let op = self.op("create");
        let id = done("create", self.d.create(&self.request(op, image)).await)?;
        self.computers.push(id.clone());
        self.wait(&id, State::Running).await?;
        Ok(id)
    }

    async fn sh(&self, id: &str, command: &str) -> Result<ExecOutput, String> {
        done("exec", self.d.exec(id, &Exec::new(command)).await)
    }

    async fn cleanup(&mut self) -> Vec<String> {
        let mut left = Vec::new();
        for id in std::mem::take(&mut self.computers) {
            if !self.d.delete(&id).await.is_done() || self.wait(&id, State::Gone).await.is_err() {
                left.push(format!("computer {id}"));
            }
        }
        for name in std::mem::take(&mut self.snapshots) {
            if !self.d.delete_snapshot(&name).await.is_done() {
                left.push(format!("snapshot {name}"));
            }
        }
        left
    }

    // ------------------------------------------------------------- Core

    async fn core_declaration(&mut self) -> R {
        let caps = self.d.capabilities();
        caps.validate().map_err(str::to_owned)?;
        if let Some(size) = &self.o.size {
            check(
                caps.sizes.contains(size),
                format!("size {size} is not declared"),
            )?;
        }
        Ok(())
    }

    async fn core_idempotent_create(&mut self) -> R {
        let op = self.op("idem");
        let a = done(
            "create",
            self.d.create(&self.request(op.clone(), None)).await,
        )?;
        self.computers.push(a.clone());
        let b = done(
            "repeat create",
            self.d.create(&self.request(op, None)).await,
        )?;
        check(
            a == b,
            format!("one operation made two computers: {a} and {b}"),
        )?;
        self.wait(&a, State::Running).await
    }

    async fn core_exec(&mut self) -> R {
        let id = self.new_running(None).await?;
        let out = self.sh(&id, "echo hi; echo err >&2; exit 3").await?;
        check(out.code == 3, format!("exit code {} (want 3)", out.code))?;
        check(out.stdout == "hi\n", format!("stdout {:?}", out.stdout))?;
        check(
            out.stderr.contains("err"),
            format!("stderr {:?}", out.stderr),
        )
    }

    async fn core_secret_env(&mut self) -> R {
        let id = self.new_running(None).await?;
        let secret = format!("s3cret-{}", self.o.prefix);
        let e = Exec::new("printf %s \"$OA_CONFORMANCE_SECRET\"")
            .secret("OA_CONFORMANCE_SECRET", &secret);
        let out = done("exec with secret", self.d.exec(&id, &e).await)?;
        check(out.stdout == secret, "the command did not see its secret")?;
        let next = self
            .sh(&id, "printf %s \"${OA_CONFORMANCE_SECRET:-}\"")
            .await?;
        check(
            next.stdout.is_empty(),
            "a later command saw an earlier command's secret",
        )?;
        // No file the computer's user can read holds it (home and /tmp,
        // plus the per-boot env oa-boat keeps under /run).
        let grep = self
            .sh(
                &id,
                &format!(
                    "grep -rIl -- {} \"$HOME\" /tmp /run/oa-boat 2>/dev/null | head -1",
                    shell_word(&secret)
                ),
            )
            .await?;
        check(
            grep.stdout.trim().is_empty(),
            format!("the secret was written to {}", grep.stdout.trim()),
        )
    }

    async fn core_files(&mut self) -> R {
        let id = self.new_running(None).await?;
        let bytes = b"conformance \x00\x01 bytes\n".to_vec();
        done("put", self.d.put_file(&id, "oa-conf/a.bin", &bytes).await)?;
        let back = done("get", self.d.get_file(&id, "oa-conf/a.bin").await)?;
        check(back == bytes, "a file did not round-trip")?;
        let seen = self.sh(&id, "wc -c < oa-conf/a.bin").await?;
        check(
            seen.stdout.trim() == bytes.len().to_string(),
            format!("a command saw {} bytes", seen.stdout.trim()),
        )
    }

    async fn core_meter(&mut self) -> R {
        let id = self.new_running(None).await?;
        let m = done("meter", self.d.meter(&id).await)?;
        check(
            m.running,
            "a running computer's meter says it is not running",
        )?;
        check(!m.evidence.is_empty(), "the meter gave no evidence")
    }

    async fn core_delete_is_final(&mut self) -> R {
        let id = self.new_running(None).await?;
        done("delete", self.d.delete(&id).await)?;
        self.wait(&id, State::Gone).await?;
        done("repeat delete", self.d.delete(&id).await)?;
        check(
            !self.d.exec(&id, &Exec::new("true")).await.is_done(),
            "a deleted computer ran a command",
        )?;
        self.computers.retain(|c| c != &id);
        Ok(())
    }

    // ---------------------------------------------------------- Suspend

    async fn suspend_keeps_files(&mut self) -> R {
        let id = self.new_running(None).await?;
        self.sh(&id, "echo kept > oa-conf-suspend.txt").await?;
        done("suspend", self.d.suspend(&id).await)?;
        self.wait(&id, State::Stopped).await?;
        check(
            !self.d.exec(&id, &Exec::new("true")).await.is_done(),
            "a stopped computer ran a command",
        )?;
        done("resume", self.d.resume(&id).await)?;
        self.wait(&id, State::Running).await?;
        // A repeated resume (a lost reply, a retry) of a computer that is
        // already running is done, never failed: `Failed` means the effect
        // did not happen, and the lifecycle trusts it (#11839).
        done("repeat resume", self.d.resume(&id).await)?;
        let out = self.sh(&id, "cat oa-conf-suspend.txt").await?;
        check(out.stdout == "kept\n", "resume lost a file")
    }

    // --------------------------------------------------------- Snapshot

    async fn snapshot_restores_files(&mut self) -> R {
        let id = self.new_running(None).await?;
        self.sh(&id, "echo saved > oa-conf-snap.txt").await?;
        // A driver may need the computer stopped to save it.
        if self.d.capabilities().supports(Profile::Suspend) {
            done("suspend", self.d.suspend(&id).await)?;
            self.wait(&id, State::Stopped).await?;
        }
        let name = format!("{}-snap", self.o.prefix).to_ascii_lowercase();
        let saved = done("snapshot", self.d.snapshot(&id, &name).await)?;
        self.snapshots.push(saved.clone());
        let until = tokio::time::Instant::now() + self.o.deadline;
        while !matches!(
            self.d.snapshot_ready(&saved).await,
            Outcome::Done { value: true }
        ) {
            if tokio::time::Instant::now() >= until {
                return Err("the snapshot never became ready".into());
            }
            tokio::time::sleep(self.o.poll).await;
        }
        let child = self.new_running(Some(saved.clone())).await?;
        let out = self.sh(&child, "cat oa-conf-snap.txt").await?;
        check(
            out.stdout == "saved\n",
            "a computer from the snapshot lacks its file",
        )?;
        done("delete snapshot", self.d.delete_snapshot(&saved).await)?;
        done(
            "repeat delete snapshot",
            self.d.delete_snapshot(&saved).await,
        )?;
        self.snapshots.retain(|s| s != &saved);
        Ok(())
    }

    // ------------------------------------------------------------- Fork

    async fn fork_copies(&mut self) -> R {
        let id = self.new_running(None).await?;
        self.sh(&id, "echo original > oa-conf-fork.txt").await?;
        let op = self.op("fork");
        let child = done("fork", self.d.fork(&id, &op).await)?;
        self.computers.push(child.clone());
        let again = done("repeat fork", self.d.fork(&id, &op).await)?;
        check(again == child, "one fork operation made two computers")?;
        self.wait(&child, State::Running).await?;
        // A source stopped for the copy comes back for the comparison.
        if let Outcome::Done { value } = self.d.observe(&id).await
            && value.state == State::Stopped
        {
            done("resume source", self.d.resume(&id).await)?;
        }
        self.wait(&id, State::Running).await?;
        let out = self.sh(&child, "cat oa-conf-fork.txt").await?;
        check(
            out.stdout == "original\n",
            "the fork lacks the source's file",
        )?;
        self.sh(&child, "echo changed > oa-conf-fork.txt").await?;
        let src = self.sh(&id, "cat oa-conf-fork.txt").await?;
        check(
            src.stdout == "original\n",
            "a change in the fork reached the source",
        )
    }

    // ------------------------------------------------------- Undeclared

    async fn refuses(&mut self, profile: Profile) -> R {
        let id = self.new_running(None).await?;
        let refused = match profile {
            Profile::Suspend => self.d.suspend(&id).await.is_unsupported(),
            Profile::Snapshot => self
                .d
                .snapshot(&id, &format!("{}-nosnap", self.o.prefix))
                .await
                .is_unsupported(),
            Profile::Fork => self.d.fork(&id, &self.op("nofork")).await.is_unsupported(),
            Profile::Core => true,
        };
        check(
            refused,
            format!("an undeclared {profile} was not refused as unsupported"),
        )?;
        let still = done("observe", self.d.observe(&id).await)?;
        check(
            still.state == State::Running,
            format!(
                "a refused {profile} changed the computer ({:?})",
                still.state
            ),
        )
    }
}

fn shell_word(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Run every case for what `driver` declares.
pub async fn run<D: Driver>(driver: &D, options: &Options) -> Report {
    let caps = driver.capabilities();
    let mut r = Run {
        d: driver,
        o: options,
        computers: Vec::new(),
        snapshots: Vec::new(),
        n: 0,
    };
    let mut cases = Vec::new();
    macro_rules! case {
        ($profile:expr, $name:literal, $call:expr) => {{
            let result = $call.await;
            cases.push(Case {
                profile: $profile,
                name: $name,
                result,
            });
        }};
    }
    case!(Profile::Core, "declaration", r.core_declaration());
    case!(
        Profile::Core,
        "create is idempotent",
        r.core_idempotent_create()
    );
    case!(Profile::Core, "exec returns output and code", r.core_exec());
    case!(
        Profile::Core,
        "secrets reach one command only",
        r.core_secret_env()
    );
    case!(Profile::Core, "files round-trip", r.core_files());
    case!(Profile::Core, "meter answers", r.core_meter());
    case!(Profile::Core, "delete is final", r.core_delete_is_final());
    if caps.supports(Profile::Suspend) {
        case!(
            Profile::Suspend,
            "resume keeps files",
            r.suspend_keeps_files()
        );
    } else {
        case!(
            Profile::Suspend,
            "undeclared is refused",
            r.refuses(Profile::Suspend)
        );
    }
    if caps.supports(Profile::Snapshot) {
        case!(
            Profile::Snapshot,
            "restore holds saved files",
            r.snapshot_restores_files()
        );
    } else {
        case!(
            Profile::Snapshot,
            "undeclared is refused",
            r.refuses(Profile::Snapshot)
        );
    }
    if caps.supports(Profile::Fork) {
        case!(
            Profile::Fork,
            "fork copies, idempotent, isolated",
            r.fork_copies()
        );
    } else {
        case!(
            Profile::Fork,
            "undeclared is refused",
            r.refuses(Profile::Fork)
        );
    }
    let leftovers = r.cleanup().await;
    Report {
        driver: caps.driver.clone(),
        declared: caps.profiles.clone(),
        cases,
        leftovers,
    }
}
