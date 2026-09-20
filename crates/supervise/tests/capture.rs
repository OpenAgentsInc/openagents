//! What a job is allowed to print, and what that costs while it prints it.
//!
//! The bound that matters is on the bytes the supervisor holds, not on the
//! string it hands back at the end, so every test here asks a program for
//! far more output than its cap allows and then asks what was kept.

use std::time::Duration;

use supervise::{Ending, Job, Limits};

/// A shell that prints `lines` lines of sixteen bytes to a stream.
fn noise(stream: &str, lines: usize) -> String {
    format!("yes 0123456789abcde | head -n {lines} {stream}")
}

#[tokio::test]
async fn oversized_output_is_cut_as_it_arrives() {
    // One megabyte offered against a four kibibyte cap.
    let ended = Job::new("sh")
        .arg("-c")
        .arg(noise("", 65536))
        .bounded(Limits::within(Duration::from_secs(30)).keeping(4096))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::Exited(Some(0)));
    assert!(ended.stdout.text.len() <= 4096);
    assert_eq!(ended.stdout.bytes, 65536 * 16);
    assert!(ended.stdout.truncated);
    assert!(ended.stdout.marked().ends_with("1048576 bytes in all"));
}

#[tokio::test]
async fn both_streams_are_bounded_at_once() {
    let ended = Job::new("sh")
        .arg("-c")
        .arg(format!(
            "{} & {}; wait",
            noise(">&2", 20000),
            noise("", 20000)
        ))
        .bounded(Limits::within(Duration::from_secs(30)).keeping(2048))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::Exited(Some(0)));
    assert!(ended.stdout.text.len() <= 2048);
    assert!(ended.stderr.text.len() <= 2048);
    assert!(ended.stdout.truncated && ended.stderr.truncated);
    assert_eq!(ended.bytes(), 2 * 20000 * 16);
}

#[tokio::test]
async fn a_producer_that_never_stops_is_bounded_and_ended() {
    let ended = Job::new("sh")
        .arg("-c")
        .arg("while :; do printf 0123456789abcdef; done")
        .bounded(Limits::within(Duration::from_millis(300)).keeping(4096))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::TimedOut);
    assert!(ended.stdout.text.len() <= 4096);
    assert!(ended.stdout.truncated);
    assert!(
        ended.elapsed < Duration::from_secs(5),
        "draining held the job open past its bound: {:?}",
        ended.elapsed
    );
}

#[tokio::test]
async fn a_timed_out_job_keeps_what_it_printed() {
    let ended = Job::new("sh")
        .arg("-c")
        .arg("printf 'the useful part'; printf 'and why' >&2; sleep 30")
        .bounded(Limits::within(Duration::from_millis(200)))
        .run()
        .await;

    assert_eq!(ended.ending, Ending::TimedOut);
    assert_eq!(ended.stdout.text, "the useful part");
    assert_eq!(ended.stderr.text, "and why");
    assert!(!ended.truncated());
}

#[tokio::test]
async fn a_cap_inside_a_character_keeps_valid_text() {
    // Every character is two bytes, so an odd cap always lands inside one.
    let ended = Job::new("sh")
        .arg("-c")
        .arg("yes 'ééééééééé' | head -n 4000")
        .bounded(Limits::within(Duration::from_secs(30)).keeping(1025))
        .run()
        .await;

    assert!(ended.stdout.truncated);
    assert!(ended.stdout.text.len() <= 1025);
    assert!(
        !ended.stdout.text.contains('\u{fffd}'),
        "a cap inside a character left a replacement behind"
    );
}
