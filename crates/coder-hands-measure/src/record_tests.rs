//! The recorder writes what it was told and what the camera sent, with
//! no camera in the test: the frames come over the same channel the
//! socket's thread sends them on.

use super::*;
use std::sync::mpsc::{self, Sender};
use std::thread;

use crate::run::Run;
use coder_hands::wire::{Point, WireHand};

/// A landmark line with one hand, or none.
fn line(t: f64, hand: bool) -> Line {
    Line {
        timestamp: t,
        hands: if hand {
            vec![WireHand {
                landmarks: vec![Point::default(); 21],
                pose: "none".into(),
            }]
        } else {
            Vec::new()
        },
        status: if hand {
            "1 hand(s) detected".into()
        } else {
            "No hands detected".into()
        },
        dropped: 0,
    }
}

/// A recorder writing to a file of its own, and the sender that stands
/// in for the daemon.
fn recorder(name: &str) -> (Recorder<std::fs::File>, Sender<Line>, PathBuf) {
    let dir = std::env::temp_dir().join(format!("coder-hands-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    let path = dir.join("run.jsonl");
    let file = std::fs::File::create(&path).expect("the run file");
    let (tx, rx) = mpsc::channel();
    (
        Recorder {
            frames: rx,
            out: file,
            path: path.clone(),
            frames_read: 0,
            frames_with_hand: 0,
        },
        tx,
        path,
    )
}

#[test]
fn a_run_goes_under_openagents_and_carries_the_moment_it_started() {
    let path = default_path(Path::new("/home/me"), 1_789_000_000.5);
    assert_eq!(
        path,
        PathBuf::from("/home/me/.openagents/hands/runs/run-1789000000.jsonl")
    );
}

#[test]
fn a_cue_writes_its_count_in_as_rest_and_its_frames_as_the_gesture() {
    let (mut recorder, tx, path) = recorder("cue");
    let cue = Cue {
        label: "point".into(),
        prompt: "Point at the screen.".into(),
        ready: 1.0,
        seconds: 0.5,
    };
    let feeder = thread::spawn(move || {
        for frame in 0..60 {
            if tx
                .send(line(1_789_000_000.0 + f64::from(frame) / 30.0, true))
                .is_err()
            {
                return;
            }
            thread::sleep(Duration::from_millis(33));
        }
    });
    let header = Header::new(1_789_000_000.0, None, None, 16.0 / 9.0, vec![cue.clone()]);
    recorder.write(&header.render()).expect("the header");
    recorder.cue(0, &cue).expect("the cue");
    assert!(recorder.frames_read > 0, "{} frames", recorder.frames_read);
    assert_eq!(recorder.frames_with_hand, recorder.frames_read);
    recorder.finish().expect("the file closes");
    drop(feeder);
    let run = Run::read(&path).expect("the run reads back");
    assert_eq!(run.header.script.len(), 1);
    assert!(
        run.rows
            .iter()
            .all(|row| row.label == "point" && row.cue == 0)
    );
    assert!(run.rows.iter().any(|row| row.phase == Phase::Ready));
    assert!(run.rows.iter().any(|row| row.phase == Phase::Record));
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/tmp")));
}

#[test]
fn an_empty_room_records_frames_and_no_hand() {
    let (mut recorder, tx, path) = recorder("empty");
    let cue = Cue {
        label: "fist".into(),
        prompt: "Close your hand into a fist.".into(),
        ready: 1.0,
        seconds: 0.3,
    };
    let feeder = thread::spawn(move || {
        for frame in 0..30 {
            if tx
                .send(line(1_789_000_000.0 + f64::from(frame) / 15.0, false))
                .is_err()
            {
                return;
            }
            thread::sleep(Duration::from_millis(66));
        }
    });
    let header = Header::new(1_789_000_000.0, None, None, 16.0 / 9.0, vec![cue.clone()]);
    recorder.write(&header.render()).expect("the header");
    recorder
        .phase(0, &cue, Phase::Record, 0.4)
        .expect("the phase");
    assert!(recorder.frames_read > 0);
    assert_eq!(recorder.frames_with_hand, 0);
    recorder.finish().expect("the file closes");
    drop(feeder);
    let run = Run::read(&path).expect("the rows read back");
    assert!(run.rows.iter().all(|row| row.line.hands.is_empty()));
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/tmp")));
}
