//! Identity-bound shell wrappers shared by provider adapters.
use crate::{CommandCursor, CommandProgress, CommandRead, CommandSpec};
use base64::Engine;

/// The wrapper's exit code when the identity was already claimed.
pub const ALREADY_CLAIMED: i32 = 97;

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn join_dir(workdir: &str, cwd: &str) -> String {
    if cwd == "." {
        workdir.to_owned()
    } else {
        format!("{workdir}/{cwd}")
    }
}

/// The detached wrapper for one identified command. `unset` lists the
/// selected credentials this command did not name.
pub fn command_script(root: &str, workdir: &str, spec: &CommandSpec, unset: &[&str]) -> String {
    let d = shell_quote(&format!("{root}/{}", spec.id));
    let mut env = vec!["env".to_owned()];
    env.extend(unset.iter().map(|n| format!("-u {n}")));
    env.extend(
        spec.env
            .iter()
            .map(|(k, v)| format!("{k}={}", shell_quote(v))),
    );
    format!(
        "mkdir -p {root} && mkdir {d} 2>/dev/null || exit {ALREADY_CLAIMED}\n\
         printf %s {digest} > {d}/spec\n\
         mkdir -p {workdir}\n\
         cd {cwd} || {{ echo 126 > {d}/exit.tmp; mv {d}/exit.tmp {d}/exit; exit 126; }}\n\
         {env} sh -c {command} </dev/null >{d}/stdout 2>{d}/stderr &\n\
         echo $! > {d}/pid\n\
         wait $!\n\
         code=$?\n\
         echo $code > {d}/exit.tmp && mv {d}/exit.tmp {d}/exit\n\
         exit $code",
        root = shell_quote(root),
        digest = shell_quote(&spec.digest),
        workdir = shell_quote(workdir),
        cwd = shell_quote(&join_dir(workdir, &spec.cwd)),
        env = env.join(" "),
        command = shell_quote(&spec.command),
    )
}

/// A read-only probe: state line, spec digest line, then base64 stdout
/// and stderr from the cursor. State is read before output, so an `exit`
/// state means the output that follows is final.
pub fn read_script(root: &str, id: &str, cursor: CommandCursor, max_bytes: u64) -> String {
    let d = shell_quote(&format!("{root}/{id}"));
    format!(
        "d={d}\n\
         if [ ! -d \"$d\" ]; then echo absent; echo; echo; echo; exit 0; fi\n\
         if [ -f \"$d/exit\" ]; then echo \"exit $(cat \"$d/exit\")\"\n\
         elif [ ! -f \"$d/pid\" ]; then echo running\n\
         elif kill -0 \"$(cat \"$d/pid\")\" 2>/dev/null; then echo running\n\
         else sleep 1; if [ -f \"$d/exit\" ]; then echo \"exit $(cat \"$d/exit\")\"; else echo lost; fi; fi\n\
         cat \"$d/spec\" 2>/dev/null; echo\n\
         for s in stdout:{o} stderr:{e}; do f=\"$d/${{s%%:*}}\"; n=${{s##*:}}; \
         if [ -f \"$f\" ]; then tail -c +$((n+1)) \"$f\" | head -c {max_bytes} | base64 | tr -d '\\n'; fi; echo; done\n\
         if [ -f \"$d/timed_out\" ]; then echo timeout; fi",
        o = cursor.stdout,
        e = cursor.stderr,
    )
}

/// Parse [`read_script`] output.
pub fn parse_read(stdout: &str) -> Result<CommandRead, &'static str> {
    let mut lines = stdout.split('\n');
    let state = lines.next().unwrap_or_default().trim();
    let digest = lines.next().unwrap_or_default().trim();
    let decode = |line: Option<&str>| {
        base64::engine::general_purpose::STANDARD
            .decode(line.unwrap_or_default().trim())
            .map_err(|_| "The command output did not decode.")
    };
    let stdout = decode(lines.next())?;
    let stderr = decode(lines.next())?;
    let progress = match state {
        "absent" => CommandProgress::Absent,
        "running" => CommandProgress::Running,
        "lost" => CommandProgress::Lost,
        _ => match state.strip_prefix("exit ").map(|c| c.trim().parse::<i64>()) {
            Some(Ok(code)) => CommandProgress::Exited { code },
            _ => return Err("The command state is not recognized."),
        },
    };
    Ok(CommandRead {
        timed_out: lines.next().is_some_and(|s| s.trim() == "timeout"),
        progress,
        digest: (!digest.is_empty()).then(|| digest.to_owned()),
        stdout,
        stderr,
    })
}

