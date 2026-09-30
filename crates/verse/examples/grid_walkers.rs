//! Simulated players walking in the Grid, for watching the desktop app's
//! backdrop without a phone.
//!
//! ```sh
//! cargo run -p verse --no-default-features --example grid_walkers -- 3
//! ```
//!
//! starts the in-process loopback relay the tests use, prints its address,
//! and walks three players with fresh in-memory keys in loops on the plaza
//! until stopped. Point the desktop
//! app at the printed address with `--verse-relay`. Nothing reaches a
//! public relay: the relay lives in this process and forgets everything
//! when it stops.

#[path = "../tests/support/loopback_relay.rs"]
mod loopback_relay;

use std::time::{Duration, Instant};

use glam::Vec3;
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::identity::{self, Identity};
use verse::session::{BARE_WORLD, PublishIntervals, Session};

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
        // Facing along the circle: the tangent, as a yaw about +Y.
        let tangent = Vec3::new(-angle.sin(), 0.0, angle.cos());
        (pos, tangent.x.atan2(tangent.z))
    }
}

fn main() -> Result<(), String> {
    let count: usize = std::env::args()
        .nth(1)
        .map_or(Ok(3), |value| value.parse())
        .map_err(|_| "usage: grid_walkers [PLAYERS]")?;
    let relay = loopback_relay::LoopbackRelay::start();
    println!("{}", relay.url);
    // Loops on the plaza in front of the spawn and beside the ball, where
    // new players appear.
    let walks = [
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
    let mut walkers = Vec::new();
    for n in 0..count {
        let id = Identity::from_secret(&format!("walker-{n}"), identity::random_secret())?;
        let mut session = Session::start_presence(id, &relay.url, BARE_WORLD)?;
        // The phones' cadence: a pose every three seconds while moving.
        session.set_publish_intervals(PublishIntervals::mobile())?;
        let (pos, yaw) = walks[n % walks.len()].at(0.0);
        let player = PlayerController::new(pos, yaw);
        let agent = Agent::new(&player);
        walkers.push((session, player, agent));
    }
    let start = Instant::now();
    loop {
        let now = Instant::now();
        let seconds = (now - start).as_secs_f32();
        for (n, (session, player, agent)) in walkers.iter_mut().enumerate() {
            let walk = &walks[n % walks.len()];
            let (pos, yaw) = walk.at(seconds + n as f32 * 7.0);
            player.pos = pos;
            player.yaw = yaw;
            player.speed = walk.speed;
            session.tick(now, player, agent);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
