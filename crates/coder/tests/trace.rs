//! A conversation records itself, and a session that is killed leaves
//! behind what it had.
//!
//! The second of those is the requirement the whole design is shaped
//! around, so it is tested the only way it can honestly be tested: this
//! file runs itself as a child process, waits for the child to record a
//! few steps, kills it outright, and reads the file back. Nothing in the
//! child gets a chance to tidy up, which is the point.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use coder::generate::{Door, StubGenerate};
use coder::{Agent, Outcome, Permit, Proposal, Recorder, Status};

/// Names the directory a child process records into. Set means "you are the
/// child"; unset means "you are the test".
const CHILD_ENV: &str = "CODER_TRACE_KILL_CHILD";

/// The test the child re-runs, which has to match the function's name.
const KILL_TEST: &str = "a_killed_session_leaves_the_steps_it_had";

/// A conversation writes itself down without anybody asking it to: the
/// person's turn, the instructions the model was given, and the answer.
#[tokio::test]
async fn a_conversation_records_itself_as_it_runs() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = Recorder::open(dir.path(), "a-model", "stub", "/tmp/repo").unwrap();
    let path = recorder.path().to_path_buf();
    let mut agent =
        Agent::new(None, Door::Stub(StubGenerate::default())).with_trace(Some(recorder));

    agent.push_user("what crates are here");
    // Without a classifier the turn generates unrouted, and the trace says
    // so rather than leaving a hole where the judgment would be.
    let skipped = agent.classify().await;
    assert!(matches!(skipped, coder::Classified::Skipped(_)));
    let coder::Turned { text: answer, .. } = agent
        .turn(
            false,
            Permit::executing(),
            &mut |_| {},
            &mut |_| {},
            &mut |_| {},
        )
        .await
        .unwrap();

    let document = atif::log::read(&path).unwrap().document();
    let steps = document["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4, "{document:#}");
    assert_eq!(steps[0]["source"], "user");
    assert_eq!(steps[0]["message"], "what crates are here");
    assert_eq!(steps[1]["source"], "system");
    assert!(
        steps[1]["message"]
            .as_str()
            .unwrap()
            .contains("decision door")
    );
    assert_eq!(steps[2]["source"], "system");
    assert_eq!(steps[2]["extra"]["kind"], "instructions");
    assert_eq!(steps[3]["source"], "agent");
    assert_eq!(steps[3]["message"], answer);
    assert_eq!(steps[3]["model_name"], "a-model");
    assert_eq!(document["extra"]["directive"], "what crates are here");
    // Still running, so the document does not claim the session ended.
    assert_eq!(document["extra"]["state"], atif::log::INTERRUPTED);

    agent.finish_trace();
    let closed = atif::log::read(&path).unwrap();
    assert_eq!(closed.session.state, atif::log::ENDED);
}

/// A session that is killed leaves a readable document of what it had.
///
/// The child writes three steps and then blocks, as a session does while a
/// turn is in flight. It is sent `SIGKILL`, so nothing flushes, nothing
/// closes, and no destructor runs. What is on disk is what the appends put
/// there.
#[test]
fn a_killed_session_leaves_the_steps_it_had() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        record_and_block(Path::new(&dir));
    }

    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([KILL_TEST, "--exact", "--nocapture"])
        .env(CHILD_ENV, dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the test binary runs itself");

    let path = wait_for_three_steps(dir.path(), &mut child);
    child.kill().expect("the child is killable");
    child.wait().expect("the child is reapable");

    let recording = atif::log::read(&path).expect("the log reads back");
    assert_eq!(recording.steps.len(), 3);
    // Nothing closed the log, so the document says what happened rather
    // than claiming the session finished.
    assert_eq!(recording.session.state, atif::log::INTERRUPTED);
    assert_eq!(recording.session.directive, "count the crates");

    let document = recording.document();
    assert_eq!(document["schema_version"], atif::SCHEMA_VERSION);
    assert_eq!(document["final_metrics"]["total_steps"], 3);
    assert_eq!(document["final_metrics"]["extra"]["tool_calls_total"], 1);
    assert_eq!(
        document["steps"][2]["observation"]["results"][0]["content"],
        "atif\ncoder"
    );
}

/// Polls the directory until the child has three steps on disk.
fn wait_for_three_steps(dir: &Path, child: &mut std::process::Child) -> PathBuf {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            panic!("the child exited early with {status}");
        }
        assert!(
            Instant::now() < deadline,
            "the child never recorded three steps"
        );
        if let Ok(found) = atif::log::list(dir)
            && let Some(path) = found.first()
            && atif::log::read(path).is_ok_and(|log| log.steps.len() >= 3)
        {
            return path.clone();
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// The child: record a few steps, then block the way a session blocks while
/// a turn is in flight. It is meant to be killed.
fn record_and_block(dir: &Path) -> ! {
    let mut recorder = Recorder::open(dir, "a-model", "stub", "/tmp/repo").unwrap();
    recorder.user("count the crates");
    recorder.instructions("you are Coder");
    recorder.command(&Outcome {
        proposal: Proposal {
            command: "ls crates".to_string(),
            why: "count them".to_string(),
        },
        status: Status::Exit(0),
        output: "atif\ncoder".to_string(),
        bytes: 10,
        elapsed: Duration::from_millis(9),
    });
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
