//! `snap.level.v1` job files and the jobs that ship with the game.
//!
//! A job file is plain text, one statement a line; `#` starts a comment.
//! Positions are in metres, x to the right and y up; times in seconds.
//!
//! ```text
//! snap.level.v1
//! name Hello, Magnets
//! number 1
//! crew pip                      # who gives the job: cog, pip, rivet, nova or tock
//! job connect A B               # connect A B | push C | spin H | dock B
//! area -1 -3 6 4                # where pieces may go: lowest x y, highest x y
//! give module 0.5 0.5 0 A       # a piece the job starts with: kind, centre, turn, tag
//! tray square 4 triangle 4      # what the player may use, and how many
//! par 3                         # pieces (struts and bolts count) for three stars
//! run 4                         # how long a run lasts after Go
//! nudge B 0 0.3 0.5 1.5         # a push on a tagged piece: force x y, from, until
//! burn 2                        # how long thrusters burn
//! beacon 8.5 0.5 1              # push: where the crate must go, and its radius
//! spin-max 0.5                  # push: the fastest spin allowed there, radians a second
//! gravity 1.5 4                 # spin: the band the pods need, metres a second squared
//! hold 2                        # spin: how long they must stay in it
//! storm 45 4 1.2 0.5            # the storm: comes after 45 s, lasts 4 s, push per square metre, gust length
//! ship 9 0.5 2.6 0.8 1.6 0.15   # dock: start x, middle y, length, height, speed, where its nose stops
//! moor 3 1.5                    # dock: how hard the ship tugs on its moorings, and how long
//! ```

use sha2::{Digest, Sha256};

use crate::build::Build;
use crate::kinds::PieceKind;
use crate::{TURNS, V, ticks};

/// The format's first line.
pub const FORMAT: &str = "snap.level.v1";

/// The robot crew, as job files name them.
pub const CREW: [&str; 5] = ["cog", "pip", "rivet", "nova", "tock"];

/// The jobs that ship with the game, in order.
pub const JOBS: [&str; 5] = [
    include_str!("../jobs/01-hello-magnets.job"),
    include_str!("../jobs/02-straight-push.job"),
    include_str!("../jobs/03-storm-bridge.job"),
    include_str!("../jobs/04-garden-spin.job"),
    include_str!("../jobs/05-cargo-dock.job"),
];

/// How many jobs ship.
pub const COUNT: usize = JOBS.len();

/// Job `number` (1-based) of the ones that ship.
///
/// # Panics
///
/// When `number` is out of range or the shipped file doesn't parse; tests
/// check every shipped file.
#[must_use]
pub fn job(number: usize) -> Level {
    parse(JOBS[number - 1]).expect("a shipped job parses")
}

/// What the job asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    /// Keep the two tagged pieces joined through the whole run.
    Connect(char, char),
    /// Push the tagged piece through the beacon without spinning.
    Push(char),
    /// Spin the build so every pod stays joined to the tagged hub and
    /// feels gravity in the band.
    Spin(char),
    /// Give the cargo ship a berth that fits it and holds when it tugs.
    Dock(char),
}

/// A piece the job starts with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Given {
    pub kind: PieceKind,
    pub centre: V,
    pub turn: u8,
    pub tag: Option<char>,
}

/// A push on a tagged piece for part of the run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nudge {
    pub tag: char,
    pub force: V,
    pub from: u32,
    pub until: u32,
}

/// The solar storm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Storm {
    /// Building time before it comes by itself, in ticks.
    pub clock: u32,
    /// How long it blows once it comes, in ticks.
    pub length: u32,
    /// Its push on each square metre of sail, in newtons.
    pub strength: f64,
    /// Each gust's length, in ticks; gusts change direction.
    pub gust: u32,
}

/// The cargo ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ship {
    /// Where its nose (left end) starts.
    pub start: f64,
    /// The height of its middle.
    pub y: f64,
    pub length: f64,
    pub height: f64,
    /// Metres a second.
    pub speed: f64,
    /// Where its nose stops.
    pub berth: f64,
}

impl Ship {
    /// The ticks it takes to reach its berth.
    #[must_use]
    pub fn arrival(&self) -> u32 {
        ticks((self.start - self.berth) / self.speed)
    }
}

/// A parsed job.
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub name: String,
    pub number: u8,
    pub crew: &'static str,
    pub job: Job,
    pub area: (V, V),
    pub given: Vec<Given>,
    pub tray: Vec<(PieceKind, u32)>,
    pub par: u32,
    /// How long a run lasts after Go, in ticks.
    pub run: u32,
    pub nudges: Vec<Nudge>,
    /// How long thrusters burn, in ticks.
    pub burn: u32,
    pub beacon: Option<(V, f64)>,
    pub spin_max: f64,
    pub gravity: (f64, f64),
    pub hold: u32,
    pub storm: Option<Storm>,
    pub ship: Option<Ship>,
    /// The ship's tug on each mooring, in newtons, and for how many ticks.
    pub moor: (f64, u32),
    pub digest: [u8; 32],
}

