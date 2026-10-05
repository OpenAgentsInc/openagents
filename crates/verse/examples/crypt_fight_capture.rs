//! The cultist fight in the great crypt, rendered offline.
//!
//! Usage: crypt_fight_capture OUT_DIR [--seconds N] [--no-video]
//!
//! Builds the fight `verse --crypt-fight` plays and writes `opening.png`
//! (the crypt from the landing), `ritual.png` (the acolytes around the
//! circle), `combat.png` (the fight in the nave, mid-cast), `boss.png`
//! (Claude awake on the dais), and `fight.mp4`, the whole run. A scripted
//! player walks down the stairs, fights through the guards with the
//! chamber's own abilities, and advances up the nave, which draws the High
//! Priest and the acolytes in; Claude wakes when the priest falls. If he
//! is still asleep three quarters of the way through, the run fast-forwards
//! the chant to its end, so the last part of the video is always the boss
//! phase. The rules, the AI, and the dice are the game's; only the player's
//! choices are scripted.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glam::Vec3;
use verse::imported::crypt_fight::Fight;
use verse::imported::play::{Ability, Game};
use verse_world::great_crypt as crypt;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FPS: f32 = 30.0;
const OVERLAY: [f32; 2] = [1280.0, 720.0];

struct Args {
    out: PathBuf,
    seconds: f32,
    video: bool,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let out = PathBuf::from(it.next().ok_or("Expected an output directory")?);
    let mut args = Args {
        out,
        seconds: 60.0,
        video: true,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--seconds" => {
                args.seconds = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--seconds takes a number")?;
            }
            "--no-video" => args.video = false,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(args)
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(file, WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| e.to_string())
}

/// The scripted player: face the nearest awake hostile (or Claude once he
/// wakes) and cast the bar in turn, shielding when hurt.
struct Player {
    next: f32,
    turn: usize,
}

impl Player {
    fn target(game: &Game) -> Option<(u64, Vec3)> {
        let ritual = crypt::ritual(game);
        let frame = game.frame();
        let boss_awake = ritual.is_some_and(|r| r.awakened.is_some());
        // The High Priest first, once he is in the fight: his fall breaks
        // the ritual.
        if let Some(priest) = frame.actors.iter().find(|a| {
            a.actor.id == crypt::LEADER
                && a.health > 0
                && ritual.is_some_and(|r| !r.holds(a.actor.id))
                && a.actor.position.distance(game.player) < 20.0
        }) {
            return Some((priest.actor.id, priest.actor.position));
        }
        frame
            .actors
            .iter()
            .filter(|a| a.actor.nameplate && a.health > 0)
            .filter(|a| ritual.is_none_or(|r| !r.holds(a.actor.id)))
            .filter(|a| {
                !boss_awake
                    || a.actor.id == crypt::BOSS
                    || a.actor.position.distance(game.player) < 7.0
            })
            .min_by(|a, b| {
                let key = |x: &verse_engine::director::ActorFrame| {
                    let d = x.actor.position.distance(game.player);
                    if boss_awake && x.actor.id == crypt::BOSS {
                        d - 30.0
                    } else {
                        d
                    }
                };
                key(a).total_cmp(&key(b))
            })
            .map(|a| (a.actor.id, a.actor.position))
    }

    fn act(&mut self, game: &mut Game, walk: bool) -> [f32; 2] {
        if walk {
            return [0.0, 1.0];
        }
        // Nothing awake in reach: advance up the nave toward the dais, which
        // draws the High Priest and the acolytes into the fight.
        let target = Self::target(game)
            .filter(|(_, at)| at.distance(game.player) < 17.0 || game.player.z < -6.0);
        let Some((id, at)) = target else {
            let goal = Vec3::new(0.0, 0.0, -7.5);
            let delta = goal - game.player;
            if delta.length() < 0.8 {
                return [0.0, 0.0];
            }
            game.yaw = (-delta.x).atan2(-delta.z);
            return [0.0, 1.0];
        };
        game.selected = id;
        let delta = at - game.player;
        if delta.length_squared() > 0.01 {
            game.yaw = (-delta.x).atan2(-delta.z);
        }
        let hp = game.snapshot().player.hp;
        if game.time >= self.next && game.casting.is_none() {
            let order = [
                Ability::Fireball,
                Ability::FireBolt,
                Ability::MagicMissile,
                Ability::FireBolt,
                Ability::Bow,
                Ability::MagicMissile,
            ];
            let distance = Vec3::new(delta.x, 0.0, delta.z).length();
            let ability = if hp < 90 && game.activate(Ability::Shield).is_ok() {
                None
            } else if distance < 4.0 {
                Some(Ability::Thunderwave)
            } else {
                Some(order[self.turn % order.len()])
            };
            if let Some(ability) = ability {
                if game.activate(ability).is_err() {
                    let _ = game.activate(Ability::FireBolt);
                }
                self.turn += 1;
            }
            self.next = game.time + 1.25;
        }
        [0.0, 0.0]
    }
}

