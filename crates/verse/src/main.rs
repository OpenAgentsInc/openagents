//! The Verse desktop binary.
//!
//! `verse` opens the window and joins the shared world. `--profile <name>`
//! picks the player key (one per profile), `--relay <ws-url>` picks the
//! relay (`VERSE_RELAY` also works), and `--offline` plays alone.
//! `--xp-relay <ws-url>` (or `VERSE_XP_RELAY`) reads NIP-XP quests and
//! awards from another relay than the world's, `--xp-referee <npub>`
//! trusts a referee beyond `~/.openagents/knowledge/xp-trust.json`, and
//! `--xp-key <npub>` counts another of your keys' XP as yours.
//!
//! `--replay <run directory or Gym run ID>` replays a Microcoder run as the
//! agent's visits beside a ghost of Fable 5.1 low's cheapest winning run;
//! `R` in the world lists the retained runs that beat it.
//! `--gym-connection <file>` supplies a signed Gym host connection. The file
//! is read only after entering the Gym; `G` opens its board while inside.
//! The ruins portal loads its verified artwork only on explicit entry. Click
//! the portal or press `F` nearby; inside, `1` through `4` activate the displayed
//! spell hotbar and return controls. Plaza subscriptions pause until you return.
//! `--studio-sim` plays Agent Studio's simulated team in Everglade: on first
//! entry it records the scripted team against a scratch repository under the
//! system's temporary directory, with no model or network. Inside, `F` at a
//! station, or a click on a seat, a monitor, or a station, opens its panel.
//! Without it, Everglade shows the live studio of the host on this computer,
//! through the control socket the desktop app uses; `--studio-socket <path>`
//! names another host's socket, such as a scratch host's. The panels send
//! the studio's intents: goals and messages from the console, answers from
//! the podium, and merge decisions from the merge station. A bar at the top
//! shows the newest goal's progress and how many decisions wait; `J`, or a
//! click on the badge, opens them. A bell rings for a new decision and a
//! chime for a finished task or goal, with a desktop notice while the
//! window is not in front; `V` mutes them, and `--studio-mute` starts muted.
//! the podium, and merge decisions from the merge station.
//! `--grove` opens straight into the Grove, the druid training field.
//! `--crypt` opens straight into the crypt lab, a candlelit laboratory hall;
//! `F` at its door returns to the plaza. `--crypt-fight` opens the cultist
//! fight in the great crypt in a window of its own, played alone with the
//! ritual chamber's controls; the crypt's models are built into the binary.
//! `--frame-times` prints one JSON line of frame times per second.
//! `--everglade` opens straight into Everglade instead of the plaza, and
//! `--studio-notice <text>` leads Everglade's caption with a notice;
//! `openagents studio up` passes both.
//!
//! `--demolition` opens straight into the demolition yard instead: two kit
//! cottages on Everglade's ground to knock down with a sledgehammer. A
//! quick left click or `1` swings, `2` aims Meteor Swarm at a circle of
//! ground that a click casts, and `R` rebuilds the cottages.
//!
//! `verse --seed-rooms <relay-key-file>` creates the NIP-29 chat rooms as
//! the relay; `scripts/verse-relay.sh` runs it.
//!
//! `verse --capture <file.png>` renders the spawn view to a PNG without a
//! window; `--orbit <degrees>`, `--pitch <degrees>`, `--distance <meters>`,
//! and `--size <width>x<height>` adjust the shot. `--board` opens the quest
//! board in the shot, reading `--xp-relay` when given. With `--replay`, the
//! shot shows the replay `--at <seconds>` in, or halfway through.

use std::process::ExitCode;