/// Signal a command's process tree; the wrapper then records exit 143.
pub fn stop_script(root: &str, id: &str) -> String {
    let d = shell_quote(&format!("{root}/{id}"));
    format!(
        "d={d}\n\
         if [ -f \"$d/exit\" ]; then echo exited; exit 0; fi\n\
         if [ ! -f \"$d/pid\" ]; then echo absent; exit 0; fi\n\
         t() {{ echo \"$1\"; for c in $(pgrep -P \"$1\"); do t \"$c\"; done; }}\n\
         p=$(cat \"$d/pid\"); kill -s TERM $(t \"$p\") 2>/dev/null; echo stopped"
    )
}

/// Capture a bounded prefix of both streams while draining excess bytes.
/// The extra byte makes truncation observable even before the process exits.
pub const OUTPUT_LIMIT: u64 = 256 * 1024;

/// Public command wrapper. Requires Linux coreutils (`setsid`, `timeout`, and
/// `realpath`, and `stdbuf`). Each command gets a session so cancellation includes children.
pub fn bounded_command_script(root: &str, workdir: &str, spec: &CommandSpec) -> String {
    let d = shell_quote(&format!("{root}/{}", spec.id));
    let env = spec
        .env
        .iter()
        .map(|(k, v)| format!("{k}={}", shell_quote(v)))
        .collect::<Vec<_>>()
        .join(" ");
    let command = shell_quote(&format!(
        "sh -c {}; code=$?; printf %s \"$code\" > {d}/command_exit; exit \"$code\"",
        shell_quote(&spec.command)
    ));
    format!(
        r#"umask 077
mkdir -p {root} || exit 125
mkdir {d} 2>/dev/null || exit {ALREADY_CLAIMED}
d={d}
printf %s {digest} > "$d/spec" || exit 125
finish() {{ echo "$1" > "$d/exit.tmp"; mv "$d/exit.tmp" "$d/exit"; }}
mkdir -p {workdir} || {{ finish 126; exit 126; }}
[ -L {workdir} ] && {{ finish 126; exit 126; }}
base=$(realpath -e {workdir}) || {{ finish 126; exit 126; }}
cwd=$(realpath -e {cwd}) || {{ finish 126; exit 126; }}
case "$cwd/" in "$base/"*) ;; *) finish 126; exit 126;; esac
cd "$cwd" || {{ finish 126; exit 126; }}
mkfifo "$d/out.pipe" "$d/err.pipe" || {{ finish 125; exit 125; }}
(failed=0; stdbuf -o0 head -c {limit} > "$d/stdout" || failed=1; cat >/dev/null || failed=1; exit "$failed") < "$d/out.pipe" &
o=$!
(failed=0; stdbuf -o0 head -c {limit} > "$d/stderr" || failed=1; cat >/dev/null || failed=1; exit "$failed") < "$d/err.pipe" &
e=$!
setsid timeout --signal=TERM --kill-after=2 {seconds} env {env} sh -c {command} </dev/null > "$d/out.pipe" 2> "$d/err.pipe" &
p=$!
# Start time and boot identity keep a stale record from signalling a reused PID.
cat /proc/sys/kernel/random/boot_id > "$d/boot"
awk '{{print $22}}' /proc/$p/stat > "$d/start" 2>/dev/null
printf %s "$p" > "$d/pid.tmp"; mv "$d/pid.tmp" "$d/pid"
wait "$p"
code=$?
if [ ! -f "$d/command_exit" ] && {{ [ "$code" = 124 ] || [ "$code" = 137 ]; }}; then touch "$d/timed_out"; fi
# Shell background jobs must not outlive a completed command.
env kill -KILL -- -"$p" 2>/dev/null || true
captured=1
wait "$o" || captured=0
wait "$e" || captured=0
rm -f "$d/out.pipe" "$d/err.pipe"
[ "$captured" = 1 ] || exit 125
finish "$code"
exit "$code"
"#,
        root = shell_quote(root),
        digest = shell_quote(&spec.digest),
        workdir = shell_quote(workdir),
        cwd = shell_quote(&join_dir(workdir, &spec.cwd)),
        limit = OUTPUT_LIMIT + 1,
        seconds = spec.timeout_seconds,
        command = command
    )
}