impl Level {
    /// The build the job starts with. The crew's own pieces come bolted
    /// together.
    #[must_use]
    pub fn start(&self) -> Build {
        let mut build = Build::new(self.area);
        for given in &self.given {
            let corners = Build::held(given.kind, given.centre, given.turn);
            build.place(given.kind, corners, true, given.tag);
        }
        for seam in &mut build.seams {
            seam.bolted = true;
        }
        build
    }

    /// How many of `kind` the tray starts with.
    #[must_use]
    pub fn tray_count(&self, kind: PieceKind) -> u32 {
        self.tray
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(0, |(_, n)| *n)
    }
}

/// SHA-256 of a job file.
#[must_use]
pub fn digest(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}

fn number(word: &str) -> Result<f64, String> {
    let value: f64 = word.parse().map_err(|_| format!("not a number: {word}"))?;
    if !value.is_finite() || value.abs() > 1000.0 {
        return Err(format!("out of range: {word}"));
    }
    Ok(value)
}

fn seconds(word: &str) -> Result<u32, String> {
    let value = number(word)?;
    if !(0.0..=600.0).contains(&value) {
        return Err(format!("not a time: {word}"));
    }
    Ok(ticks(value))
}

fn tag(word: &str) -> Result<char, String> {
    let mut chars = word.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_uppercase() => Ok(c),
        _ => Err(format!("not a tag: {word}")),
    }
}

fn args<'a>(words: &[&'a str], count: usize, line: &str) -> Result<Vec<&'a str>, String> {
    if words.len() < count {
        return Err(format!("too few words: {line}"));
    }
    Ok(words.to_vec())
}

