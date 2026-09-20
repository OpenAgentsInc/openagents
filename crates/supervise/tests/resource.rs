//! Resources used by a subprocess outlive cancellation of its caller.

use std::path::PathBuf;
use std::time::Duration;

use supervise::{Job, Limits};

struct Resource(PathBuf);

impl Drop for Resource {
    fn drop(&mut self) {
        std::fs::write(&self.0, b"released").unwrap();
    }
}

async fn wait_for(path: &std::path::Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the subprocess reached the checkpoint");
}

#[tokio::test]
async fn a_cancelled_caller_does_not_release_a_live_jobs_resource() {
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("pid");
    let released = directory.path().join("released");
    let command = Job::new("sh")
        .args([
            "-c",
            "trap '' TERM; echo $$ > pid; while :; do sleep 1; done",
        ])
        .in_directory(directory.path())
        .bounded(Limits::within(Duration::from_secs(30)));
    let task = tokio::spawn(command.run_holding(Resource(released.clone())));
    wait_for(&pid_file).await;
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    wait_for(&released).await;
    // SAFETY: signal zero checks existence and sends no signal. The PID
    // was read from the child this test started, which must be reaped now.
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[tokio::test]
async fn a_prepared_command_keeps_its_directory_and_environment_policy() {
    let directory = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new("/bin/sh");
    command.env_clear().env("BOUNDARY_MARKER", "kept");
    command.current_dir(directory.path()).args([
        "-c",
        "test -z \"${HOME+x}\" && test \"$BOUNDARY_MARKER\" = kept && pwd",
    ]);
    let ended = Job::from_command(command)
        .bounded(Limits::within(Duration::from_secs(5)))
        .run()
        .await;
    assert_eq!(ended.ending, supervise::Ending::Exited(Some(0)));
    assert_eq!(
        std::path::Path::new(ended.stdout.text.trim())
            .canonicalize()
            .unwrap(),
        directory.path().canonicalize().unwrap(),
    );
}