/// Cancellation for bounded commands. An acknowledgement means only that the
/// signal was sent; callers still observe the command's exit record.
pub fn bounded_stop_script(root: &str, id: &str) -> String {
    let d = shell_quote(&format!("{root}/{id}"));
    format!(
        r#"d={d}
if [ -f "$d/exit" ]; then echo exited; exit 0; fi
if [ ! -f "$d/pid" ]; then echo unknown; exit 0; fi
p=$(cat "$d/pid")
case "$p" in ''|*[!0-9]*) echo unknown; exit 0;; esac
boot=$(cat /proc/sys/kernel/random/boot_id)
start=$(awk '{{print $22}}' /proc/$p/stat 2>/dev/null)
if [ "$boot" != "$(cat "$d/boot")" ] || [ -z "$start" ] || [ "$start" != "$(cat "$d/start")" ]; then echo unknown; exit 0; fi
env kill -TERM -- -"$p" 2>/dev/null || true
sleep 2
# The process group may still contain children after its leader exits.
now=$(awk '{{print $22}}' /proc/$p/stat 2>/dev/null)
if [ -n "$now" ] && [ "$now" != "$start" ]; then echo unknown; exit 0; fi
env kill -KILL -- -"$p" 2>/dev/null || true
echo signalled
"#
    )
}

