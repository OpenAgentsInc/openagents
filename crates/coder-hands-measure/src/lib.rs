//! Record a labelled run of hand gestures, and score the desk's rules
//! against it.
//!
//! `record` is the part that needs you: it prompts for one gesture at a
//! time, counts you in, and writes every landmark line the camera
//! published with the prompt as its label. `score` replays that file
//! through the rules the desk runs and prints what they decided. `ask`
//! sends the windows the rules could not settle to the seam once and
//! keeps the answers beside the run, so a later score reads the file
//! rather than the network.
//!
//! The gesture constants and the seam's floors were both chosen by reading rather
//! than by measuring, and both need a hand in front of a camera. Your
//! time is the scarce part, so the recorder keeps it to one scripted run
//! and [`replay`] makes that run last: it reads the rules and the window
//! themselves, so a constant or a floor that moves is scored against
//! every recorded run again and nobody waves at a camera twice.
//!
//! The camera is the CoderOS daemon's hands socket, through [`camera`],
//! and the run says which camera it came from, because a run recorded
//! from one camera is not a run recorded from another. Everything after
//! the recording is the same code on every platform: a run is data,
//! scoring is a pure function of it, and the same file scores the same on
//! a Mac and on a CoderOS host.
//!
//! The format the runs are written in is [`run`], the answers are
//! [`answers`], [`score`] counts, and [`report`] turns a score into
//! columns. `docs/os/camera-and-hands.md` says what to run at the camera
//! and for how long.

pub mod answers;
pub mod ask;
pub mod camera;
pub mod record;
pub mod replay;
pub mod report;
pub mod run;
pub mod score;
