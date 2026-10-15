//! Every word the game shows. The robot crew speak for themselves; the
//! game's own words are short, so a child who can't read yet can follow
//! the pictures and a grown-up can read along.

use snap_rules::{Fail, PieceKind};

use crate::kit::Crew;

pub const TITLE: &str = "Snap Station";
pub const TAGLINE: &str = "Build in space with magnetic pieces.";
pub const JOBS: &str = "Jobs from the robot crew";
pub const NO_WEBGL: &str =
    "This browser can't show the game. Try a recent Chrome, Safari, or Firefox.";

pub const GO: &str = "Go";
pub const TURN: &str = "Turn";
pub const UNDO: &str = "Undo";
pub const FIX: &str = "Fix it";
pub const STOP: &str = "Stop";
pub const TRY_AGAIN: &str = "Try again";
pub const NEXT_JOB: &str = "Next job";
pub const ALL_JOBS: &str = "Save and visit Crew Deck";
pub const OPTIONS: &str = "Options";
pub const MENU: &str = "Menu";
pub const PAUSED: &str = "Paused";
pub const RESUME: &str = "Back to building";
pub const RESTART: &str = "Start the job again";
pub const HOME: &str = "Back to OpenAgents";
pub const CLOSE: &str = "Close";
pub const PIECES: &str = "Pieces";
pub const STORM: &str = "Storm coming";
pub const GRAVITY: &str = "Plant gravity";
pub const DRAG_HINT: &str = "Drag a piece onto the station.";
pub const KEYS_HINT: &str = "Or use the keyboard: \u{2190} \u{2192} pick a piece, Enter picks it up, the arrows move it, Enter snaps it on.";
pub const NO_FIT: &str = "It didn't snap there. Hold it close to a side, lined up.";
pub const JOB_DONE: &str = "Job done!";
pub const NOT_YET: &str = "Not quite!";
pub const LOCKED: &str = "Finish the job before it to open this one.";

/// The tutorial: its button, its own buttons, and the link to the written
/// guide on the Blue Rush Studios site.
pub const HOW_TO_PLAY: &str = "How to play";
pub const TOUR_NEXT: &str = "Next";
pub const TOUR_BACK: &str = "Back";
pub const TOUR_SKIP: &str = "Skip";
pub const TOUR_DONE: &str = "Start building";
pub const TOUR_CLOSE: &str = "Got it";
pub const GUIDE: &str = "Read the full guide";

/// Sound on or off, for the menu.
#[must_use]
pub fn sound(on: bool) -> &'static str {
    if on { "Sound: on" } else { "Sound: off" }
}

/// High contrast on or off, for the menu.
#[must_use]
pub fn contrast(on: bool) -> &'static str {
    if on {
        "High contrast: on"
    } else {
        "High contrast: off"
    }
}

/// A piece's name.
#[must_use]
pub fn piece(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Square => "Square",
        PieceKind::Triangle => "Triangle",
        PieceKind::Wedge => "Half square",
        PieceKind::Panel => "Panel",
        PieceKind::Solar => "Solar wing",
        PieceKind::Thruster => "Thruster",
        PieceKind::Cargo => "Crate",
        PieceKind::Pod => "Plant pod",
        PieceKind::Module => "Module",
        PieceKind::Strut => "Strut",
        PieceKind::Bolt => "Bolt",
    }
}

/// A crew member's name.
#[must_use]
pub fn crew(crew: Crew) -> &'static str {
    match crew {
        Crew::Cog => "Captain Cog",
        Crew::Pip => "Pip",
        Crew::Rivet => "Rivet",
        Crew::Nova => "Nova",
        Crew::Tock => "Tock",
    }
}

/// What each crew member does on the station.
#[must_use]
pub fn crew_role(crew: Crew) -> &'static str {
    match crew {
        Crew::Cog => "the captain",
        Crew::Pip => "the helper drone",
        Crew::Rivet => "the builder",
        Crew::Nova => "the scientist",
        Crew::Tock => "the cargo bot",
    }
}

