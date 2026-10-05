//! The bare world's shared ball and blocks through real scenes over a
//! loopback relay: one player pushes the ball and another sees it roll to
//! the same rest; a player who joins after everyone left finds it there; the
//! pillar's reset reaches a player online and one who joins later; and each
//! scene stays inside the mobile relay's event budget. The relay is a local
//! fixture, not the production relay.
use super::{Config, PointerPhase, Scene};
use std::time::{Duration, Instant};
use verse::session::{EVENT_BUDGET, Status};

use super::bare_presence_tests::loopback_relay;

fn bare_scene(relay: &str) -> Scene {
    let mut scene = Scene::new(Config {
        world_offline: true,
        ..crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)
    })
    .unwrap();
    // The loopback fixture speaks plain `ws://`, which the production relay
    // policy refuses; select it directly. No saved position to restore.
    scene.relay = Some(relay.to_owned());
    scene.restore_spawn = false;
    scene
}

/// Advances every scene on real time until `done` or `limit`.
fn run(
    scenes: &mut [&mut Scene],
    clock: Instant,
    limit: Duration,
    mut done: impl FnMut(&mut [&mut Scene]) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        for scene in scenes.iter_mut() {
            scene.update(clock.elapsed().as_secs_f64()).unwrap();
        }
        if done(scenes) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    false
}

fn online(scene: &Scene) -> bool {
    scene
        .session
        .as_ref()
        .is_some_and(|s| s.status == Status::Online)
}

/// Where the ball is, m.
fn ball(scene: &Scene) -> [f64; 3] {
    scene.world.ball().unwrap().body().pos.to_array()
}

fn apart(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt()
}

fn displaced(scene: &Scene) -> bool {
    let ball = scene.world.ball().unwrap();
    ball.shared().displaced(ball.world())
}

fn asleep(scene: &Scene) -> bool {
    scene.world.ball().unwrap().body().sleeping
}

/// Holds the stick forward while every scene runs, until `done` or `limit`.
fn walk(
    scenes: &mut [&mut Scene],
    who: usize,
    clock: Instant,
    limit: Duration,
    done: impl FnMut(&mut [&mut Scene]) -> bool,
) -> bool {
    let [sx, sy] = scenes[who].stick_center();
    scenes[who].pointer(7, PointerPhase::Down, sx, sy).unwrap();
    scenes[who]
        .pointer(7, PointerPhase::Move, sx, sy - 80.0)
        .unwrap();
    let reached = run(scenes, clock, limit, done);
    scenes[who]
        .pointer(7, PointerPhase::Up, sx, sy - 80.0)
        .unwrap();
    reached
}

/// The most events `pubkey` published with `created_at` inside any
/// 60-second window.
fn busiest_minute(relay: &loopback_relay::LoopbackRelay, pubkey: &str) -> usize {
    let times: Vec<u64> = relay
        .published()
        .into_iter()
        .filter(|event| event.pubkey == pubkey)
        .map(|event| event.created_at)
        .collect();
    times
        .iter()
        .map(|start| {
            times
                .iter()
                .filter(|t| **t >= *start && **t < start + 60)
                .count()
        })
        .max()
        .unwrap_or(0)
}

