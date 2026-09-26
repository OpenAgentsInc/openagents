//! An explicit local Docker boundary with workspace-only command persistence.
//!
//! Docker's entire container is reconciled after each command. Supervising the
//! Docker client alone does not prove that the processes it started stopped.
use super::*;
use std::ffi::OsString;
use std::os::unix::fs::FileTypeExt;

const SCHEMA: &str = "openagents.microcoder.container.v1";
const OWNER_LABEL: &str = "com.openagents.task-effect";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub docker_program: PathBuf,
    pub docker_digest: String,
    pub socket: PathBuf,
    pub image: String,
    pub uid: u32,
    pub gid: u32,
}

impl Profile {
    pub fn validate(&self) -> Result<(), Error> {
        let digest = |value: &str| {
            value.strip_prefix("sha256:").is_some_and(|hex| {
                hex.len() == 64
                    && hex
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
        };
        if self.schema != SCHEMA
            || !self.docker_program.is_absolute()
            || !self.socket.is_absolute()
            || !digest(&self.docker_digest)
            || !digest(&self.image)
            || self.uid == 0
            || self.uid > i32::MAX as u32
            || self.gid > i32::MAX as u32
        {
            return Err(Error::InvalidCommand("invalid pinned container profile"));
        }
        Ok(())
    }

    fn executable(&self) -> Result<(), Error> {
        if self.docker_program.canonicalize()? != self.docker_program
            || digest_bytes(&std::fs::read(&self.docker_program)?) != self.docker_digest
            || !std::fs::metadata(&self.socket)?.file_type().is_socket()
        {
            return Err(Error::InvalidCommand(
                "Docker executable or local socket differs from admission",
            ));
        }
        Ok(())
    }

    fn process(&self, config: &Path, args: &[OsString]) -> std::process::Command {
        let mut process = std::process::Command::new(&self.docker_program);
        process.env_clear().env("PATH", "/usr/bin:/bin");
        process
            .arg("--config")
            .arg(config)
            .arg("--host")
            .arg(format!("unix://{}", self.socket.display()))
            .args(args);
        process
    }

    pub(super) async fn admit(&self, workspace: &Path, store: &Path) -> Result<(), Error> {
        self.validate()?;
        self.executable()?;
        if self.docker_program.starts_with(workspace)
            || self.socket.starts_with(workspace)
            || self.docker_program.starts_with(store)
        {
            return Err(Error::UnsafePath);
        }
        let args = strings(&["image", "inspect", &self.image, "--format", "{{json .}}"]);
        let observed = Job::from_command(self.process(store, &args))
            .bounded(Limits::within(Duration::from_secs(10)).keeping(128 * 1024))
            .run()
            .await;
        if !observed.ending.success() || observed.stdout.truncated {
            return Err(Error::InvalidCommand(
                "the exact local container image is unavailable",
            ));
        }
        let image: Value = serde_json::from_str(&observed.stdout.text)
            .map_err(|_| Error::InvalidCommand("container image metadata is unavailable"))?;
        if image["Id"].as_str() != Some(&self.image)
            || image["Config"]["Volumes"]
                .as_object()
                .is_some_and(|volumes| !volumes.is_empty())
        {
            return Err(Error::InvalidCommand(
                "container image identity or implicit mounts are unsupported",
            ));
        }
        Ok(())
    }
}

fn strings(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

async fn control(
    host: &Host,
    profile: &Profile,
    sequence: usize,
    phase: &str,
    args: Vec<OsString>,
    cleanup: bool,
) -> Result<supervise::Ended, Error> {
    profile.executable()?;
    let intent = host.append(
        &Step::said(Source::System, "Container control intent retained.").noting(
            "container_control",
            json!({"effect":sequence,"phase":phase,"arguments":args}),
        ),
    );
    if !cleanup {
        intent?;
    }
    let remaining = host
        .wall_seconds()
        .saturating_sub(host.started.elapsed().as_secs());
    let seconds = if cleanup { 5 } else { remaining.min(10) };
    if seconds == 0 {
        return Err(Error::LimitExceeded);
    }
    let observed = Job::from_command(
        profile.process(host.boundary.scratch().ok_or(Error::UnsafePath)?, &args),
    )
    .bounded(
        Limits::within(Duration::from_secs(seconds))
            .keeping(128 * 1024)
            .memory(Some(host.admission.grant.memory_bytes)),
    )
    .run()
    .await;
    let retained = host.append(
        &Step::said(Source::System, "Container control result retained.").noting(
            "container_control_result",
            json!({"effect":sequence,"phase":phase,
            "exit":observed.ending.code(),"ending":observed.ending.to_string(),
            "stdout":observed.stdout.text,"stderr":observed.stderr.text,
            "stdout_bytes":observed.stdout.bytes,"stderr_bytes":observed.stderr.bytes,
            "truncated":observed.stdout.truncated || observed.stderr.truncated}),
        ),
    );
    if !cleanup {
        retained?;
    }
    Ok(observed)
}

async fn inspect(
    host: &Host,
    profile: &Profile,
    sequence: usize,
    id: &str,
) -> Result<Value, Error> {
    let observed = control(
        host,
        profile,
        sequence,
        "inspect",
        strings(&["container", "inspect", id, "--format", "{{json .}}"]),
        true,
    )
    .await?;
    if !observed.ending.success() || observed.stdout.truncated {
        return Err(Error::InvalidCommand(
            "container disposition is unavailable",
        ));
    }
    serde_json::from_str(&observed.stdout.text).map_err(|_| Error::UnsupportedSchema)
}

async fn cleanup(host: &Host, profile: &Profile, sequence: usize, id: &str, label: &str) -> bool {
    let Ok(state) = inspect(host, profile, sequence, id).await else {
        return false;
    };
    if state["Image"].as_str() != Some(&profile.image)
        || state["Config"]["Labels"][OWNER_LABEL].as_str() != Some(label)
    {
        return false;
    }
    if state["State"]["Running"].as_bool() != Some(false) {
        if control(
            host,
            profile,
            sequence,
            "kill",
            strings(&["kill", "--signal", "KILL", id]),
            true,
        )
        .await
        .is_err()
        {
            return false;
        }
        let Ok(stopped) = inspect(host, profile, sequence, id).await else {
            return false;
        };
        if stopped["State"]["Running"].as_bool() != Some(false) {
            return false;
        }
    }
    control(
        host,
        profile,
        sequence,
        "remove",
        strings(&["rm", id]),
        true,
    )
    .await
    .is_ok_and(|removed| removed.ending.success())
}

pub(super) async fn command(
    host: &Host,
    profile: &Profile,
    script: &str,
    deadline: Duration,
) -> Result<CommandObservation, Error> {
    if script.len() > 64 * 1024 || script.contains('\0') {
        return Err(Error::LimitExceeded);
    }
    profile.executable()?;
    let sequence = host.effect("container_command", json!({"script":script,"profile":profile,
        "workspace":host.workspace(),"container_workdir":"/workspace","network":"none",
        "persistence":"workspace-only","pids_limit":128,"cpus":1,"memory_bytes":host.admission.grant.memory_bytes}))?;
    let label = format!(
        "{}:{}:{sequence}",
        host.task.task_id, host.admission.grant_digest
    );
    let name = format!(
        "oa-repo-{}-{}-{sequence}",
        &host.admission.grant_digest[7..19],
        std::process::id()
    );
    let mut args = strings(&[
        "create",
        "--name",
        &name,
        "--label",
        &format!("{OWNER_LABEL}={label}"),
        "--pull",
        "never",
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--pids-limit",
        "128",
        "--cpus",
        "1",
        "--memory",
        &host.admission.grant.memory_bytes.to_string(),
        "--memory-swap",
        &host.admission.grant.memory_bytes.to_string(),
        "--user",
        &format!("{}:{}", profile.uid, profile.gid),
        "--no-healthcheck",
        "--tmpfs",
        "/tmp:rw,nosuid,nodev,size=67108864,mode=1777",
        "--workdir",
        "/workspace",
        "--entrypoint",
        "/usr/bin/env",
        "--mount",
    ]);
    // Docker's mount grammar cannot safely encode these path separators.
    let source = host
        .workspace()
        .to_str()
        .filter(|path| !path.contains([',', '\n', '\r']))
        .ok_or(Error::UnsafePath)?;
    args.push(
        format!(
            "type=bind,source={source},target=/workspace{}",
            if host.admission.grant.write_workspace {
                ""
            } else {
                ",readonly"
            }
        )
        .into(),
    );
    if host.admission.grant.write_workspace {
        args.push("--mount".into());
        args.push(format!("type=bind,source={source}/.git,target=/workspace/.git,readonly").into());
    }
    args.extend(strings(&[
        &profile.image,
        "-i",
        "PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin",
        "RUSTUP_HOME=/usr/local/rustup",
        "HOME=/tmp",
        "TMPDIR=/tmp",
        "/bin/bash",
        "-c",
        script,
    ]));
    host.group_clear.set(false);
    let created = control(host, profile, sequence, "create", args, false).await;
    let id = match created {
        Ok(ref result) if result.ending.success() && !result.stdout.truncated => {
            result.stdout.text.trim().to_string()
        }
        _ => {
            // Creation can take effect before its reply is lost. Reconcile only
            // the exact retained name and ownership label; never reuse it.
            let clear = cleanup(host, profile, sequence, &name, &label).await;
            host.group_clear.set(clear);
            host.fail("container creation was incomplete");
            return Err(Error::InvalidTransition);
        }
    };
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        host.group_clear
            .set(cleanup(host, profile, sequence, &name, &label).await);
        host.fail("container identity was unavailable");
        return Err(Error::InvalidTransition);
    }
    let observation = run_started(host, profile, sequence, &id, deadline).await;
    let clear = cleanup(host, profile, sequence, &id, &label).await;
    host.group_clear.set(clear);
    if !clear {
        host.fail("container cleanup is unknown; no further effect is admitted");
    }
    let mut observation = observation?;
    observation.group_clear = clear;
    host.result(
        sequence,
        "container_command",
        json!({"container":id,"exit":observation.exit,
        "timed_out":observation.timed_out,"seconds":observation.seconds,"container_removed":clear,
        "cleanup_known":clear,"output":observation.output}),
    )?;
    Ok(observation)
}

async fn run_started(
    host: &Host,
    profile: &Profile,
    sequence: usize,
    id: &str,
    deadline: Duration,
) -> Result<CommandObservation, Error> {
    if host.cancelled() {
        return Err(Error::InvalidTransition);
    }
    let remaining = Duration::from_secs(host.wall_seconds()).saturating_sub(host.started.elapsed());
    if remaining.is_zero() {
        return Err(Error::LimitExceeded);
    }
    host.append(
        &Step::said(Source::System, "Container start intent retained.")
            .noting("container_start", json!({"effect":sequence,"container":id})),
    )?;
    let live = {
        let store = Store::open(&host.owner.dir)?;
        if store.show(&host.task.task_id)?.status != Status::Running {
            return Err(Error::InvalidTransition);
        }
        Job::from_command(profile.process(
            host.boundary.scratch().ok_or(Error::UnsafePath)?,
            &strings(&["start", "--attach", id]),
        ))
        .bounded(
            Limits::within(deadline.min(remaining))
                .keeping(host.admission.grant.stream_bytes)
                .memory(Some(host.admission.grant.memory_bytes)),
        )
        .start(Input::Null)
        .map_err(|_| Error::InvalidTransition)?
    };
    host.append(
        &Step::said(Source::System, "Container command started.").noting(
            "process",
            json!({"effect":sequence,"client_pid":live.pid(),"container":id}),
        ),
    )?;
    let mut stdout = Vec::new();
    let ended = loop {
        let delivery = live.take();
        if !delivery.gaps.is_empty() {
            host.output_incomplete.set(true);
        }
        stdout.extend_from_slice(&delivery.bytes);
        if !delivery.is_empty() && host.append(&Step::said(Source::System,&String::from_utf8_lossy(&delivery.bytes))
            .noting("stream",json!({"effect":sequence,"name":"stdout","offset":delivery.offset,"bytes":delivery.bytes,
                "gaps":delivery.gaps.iter().map(|gap|json!({"offset":gap.offset,"bytes":gap.bytes})).collect::<Vec<_>>()}))).is_err()
        { break live.stop().await; }
        if stdout.len() >= 2 * 1024 * 1024 {
            host.output_incomplete.set(true);
            break live.stop().await;
        }
        if host.cancelled() {
            break live.stop().await;
        }
        if live.finished() {
            break live.wait().await;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    stdout.extend_from_slice(&ended.rest.bytes);
    if !ended.rest.gaps.is_empty() || ended.stderr.truncated {
        host.output_incomplete.set(true);
    }
    host.append(&Step::said(Source::System,"Container command client stopped.")
        .noting("container_client_result",json!({"effect":sequence,"container":id,"client_group_clear":ended.group_clear,
            "exit":ended.ending.code(),"stdout_remainder":ended.rest.bytes,"stderr":ended.stderr.text,
            "stdout_bytes":ended.stdout_bytes,"stderr_bytes":ended.stderr.bytes})))?;
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&stdout),
        ended.stderr.marked()
    );
    Ok(CommandObservation {
        exit: ended.ending.code(),
        timed_out: ended.ending == supervise::Ending::TimedOut,
        seconds: ended.elapsed.as_secs_f64(),
        output,
        group_clear: false,
    })
}
