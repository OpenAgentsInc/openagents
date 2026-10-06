//! Simulated Grid players and the viewer that measures them: `openagents
//! verse walkers` publishes N players walking loops in front of the spawn,
//! and `openagents verse load` joins the same world as a reader and reports
//! what arrives from each publisher, as the phone's crowd would receive it.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use glam::Vec3;
use serde_json::{Value, json};
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::identity::{self, Identity};
use verse::mv::{self, Received};
use verse::session::{PublishIntervals, Session, Status};

use crate::relay::relay_url;
use crate::world::{Context, Scene};
use crate::{Args, Output};

/// How often the walkers report progress, in seconds.
const REPORT_EVERY: u64 = 5;
/// Most simulated players one command runs.
const MAX_WALKERS: usize = 64;

/// One walker's loop: a circle of `radius` m around `center`, walked at
/// `speed` m/s.
struct Walk {
    center: Vec3,
    radius: f32,
    speed: f32,
}

impl Walk {
    fn at(&self, seconds: f32) -> (Vec3, f32) {
        let angle = seconds * self.speed / self.radius;
        let pos = self.center + Vec3::new(angle.cos(), 0.0, angle.sin()) * self.radius;
        let tangent = Vec3::new(-angle.sin(), 0.0, angle.cos());
        (pos, tangent.x.atan2(tangent.z))
    }
}

/// Loops on the plaza in front of the spawn and beside the ball, where new
/// players appear.
const WALKS: [Walk; 3] = [
    Walk {
        center: Vec3::new(0.0, 0.0, -16.0),
        radius: 7.0,
        speed: 2.5,
    },
    Walk {
        center: Vec3::new(12.0, 0.0, -4.0),
        radius: 5.0,
        speed: 1.8,
    },
    Walk {
        center: Vec3::new(-12.0, 0.0, 2.0),
        radius: 6.0,
        speed: 3.2,
    },
];

/// The cadence `--hz` asks for: the shared 5 Hz profile by default, otherwise
/// `hz` moving frames a second within the session's bounds.
pub fn intervals(hz: Option<f32>) -> Result<PublishIntervals, String> {
    let Some(hz) = hz else {
        return Ok(PublishIntervals::mobile());
    };
    if !hz.is_finite() || hz <= 0.0 || hz > 10.0 {
        return Err("--hz takes a rate from 0.02 to 10 frames a second".into());
    }
    let moving = Duration::from_millis(((1000.0 / hz).round() as u64).max(100));
    Ok(PublishIntervals {
        moving,
        idle: Duration::from_secs(5).max(moving),
        state: Duration::from_secs(30),
    })
}

/// The `--world` option: a NIP-MV world identifier, or a zone's name
/// (`everglade`, `lagrange-1`), which stands for that zone's shared world.
fn world_option(option: Option<&str>) -> &'static str {
    let Some(name) = option else {
        return verse::session::BARE_WORLD;
    };
    match verse::zones::ZoneId::from_name(name) {
        Some(zone) => zone.world_id(),
        None => Box::leak(name.to_owned().into_boxed_str()),
    }
}

