//! Bounded, identity-bound commands for qualified proxy templates. The spool
//! contains command metadata and output; real credentials stay in the broker.
use super::*;
const ROOT: &str = "/tmp/oa-public-commands";
const WORKSPACE: &str = "/workspace";
fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn valid(spec: &CommandSpec) -> bool {
    id_ok(&spec.id)
        && !spec.command.is_empty()
        && spec.command.len() <= 65536
        && !spec.command.contains('\0')
        && !spec.cwd.is_empty()
        && !spec.cwd.starts_with('/')
        && !spec.cwd.chars().any(char::is_control)
        && spec.cwd.split('/').all(|p| !p.is_empty() && p != "..")
        && (1..=3600).contains(&spec.timeout_seconds)
        && spec.digest.len() == 64
        && spec.digest.bytes().all(|b| b.is_ascii_hexdigit())
        && spec.credential_names.is_empty()
        && spec.env.len() <= 16
        && spec
            .env
            .iter()
            .all(|(k, v)| env_name_ok(k) && v.len() <= 4096 && !v.contains('\0'))
}
fn launch_script(root: &str, workspace: &str, spec: &CommandSpec) -> String {
    let script = crate::identified::bounded_command_script(root, workspace, spec);
    let directory = quote(&format!("{root}/{}", spec.id));
    format!(
        "umask 077\nnohup setsid sh -c {} </dev/null >/dev/null 2>&1 &\nfor attempt in $(seq 1 100); do\n if [ -f {directory}/spec ]; then [ \"$(cat {directory}/spec)\" = {} ] && exit 0; exit 98; fi\n sleep 0.02\ndone\nexit 125",
        quote(&script),
        quote(&spec.digest)
    )
}
impl SpaceDriver {
    pub(super) async fn start_identified(
        &self,
        computer: &str,
        spec: &CommandSpec,
    ) -> Outcome<String> {
        if !self.identified_commands {
            return Outcome::failed("This computer cannot start command sessions.");
        }
        if !valid(spec) {
            return Outcome::failed("Use valid command settings without guest credentials.");
        }
        let mut exec = Exec::new(launch_script(ROOT, WORKSPACE, spec));
        exec.timeout_seconds = 10;
        match self.exec(computer, &exec).await {
            Outcome::Done { value } if value.code == 0 => {
                Outcome::done(format!("space-command:{}", spec.id))
            }
            Outcome::Done { value } if value.code == 98 => {
                Outcome::failed("That command identity already has different settings.")
            }
            Outcome::Done { .. } => Outcome::unknown(
                "The command start could not be confirmed. Check the same command before retrying.",
            ),
            Outcome::Failed { reason } => Outcome::failed(reason),
            Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }
    pub(super) async fn read_identified(
        &self,
        computer: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        if !self.identified_commands || !id_ok(id) {
            return Outcome::failed("This command session is unsupported.");
        }
        let mut exec = Exec::new(crate::identified::read_script(
            ROOT,
            id,
            cursor,
            max_bytes.clamp(1, 65536),
        ));
        exec.timeout_seconds = 10;
        match self.exec(computer, &exec).await {
            Outcome::Done { value } if value.code == 0 => {
                match crate::identified::parse_read(&value.stdout) {
                    Ok(read) => Outcome::done(read),
                    Err(_) => Outcome::unknown("The command output is unavailable."),
                }
            }
            _ => Outcome::unknown("The command output is unavailable."),
        }
    }
    pub(super) async fn stop_identified(&self, computer: &str, id: &str) -> Outcome<String> {
        if !self.identified_commands || !id_ok(id) {
            return Outcome::failed("This command session is unsupported.");
        }
        let mut exec = Exec::new(crate::identified::bounded_stop_script(ROOT, id));
        exec.timeout_seconds = 10;
        match self.exec(computer, &exec).await {
            Outcome::Done { value } if value.code == 0 => Outcome::done(value.stdout),
            _ => Outcome::unknown(
                "The command stop could not be confirmed. Read its status before continuing.",
            ),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> CommandSpec {
        CommandSpec {
            id: "fixture".into(),
            command: "echo run >> invocations; printf proxy-command".into(),
            cwd: ".".into(),
            credential_names: BTreeSet::new(),
            env: BTreeMap::new(),
            timeout_seconds: 5,
            digest: "a".repeat(64),
        }
    }
    fn run(home: &std::path::Path, script: &str) -> std::process::Output {
        std::process::Command::new("sh")
            .args(["-c", script])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home)
            .output()
            .unwrap()
    }
    #[test]
    fn detached_commands_keep_one_identity_and_read_bounded_output() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("commands");
        let workspace = temp.path().join("workspace");
        let s = spec();
        let script = launch_script(root.to_str().unwrap(), workspace.to_str().unwrap(), &s);
        assert!(run(temp.path(), &script).status.success());
        assert!(run(temp.path(), &script).status.success());
        let mut final_read = None;
        for _ in 0..100 {
            let out = run(
                temp.path(),
                &crate::identified::read_script(
                    root.to_str().unwrap(),
                    &s.id,
                    CommandCursor::default(),
                    5,
                ),
            );
            let read =
                crate::identified::parse_read(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
            if matches!(read.progress, CommandProgress::Exited { .. }) {
                final_read = Some(read);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let read = final_read.unwrap();
        assert_eq!(read.stdout, b"proxy");
        assert_eq!(read.digest, Some(s.digest.clone()));
        assert_eq!(
            std::fs::read(workspace.join("invocations")).unwrap(),
            b"run\n"
        );
        let mut conflict = s;
        conflict.digest = "b".repeat(64);
        assert_eq!(
            run(
                temp.path(),
                &launch_script(
                    root.to_str().unwrap(),
                    workspace.to_str().unwrap(),
                    &conflict
                )
            )
            .status
            .code(),
            Some(98)
        );
    }
    #[tokio::test]
    async fn raw_credentials_and_unqualified_templates_refuse_before_network() {
        let driver = SpaceDriver::new(
            "https://unused.example",
            "https://unused.example",
            Secret::new("fixture"),
            "seed:latest",
        )
        .unwrap();
        assert!(!driver.capabilities().command_sessions);
        assert!(matches!(
            driver.start_command("sbx_fixture", &spec()).await,
            Outcome::Failed { .. }
        ));
        let profile = computer_credentials::policy::Profile::new(
            "test-v1",
            [computer_credentials::policy::Host::parse("api.example.com").unwrap()].into(),
        );
        let driver = driver
            .with_network_profile(profile)
            .unwrap()
            .with_credential_commands()
            .unwrap();
        assert!(driver.capabilities().command_sessions);
        let mut raw = spec();
        raw.credential_names.insert("ANTHROPIC_API_KEY".into());
        raw.env
            .insert("ANTHROPIC_API_KEY".into(), "synthetic-fixture".into());
        assert!(matches!(
            driver.start_command("sbx_fixture", &raw).await,
            Outcome::Failed { .. }
        ));
    }
}