/// Eases the follow camera toward a shoulder view of the player.
fn follow(game: &mut Game, dt: f32) {
    let turn = (game.yaw - game.camera.yaw + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    game.camera.yaw += turn * (dt * 2.5).min(1.0);
    game.camera.pitch = 0.3;
    game.camera.distance = 7.5;
    game.camera.target_distance = 7.5;
}

fn main() -> Result<(), String> {
    let args = args()?;
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let dir =
        std::env::temp_dir().join(format!("verse-crypt-fight-capture-{}", std::process::id()));
    eprintln!("building the fight in {}", dir.display());
    let mut fight = Fight::load(&dir)?;
    let mut renderer = fight.renderer(WIDTH, HEIGHT)?;
    renderer.set_overlay_size(OVERLAY[0], OVERLAY[1]);
    let mut shoot = |fight: &Fight, camera: Option<(Vec3, Vec3)>| -> Result<Vec<u8>, String> {
        let c = fight.compose(1.0, [WIDTH, HEIGHT], OVERLAY, [-100.0; 2], camera)?;
        renderer.draw(c.view, &c.instances, &c.ui, &c.lighting)
    };
    // Stills before the fight moves: the crypt from the landing, and the
    // acolytes around the circle.
    fight.game.tick(1.0 / FPS, [0.0; 2])?;
    let opening = shoot(
        &fight,
        Some((Vec3::new(1.8, 4.6, 16.9), Vec3::new(0.0, 1.6, -14.0))),
    )?;
    write_png(&args.out.join("opening.png"), &opening)?;
    let ritual = shoot(
        &fight,
        Some((Vec3::new(4.6, 3.1, -10.4), Vec3::new(0.0, 1.3, -17.6))),
    )?;
    write_png(&args.out.join("ritual.png"), &ritual)?;
    let third = shoot(&fight, None)?;
    write_png(&args.out.join("start.png"), &third)?;
    eprintln!("stills written");

    let mut encoder = if args.video {
        let path = args.out.join("fight.mp4");
        Some(
            Command::new("ffmpeg")
                .args([
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgba",
                    "-s",
                ])
                .arg(format!("{WIDTH}x{HEIGHT}"))
                .args([
                    "-r", "30", "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20",
                ])
                .arg(&path)
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Cannot start ffmpeg: {e}"))?,
        )
    } else {
        None
    };
    let dt = 1.0 / FPS;
    let mut player = Player { next: 0.0, turn: 0 };
    let frames = (args.seconds * FPS) as usize;
    let boss_at = args.seconds * 0.75;
    let mut best_combat = (0usize, Vec::new());
    let mut boss_shot = false;
    let mut deaths = 0;
    for k in 0..frames {
        let t = k as f32 * dt;
        // Down the stairs first, then fight from the nave.
        let walk = t < 1.5;
        if walk {
            fight.game.yaw = 0.0;
        }
        if t >= boss_at {
            if let Some(ritual) = fight
                .game
                .encounter
                .as_mut()
                .and_then(|e| e.ritual.as_mut())
                .filter(|r| r.awakened.is_none())
            {
                // Fast-forward the chant to its end.
                ritual.progress = crypt::RITUAL_SECONDS - 0.01;
            }
        }
        let movement = player.act(&mut fight.game, walk);
        follow(&mut fight.game, dt);
        fight.game.tick(dt, movement)?;
        if fight.game.snapshot().player.hp == 0 {
            deaths += 1;
            fight.game.respawn_player()?;
        }
        // Open on a slow pass over the nave before the shoulder view.
        let intro = (1.0 - t / 2.5).clamp(0.0, 1.0);
        let camera = (intro > 0.0).then(|| {
            let s = intro * intro * (3.0 - 2.0 * intro);
            let eye = Vec3::new(0.0, 4.6, 16.9).lerp(fight.game.frame().eye, 1.0 - s);
            let target = Vec3::new(0.0, 1.6, -14.0).lerp(fight.game.frame().target, 1.0 - s);
            (eye, target)
        });
        let pixels = shoot(&fight, camera)?;
        let visuals = verse_world::visuals::Combat::extract(&fight.game);
        let busy = visuals.projectiles.len() + visuals.hostile.len() + visuals.impacts.len();
        if t > 5.0 && t < boss_at && busy > best_combat.0 {
            best_combat = (busy, pixels.clone());
        }
        let awake = crypt::ritual(&fight.game).and_then(|r| r.awakened);
        if !boss_shot
            && awake.is_some_and(|at| fight.game.time - at > 2.5)
            && fight
                .game
                .encounter
                .as_ref()
                .is_some_and(|e| e.casts.iter().any(|c| c.actor == crypt::BOSS))
        {
            write_png(&args.out.join("boss.png"), &pixels)?;
            let close = shoot(
                &fight,
                Some((
                    fight.game.player + Vec3::new(1.6, 2.6, 3.4),
                    crypt::CIRCLE + Vec3::Y * 2.6,
                )),
            )?;
            write_png(&args.out.join("boss-close.png"), &close)?;
            boss_shot = true;
        }
        if let Some(child) = &mut encoder {
            child
                .stdin
                .as_mut()
                .ok_or("ffmpeg has no input")?
                .write_all(&pixels)
                .map_err(|e| e.to_string())?;
        }
        if k % 150 == 0 {
            let r = crypt::ritual(&fight.game);
            eprintln!(
                "t {t:.1} hp {} player {:.1} ritual {:.0}% awake {:?} deaths {deaths}",
                fight.game.snapshot().player.hp,
                fight.game.player,
                r.map_or(0.0, |r| r.fraction() * 100.0),
                r.and_then(|r| r.awakened),
            );
        }
    }
    if !best_combat.1.is_empty() {
        write_png(&args.out.join("combat.png"), &best_combat.1)?;
    }
    if let Some(mut child) = encoder {
        drop(child.stdin.take());
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("ffmpeg failed: {status}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("wrote {}", args.out.display());
    Ok(())
}