// The scripts read /proc, which only Linux has.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Duration;
    use tokio::process::Command;

    fn spec(command: &str) -> CommandSpec {
        CommandSpec {
            id: "trn_0123456789abcdef0123".into(),
            command: command.into(),
            cwd: ".".into(),
            timeout_seconds: 30,
            credential_names: BTreeSet::new(),
            env: BTreeMap::new(),
            digest: "a".repeat(64),
        }
    }
    async fn shell(script: String) -> std::process::Output {
        Command::new("sh")
            .arg("-c")
            .arg(script)
            .output()
            .await
            .unwrap()
    }
    async fn read(root: &str, id: &str, cursor: CommandCursor) -> CommandRead {
        let o = shell(read_script(root, id, cursor, 65536)).await;
        assert!(o.status.success());
        parse_read(&String::from_utf8(o.stdout).unwrap()).unwrap()
    }
    async fn completed(root: &str, id: &str) -> CommandRead {
        let until = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let r = read(root, id, CommandCursor::default()).await;
            if matches!(r.progress, CommandProgress::Exited { .. }) {
                return r;
            }
            assert!(tokio::time::Instant::now() < until);
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }
    #[tokio::test]
    async fn retained_output_precedes_attachment_and_duplicate_start_has_one_effect() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("commands");
        let work = dir.path().join("workspace");
        let root = root.to_str().unwrap();
        let work = work.to_str().unwrap();
        let s = spec("echo effect >> counter; printf early; printf error >&2; exit 7");
        let script = bounded_command_script(root, work, &s);
        let a = Command::new("sh").arg("-c").arg(&script).spawn().unwrap();
        let b = shell(script).await;
        let _ = a.wait_with_output().await.unwrap();
        assert!(b.status.code() == Some(ALREADY_CLAIMED) || b.status.code() == Some(7));
        let r = completed(root, &s.id).await;
        assert_eq!(r.progress, CommandProgress::Exited { code: 7 });
        assert!(!r.timed_out);
        assert_eq!(r.stdout, b"early");
        assert_eq!(r.stderr, b"error");
        let next = read(
            root,
            &s.id,
            CommandCursor {
                stdout: 2,
                stderr: 1,
            },
        )
        .await;
        assert_eq!(next.stdout, b"rly");
        assert_eq!(next.stderr, b"rror");
        assert_eq!(
            std::fs::read_to_string(format!("{work}/counter")).unwrap(),
            "effect\n"
        );
        assert_eq!(
            shell(bounded_command_script(root, work, &s))
                .await
                .status
                .code(),
            Some(ALREADY_CLAIMED)
        );
    }
    #[tokio::test]
    async fn short_output_is_readable_while_the_command_is_still_running() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("commands");
        let work = dir.path().join("workspace");
        let root = root.to_str().unwrap();
        let work = work.to_str().unwrap();
        let s = spec("printf early; printf error >&2; sleep 30");
        let wrapper = Command::new("sh")
            .arg("-c")
            .arg(bounded_command_script(root, work, &s))
            .spawn()
            .unwrap();
        let live = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let r = read(root, &s.id, CommandCursor::default()).await;
                if r.stdout == b"early" && r.stderr == b"error" {
                    assert_eq!(r.progress, CommandProgress::Running);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        let _ = shell(bounded_stop_script(root, &s.id)).await;
        tokio::time::timeout(Duration::from_secs(8), wrapper.wait_with_output())
            .await
            .unwrap()
            .unwrap();
        assert!(live.is_ok(), "short output was buffered until command exit");
    }

    #[tokio::test]
    async fn flooding_stays_bounded_without_any_reader() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("commands");
        let work = dir.path().join("workspace");
        let root = root.to_str().unwrap();
        let work = work.to_str().unwrap();
        let s = spec("head -c 8388608 /dev/zero; head -c 8388608 /dev/zero >&2");
        assert!(
            shell(bounded_command_script(root, work, &s))
                .await
                .status
                .success()
        );
        for stream in ["stdout", "stderr"] {
            assert_eq!(
                std::fs::metadata(format!("{root}/{}/{stream}", s.id))
                    .unwrap()
                    .len(),
                OUTPUT_LIMIT + 1
            );
        }
        let r = read(
            root,
            &s.id,
            CommandCursor {
                stdout: OUTPUT_LIMIT,
                stderr: OUTPUT_LIMIT,
            },
        )
        .await;
        assert_eq!(r.stdout.len(), 1);
        assert_eq!(r.stderr.len(), 1);
    }
    fn alive(pid: &str) -> bool {
        std::fs::read_to_string(format!("/proc/{}/stat", pid.trim()))
            .ok()
            .is_some_and(|s| !s.contains(") Z "))
    }
    #[tokio::test]
    async fn cancellation_and_timeout_end_the_parent_and_child() {
        for cancel in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("commands");
            let work = dir.path().join("workspace");
            let root = root.to_str().unwrap();
            let work = work.to_str().unwrap();
            let mut s = spec("echo $$ > parent; trap '' TERM; sleep 100 & echo $! > child; wait");
            s.timeout_seconds = if cancel { 30 } else { 1 };
            let wrapper = Command::new("sh")
                .arg("-c")
                .arg(bounded_command_script(root, work, &s))
                .spawn()
                .unwrap();
            let until = tokio::time::Instant::now() + Duration::from_secs(5);
            while !std::path::Path::new(&format!("{work}/child")).exists() {
                assert!(tokio::time::Instant::now() < until);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if cancel {
                let _ = shell(bounded_stop_script(root, &s.id)).await;
            }
            let _ = tokio::time::timeout(Duration::from_secs(8), wrapper.wait_with_output())
                .await
                .unwrap()
                .unwrap();
            let r = completed(root, &s.id).await;
            assert_eq!(r.timed_out, !cancel);
            for name in ["parent", "child"] {
                let pid = std::fs::read_to_string(format!("{work}/{name}")).unwrap();
                assert!(!alive(&pid), "{name} {pid} survived");
            }
        }
    }
    #[tokio::test]
    async fn source_symlink_cannot_escape_and_shell_exit_124_is_not_a_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("commands");
        let work = dir.path().join("workspace");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, work.join("escape")).unwrap();
        let root = root.to_str().unwrap();
        let work = work.to_str().unwrap();
        let mut s = spec("touch unexpected");
        s.cwd = "escape".into();
        assert_eq!(
            shell(bounded_command_script(root, work, &s))
                .await
                .status
                .code(),
            Some(126)
        );
        assert!(!outside.join("unexpected").exists());
        s.id.push('b');
        s.cwd = ".".into();
        s.command = "exit 124".into();
        let _ = shell(bounded_command_script(root, work, &s)).await;
        let r = completed(root, &s.id).await;
        assert_eq!(r.progress, CommandProgress::Exited { code: 124 });
        assert!(!r.timed_out);
    }
}
