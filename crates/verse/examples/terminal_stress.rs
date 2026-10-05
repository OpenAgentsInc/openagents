//! A windowed stress run of the terminal overlay over Everglade's town.
//! Usage: terminal_stress [--busy N] [--seconds S] [--warmup S]
//!        [--no-spells] [--label TEXT] [--out REPORT.json]
//!
//! Opens Verse offline straight into Everglade, then the overlay with a
//! typing pane (`cat`) and N panes of heavy output (`yes`, `cat` of a large
//! file, a colored build log, `seq`, and `top`, in turn). It raises Wind
//! Walls and casts Meteor Swarm on the town, types a key every 150 ms, and
//! records frame times and key-to-glyph latency for S seconds after the
//! warm-up. The report goes to REPORT.json and standard output. The panes
//! run in a temporary directory that is also their home.
//! Read `docs/verse/verification/2026-10-05-terminal-performance/`.
use std::path::PathBuf;

use verse::app::Options;
use verse::terminal::stress::Plan;

fn main() -> Result<(), String> {
    let mut plan = Plan {
        busy: 8,
        seconds: 20,
        warmup: 5,
        out: PathBuf::from("terminal-stress.json"),
        spells: true,
        label: String::new(),
    };
    let mut args = std::env::args().skip(1);
    let number = |value: Option<String>| -> Result<u32, String> {
        value
            .ok_or("Expected a number")?
            .parse()
            .map_err(|_| "Expected a number".to_owned())
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--busy" => plan.busy = number(args.next())? as usize,
            "--seconds" => plan.seconds = number(args.next())?,
            "--warmup" => plan.warmup = number(args.next())?,
            "--no-spells" => plan.spells = false,
            "--label" => plan.label = args.next().ok_or("Expected a label")?,
            "--out" => plan.out = PathBuf::from(args.next().ok_or("Expected a path")?),
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    let options = Options {
        relay: None,
        everglade: true,
        terminal_stress: Some(plan),
        ..Options::default()
    };
    verse::app::run(&options)
}
