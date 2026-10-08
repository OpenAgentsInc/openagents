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
//! A portal's zone loads only on explicit entry. Click the portal or press
//! `F` nearby. Plaza subscriptions pause until you return.
//! `--pylon-sim` feeds Everglade's Pylon Field from a labeled DEMO pool
//! instead of this computer's lease table.
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
//! `--meteor-stress-test` opens an offline castle with five meteor casters.
//! `--meteor-showcase` opens two medieval kit houses at golden hour, where a
//! caster calls an eight-meteor swarm down on them; `R` rebuilds them.
//! `--crypt` opens straight into the crypt lab, a candlelit laboratory hall;
//! `F` at its door returns to the plaza. `--crypt-fight` opens the cultist
//! fight in the great crypt in a window of its own, played alone with the
//! ritual chamber's controls; the crypt's models are built into the binary.
//! `--water-lab` opens straight into the Water Lab, a cove with a sea, a
//! river, and a waterfall: `1` to `5` cast its water spells, holding `6`
//! grows a Water Orb that flies where the pointer aims when let go, `7`
//! calls down a Thunderbolt, `B` drops a crate, a barrel, or a plank to
//! float, `T` turns the hour, and `F` at the lantern on the beach returns to
//! the plaza.
//! `--frame-times` prints one JSON line of frame times per second.
//! `--everglade` opens straight into Everglade instead of the plaza, and
//! `--studio-notice <text>` leads Everglade's caption with a notice;
//! `openagents studio up` passes both.
//!
//! In Everglade, `alice`, the workshop agent, stands at her workstation in
//! the owner's house at the east end of Library Way; walk in through the
//! front door, up to her, and press F to talk (`docs/verse/workshop-agent.md`).
//! `--owners-house` opens Everglade just inside the owner's house's front
//! door, facing her. `--alice-outfit coat|light|summer` (or
//! `VERSE_ALICE_OUTFIT`) dresses her: the fitted coat (the default), the coat
//! off, or a summer dress.
//! `--workshop-ask <text>` walks you up to her once she is at her desk,
//! types that request into her panel, and sends it six seconds later, for
//! a demo or a capture.
//!
//! `--demolition` opens straight into the demolition yard instead: two kit
//! cottages on Everglade's ground to knock down with a sledgehammer. A
//! quick left click or `1` swings, `2` aims Meteor Swarm at a circle of
//! ground that a click casts, and `R` rebuilds the cottages.
//!
//! Everglade has no arch back to the plaza; `G` leaves for the Grid. Its
//! hotbar holds no offensive spell. `--dev-destruction`, or
//! `VERSE_DEV_DESTRUCTION=1`, puts Meteor Swarm (6) and the sledgehammer (7)
//! back on it to test destruction locally, in a build with the
//! `dev-destruction` feature only (`cargo run -p verse --features
//! dev-destruction -- --everglade --dev-destruction`); any other build
//! refuses the flag and ignores the variable.
//!
//! `verse --seed-rooms <relay-key-file>` creates the NIP-29 chat rooms as
//! the relay; `scripts/verse-relay.sh` runs it.
//!
//! An agent (`OPENAGENTS_SESSION`, `CLAUDECODE`, and the like) opens no
//! window without a `screen` lease; it renders offscreen with `--capture`.
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
    // Every path but a capture opens a window: an agent needs a screen
    // lease for one (`coder_lease::screen_refusal`, issue #10762).
    if !args.iter().any(|arg| arg == "--capture")
        && let Some(refusal) = coder_lease::screen_refusal_here("verse")
    {
        eprintln!("verse: {refusal}");
        return ExitCode::FAILURE;
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
    // The town clock runs unless `--town-clock off` or VERSE_TOWN_CLOCK=off
    // stops it in late-morning daylight; `--town-hour` and VERSE_TOWN_HOUR
    // pin the time of day either way.
    let mut clock_mode = match std::env::var("VERSE_TOWN_CLOCK") {
        Ok(mode) => Some(town_clock::Setting::parse(&mode)?),
        Err(_) => None,
    };
    if let Ok(outfit) = std::env::var("VERSE_ALICE_OUTFIT") {
        verse::zones::everglade::npcs::set_alice_outfit(&outfit)?;
    }
    let mut clock_hour = match std::env::var("VERSE_TOWN_HOUR") {
        Ok(hour) => Some(town_clock::parse_hour(&hour)?),
        Err(_) => None,
    };
    // A build without the dev-destruction feature ignores the variable.
    if verse::zones::everglade::hotbar::DEV_DESTRUCTION
        && std::env::var("VERSE_DEV_DESTRUCTION").is_ok_and(|v| v == "1")
    {
        options.dev_destruction = true;
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
            "--pylon-sim" => options.pylon_sim = true,
            "--capability-flow" => options.capability_flow = Some(value()?.into()),
            "--onboarding-practice" => options.onboarding_practice = Some(value()?.into()),
            "--onboarding-workbench" => options.onboarding_workbench = Some(value()?.into()),
            "--quest-workbench" => options.quest_workbench = Some(value()?.into()),
            "--contribution-workbench" => options.contribution_workbench = Some(value()?.into()),
            "--compute-workbench" => options.compute_workbench = Some(value()?.into()),
            "--workbench-screen" => {
                options.workbench_screen = Some(match value()?.as_str() {
                    "watch" => terminal_gfx::screen::Mode::Watch,
                    "drive" => terminal_gfx::screen::Mode::Drive,
                    _ => return Err("--workbench-screen requires watch or drive".into()),
                })
            }
            "--workbench-screen-bounds" => {
                let raw = value()?;
                let values = raw
                    .split(',')
                    .map(str::parse::<u16>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "screen bounds require four positive integers".to_owned())?;
                options.workbench_screen_bounds = values
                    .try_into()
                    .map_err(|_| "screen bounds require x,y,width,height")?;
                if options.workbench_screen_bounds[2..]
                    .iter()
                    .any(|value| *value < 160)
                {
                    return Err("screen width and height must be at least 160 points".into());
                }
            }
            "--studio-socket" => options.studio_socket = Some(value()?.into()),
            "--terminal-task" => options.terminal_task = Some(value()?),
            "--terminal-host" => options.terminal_host = Some(value()?),
            "--terminal-store" => options.terminal_store = Some(value()?.into()),
            "--terminal-reference" => options.terminal_reference = Some(value()?),
            "--studio-mute" => options.studio_muted = true,
            "--everglade" => options.everglade = true,
            "--grove" => options.grove = true,
            "--meteor-stress-test" => {
                options.meteor_stress_test = true;
                options.relay = None;
            }
            "--meteor-showcase" => {
                options.meteor_showcase = true;
                options.relay = None;
            }
            "--crypt" => options.crypt = true,
            "--water-lab" => options.water_lab = true,
            #[cfg(feature = "remote-chamber")]
            "--join" => options.chamber = Some(value()?.into()),
            "--demolition" => {
                options.everglade = true;
                options.demolition = true;
            }
            "--dev-destruction" => {
                if !verse::zones::everglade::hotbar::DEV_DESTRUCTION {
                    return Err("--dev-destruction needs a build with the dev-destruction \
                         feature: cargo run -p verse --features dev-destruction"
                        .into());
                }
                options.dev_destruction = true;
            }
            "--frame-times" => options.frame_times = true,
            "--studio-notice" => options.studio_notice = Some(value()?),
            "--workshop-ask" => options.workshop_ask = Some(value()?),
            "--place" => {
                let v = value()?;
                let parts: Vec<f32> = v
                    .split(',')
                    .map(|part| part.trim().parse::<f32>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| format!("--place takes X,Z[,YAW], got {v}"))?;
                let at = |v: f32| (v * 100.0).round() as i32;
                options.place = match parts[..] {
                    [x, z] => Some([at(x), at(z), 0]),
                    [x, z, yaw] => Some([at(x), at(z), at(yaw)]),
                    _ => return Err(format!("--place takes X,Z[,YAW], got {v}")),
                };
            }
            "--town-hour" => clock_hour = Some(town_clock::parse_hour(&value()?)?),
            "--town-clock" => clock_mode = Some(town_clock::Setting::parse(&value()?)?),
            "--owners-house" => {
                options.everglade = true;
                options.owners_house = true;
            }
            "--alice-outfit" => verse::zones::everglade::npcs::set_alice_outfit(&value()?)?,
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
    // A capture that names no time stays in daylight, so the same command
    // draws the same picture at any hour.
    if shot.is_some() && clock_mode.is_none() && clock_hour.is_none() {
        clock_mode = Some(town_clock::Setting::Off);
    }
    options.town_clock = town_clock::Clock::from_settings(clock_mode, clock_hour);
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
    fn town_clock_flags_pin_the_hour_and_set_the_mode() {
        let (_, options) = parse(
            ["--town-hour", "18:30", "--town-clock", "wall:-300"]
                .map(str::to_owned)
                .into_iter(),
        )
        .unwrap();
        assert_eq!(options.town_clock.pinned_hour(), Some(18.5));
        assert_eq!(
            options.town_clock.mode,
            town_clock::Mode::WallClock {
                utc_offset_minutes: -300
            }
        );
        assert!(parse(["--town-hour", "dusk"].map(str::to_owned).into_iter()).is_err());
        // The cycle runs by default, and `off` stops it in daylight.
        if std::env::var_os("VERSE_TOWN_CLOCK").is_none()
            && std::env::var_os("VERSE_TOWN_HOUR").is_none()
        {
            let (_, options) = parse(std::iter::empty()).unwrap();
            assert_eq!(options.town_clock, town_clock::Clock::RUNNING);
            // A capture that names no time is in daylight.
            let (_, options) =
                parse(["--capture", "a.png"].map(str::to_owned).into_iter()).unwrap();
            assert_eq!(options.town_clock, town_clock::Clock::DAYTIME);
        }
        let (_, options) = parse(["--town-clock", "off"].map(str::to_owned).into_iter()).unwrap();
        assert_eq!(options.town_clock, town_clock::Clock::DAYTIME);
        let (_, options) = parse(
            ["--town-clock", "off", "--town-hour", "21"]
                .map(str::to_owned)
                .into_iter(),
        )
        .unwrap();
        assert_eq!(options.town_clock.pinned_hour(), Some(21.0));
    }

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