/// A job's short goal, for the HUD, by job number.
#[must_use]
pub fn goal(job: u8) -> &'static str {
    match job {
        1 => "Join the two modules",
        2 => "Push the crate into the ring",
        3 => "Join them before the storm",
        4 => "Spin the garden for the plants",
        5 => "Give the cargo ship a berth",
        _ => "Do the job",
    }
}

/// What the crew member asks, by job number.
#[must_use]
pub fn ask(job: u8) -> &'static str {
    match job {
        1 => "Can you join my two modules? Snap pieces from one to the other.",
        2 => "Push my crate into the green ring, nice and straight!",
        3 => {
            "A solar storm is coming! Join the modules before it hits, and make the bridge strong."
        }
        4 => "Spin my garden so the plants feel gravity. Not too little, not too much!",
        5 => "A cargo ship is coming in. Build it a berth that fits, and hold on tight!",
        _ => "Can you help me build something?",
    }
}

/// Why a run didn't do the job, and what to try.
#[must_use]
pub fn fail(why: Fail) -> &'static str {
    match why {
        Fail::CameApart => "It came apart. Bolts lock magnets, and a strut stops a square leaning.",
        Fail::Missed => "The crate missed the ring. Push it harder toward the ring.",
        Fail::Spun => "It spun around. Push through the middle so it goes straight.",
        Fail::PodLost => "A plant pod flew off! Bolt the joins out to the pods.",
        Fail::TooLight => "The plants floated. Spin faster: add thrusters out by the pods.",
        Fail::TooHeavy => "Too much gravity for the plants. Spin slower: use fewer thrusters.",
        Fail::NoRoom => "The ship bumped into the build. Leave it room to fly in.",
        Fail::NoMooring => "The ship had nothing to hold on to. Build arms as long as the ship.",
        Fail::Broke => "The dock broke when the ship tugged. Brace it and bolt it.",
    }
}

/// The thanks after a win.
#[must_use]
pub fn thanks(crew: Crew) -> String {
    format!("Thanks, builder! \u{2014} {}", self::crew(crew))
}

/// How many pieces a win used, and what earns three stars.
#[must_use]
pub fn used_line(used: u32, par: u32, stars: u8) -> String {
    let pieces = if used == 1 { "piece" } else { "pieces" };
    if stars >= 3 {
        format!("You used {used} {pieces}. Three stars!")
    } else {
        format!("You used {used} {pieces}. Use {par} or fewer for three stars.")
    }
}

/// A job's button on the job list.
#[must_use]
pub fn job_button(number: u8, name: &str) -> String {
    format!("{number}. {name}")
}

/// Stars as a row of filled and empty stars.
#[must_use]
pub fn stars(count: u8) -> String {
    (0..3)
        .map(|i| if i < count { '\u{2605}' } else { '\u{2606}' })
        .collect()
}

/// How many of a piece are left, for its tray button's label.
#[must_use]
pub fn tray_label(kind: PieceKind, left: u32) -> String {
    format!("{}: {left} left", piece(kind))
}

/// The tutorial's step counter.
#[must_use]
pub fn tour_step_line(step: usize, of: usize) -> String {
    format!("{step} of {of}")
}

/// One step of the tutorial: a title, then its lines for a keyboard and
/// mouse and for a touch screen.
pub struct TourStep {
    pub title: &'static str,
    pub keys: &'static [&'static str],
    pub touch: &'static [&'static str],
}