#[test]
#[ignore = "the Grid's ball is off (owner, 2026-10-01); this exercises the ball"]
fn bare_world_players_share_the_ball_its_rest_and_the_reset() {
    let relay = loopback_relay::LoopbackRelay::start();
    let clock = Instant::now();
    let (mut a, mut b) = (bare_scene(&relay.url), bare_scene(&relay.url));
    a.activate(true).unwrap();
    b.activate(true).unwrap();
    let start = verse::ball::START.to_array();
    assert!(apart(ball(&a), start) < 1e-9 && apart(ball(&b), start) < 1e-9);
    assert!(
        run(&mut [&mut a, &mut b], clock, Duration::from_secs(8), |s| {
            s.iter().all(|scene| online(scene))
        }),
        "the two scenes never came online"
    );

    // A walks into the ball, which stands ahead of the spawn. B sees it move
    // within a couple of mobile body intervals.
    let pushed = walk(
        &mut [&mut a, &mut b],
        0,
        clock,
        Duration::from_secs(12),
        |s| ball(s[0])[2] > start[2] + 0.5,
    );
    assert!(pushed, "A never pushed the ball: {:?}", ball(&a));
    assert!(a.world.ball().unwrap().shared().owns("ball"));
    let seen = run(&mut [&mut a, &mut b], clock, Duration::from_secs(6), |s| {
        ball(s[1])[2] > start[2] + 1.0
    });
    assert!(seen, "B never saw the ball move");
    assert!(!b.world.ball().unwrap().shared().owns("ball"));

    // Both see it come to rest in the same place. A loaded machine runs the
    // scenes' simulations slower than real time (a frame advances at most
    // 0.1 s), so the wait is generous.
    let rested = run(&mut [&mut a, &mut b], clock, Duration::from_secs(60), |s| {
        asleep(s[0]) && asleep(s[1]) && apart(ball(s[0]), ball(s[1])) < 1e-3
    });
    assert!(
        rested,
        "A {:?} asleep {}, B {:?} asleep {}, B stamp {:?}",
        ball(&a),
        asleep(&a),
        ball(&b),
        asleep(&b),
        b.world.ball().unwrap().shared().stamp("ball")
    );
    let rest = ball(&a);
    assert!(rest[2] > start[2] + 3.0);

    // A records the rest pose. Then everyone leaves.
    let snapshot = |relay: &loopback_relay::LoopbackRelay, who: &str| {
        relay
            .published_by(who, verse::mv::STATE_KIND)
            .into_iter()
            .filter(|e| {
                e.tag_values("d").eq([verse::mv::state_address(
                    verse::session::BARE_WORLD,
                    "bodies",
                )])
            })
            .count()
    };
    let a_key = a.public_key.clone();
    assert!(
        run(
            &mut [&mut a, &mut b],
            clock,
            Duration::from_secs(15),
            |_| { snapshot(&relay, &a_key) > 0 }
        ),
        "A never recorded the rest pose"
    );
    a.activate(false).unwrap();
    b.activate(false).unwrap();

    // C joins an empty world and finds the ball where it came to rest.
    let mut c = bare_scene(&relay.url);
    c.activate(true).unwrap();
    let found = run(&mut [&mut c], clock, Duration::from_secs(8), |s| {
        online(s[0]) && apart(ball(s[0]), rest) < 1e-3 && asleep(s[0])
    });
    assert!(found, "C found the ball at {:?}, not {rest:?}", ball(&c));

    // B comes back, and C walks into the pillar. The reset reaches B at
    // once, and A, who joins afterward, finds everything home.
    b.activate(true).unwrap();
    assert!(
        run(&mut [&mut b, &mut c], clock, Duration::from_secs(8), |s| {
            online(s[0]) && apart(ball(s[0]), rest) < 1e-3
        }),
        "B did not find the ball where it rested"
    );
    // Stand three meters in front of the pillar, facing it.
    let mut front = verse::pillar::AT.as_vec3();
    front.z -= 3.0;
    c.world.set_spawn(front, 0.0).unwrap();
    let pressed = walk(
        &mut [&mut c, &mut b],
        0,
        clock,
        Duration::from_secs(10),
        |s| s[0].world.ball().unwrap().pillar().presses() > 0,
    );
    assert!(
        pressed,
        "C never pressed the pillar: {:?}",
        c.world.player.pos
    );
    assert_eq!(c.world.ball().unwrap().pillar().presses(), 1);
    assert!(!displaced(&c));
    let reset = run(&mut [&mut b, &mut c], clock, Duration::from_secs(5), |s| {
        !displaced(s[0])
    });
    assert!(reset, "B still sees the ball at {:?}", ball(&b));
    a.activate(true).unwrap();
    let home = run(
        &mut [&mut a, &mut b, &mut c],
        clock,
        Duration::from_secs(8),
        |s| online(s[0]) && !displaced(s[0]),
    );
    assert!(home, "A rejoined to the ball at {:?}", ball(&a));

    // Every scene stayed inside the mobile relay's budget.
    for key in [&a.public_key, &b.public_key, &c.public_key] {
        let busiest = busiest_minute(&relay, key);
        assert!(busiest <= EVENT_BUDGET, "{busiest} events in one minute");
    }
}