/// `verse walkers N`: N players with fresh keys walk loops in `--world` on
/// `--relay`, or on an in-process loopback relay, for `wait` seconds (0:
/// until stopped). One line per join, every five seconds a progress line,
/// and a final line with the totals.
pub fn walkers(output: &Output, args: &Args, wait: u64) -> Result<u8, String> {
    let count: usize = args.positional()[0]
        .parse()
        .map_err(|_| "walkers takes a player count".to_owned())?;
    if count == 0 || count > MAX_WALKERS {
        return Err(format!("walkers runs 1 to {MAX_WALKERS} players"));
    }
    let hz = match args.option("hz") {
        Some(text) => Some(
            text.parse::<f32>()
                .map_err(|_| "--hz takes a number".to_owned())?,
        ),
        None => None,
    };
    let intervals = intervals(hz)?;
    let loopback = args
        .switch("loopback")
        .then(verse::loopback::LoopbackRelay::start);
    let relay = loopback.as_ref().map_or_else(
        || relay_url(args.option("relay")),
        |relay| relay.url.clone(),
    );
    let world = world_option(args.option("world"));
    output.line(
        &json!({"type": "relay", "url": relay, "world": world, "players": count,
            "moving_ms": intervals.moving.as_millis() as u64,
            "idle_ms": intervals.idle.as_millis() as u64}),
        |v| {
            format!(
                "relay   {} world {} ({} players)",
                v["url"], v["world"], v["players"]
            )
        },
    );
    let mut walkers = Vec::with_capacity(count);
    for n in 0..count {
        let id = Identity::from_secret(&format!("walker-{n}"), identity::random_secret())?;
        let pubkey = id.signer.pubkey().to_owned();
        let mut session = Session::start_presence(id, &relay, world)?;
        session.set_display_name(Some(&format!("walker-{n}")));
        session.set_publish_intervals(intervals)?;
        let (pos, yaw) = WALKS[n % WALKS.len()].at(n as f32 * 7.0);
        let player = PlayerController::new(pos, yaw);
        let agent = Agent::new(&player);
        output.line(
            &json!({"type": "join", "n": n, "pubkey": pubkey, "at": pos.to_array()}),
            |v| format!("join    {} {}", v["n"], v["pubkey"].as_str().unwrap_or("")),
        );
        walkers.push((session, player, agent));
    }
    let start = Instant::now();
    let deadline = (wait > 0).then(|| start + Duration::from_secs(wait));
    let mut reported = 0u64;
    loop {
        let now = Instant::now();
        let seconds = (now - start).as_secs_f32();
        for (n, (session, player, agent)) in walkers.iter_mut().enumerate() {
            let walk = &WALKS[n % WALKS.len()];
            let (pos, yaw) = walk.at(seconds + n as f32 * 7.0);
            player.pos = pos;
            player.yaw = yaw;
            player.speed = walk.speed;
            session.tick(now, player, agent);
        }
        let elapsed = (now - start).as_secs();
        let done = deadline.is_some_and(|deadline| now >= deadline);
        if elapsed / REPORT_EVERY > reported || done {
            reported = elapsed / REPORT_EVERY;
            output.line(&progress(&walkers, now, elapsed, done), |v| {
                format!(
                    "{:<7} {}s {} online, {} frames, {} refusals",
                    if v["type"] == "done" { "done" } else { "tick" },
                    v["seconds"],
                    v["online"],
                    v["frames_published"],
                    v["refusals"]
                )
            });
        }
        if done {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for (session, player, agent) in &mut walkers {
        session.leave(player, agent);
    }
    Ok(0)
}

fn progress(
    walkers: &[(Session, PlayerController, Agent)],
    now: Instant,
    seconds: u64,
    done: bool,
) -> Value {
    let online = walkers
        .iter()
        .filter(|(session, _, _)| matches!(session.status, Status::Online))
        .count();
    let frames: u64 = walkers.iter().map(|(s, _, _)| s.frames_published()).sum();
    let refusals: u64 = walkers.iter().map(|(s, _, _)| s.refusals()).sum();
    let throttled = walkers
        .iter()
        .filter(|(session, _, _)| session.throttled(now))
        .count();
    let refused: Vec<Value> = walkers
        .iter()
        .enumerate()
        .filter(|(_, (session, _, _))| !matches!(session.status, Status::Online))
        .map(|(n, (session, _, _))| {
            let mut diagnostics = session.diagnostics();
            diagnostics["n"] = json!(n);
            diagnostics
        })
        .collect();
    json!({
        "type": if done { "done" } else { "tick" },
        "not_online": refused,
        "seconds": seconds,
        "players": walkers.len(),
        "online": online,
        "frames_published": frames,
        "refusals": refusals,
        "throttled": throttled,
    })
}

/// What one publisher sent during a `verse load` run.
#[derive(Debug, Default)]
pub struct Publisher {
    frames: u64,
    first: Option<Instant>,
    last: Option<Instant>,
    /// Gaps between consecutive frames, in milliseconds.
    gaps_ms: Vec<u64>,
    /// Publisher time to arrival, in milliseconds (clocks permitting).
    ages_ms: Vec<i64>,
    entities: usize,
}

impl Publisher {
    fn frame(&mut self, now: Instant, published_ms: u64, entities: usize) {
        if let Some(last) = self.last {
            self.gaps_ms.push((now - last).as_millis() as u64);
        }
        self.first.get_or_insert(now);
        self.last = Some(now);
        self.frames += 1;
        self.entities = self.entities.max(entities);
        let arrived = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        self.ages_ms.push(arrived - published_ms as i64);
    }

    fn report(&self) -> Value {
        let span = match (self.first, self.last) {
            (Some(first), Some(last)) if last > first => (last - first).as_secs_f64(),
            _ => 0.0,
        };
        let hz = if span > 0.0 && self.frames > 1 {
            (self.frames - 1) as f64 / span
        } else {
            0.0
        };
        let mut ages = self.ages_ms.clone();
        ages.sort_unstable();
        json!({
            "frames": self.frames,
            "hz": (hz * 100.0).round() / 100.0,
            "max_gap_ms": self.gaps_ms.iter().max().copied().unwrap_or(0),
            "age_p50_ms": percentile(&ages, 50),
            "age_p95_ms": percentile(&ages, 95),
            "age_max_ms": ages.last().copied(),
            "entities": self.entities,
        })
    }
}

fn percentile(sorted: &[i64], p: usize) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    let index = (sorted.len() - 1) * p / 100;
    Some(sorted[index])
}

/// `verse load`: listens to every pose frame in the world for `wait`
/// seconds (default 30) and reports what each publisher delivered. With
/// `--players N`, exits 1 when fewer than N publishers sent a frame; with
/// `--max-age-ms MS`, also when any frame arrived more than MS after its
/// publisher stamped it.
pub fn load(output: &Output, context: &mut Context, args: &Args, wait: u64) -> Result<u8, String> {
    let expected: usize = args.number("players", 0)?;
    let max_age: Option<i64> = match args.option("max-age-ms") {
        Some(text) => Some(
            text.parse::<i64>()
                .ok()
                .filter(|ms| *ms > 0)
                .ok_or_else(|| "--max-age-ms takes a positive number".to_owned())?,
        ),
        None => None,
    };
    let wait = Duration::from_secs(if wait == 0 { 30 } else { wait });
    let world = context.world.clone();
    let mut scene = Scene::default();
    let mut publishers: BTreeMap<String, Publisher> = BTreeMap::new();
    let started = Instant::now();
    context.client.subscribe(
        vec![json!({
            "kinds": [mv::FRAME_KIND],
            "#w": [world],
        })],
        true,
        wait,
        |event| {
            let now = Instant::now();
            if let Some(Received::Frame { pubkey, frame }) = scene.absorb(event, &world) {
                publishers
                    .entry(pubkey)
                    .or_default()
                    .frame(now, frame.t, frame.e.len());
            }
        },
    )?;
    let seconds = started.elapsed().as_secs_f64();
    let frames: u64 = publishers.values().map(|p| p.frames).sum();
    let mut gaps: Vec<u64> = publishers
        .values()
        .filter_map(|p| p.gaps_ms.iter().max().copied())
        .collect();
    gaps.sort_unstable();
    let seen = publishers.len();
    let missing = expected.saturating_sub(seen);
    let oldest = publishers
        .values()
        .filter_map(|p| p.ages_ms.iter().max().copied())
        .max();
    let too_old = max_age.is_some_and(|max| oldest.is_some_and(|oldest| oldest > max));
    let receipt = json!({
        "world": world,
        "relay": relay_url(args.option("relay")),
        "seconds": (seconds * 10.0).round() / 10.0,
        "expected_players": expected,
        "publishers": seen,
        "missing": missing,
        "frames": frames,
        "frames_per_second": if seconds > 0.0 { (frames as f64 / seconds * 100.0).round() / 100.0 } else { 0.0 },
        "worst_gap_ms": gaps.last().copied().unwrap_or(0),
        "oldest_age_ms": oldest,
        "max_age_ms": max_age,
        "too_old": too_old,
        "per_publisher": publishers.iter().map(|(k, p)| (k.clone(), p.report())).collect::<BTreeMap<_, _>>(),
    });
    output.emit(&receipt, |v| {
        let mut text = format!(
            "{} publishers, {} frames in {}s ({} frames/s), worst gap {} ms",
            v["publishers"], v["frames"], v["seconds"], v["frames_per_second"], v["worst_gap_ms"]
        );
        if let Some(rows) = v["per_publisher"].as_object() {
            for (key, row) in rows {
                text.push_str(&format!(
                    "\n  {} {:>4} frames {:>5} Hz gap {:>6} ms age p95 {} ms",
                    &key[..key.len().min(8)],
                    row["frames"],
                    row["hz"],
                    row["max_gap_ms"],
                    row["age_p95_ms"]
                ));
            }
        }
        if v["too_old"] == true {
            text.push_str(&format!(
                "\na frame arrived {} ms after it was stamped, over {} ms",
                v["oldest_age_ms"], v["max_age_ms"]
            ));
        }
        if v["missing"].as_u64().unwrap_or(0) > 0 {
            text.push_str(&format!("\n{} expected players sent nothing", v["missing"]));
        }
        text
    });
    Ok(u8::from(missing > 0 || too_old))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_world_option_names_zones_or_passes_a_world_through() {
        assert_eq!(super::world_option(None), verse::session::BARE_WORLD);
        assert_eq!(super::world_option(Some("everglade")), "verse-everglade");
        assert_eq!(super::world_option(Some("lagrange-1")), "verse-lagrange-1");
        assert_eq!(super::world_option(Some("verse-bare")), "verse-bare");
        assert_eq!(super::world_option(Some("my-world")), "my-world");
    }

    use super::*;

    #[test]
    fn hz_sets_the_moving_cadence_within_bounds() {
        assert_eq!(intervals(None).unwrap().moving, PublishIntervals::MOVING);
        assert_eq!(
            intervals(Some(5.0)).unwrap().moving,
            Duration::from_millis(200)
        );
        assert_eq!(
            intervals(Some(10.0)).unwrap().moving,
            Duration::from_millis(100)
        );
        assert!(intervals(Some(0.0)).is_err());
        assert!(intervals(Some(11.0)).is_err());
    }

    #[test]
    fn walks_stay_on_their_circles() {
        for walk in &WALKS {
            for step in 0..20 {
                let (pos, _) = walk.at(step as f32 * 0.7);
                let distance = (pos - walk.center).length();
                assert!((distance - walk.radius).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn a_publisher_reports_rate_gaps_and_ages() {
        let mut publisher = Publisher::default();
        let start = Instant::now();
        for n in 0..6u32 {
            publisher.frame(start + Duration::from_millis(200 * u64::from(n)), 0, 1);
        }
        let report = publisher.report();
        assert_eq!(report["frames"], 6);
        assert_eq!(report["hz"], 5.0);
        assert_eq!(report["max_gap_ms"], 200);
        assert_eq!(report["entities"], 1);
        assert!(report["age_p95_ms"].as_i64().is_some());
        assert!(report["age_max_ms"].as_i64() >= report["age_p95_ms"].as_i64());
        assert_eq!(percentile(&[1, 2, 3, 4], 50), Some(2));
        assert_eq!(percentile(&[], 50), None);
    }
}
