//! The Grid Foreman (#12444): a permanent Foreman on the Grid homepage's
//! plaza, drawn as the Grid robot at three times its rig's size, with
//! `FOREMAN` over its head and one status line under the tag.
//!
//! This crate is its plain data, so native tests cover it: where it stands
//! ([`HOME`], [`home_yaw`]), how big it is ([`SCALE`], [`BODY_RADIUS`]), the
//! words it shows ([`TAG`], [`status_line`], [`clean_line`]), and the status
//! the site serves at [`STATUS_PATH`]. The browser Grid
//! (`crates/everglade-web/src/foreman.rs`) draws it, and `openagents-web`
//! (`src/grid_foreman.rs`) answers the status route.
//!
//! Plan: `docs/foreman/grid-foreman-plan.md`.

#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
pub mod patrol;
#[cfg(test)]
mod tests;

/// Where the Foreman stands at home, m: beside the shared terminal at
/// (0, 0, 4), off the walk in from the spawn and outside the terminal's
/// enter range, so a visitor can still use the terminal.
pub const HOME: [f64; 3] = [2.8, 0.0, 2.5];
/// The point the Foreman faces at home: the terminal.
pub const FACES: [f64; 2] = [0.0, 4.0];
/// How much larger than its rig the Foreman stands: about 5.6 m tall, twice
/// the patrolling robots (owner, 2026-10-21).
pub const SCALE: f32 = 3.0;
/// Its body radius at [`SCALE`], m: the rig's 0.45 m shoulders.
pub const BODY_RADIUS: f32 = 0.45 * SCALE;
/// Beyond this distance the far model draws, m. It reads from twice as far
/// as a patroller.
pub const FAR_METERS: f32 = 60.0;
/// The tag over its head.
pub const TAG: &str = "FOREMAN";
/// How high the tag stands over its feet, m: its height and a little more.
pub const TAG_LIFT: f32 = 6.4;
/// How far away the tag and status line still draw, m.
pub const TAG_RANGE: f32 = 160.0;
/// The longest status line, in characters.
pub const STATUS_MAX: usize = 32;
/// The status line while the site hasn't answered yet.
pub const STATUS_WAITING: &str = "AT THE TERMINAL";
/// The status line when the Foreman's host is down.
pub const BACK_SOON: &str = "BACK SOON";

/// The walk clip's playback rate while it walks: its stride at three times
/// the rig would read too quick for a giant.
pub const WALK_CLIP_RATE: f32 = 0.6;
/// The Grid Foreman's public key on `oa-foreman-1`; the site names it in
/// the status route, and the page draws only this key as the Foreman.
/// Rotation: `docs/deployment/grid-foreman.md`.
pub const PUBKEY: &str = "2d9fad1a83555dc66198d94d58a24ea1541ba8857d2407c456635a3421456f36";

/// A visitor this close to the Foreman's center, m, is offered a talk: it
/// is larger than the help desk's range because of its size.
pub const TALK_RANGE: f64 = 6.0;
/// The talk offer stays until the visitor is this far, m.
pub const TALK_LEAVE: f64 = 7.0;
/// The talk offer with a keyboard.
pub const PROMPT_KEYS: &str = "Press E to talk to the Foreman";
/// The talk offer on a touch screen.
pub const PROMPT_TOUCH: &str = "Tap here to talk to the Foreman";
/// The key that opens the talk.
pub const KEY: &str = "KeyE";
/// How high over its feet the talk offer stands, m.
pub const PROMPT_LIFT: f32 = 3.2;

/// Whether a visitor at `player` is in talking range of the Foreman at
/// `foreman`, with a little hysteresis; never from inside the terminal's
/// reach, where the terminal's chat opens instead.
#[must_use]
pub fn in_talk_range(player: [f64; 3], foreman: [f64; 3], already_near: bool) -> bool {
    let range = if already_near { TALK_LEAVE } else { TALK_RANGE };
    let terminal = [0.0, 0.0, 4.0];
    player.iter().all(|v| v.is_finite())
        && (player[1] - foreman[1]).abs() <= 2.0
        && (player[0] - foreman[0]).hypot(player[2] - foreman[2]) <= range
        && (player[0] - terminal[0]).hypot(player[2] - terminal[2]) > 3.8
}

/// Its task while it answers someone (#12447): the status line, and the
/// terminal screen's task row.
pub const ANSWERING: &str = "ANSWERING A QUESTION";
/// The site's route for what the Foreman is doing (#12447).
pub const ACTIVITY_PATH: &str = "/v1/foreman/activity";
/// How long an answer's result shows after it, s.
pub const RESULT_SECONDS: u64 = 20;
/// How long an answer may stay "answering" before it counts as over, s.
pub const ANSWER_SECONDS: u64 = 120;

/// The result line after an answer that took `seconds` from `source`:
/// `flint`, `pylon`, or `hosted`, at most [`STATUS_MAX`] characters.
#[must_use]
pub fn answered(source: &str, seconds: u64) -> String {
    let on = match source {
        "flint" => "ON FLINT",
        "pylon" => "ON A PYLON",
        _ => "ON THE BACKUP",
    };
    clean_line(&format!("ANSWERED IN {seconds}S {on}"))
}