use verse::app::Options;
use verse::camera::FollowCamera;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--seed-rooms") {
        return seed(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("--crypt-fight") {
        return match verse::imported::crypt_fight::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("verse: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("--chamber") {
        return match chamber(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("verse: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let result = match parse(args.into_iter()) {
        Ok((None, options)) => verse::app::run(&options),
        Ok((Some(shot), options)) => {
            let xp = verse::app::CaptureXp {
                relay: options.xp_relay.clone(),
                keys: options.xp_keys.clone(),
                referees: options.xp_referees.clone(),
                board: shot.board,
                replay: options.replay.clone(),
                at: shot.at,
            };
            verse::app::capture(&shot.path, shot.width, shot.height, shot.camera, &xp)
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("verse: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `verse --seed-rooms <relay-key-file> [--relay <url>]`: create the Verse
/// NIP-29 rooms as the relay. Used by `scripts/verse-relay.sh`.
fn seed(args: &[String]) -> ExitCode {
    let Some(key_file) = args.first() else {
        eprintln!("verse: --seed-rooms needs the relay key file");
        return ExitCode::FAILURE;
    };
    let relay = args
        .iter()
        .position(|a| a == "--relay")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| verse::session::DEFAULT_RELAY.to_owned());
    let result = std::fs::read_to_string(key_file)
        .map_err(|e| format!("cannot read {key_file}: {e}"))
        .and_then(|key| verse::session::seed_rooms(&relay, &key));
    match result {
        Ok(n) => {
            eprintln!("verse: {n} new rooms on {relay}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("verse: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Shot {
    path: std::path::PathBuf,
    width: u32,
    height: u32,
    camera: FollowCamera,
    board: bool,
    at: Option<f64>,
}

/// `--chamber CONFIG [--profile NAME]`: the chamber window a RITUAL crossing
/// opens, in its own process so the Grid window stays.
fn chamber(args: &[String]) -> Result<(), String> {
    let usage = "usage: verse --chamber CONFIG [--profile NAME]";
    let mut args = args.iter();
    let config = args.next().ok_or(usage)?;
    let mut profile = "default".to_owned();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--profile" => profile = args.next().ok_or(usage)?.clone(),
            _ => return Err(usage.into()),
        }
    }
    verse::ritual::run(std::path::Path::new(config), &profile)
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<(Option<Shot>, Options), String> {
    let mut options = Options::default();
    if let Ok(relay) = std::env::var("VERSE_RELAY") {
        options.relay = Some(relay);
    }
    if let Ok(relay) = std::env::var("VERSE_XP_RELAY") {
        options.xp_relay = Some(relay);
    }
    let mut board = false;
    let mut at = None;
    let mut shot: Option<Shot> = None;
    let mut camera = FollowCamera::default();
    let (mut width, mut height) = (1600, 1000);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        let degrees = |v: String| {
            v.parse::<f32>()
                .map(f32::to_radians)
                .map_err(|_| format!("{flag} takes degrees, got {v}"))
        };
        match flag.as_str() {
            "--capture" => {
                shot = Some(Shot {
                    path: value()?.into(),
                    width,
                    height,
                    camera,
                    board,
                    at,
                });
            }
            "--orbit" => camera.yaw_offset = degrees(value()?)?,
            "--pitch" => camera.pitch = degrees(value()?)?,
            "--distance" => {
                let v = value()?;
                camera.distance = v
                    .parse()
                    .map_err(|_| format!("--distance takes meters, got {v}"))?;
            }
            "--size" => {
                let v = value()?;
                let (w, h) = v
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .ok_or(format!("--size takes <width>x<height>, got {v}"))?;
                (width, height) = (w, h);
            }
            "--profile" => options.profile = value()?,
            "--relay" => options.relay = Some(value()?),
            "--offline" => options.relay = None,
            "--xp-relay" => options.xp_relay = Some(value()?),
            "--xp-key" => options.xp_keys.push(value()?),
            "--xp-referee" => options.xp_referees.push(value()?),
            "--board" => board = true,
            "--replay" => options.replay = Some(value()?),
            "--gym-connection" => options.gym_connection = Some(value()?.into()),
            "--studio-sim" => options.studio_sim = true,
            "--studio-socket" => options.studio_socket = Some(value()?.into()),
            "--studio-mute" => options.studio_muted = true,
            "--everglade" => options.everglade = true,
            "--grove" => options.grove = true,
            "--crypt" => options.crypt = true,
            #[cfg(feature = "remote-chamber")]
            "--join" => options.chamber = Some(value()?.into()),
            "--demolition" => {
                options.everglade = true;
                options.demolition = true;
            }
            "--frame-times" => options.frame_times = true,
            "--studio-notice" => options.studio_notice = Some(value()?),
            "--ritual" => options.ritual = Some(value()?.into()),
            "--no-ritual" => options.ritual = None,
            "--at" => {
                let v = value()?;
                at = Some(
                    v.parse::<f64>()
                        .map_err(|_| format!("--at takes seconds, got {v}"))?,
                );
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let shot = shot.map(|s| Shot {
        width,
        height,
        camera,
        board,
        at,
        ..s
    });
    Ok((shot, options))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_gym_connection_path_is_inert_until_world_entry() {
        let (shot, options) = parse(
            ["--offline", "--gym-connection", "/nonexistent/gym.code"]
                .map(str::to_owned)
                .into_iter(),
        )
        .unwrap();
        assert!(shot.is_none());
        assert!(options.relay.is_none());
        assert_eq!(
            options.gym_connection.as_deref(),
            Some(std::path::Path::new("/nonexistent/gym.code"))
        );
    }
}
