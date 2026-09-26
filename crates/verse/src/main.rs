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
//! `verse --seed-rooms <relay-key-file>` creates the NIP-29 chat rooms as
//! the relay; `scripts/verse-relay.sh` runs it.
//!
//! `verse --capture <file.png>` renders the spawn view to a PNG without a
//! window; `--orbit <degrees>`, `--pitch <degrees>`, `--distance <meters>`,
//! and `--size <width>x<height>` adjust the shot. `--board` opens the quest
//! board in the shot, reading `--xp-relay` when given.

use std::process::ExitCode;

use verse::app::Options;
use verse::camera::FollowCamera;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--seed-rooms") {
        return seed(&args[1..]);
    }
    let result = match parse(args.into_iter()) {
        Ok((None, options)) => verse::app::run(&options),
        Ok((Some(shot), options)) => {
            let xp = verse::app::CaptureXp {
                relay: options.xp_relay.clone(),
                keys: options.xp_keys.clone(),
                referees: options.xp_referees.clone(),
                board: shot.board,
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
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let shot = shot.map(|s| Shot {
        width,
        height,
        camera,
        board,
        ..s
    });
    Ok((shot, options))
}