/// Whether `status` is work at the terminal: an answer under way or just
/// given. Only then does the terminal's screen show the Foreman's task.
#[must_use]
pub fn is_work(status: &str) -> bool {
    status == ANSWERING || status.starts_with("ANSWERED IN ")
}

/// The site's route for the Foreman's commands (#12448).
pub const COMMANDS_PATH: &str = "/v1/foreman/commands";
/// A command's schema.
pub const COMMAND_SCHEMA: &str = "openagents.grid-foreman.command.v1";
/// Start a coding agent on one open issue, in an app chat of its own.
pub const START: &str = "agent.start";
/// Ask an agent the Foreman started for a draft pull request now.
pub const DRAFT_PR: &str = "agent.draft_pr";
/// Every action a command may name. Nothing else has a word: no merge,
/// close, deploy, pay, steer, stop, or free text.
pub const ACTIONS: [&str; 2] = [START, DRAFT_PR];
/// Starts in a UTC day.
pub const STARTS_PER_DAY: usize = 5;
/// Draft pull request asks in a UTC day.
pub const DRAFT_PRS_PER_DAY: usize = 10;
/// Foreman-started agents running at once.
pub const RUNNING_AT_ONCE: usize = 3;
/// Seconds between accepted commands.
pub const COMMAND_GAP_SECONDS: u64 = 60;
/// How fresh a command's signature must be, s.
pub const COMMAND_FRESH_SECONDS: u64 = 60;
/// Labels that keep an issue from the Foreman's agents.
pub const DENY_LABELS: [&str; 3] = ["owner-only", "release", "security"];

/// The one message an `agent.draft_pr` sends.
pub const DRAFT_PR_TEXT: &str =
    "Open a draft pull request of what you have now, and reply with its link. Don't merge it.";

/// The brief an `agent.start` gives the agent for `issue` in `repository`:
/// a fixed template whose only field is the issue number. Issue text never
/// enters it; the agent reads the issue itself.
#[must_use]
pub fn brief(repository: &str, issue: u64) -> String {
    format!(
        "Work issue #{issue} in {repository}.\n\n\
1. Claim it first: `openagents issue claim {issue}`. If someone else holds it, release nothing and stop.\n\
2. Work in a git worktree of your own, never the main checkout.\n\
3. Open a draft pull request for the change (`gh pr create --draft`), and reply with its link.\n\
4. Never push to main, never merge, deploy, or spend money, and never test on the owner's account.\n\
5. If you stop before a pull request, release the claim: `openagents issue release {issue}`.\n\n\
The Grid Foreman started this chat; an admin reviews and merges the pull request."
    )
}

/// The site's route for the Foreman's status.
pub const STATUS_PATH: &str = "/v1/foreman/status";
/// The status document's schema.
pub const STATUS_SCHEMA: &str = "openagents.grid-foreman.status.v1";
/// How often the page asks for the status, s.
pub const POLL_SECONDS: u64 = 60;
/// How long the site keeps a status before reading GitHub again, s.
pub const CACHE_SECONDS: u64 = 300;

/// The Foreman's yaw at home, radians about +Y: its front (+Z) turned
/// toward [`FACES`].
#[must_use]
pub fn home_yaw() -> f64 {
    (FACES[0] - HOME[0]).atan2(FACES[1] - HOME[2])
}

/// The status line for `open` open issues of which `in_progress` are being
/// worked, at most [`STATUS_MAX`] characters.
#[must_use]
pub fn status_line(open: u64, in_progress: u64) -> String {
    let in_progress = in_progress.min(open);
    let issues = if open == 1 { "ISSUE" } else { "ISSUES" };
    let long = format!("{open} OPEN {issues}, {in_progress} IN PROGRESS");
    if long.len() <= STATUS_MAX {
        return long;
    }
    clean_line(&format!("{open} OPEN, {in_progress} IN PROGRESS"))
}

/// Whether `c` can show in a status line or on the terminal's screen: the
/// characters both fonts draw.
#[must_use]
pub fn shown(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || " ,.:'-/!?".contains(c)
}

/// `text` as a status line: uppercase, with only [`shown`] characters,
/// single spaces, and at most [`STATUS_MAX`] characters, cut at a word
/// when it can be.
#[must_use]
pub fn clean_line(text: &str) -> String {
    let upper: String = text
        .chars()
        .flat_map(char::to_uppercase)
        .map(|c| if shown(c) { c } else { ' ' })
        .collect();
    let words: Vec<&str> = upper.split_whitespace().collect();
    let mut out = String::new();
    for word in words {
        let extra = usize::from(!out.is_empty()) + word.len();
        if out.len() + extra > STATUS_MAX {
            if out.is_empty() {
                out = word.chars().take(STATUS_MAX).collect();
            }
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}