/// Parses a job file.
///
/// # Errors
///
/// A line it can't read, a missing statement, or a job that names a tag no
/// given piece has.
pub fn parse(text: &str) -> Result<Level, String> {
    let mut lines = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty());
    if lines.next() != Some(FORMAT) {
        return Err(format!("not a {FORMAT} file"));
    }
    let mut name = None;
    let mut number_ = None;
    let mut crew = None;
    let mut job = None;
    let mut area = None;
    let mut given = Vec::new();
    let mut tray = Vec::new();
    let mut par = 0;
    let mut run = None;
    let mut nudges = Vec::new();
    let mut burn = ticks(2.0);
    let mut beacon = None;
    let mut spin_max = 0.5;
    let mut gravity = (0.0, 0.0);
    let mut hold = ticks(2.0);
    let mut storm = None;
    let mut ship = None;
    let mut moor = (0.0, 0);
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        let rest = &words[1..];
        match words[0] {
            "name" => name = Some(line[4..].trim().to_owned()),
            "number" => {
                let n: u8 = args(rest, 1, line)?[0]
                    .parse()
                    .map_err(|_| format!("not a number: {line}"))?;
                number_ = Some(n);
            }
            "crew" => {
                let word = args(rest, 1, line)?[0];
                crew = Some(
                    *CREW
                        .iter()
                        .find(|c| **c == word)
                        .ok_or_else(|| format!("no such crew: {word}"))?,
                );
            }
            "job" => {
                let a = args(rest, 2, line)?;
                job = Some(match a[0] {
                    "connect" => Job::Connect(tag(a[1])?, tag(args(rest, 3, line)?[2])?),
                    "push" => Job::Push(tag(a[1])?),
                    "spin" => Job::Spin(tag(a[1])?),
                    "dock" => Job::Dock(tag(a[1])?),
                    other => return Err(format!("no such job: {other}")),
                });
            }
            "area" => {
                let a = args(rest, 4, line)?;
                area = Some((
                    V::new(number(a[0])?, number(a[1])?),
                    V::new(number(a[2])?, number(a[3])?),
                ));
            }
            "give" => {
                let a = args(rest, 4, line)?;
                let kind = PieceKind::from_word(a[0])
                    .filter(|k| k.is_tile())
                    .ok_or_else(|| format!("not a tile: {}", a[0]))?;
                let turn: u8 = a[3].parse().map_err(|_| format!("not a turn: {line}"))?;
                if turn >= TURNS {
                    return Err(format!("not a turn: {line}"));
                }
                given.push(Given {
                    kind,
                    centre: V::new(number(a[1])?, number(a[2])?),
                    turn,
                    tag: a.get(4).map(|t| tag(t)).transpose()?,
                });
            }
            "tray" => {
                if rest.is_empty() || rest.len() % 2 != 0 {
                    return Err(format!("tray needs kinds and counts: {line}"));
                }
                for pair in rest.chunks(2) {
                    let kind = PieceKind::from_word(pair[0])
                        .filter(|k| *k != PieceKind::Module)
                        .ok_or_else(|| format!("not a tray piece: {}", pair[0]))?;
                    let count: u32 = pair[1]
                        .parse()
                        .map_err(|_| format!("not a count: {line}"))?;
                    tray.push((kind, count.min(99)));
                }
            }
            "par" => {
                par = args(rest, 1, line)?[0]
                    .parse()
                    .map_err(|_| format!("not a number: {line}"))?;
            }
            "run" => run = Some(seconds(args(rest, 1, line)?[0])?),
            "nudge" => {
                let a = args(rest, 5, line)?;
                nudges.push(Nudge {
                    tag: tag(a[0])?,
                    force: V::new(number(a[1])?, number(a[2])?),
                    from: seconds(a[3])?,
                    until: seconds(a[4])?,
                });
            }
            "burn" => burn = seconds(args(rest, 1, line)?[0])?,
            "beacon" => {
                let a = args(rest, 3, line)?;
                beacon = Some((V::new(number(a[0])?, number(a[1])?), number(a[2])?));
            }
            "spin-max" => spin_max = number(args(rest, 1, line)?[0])?,
            "gravity" => {
                let a = args(rest, 2, line)?;
                gravity = (number(a[0])?, number(a[1])?);
            }
            "hold" => hold = seconds(args(rest, 1, line)?[0])?,
            "storm" => {
                let a = args(rest, 4, line)?;
                storm = Some(Storm {
                    clock: seconds(a[0])?,
                    length: seconds(a[1])?,
                    strength: number(a[2])?,
                    gust: seconds(a[3])?.max(1),
                });
            }
            "ship" => {
                let a = args(rest, 6, line)?;
                let speed = number(a[4])?;
                if speed <= 0.0 {
                    return Err(format!("the ship must move: {line}"));
                }
                ship = Some(Ship {
                    start: number(a[0])?,
                    y: number(a[1])?,
                    length: number(a[2])?,
                    height: number(a[3])?,
                    speed,
                    berth: number(a[5])?,
                });
            }
            "moor" => {
                let a = args(rest, 2, line)?;
                moor = (number(a[0])?, seconds(a[1])?);
            }
            other => return Err(format!("unknown statement: {other}")),
        }
    }
    let job = job.ok_or("no job")?;
    let tags: Vec<char> = given.iter().filter_map(|g| g.tag).collect();
    let needs: Vec<char> = match job {
        Job::Connect(a, b) => vec![a, b],
        Job::Push(t) | Job::Spin(t) | Job::Dock(t) => vec![t],
    };
    if let Some(missing) = needs.iter().find(|t| !tags.contains(t)) {
        return Err(format!("no given piece is tagged {missing}"));
    }
    match job {
        Job::Push(_) if beacon.is_none() => return Err("a push job needs a beacon".into()),
        Job::Spin(_) if gravity.1 <= gravity.0 => {
            return Err("a spin job needs a gravity band".into());
        }
        Job::Dock(_) if ship.is_none() => return Err("a dock job needs a ship".into()),
        _ => {}
    }
    Ok(Level {
        name: name.ok_or("no name")?,
        number: number_.ok_or("no number")?,
        crew: crew.ok_or("no crew")?,
        job,
        area: area.ok_or("no area")?,
        given,
        tray,
        par,
        run: run.ok_or("no run")?,
        nudges,
        burn,
        beacon,
        spin_max,
        gravity,
        hold,
        storm,
        ship,
        moor,
        digest: digest(text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_job_parses_and_its_given_pieces_fit() {
        for (index, text) in JOBS.iter().enumerate() {
            let level = parse(text).unwrap_or_else(|e| panic!("job {}: {e}", index + 1));
            assert_eq!(usize::from(level.number), index + 1);
            let build = level.start();
            assert_eq!(build.pieces.len(), level.given.len());
            for (i, a) in build.pieces.iter().enumerate() {
                for b in &build.pieces[i + 1..] {
                    assert!(
                        !crate::build::overlaps(&a.at, &b.at),
                        "job {}: given pieces overlap",
                        level.number
                    );
                }
            }
            if let Some(ship) = level.ship {
                assert!(
                    level.run > ship.arrival() + level.moor.1,
                    "job {}: the run outlasts the ship's tug",
                    level.number
                );
            }
        }
    }

    #[test]
    fn bad_files_are_refused_with_a_reason() {
        assert!(parse("snap.level.v2\n").is_err());
        let base = "snap.level.v1\nname X\nnumber 1\ncrew pip\narea 0 0 5 5\nrun 3\n";
        assert!(parse(base).unwrap_err().contains("no job"));
        let missing = format!("{base}job connect A B\ngive module 0.5 0.5 0 A\n");
        assert!(parse(&missing).unwrap_err().contains("tagged B"));
        let crew = base.replace("crew pip", "crew zed");
        assert!(parse(&crew).unwrap_err().contains("crew"));
        let turn = format!("{base}job push A\ngive module 0.5 0.5 12 A\nbeacon 1 1 1\n");
        assert!(parse(&turn).unwrap_err().contains("turn"));
        let tray =
            format!("{base}job push A\ngive module 0.5 0.5 0 A\nbeacon 1 1 1\ntray module 2\n");
        assert!(parse(&tray).unwrap_err().contains("tray"));
    }
}
