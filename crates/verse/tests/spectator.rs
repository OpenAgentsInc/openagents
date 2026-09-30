//! The desktop app's backdrop over a loopback relay: a spectator of the Grid
//! sees a simulated player walk, draws its avatar where it walks, and sends
//! the relay nothing but its subscriptions.

#[path = "support/loopback_relay.rs"]
mod loopback_relay;

use std::time::{Duration, Instant};

use glam::Vec3;
use loopback_relay::LoopbackRelay;
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::identity::{self, Identity};
use verse::session::{BARE_WORLD, Session, Status};
use verse::spectator::Overlook;

/// Ticks the spectator (and the walker, when there is one) until `done`
/// holds or the deadline passes.
fn run(
    overlook: &mut Overlook,
    mut walker: Option<(&mut Session, &mut PlayerController, &Agent)>,
    deadline: Duration,
    mut done: impl FnMut(&mut Overlook, Option<&mut PlayerController>, Instant) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        let now = Instant::now();
        overlook.tick(now);
        if let Some((session, player, agent)) = &mut walker {
            session.tick(now, player, agent);
        }
        if done(
            overlook,
            walker.as_mut().map(|(_, player, _)| &mut **player),
            now,
        ) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn a_spectator_sees_a_walking_player_and_publishes_nothing() {
    let relay = LoopbackRelay::start();
    // The spectator connects first, so it is the relay's connection 0.
    let mut overlook = Overlook::new(&relay.url);
    let online = run(&mut overlook, None, Duration::from_secs(5), |o, _, _| {
        o.status() == Some(Status::Online)
    });
    assert!(online, "the spectator never subscribed");

    // Nobody is in the Grid: nothing is drawn but the Grid itself.
    let empty = overlook.mesh(Instant::now(), 0.0);
    assert!(overlook.players(Instant::now()).is_empty());

    // A player walks across the plaza.
    let id = Identity::from_secret("walker", identity::random_secret()).unwrap();
    let mut walker = Session::start_presence(id, &relay.url, BARE_WORLD).unwrap();
    let walker_key = walker.pubkey().to_owned();
    let mut player = PlayerController::new(Vec3::new(-6.0, 0.0, 0.0), 0.0);
    let agent = Agent::new(&player);
    let mut seen = Vec::new();
    let begin = Instant::now();
    let walked = run(
        &mut overlook,
        Some((&mut walker, &mut player, &agent)),
        Duration::from_secs(20),
        |o, player, now| {
            let player = player.expect("the walker");
            // 3 m/s along +x, starting over every 12 m.
            player.pos.x = -6.0 + (now - begin).as_secs_f32() * 3.0 % 12.0;
            player.speed = 3.0;
            if let Some(shown) = o.players(now).first() {
                assert_eq!(shown.pubkey, walker_key);
                seen.push(shown.pos);
            }
            let moved = seen
                .first()
                .zip(seen.last())
                .is_some_and(|(a, b)| a.distance(*b) > 1.0);
            moved && seen.len() > 10
        },
    );
    assert!(walked, "the spectator never saw the player walk: {seen:?}");
    // Its avatar is drawn: the frame has more geometry than the empty Grid.
    let now = Instant::now();
    let drawn = overlook.mesh(now, 0.033);
    assert!(overlook.lively(now));
    assert!(drawn.faces.len() > empty.faces.len());
    // Every drawn position is on the walker's path.
    assert!(
        seen.iter()
            .all(|p| p.z.abs() < 0.5 && p.x > -7.0 && p.x < 7.0)
    );

    // The spectator sent only subscriptions: no event, no presence.
    let spectator = relay.sent_by(0);
    assert!(!spectator.is_empty());
    assert!(spectator.iter().all(|verb| verb == "REQ"), "{spectator:?}");
    // Everything the relay accepted is the walker's.
    let published = relay.published();
    assert!(!published.is_empty());
    assert!(published.iter().all(|event| event.pubkey == walker_key));

    // Paused (the window hidden), the spectator closes its connection and
    // forgets the players.
    overlook.pause();
    assert!(overlook.players(Instant::now()).is_empty());
    assert!(!overlook.connected());
}