/// The tutorial's steps, in order.
pub const TOUR: [TourStep; 5] = [
    TourStep {
        title: "Welcome to Snap Station",
        keys: &[
            "The robot crew need a builder. Each job comes from one of them.",
            "Build with magnetic pieces, then press Go to see if it works.",
        ],
        touch: &[
            "The robot crew need a builder. Each job comes from one of them.",
            "Build with magnetic pieces, then press Go to see if it works.",
        ],
    },
    TourStep {
        title: "Snap pieces on",
        keys: &[
            "Drag a piece from the tray at the bottom and let go near the station.",
            "When it's close and lined up, it snaps on like a magnet. R or the mouse wheel turns it. Drag a piece off to take it back.",
        ],
        touch: &[
            "Drag a piece from the tray at the bottom and let go near the station.",
            "When it's close and lined up, it snaps on like a magnet. Tap Turn to turn it. Drag a piece off to take it back.",
        ],
    },
    TourStep {
        title: "Press Go",
        keys: &[
            "Go tests your build in space.",
            "Thrusters push, spinning makes gravity, and wobbly builds shake apart.",
        ],
        touch: &[
            "Go tests your build in space.",
            "Thrusters push, spinning makes gravity, and wobbly builds shake apart.",
        ],
    },
    TourStep {
        title: "Make it strong",
        keys: &[
            "Squares lean when they're pushed sideways. Triangles don't.",
            "Drop a strut on a square so it can't lean, and a bolt on a join so the magnets can't let go.",
        ],
        touch: &[
            "Squares lean when they're pushed sideways. Triangles don't.",
            "Drop a strut on a square so it can't lean, and a bolt on a join so the magnets can't let go.",
        ],
    },
    TourStep {
        title: "Fix it and try again",
        keys: &[
            "If something breaks, press Fix it to change your build, then Go again.",
            "Do the job with fewer pieces to earn more stars.",
        ],
        touch: &[
            "If something breaks, press Fix it to change your build, then Go again.",
            "Do the job with fewer pieces to earn more stars.",
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_reads_plainly() {
        let mut all: Vec<String> = [
            crate::deck::TITLE,
            crate::deck::SAVED,
            crate::deck::STARS,
            crate::deck::HINT,
            crate::deck::EMPTY,
            TITLE,
            TAGLINE,
            JOBS,
            NO_WEBGL,
            GO,
            TURN,
            UNDO,
            FIX,
            STOP,
            TRY_AGAIN,
            NEXT_JOB,
            ALL_JOBS,
            MENU,
            OPTIONS,
            PAUSED,
            RESUME,
            RESTART,
            HOME,
            CLOSE,
            PIECES,
            STORM,
            GRAVITY,
            DRAG_HINT,
            KEYS_HINT,
            NO_FIT,
            JOB_DONE,
            NOT_YET,
            LOCKED,
            HOW_TO_PLAY,
            TOUR_NEXT,
            TOUR_BACK,
            TOUR_SKIP,
            TOUR_DONE,
            TOUR_CLOSE,
            GUIDE,
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        for on in [true, false] {
            all.push(sound(on).to_owned());
            all.push(contrast(on).to_owned());
        }
        for kind in PieceKind::ALL {
            all.push(piece(kind).to_owned());
            all.push(tray_label(kind, 2));
        }
        all.extend(crate::deck::BOARDS.iter().map(|b| b.title.to_owned()));
        for member in Crew::ALL {
            all.push(crew(member).to_owned());
            all.push(crew_role(member).to_owned());
            all.push(thanks(member));
        }
        for job in 1..=snap_rules::level::COUNT as u8 {
            all.push(goal(job).to_owned());
            all.push(ask(job).to_owned());
            all.push(job_button(
                job,
                &snap_rules::level::job(usize::from(job)).name,
            ));
        }
        for why in [
            Fail::CameApart,
            Fail::Missed,
            Fail::Spun,
            Fail::PodLost,
            Fail::TooLight,
            Fail::TooHeavy,
            Fail::NoRoom,
            Fail::NoMooring,
            Fail::Broke,
        ] {
            all.push(fail(why).to_owned());
        }
        all.push(used_line(9, 10, 3));
        assert_eq!(used_line(1, 1, 3), "You used 1 piece. Three stars!");
        all.push(used_line(12, 10, 2));
        all.push(tour_step_line(1, TOUR.len()));
        for step in &TOUR {
            all.push(step.title.to_owned());
            all.extend(step.keys.iter().chain(step.touch).map(|l| (*l).to_owned()));
        }
        for text in all {
            assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
        }
        assert_eq!(stars(2), "\u{2605}\u{2605}\u{2606}");
        // Every job has its own words.
        assert_ne!(goal(5), goal(6));
    }
}
