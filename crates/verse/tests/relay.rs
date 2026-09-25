//! Two players on a live relay see each other's avatars and agents.
//!
//! Runs only when `VERSE_TEST_RELAY` names a relay, for example after
//! `scripts/verse-relay.sh`:
//!
//! ```sh
//! VERSE_TEST_RELAY=ws://127.0.0.1:7447 cargo test -p verse --test relay
//! ```

use std::time::{Duration, Instant};

use glam::Vec3;
use verse::agent::Agent;
use verse::controller::{InputState, PlayerController};
use verse::session::Session;

#[test]
fn two_players_see_each_other() {
    let Ok(relay) = std::env::var("VERSE_TEST_RELAY") else {
        eprintln!("skipped: set VERSE_TEST_RELAY to run against a relay");
        return;
    };
    let dir = std::env::temp_dir().join(format!("verse-relay-test-{}", std::process::id()));
    let world = verse::world::build();
    let mut a = Session::start_in(&dir, "alice", &relay).expect("alice signs up");
    let mut b = Session::start_in(&dir, "bob", &relay).expect("bob signs up");
    let wait = Duration::from_secs(3);
    let spawn_a = a.spawn(&world.blockers, verse::world::HALF, wait);
    let spawn_b = b.spawn(&world.blockers, verse::world::HALF, wait);

    let mut pa = PlayerController::new(spawn_a.pos, spawn_a.yaw);
    let mut pb = PlayerController::new(spawn_b.pos, spawn_b.yaw);
    let mut ga = Agent::new(&pa);
    let mut gb = Agent::new(&pb);
    let walk = InputState {
        forward: true,
        ..Default::default()
    };
    let start = Instant::now();
    let mut saw = (false, false);
    while start.elapsed() < Duration::from_secs(8) && !(saw.0 && saw.1) {
        let dt = 1.0 / 60.0;
        pa.update(&walk, dt, &world.blockers, verse::world::HALF);
        pb.update(
            &InputState::default(),
            dt,
            &world.blockers,
            verse::world::HALF,
        );
        ga.update(&pa, dt);
        gb.update(&pb, dt);
        let now = Instant::now();
        a.tick(now, &pa, &ga);
        b.tick(now, &pb, &gb);
        let near = |shown: Vec<verse::crowd::Shown>, who: &str, at: Vec3| {
            let avatar = shown.iter().any(|e| {
                e.pubkey == who && e.role == "avatar" && e.online && e.pos.distance(at) < 3.0
            });
            let agent = shown.iter().any(|e| e.pubkey == who && e.role == "agent");
            avatar && agent
        };
        saw.0 |= near(a.crowd.shown(now), b.pubkey(), pb.pos);
        saw.1 |= near(b.crowd.shown(now), a.pubkey(), pa.pos);
        std::thread::sleep(Duration::from_millis(16));
    }
    a.leave(&pa, &ga);
    b.leave(&pb, &gb);
    drop(a);
    assert!(saw.0, "alice never saw bob's avatar and agent");
    assert!(saw.1, "bob never saw alice's avatar and agent");

    // The relay kept alice's avatar state; the next launch resumes there.
    let mut again = Session::start_in(&dir, "alice", &relay).expect("alice returns");
    let back = again.spawn(&world.blockers, verse::world::HALF, wait);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(back.resumed, "alice did not resume");
    assert!(back.pos.distance(Vec3::new(pa.pos.x, 0.0, pa.pos.z)) < 0.6);
}

#[test]
fn two_agents_that_meet_greet_each_other() {
    let Ok(relay) = std::env::var("VERSE_TEST_RELAY") else {
        eprintln!("skipped: set VERSE_TEST_RELAY to run against a relay");
        return;
    };
    let dir = std::env::temp_dir().join(format!("verse-greet-test-{}", std::process::id()));
    let mut a = Session::start_in(&dir, "greeter-a", &relay).expect("signs up");
    let mut b = Session::start_in(&dir, "greeter-b", &relay).expect("signs up");
    // Stand the two players a few meters apart on the plaza.
    let pa = PlayerController::new(Vec3::new(-2.0, 0.0, -6.0), 0.0);
    let pb = PlayerController::new(Vec3::new(2.0, 0.0, -6.0), 0.0);
    let mut ga = Agent::new(&pa);
    let mut gb = Agent::new(&pb);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8)
        && (a.greets_received() == 0 || b.greets_received() == 0)
    {
        let dt = 1.0 / 60.0;
        ga.update(&pa, dt);
        gb.update(&pb, dt);
        let now = Instant::now();
        for (session, agent, player) in [(&mut a, &mut ga, &pa), (&mut b, &mut gb, &pb)] {
            session.tick(now, player, agent);
            if let Some((pubkey, at)) = session.greeting(now, agent)
                && agent.greet(at)
            {
                session.greeted(&pubkey, at, agent, now);
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let (got_a, got_b) = (a.greets_received(), b.greets_received());
    a.leave(&pa, &ga);
    b.leave(&pb, &gb);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(got_a >= 1, "the first agent was never greeted");
    assert!(got_b >= 1, "the second agent was never greeted");
}
