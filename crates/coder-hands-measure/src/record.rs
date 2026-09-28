//! The recorder: a scripted minute at a camera, written down.
//!
//! It opens whichever camera this machine has through [`crate::camera`],
//! reads the landmark lines it publishes, and prints one prompt at a
//! time with a count-in before each. Every line it reads goes to the run
//! file with the prompt that was on screen as its label, so the file
//! says what you were doing as well as what the camera saw. You type
//! nothing between gestures.
//!
//! On a CoderOS host the recorder reads the same socket the desk reads
//! and changes nothing else, so it runs while the desk runs. It leaves
//! the tracker on, the way it finds it on a desk whose hands drive the
//! pointer.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

use coder_hands::gestures;
use coder_hands::wire::Line;

use crate::camera;
use crate::run::{self, Cue, Header, Phase, Row};

/// How long a pass of the loop waits for a frame before it reads the
/// clock again.
const TICK: Duration = Duration::from_millis(100);

/// What a run is recorded with.
#[derive(Clone, Debug)]
pub struct Options {
    /// Where the run is written. The default is a file named for the
    /// moment it started, under `~/.openagents/hands/runs/`.
    pub out: Option<PathBuf>,
    /// How many times the run asks for the whole gesture list.
    pub passes: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            out: None,
            passes: run::PASSES,
        }
    }
}

/// Records one run and answers where it was written.
///
/// # Errors
///
/// Returns the sentence that names what stopped the run: no daemon, no
/// socket, or a file that would not open.
pub fn record(options: &Options) -> Result<PathBuf, String> {
    let script = run::script(options.passes.max(1));
    let started = started();
    let path = match &options.out {
        Some(path) => path.clone(),
        None => {
            let home = std::env::var("HOME").map_err(|_| "HOME names no directory".to_string())?;
            default_path(Path::new(&home), started)
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    say(&format!(
        "This run asks for {} gesture(s) and takes about {:.0} seconds.",
        script.len(),
        run::script_seconds(&script)
    ));
    say("Sit where the camera sees one hand, and follow each prompt.");
    let camera = camera::open(say)?;
    say(&format!("Recording from {}.", camera.source.word()));
    let header = Header::new(
        started,
        Some(camera.source),
        camera.capture.clone(),
        gestures::aspect_from(camera.capture),
        script.clone(),
    );
    let file = std::fs::File::create(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut recorder = Recorder {
        frames: camera.frames,
        out: std::io::BufWriter::new(file),
        path: path.clone(),
        frames_read: 0,
        frames_with_hand: 0,
    };
    recorder.write(&header.render())?;
    for (index, cue) in script.iter().enumerate() {
        recorder.cue(index, cue)?;
    }
    recorder.finish()
}

/// The loop that prompts, reads, and writes.
struct Recorder<W: Write> {
    frames: Receiver<Line>,
    out: W,
    path: PathBuf,
    frames_read: usize,
    frames_with_hand: usize,
}

impl<W: Write> Recorder<W> {
    /// One cue: the count-in, then the frames the scorer reads.
    fn cue(&mut self, index: usize, cue: &Cue) -> Result<(), String> {
        say("");
        say(&format!("Next: {}", cue.prompt));
        let steps = cue.ready.max(1.0).round() as usize;
        for step in (1..=steps).rev() {
            say(&format!("{step}"));
            self.phase(index, cue, Phase::Ready, 1.0)?;
        }
        say(&format!("Now: {}", cue.prompt));
        if self.phase(index, cue, Phase::Record, cue.seconds)? == 0 {
            say("No hand in the frame for that one.");
        }
        Ok(())
    }

    /// Frames for `seconds`, each written with this cue and phase.
    /// Answers how many of them carried a hand.
    fn phase(
        &mut self,
        index: usize,
        cue: &Cue,
        phase: Phase,
        seconds: f64,
    ) -> Result<usize, String> {
        let until = Instant::now() + Duration::from_secs_f64(seconds.max(0.0));
        let mut held = 0;
        while Instant::now() < until {
            match self.frames.recv_timeout(TICK) {
                Ok(line) => {
                    self.frames_read += 1;
                    if !line.hands.is_empty() {
                        self.frames_with_hand += 1;
                        held += 1;
                    }
                    let row = Row {
                        cue: index,
                        label: cue.label.clone(),
                        phase,
                        line,
                    };
                    self.write(&row.render())?;
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("the camera stopped publishing".to_string());
                }
            }
        }
        Ok(held)
    }

    /// One line to the run file.
    fn write(&mut self, text: &str) -> Result<(), String> {
        self.out
            .write_all(text.as_bytes())
            .map_err(|err| format!("{}: {err}", self.path.display()))
    }

    /// Closes the file and says what the run holds.
    fn finish(mut self) -> Result<PathBuf, String> {
        self.out
            .flush()
            .map_err(|err| format!("{}: {err}", self.path.display()))?;
        say("");
        say(&format!(
            "Recorded {} frame(s), {} of them with a hand, to {}.",
            self.frames_read,
            self.frames_with_hand,
            self.path.display()
        ));
        if self.frames_with_hand == 0 {
            say("No frame carried a hand, so the run holds nothing to score.");
        }
        say(&format!(
            "Score it: coder-hands-measure score {}",
            self.path.display()
        ));
        Ok(self.path)
    }
}

/// One line to the person at the camera.
fn say(text: &str) {
    println!("{text}");
    let _ = std::io::stdout().flush();
}

/// Seconds since the Unix epoch, the stamp a run is named and headed
/// with.
fn started() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs_f64())
        .unwrap_or_default()
}

/// Where a run goes when you name no file: under `~/.openagents`, the
/// one directory this product writes a person's files to.
fn default_path(home: &Path, started: f64) -> PathBuf {
    home.join(".openagents/hands/runs")
        .join(format!("run-{}.jsonl", started as u64))
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
