//! The stdin guards on `--body-file -` and `--token-stdin` (#178), and the
//! tool-path stdin fix (#180): a spawn through the coder tool registry must
//! fail fast instead of hanging when stdin never arrives.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Where the CLI binary under test comes from: the crate's own build.
fn cli_path() -> std::path::PathBuf {
    // Cargo provides the exact path to the integration test's sibling binary.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_openagents"))
}

/// Spawn the CLI with stdin **closed** (what #180 now guarantees at the tool
/// path) and assert it exits inside the bound — the no-hang property.
fn assert_exits_with_closed_stdin(args: &[&str], bound: Duration) -> String {
    let start = Instant::now();
    let mut child = Command::new(cli_path())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the CLI");
    loop {
        match child.try_wait().expect("wait on the CLI") {
            Some(status) => {
                assert!(!status.success(), "expected a refusal, got success");
                let output = child.wait_with_output().expect("collect the output");
                return format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            None if start.elapsed() > bound => {
                let _ = child.kill();
                panic!(
                    "the CLI did not exit within {bound:?} with closed stdin: \
                     the stdin hang (#178) is back"
                );
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[test]
fn body_file_dash_with_closed_stdin_refuses_instead_of_hanging() {
    // The exact call that hung the session in #178: no stdin ever arrives.
    let message = assert_exits_with_closed_stdin(
        &["issue", "create", "--title", "x", "--body-file", "-"],
        Duration::from_secs(10),
    );
    assert!(
        message.to_lowercase().contains("stdin") || message.to_lowercase().contains("empty"),
        "the refusal should name stdin or the empty body: {message}"
    );
}

#[test]
fn body_file_dash_with_empty_pipe_refuses_with_empty_body_guidance() {
    let mut child = Command::new(cli_path())
        .args(["issue", "create", "--title", "x", "--body-file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the CLI");
    // Closing stdin immediately delivers EOF with no content.
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait");
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("the body read from stdin is empty"),
        "the refusal should say the body was empty: {text}"
    );
}

#[test]
fn body_file_dash_with_piped_content_still_works() {
    // The legitimate pipeline case #178 pins: EOF arrives with content, so no
    // guard fires. It must reach the API (and fail on auth/network, not on a
    // body guard) — or succeed where a stub server exists.
    let mut child = Command::new(cli_path())
        .args(["issue", "create", "--title", "x", "--body-file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the CLI");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(b"a real body\n")
        .expect("write the body");
    let output = child.wait_with_output().expect("wait");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let refused_by_guard = text.contains("the body read from stdin is empty")
        || text.contains("refusing to read the body from a terminal");
    assert!(
        !refused_by_guard,
        "a piped body must not be refused by the stdin guards: {text}"
    );
}

#[test]
fn body_file_dash_from_a_tty_refuses_immediately() {
    // A pty makes stdin a terminal. Without the TTY guard the CLI would wait
    // for a human to press Ctrl-D forever. The child is spawned with a real
    // pty as its controlling terminal via Python; macOS can refuse concurrent
    // pty opens under a full workspace run, so a probe failure to *open* the
    // pty is a quiet skip — the closed-stdin test above already pins the
    // no-hang property.
    let probe = std::process::Command::new("python3")
        .arg("-c")
        .arg("import os, fcntl, termios; m, s = os.openpty(); print('ok')")
        .stdin(Stdio::null())
        .output()
        .expect("probe the pty system");
    if !probe.status.success() {
        return;
    }
    // Spawn through Python's pty module the way a real session would: the
    // child owns the slave as its controlling terminal and stdio. portable-
    // pty's own `spawn_command` was flaky here under the test harness.
    let exe = env!("CARGO_BIN_EXE_openagents");
    let script = format!(
        "import os, sys, time, fcntl, termios\n\
         master, slave = os.openpty()\n\
         pid = os.fork()\n\
         if pid == 0:\n\
         \x20   os.setsid()\n\
         \x20   fcntl.ioctl(slave, termios.TIOCSCTTY, 0)\n\
         \x20   os.dup2(slave, 0); os.dup2(slave, 1); os.dup2(slave, 2)\n\
         \x20   os.close(master); os.close(slave)\n\
         \x20   os.execv({exe:?}, [{exe:?}, 'issue', 'create', '--title', 'x',\n\
         \x20                       '--body-file', '-', '-R', 'OpenAgentsInc/openagents'])\n\
         os.close(slave)\n\
         start = time.time()\n\
         while time.time() - start < 8:\n\
         \x20   done, status = os.waitpid(pid, os.WNOHANG)\n\
         \x20   if done:\n\
         \x20       print(os.waitstatus_to_exitcode(status))\n\
         \x20       sys.exit(0)\n\
         \x20   time.sleep(0.2)\n\
         print('HUNG')\n\
         os.kill(pid, 9)\n\
         sys.exit(1)\n"
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run the pty probe");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !text.contains("HUNG"),
        "the CLI hung on a TTY stdin; the TTY guard is missing"
    );
    assert!(
        text.trim().ends_with('2'),
        "a TTY stdin must be refused (exit 2), got: {text}"
    );
}

#[test]
fn token_stdin_with_closed_stdin_refuses_instead_of_hanging() {
    let message = assert_exits_with_closed_stdin(
        &["auth", "login", "--token-stdin"],
        Duration::from_secs(10),
    );
    assert!(
        message.to_lowercase().contains("stdin") || message.to_lowercase().contains("token"),
        "the refusal should name stdin or the token: {message}"
    );
}
