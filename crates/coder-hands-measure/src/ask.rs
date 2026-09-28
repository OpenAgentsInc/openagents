//! The one pass that asks the seam about a recorded run.
//!
//! A replay finds the windows the rules could not settle. This pass
//! sends each one, waits for the answer, and writes it beside the run.
//! It runs once a run: after it, the scorer reads the file, so a floor
//! that moves is measured against the same answers without another
//! request and without another minute at the camera.
//!
//! The pass sends what the desk sends. It builds the state through the
//! same `coder_hands::watch` call the compositor uses and runs the
//! request on `coder_hands::seam`, which is the thread the desk asks on,
//! so a round trip here is a round trip there.

use std::thread;
use std::time::{Duration, Instant};

use coder_hands::seam::Seam;

use crate::answers::Answer;
use crate::replay::Replay;

/// How long one window may take before the pass gives up on it.
const WAIT: Duration = Duration::from_secs(120);

/// How long a pass of the wait sleeps.
const TICK: Duration = Duration::from_millis(20);

/// Asks the seam about every ambiguous window in `replay` and answers
/// what came back, in row order.
///
/// # Errors
///
/// Returns the sentence that says why there is no seam, which for a
/// machine with no key says so without printing one, or the sentence
/// that names the window that never answered.
pub fn ask(replay: &Replay, say: impl Fn(&str)) -> Result<Vec<Answer>, String> {
    let windows: Vec<&crate::replay::Beat> = replay
        .beats
        .iter()
        .filter(|beat| beat.ask.is_some())
        .collect();
    if windows.is_empty() {
        say("No window in this run was ambiguous, so the seam has nothing to answer.");
        return Ok(Vec::new());
    }
    let (mut seam, _model) = coder_hands::seam::configured()?;
    say(&format!(
        "Asking about {} window(s), one request at a time.",
        windows.len()
    ));
    let mut out = Vec::new();
    for (done, beat) in windows.iter().enumerate() {
        let Some(request) = &beat.ask else {
            continue;
        };
        if request.findings > 0 {
            say(&format!(
                "Row {}: the scan took {} finding(s) out of the window before it went.",
                beat.row, request.findings
            ));
        }
        seam.ask(request.meta.clone(), request.state.clone())
            .map_err(|skip| format!("row {}: the seam sent nothing: {}", beat.row, skip.word()))?;
        let answer = wait(&mut seam, beat.row)?;
        out.push(Answer {
            row: beat.row,
            cue: beat.cue,
            label: beat.label.clone(),
            phase: beat.phase,
            window: answer.meta.window,
            pose: answer.meta.pose.clone(),
            margin: answer.meta.margin,
            rules: answer.meta.rules.clone(),
            elapsed_ms: answer.elapsed.as_millis() as u64,
            met_deadline: answer.met_deadline,
            report: answer.report.as_ref().ok().map(|report| report.view()),
            failed: answer.report.as_ref().err().cloned(),
        });
        if (done + 1) % 10 == 0 {
            say(&format!("{} of {} answered.", done + 1, windows.len()));
        }
    }
    Ok(out)
}

/// The answer to the window just sent. One request runs at a time, so
/// the next answer off the seam is this window's.
fn wait(seam: &mut Seam, row: usize) -> Result<coder_hands::seam::Answer, String> {
    let until = Instant::now() + WAIT;
    loop {
        if let Some(answer) = seam.take().into_iter().next() {
            return Ok(answer);
        }
        if Instant::now() >= until {
            return Err(format!(
                "row {row}: no answer came back inside {} seconds",
                WAIT.as_secs()
            ));
        }
        thread::sleep(TICK);
    }
}
