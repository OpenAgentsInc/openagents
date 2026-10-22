//! Walk-in portals: arches the player enters by walking (or flying) through
//! the opening, with no button. The OpenAgents app's bare world ("the Grid")
//! has two, to Lagrange 1 and to Everglade, and a zone entered from it
//! returns through its own arch the same way. Coder's plaza keeps its
//! tap-and-button arches.
//!
//! The portal to Lagrange 1 is hidden for now ([`GRID_PORTAL_OPEN`] is
//! `false`): no arch, no lettering, and no crossing in any build of the phone
//! apps. The portals to Everglade ([`GRID_EVERGLADE_OPEN`]) and to the
//! Lagrange 2 factory ([`GRID_LAGRANGE2_OPEN`]) are shown.
//!
//! A gate is an arch at `at`, turned `yaw` about the vertical. Its local +Z
//! points through the opening, away from the side the player approaches:
//! the side the arch's lettering reads from.

use glam::{Quat, Vec3};

use super::ZoneId;
use crate::mesh::Mesh;

/// Whether the Grid (the OpenAgents app's bare world) shows its walk-in
/// portal to Lagrange 1. Off for now, in debug and release builds alike:
/// the Grid draws no arch or lettering and nothing on it enters Lagrange 1.
/// Lagrange 1 itself (the zone, `verse-lagrange`, its CLI) is unchanged, and
/// Coder's plaza arches do not read this. Restore the portal by setting this
/// to `true` (and reverting the "hidden for now" notes in
/// `docs/verse/mobile.md` and the phone apps' READMEs).
pub const GRID_PORTAL_OPEN: bool = false;
/// The Grid's portal in the ball's layout frame: to the side opposite the
/// dominoes and short of the stack, so the ball, the stack, and the
/// dominoes all stand clear of the arch and its approach. Side, forward, m.
pub const GRID_PORTAL_AT: [f64; 2] = [-9.0, 11.0];
/// Whether the Grid shows its walk-in portal to Everglade. On: a world with
/// zone storage draws the arch, and walking through it loads Everglade's
/// pinned pack and enters. A world without zone storage draws no arch.
pub const GRID_EVERGLADE_OPEN: bool = true;
/// The Grid's portal to Everglade in the same frame: the hidden Lagrange 1
/// arch's mirror image, on the dominoes' side and short of them, so the ball,
/// the stack, the dominoes, the pillar, and the Gym all stand clear of it.
/// Side, forward, m.
pub const GRID_EVERGLADE_AT: [f64; 2] = [9.0, 11.0];
/// Whether the Grid shows its walk-in portal to Lagrange 2, the spaceship
/// factory at Earth–Moon L2. On: like Lagrange 1 it needs no download, so
/// every Grid shows it, and walking through enters at once.
pub const GRID_LAGRANGE2_OPEN: bool = true;
/// The Grid's portal to Lagrange 2 in the same frame: beside the hidden
/// Lagrange 1 arch and farther out, clear of the ball, the blocks, the
/// pillar, the Gym, and the other arches' approaches. Side, forward, m.
pub const GRID_LAGRANGE2_AT: [f64; 2] = [-20.0, 19.0];
/// The Grid's RITUAL arch in the same frame: centered, beyond the stack
/// and the dominoes, so a player walking out from the spawn passes between
/// the side arches and the blocks before reaching it. Shown only when the
/// desktop has a pinned chamber to join ([`crate::ritual`]). Side, forward, m.
pub const GRID_RITUAL_AT: [f64; 2] = [0.0, 26.0];
/// The Grid's first arch to a hosted zone in the same frame (THE ICE CREAM
/// MAN's): the Lagrange 2 arch's mirror image, on the dominoes' side and
/// farther out, clear of the blocks, the pillar, and the other arches'
/// approaches. Side, forward, m.
pub const GRID_HOSTED_AT: [f64; 2] = [20.0, 19.0];
/// THE HAUNTED HOUSE's arch, 7 m outboard of the first hosted arch, because
/// both are horror games. Side, forward, m.
pub const GRID_HAUNT_AT: [f64; 2] = [27.0, 19.0];
/// How far apart further hosted arches stand, outboard of the last, m.
pub const HOSTED_SPACING: f64 = 7.0;
/// The most hosted arches the Grid shows.
pub const MAX_HOSTED: usize = 6;
/// How near the hosted arch a page starts fetching the zone's first play,
/// m: a few seconds' walk.
pub const HOSTED_PREFETCH: f32 = 24.0;
/// The RITUAL arch's lettering.
pub const RITUAL_SIGN: &str = "RITUAL";
/// Half the arch's clear opening between its pillars, m.
pub const OPENING_HALF: f32 = 1.5;
/// How far either side of the arch's plane counts as inside it, m.
const DEPTH: f32 = 0.6;
/// Feet heights relative to the arch's base that pass through the opening.
const FEET: [f32; 2] = [-1.0, 3.2];
/// Where the player stands after coming back through: in front of the arch,
/// facing away from it, m.
pub const RETURN_DISTANCE: f32 = 3.5;
/// Seconds after a crossing before a gate admits another.
pub const COOLDOWN: f32 = 1.0;

/// A zone hosted on livehost that the Grid has an arch to: a tenant whose
/// page the application opens when the player walks through
/// (`docs/verse/ice-cream-man-live.md`). The world draws the arch and says
/// when it is crossed; it never loads the zone itself. The Grid takes a
/// list of them as data (tenant, sign, site), so an arch can be added
/// without a new build.
#[derive(Clone, Debug, PartialEq)]
pub struct HostedPortal {
    /// The livehost tenant, such as `icecream`.
    pub tenant: String,
    /// The arch's lettering.
    pub sign: String,
    /// Where it stands: side, forward, m, in the frame the Grid's other
    /// arches use.
    pub site: [f64; 2],
}

/// The arches the Grid knows by tenant: their lettering and site. Others
/// get their tenant as lettering and the next free site.
pub const KNOWN_HOSTED: [(&str, &str, [f64; 2]); 2] = [
    ("icecream", "ICE CREAM MAN", GRID_HOSTED_AT),
    ("haunt", "HAUNTED HOUSE", GRID_HAUNT_AT),
];

impl HostedPortal {
    /// A portal to `tenant` lettered `sign`, at its known site or the
    /// first hosted arch's.
    ///
    /// # Errors
    /// The tenant isn't a livehost name, or the sign has letters the arch
    /// can't draw or is longer than 16.
    pub fn new(tenant: &str, sign: &str) -> Result<Self, String> {
        let site = KNOWN_HOSTED
            .iter()
            .find(|(t, _, _)| *t == tenant)
            .map_or(GRID_HOSTED_AT, |(_, _, site)| *site);
        Self::at(tenant, sign, site)
    }

    /// The lettering an arch to `tenant` has when the page names none.
    #[must_use]
    pub fn default_sign(tenant: &str) -> String {
        KNOWN_HOSTED
            .iter()
            .find(|(t, _, _)| *t == tenant)
            .map_or_else(
                || tenant.replace('-', " ").to_ascii_uppercase(),
                |(_, sign, _)| (*sign).to_owned(),
            )
    }

    /// The arches a page asks for, in order: each tenant at its known site,
    /// or outboard of the last arch, [`HOSTED_SPACING`] apart. `signs`
    /// letters them in order where given. Duplicates and names that aren't
    /// tenants are dropped (and named in the second list); at most
    /// [`MAX_HOSTED`].
    #[must_use]
    pub fn list(tenants: &[String], signs: &[String]) -> (Vec<Self>, Vec<String>) {
        let mut out: Vec<Self> = Vec::new();
        let mut refused = Vec::new();
        for (i, tenant) in tenants.iter().enumerate() {
            let tenant = tenant.trim();
            if tenant.is_empty() || out.iter().any(|p| p.tenant == tenant) {
                continue;
            }
            if out.len() >= MAX_HOSTED {
                refused.push(format!(
                    "{tenant}: the Grid shows at most {MAX_HOSTED} hosted arches"
                ));
                continue;
            }
            let sign = signs
                .get(i)
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| Self::default_sign(tenant));
            let known = KNOWN_HOSTED.iter().find(|(t, _, _)| *t == tenant);
            let site = match known {
                Some((_, _, site)) if !out.iter().any(|p| p.site == *site) => *site,
                _ => {
                    // Outboard of the farthest arch so far.
                    let side = out
                        .iter()
                        .map(|p| p.site[0])
                        .chain(KNOWN_HOSTED.iter().map(|(_, _, s)| s[0]))
                        .fold(GRID_HOSTED_AT[0], f64::max);
                    let taken = |x: f64| out.iter().any(|p| (p.site[0] - x).abs() < 1.0);
                    let mut x = side + HOSTED_SPACING;
                    while taken(x) {
                        x += HOSTED_SPACING;
                    }
                    [x, GRID_HOSTED_AT[1]]
                }
            };
            match Self::at(tenant, &sign, site) {
                Ok(portal) => out.push(portal),
                Err(error) => refused.push(error),
            }
        }
        (out, refused)
    }

    /// A portal to `tenant` lettered `sign` at `site`.
    ///
    /// # Errors
    /// As [`Self::new`], or the site isn't a finite point.
    pub fn at(tenant: &str, sign: &str, site: [f64; 2]) -> Result<Self, String> {
        if !site.iter().all(|v| v.is_finite()) {
            return Err(format!("{tenant}'s arch needs a real place"));
        }
        let name_ok = !tenant.is_empty()
            && tenant.len() <= 64
            && tenant
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !name_ok {
            return Err(format!("{tenant:?} is not a tenant name"));
        }
        let sign = sign.trim().to_ascii_uppercase();
        let sign_ok = !sign.is_empty()
            && sign.len() <= 16
            && sign
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b" /.".contains(&b));
        if !sign_ok {
            return Err(format!("the arch can't letter {sign:?}"));
        }
        Ok(Self {
            tenant: tenant.into(),
            sign,
            site,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub at: Vec3,
    pub yaw: f32,
}

impl Gate {
    /// The Grid's portal to Lagrange 1, placed in the frame the ball and
    /// blocks were laid out in and turned to face that frame's origin (the
    /// spawn), so it reads face-on from where the player starts.
    #[must_use]
    pub fn grid(layout: &crate::blocks::Layout) -> Self {
        Self::on_grid(layout, GRID_PORTAL_AT)
    }

    /// The Grid's portal to Everglade, placed and turned as [`Self::grid`].
    #[must_use]
    pub fn everglade(layout: &crate::blocks::Layout) -> Self {
        Self::on_grid(layout, GRID_EVERGLADE_AT)
    }

    /// The Grid's portal to Lagrange 2, placed and turned as [`Self::grid`].
    #[must_use]
    pub fn lagrange2(layout: &crate::blocks::Layout) -> Self {
        Self::on_grid(layout, GRID_LAGRANGE2_AT)
    }

    /// The Grid's first arch to a hosted zone, placed and turned as
    /// [`Self::grid`].
    #[must_use]
    pub fn hosted(layout: &crate::blocks::Layout) -> Self {
        Self::on_grid(layout, GRID_HOSTED_AT)
    }

    /// A hosted arch at `site` (side, forward), placed and turned as
    /// [`Self::grid`].
    #[must_use]
    pub fn hosted_at(layout: &crate::blocks::Layout, site: [f64; 2]) -> Self {
        Self::on_grid(layout, site)
    }

    /// The Grid's RITUAL arch to the pinned chamber, placed and turned as
    /// [`Self::grid`].
    #[must_use]
    pub fn ritual(layout: &crate::blocks::Layout) -> Self {
        Self::on_grid(layout, GRID_RITUAL_AT)
    }

    /// An arch at `site` (side, forward) in `layout`, inside the world's
    /// walls, facing the layout's origin.
    fn on_grid(layout: &crate::blocks::Layout, site: [f64; 2]) -> Self {
        let limit = f64::from(crate::world::HALF) - 6.0;
        let point = layout.at(site[0], site[1], 0.0);
        let point = glam::DVec3::new(
            point.x.clamp(-limit, limit),
            0.0,
            point.z.clamp(-limit, limit),
        );
        let away = (point - layout.origin).as_vec3();
        let away = Vec3::new(away.x, 0.0, away.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        Self {
            at: point.as_vec3(),
            yaw: away.x.atan2(away.z),
        }
    }

    /// A zone's axis-aligned return arch.
    #[must_use]
    pub fn fixed(at: Vec3) -> Self {
        Self { at, yaw: 0.0 }
    }

    fn rotation(self) -> Quat {
        Quat::from_rotation_y(self.yaw)
    }

    /// A world point in the arch's frame.
    #[must_use]
    pub fn local(self, point: Vec3) -> Vec3 {
        self.rotation().inverse() * (point - self.at)
    }

    /// Whether feet moving from `from` to `to` pass through the opening:
    /// inside its width and height, and either crossing its plane or
    /// standing within it.
    #[must_use]
    pub fn crossed(self, from: Vec3, to: Vec3) -> bool {
        let (a, b) = (self.local(from), self.local(to));
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let inside = |p: Vec3| p.x.abs() <= OPENING_HALF && (FEET[0]..=FEET[1]).contains(&p.y);
        if b.z.abs() <= DEPTH {
            return inside(b);
        }
        // A fast step can jump the band: check where it crossed the plane.
        if a.z.signum() != b.z.signum() {
            let t = a.z / (a.z - b.z);
            return inside(a.lerp(b, t));
        }
        false
    }

    /// Where the player stands after coming back through the arch, and the
    /// yaw that faces away from it.
    #[must_use]
    pub fn front(self) -> (Vec3, f32) {
        let back = self.rotation() * Vec3::NEG_Z;
        let pos = self.at + back * RETURN_DISTANCE;
        (Vec3::new(pos.x, self.at.y, pos.z), back.x.atan2(back.z))
    }

    /// The arch, lettered for `destination`, in the neutral palette.
    #[must_use]
    pub fn mesh(self, zone: ZoneId, sign: &str, elapsed: f32) -> Mesh {
        let mut mesh = Mesh::default();
        super::arch(&mut mesh, zone, sign, Vec3::ZERO, elapsed);
        mesh.neutralize();
        let transform = glam::Mat4::from_rotation_translation(self.rotation(), self.at);
        for vertex in mesh.lines.iter_mut().chain(mesh.faces.iter_mut()) {
            vertex.pos = transform
                .transform_point3(Vec3::from_array(vertex.pos))
                .to_array();
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn layout() -> crate::blocks::Layout {
        crate::blocks::Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z)
    }

    #[test]
    fn the_opening_admits_feet_passing_through_and_nothing_beside_it() {
        let gate = Gate::grid(&layout());
        let world =
            |x: f32, y: f32, z: f32| gate.at + Quat::from_rotation_y(gate.yaw) * Vec3::new(x, y, z);
        assert!(gate.crossed(world(0.0, 0.0, -2.0), world(0.0, 0.0, -0.4)));
        assert!(gate.crossed(world(1.2, 0.0, -1.0), world(1.2, 0.0, 1.0)));
        // A long step over the plane still counts where it crossed.
        assert!(gate.crossed(world(0.0, 0.0, -3.0), world(0.5, 0.0, 3.0)));
        // Walking past the pillars, approaching, or leaping over does not.
        assert!(!gate.crossed(world(2.5, 0.0, -1.0), world(2.5, 0.0, 1.0)));
        assert!(!gate.crossed(world(0.0, 0.0, -3.0), world(0.0, 0.0, -1.0)));
        assert!(!gate.crossed(world(0.0, 4.0, -1.0), world(0.0, 4.0, 1.0)));
        assert!(!gate.crossed(world(0.0, 0.0, -1.0), Vec3::NAN));
    }

    #[test]
    fn the_return_stands_in_front_facing_away_and_outside_the_opening() {
        for gate in [
            Gate::grid(&layout()),
            Gate::fixed(Vec3::new(-5.0, 4.4, 22.5)),
        ] {
            let (pos, yaw) = gate.front();
            assert!(!gate.crossed(pos, pos));
            assert!((pos.distance(gate.at) - RETURN_DISTANCE).abs() < 1e-4);
            assert!(gate.local(pos).z < 0.0);
            // Walking forward from there leads away from the arch.
            let ahead = pos + crate::controller::forward(yaw) * 2.0;
            assert!(ahead.distance(gate.at) > pos.distance(gate.at));
        }
    }

    #[test]
    fn the_grid_portal_faces_the_spawn_clear_of_the_ball_and_blocks() {
        let layout = layout();
        let gate = Gate::grid(&layout);
        // The spawn is on the lettered side, and the arch faces it.
        let spawn = crate::world::SPAWN;
        assert!(gate.local(spawn).z < -8.0);
        assert!(gate.local(spawn).x.abs() < 0.01);
        // Every block and the ball, with the pools of light under them.
        let ball = crate::ball::Ball::new();
        let world = ball.world();
        let blocks = ball.blocks();
        let ball_at = ball.body().pos.as_vec3();
        assert!(ball_at.with_y(0.0).distance(gate.at) - crate::ball::RADIUS as f32 > 6.0);
        for &id in blocks.cubes().iter().chain(blocks.dominoes()) {
            let p = world[id].pos.as_vec3().with_y(0.0);
            assert!(
                p.distance(gate.at) > 5.0,
                "{p} is near the arch at {}",
                gate.at
            );
        }
        for (center, radius) in [blocks.stack_pool(), blocks.domino_pool()] {
            assert!(center.with_y(0.0).distance(gate.at) - radius > 1.0);
        }
        // Near the world's edge the arch stays inside the walls.
        let edge = crate::blocks::Layout::new(
            DVec3::new(f64::from(crate::world::HALF) - 1.0, 0.0, 0.0),
            DVec3::X,
        );
        let gate = Gate::grid(&edge);
        assert!(gate.at.x.abs() < crate::world::HALF - 5.0);
        assert!(gate.at.z.abs() < crate::world::HALF - 5.0);
    }

    #[test]
    fn the_everglade_portal_faces_the_spawn_clear_of_everything_on_the_grid() {
        let layout = crate::blocks::Layout::grid();
        let gate = Gate::everglade(&layout);
        // The spawn is on the lettered side, and the arch faces it.
        let spawn = crate::world::SPAWN;
        assert!(gate.local(spawn).z < -8.0);
        assert!(gate.local(spawn).x.abs() < 0.01);
        // The ball, every block, and the pools of light under them, though
        // the Grid shows none of them for now.
        let ball = crate::ball::Ball::new();
        let world = ball.world();
        let blocks = ball.blocks();
        let ball_at = ball.body().pos.as_vec3();
        assert!(ball_at.with_y(0.0).distance(gate.at) - crate::ball::RADIUS as f32 > 6.0);
        for &id in blocks.cubes().iter().chain(blocks.dominoes()) {
            let p = world[id].pos.as_vec3().with_y(0.0);
            assert!(p.distance(gate.at) > 5.0, "{p} is near the arch");
        }
        for (center, radius) in [blocks.stack_pool(), blocks.domino_pool()] {
            assert!(center.with_y(0.0).distance(gate.at) - radius > 1.0);
        }
        // The reset pillar, and the hidden Lagrange 1 arch and its approach.
        let pillar = crate::pillar::AT.as_vec3().with_y(0.0);
        assert!(pillar.distance(gate.at) > 6.0);
        let lagrange = Gate::grid(&layout);
        assert!(lagrange.at.distance(gate.at) > 12.0);
        for arch in [gate, lagrange] {
            let (front, _) = arch.front();
            let other = if arch == gate { lagrange } else { gate };
            assert!(front.distance(other.at) > 10.0);
        }
        // The Gym's walls, with room to walk around the arch.
        for wall in crate::world::GymSite::GRID.walls() {
            let nearest = Vec3::new(
                gate.at.x.clamp(wall.min[0], wall.max[0]),
                0.0,
                gate.at.z.clamp(wall.min[1], wall.max[1]),
            );
            assert!(nearest.distance(gate.at) > 14.0, "{wall:?}");
        }
        // Inside the world's walls.
        assert!(gate.at.x.abs() < crate::world::HALF - 5.0);
        assert!(gate.at.z.abs() < crate::world::HALF - 5.0);
    }

    #[test]
    fn the_lagrange2_portal_faces_the_spawn_clear_of_everything_on_the_grid() {
        let layout = crate::blocks::Layout::grid();
        let gate = Gate::lagrange2(&layout);
        let spawn = crate::world::SPAWN;
        assert!(gate.local(spawn).z < -8.0);
        assert!(gate.local(spawn).x.abs() < 0.01);
        let ball = crate::ball::Ball::new();
        let world = ball.world();
        let blocks = ball.blocks();
        let ball_at = ball.body().pos.as_vec3();
        assert!(ball_at.with_y(0.0).distance(gate.at) - crate::ball::RADIUS as f32 > 6.0);
        for &id in blocks.cubes().iter().chain(blocks.dominoes()) {
            let p = world[id].pos.as_vec3().with_y(0.0);
            assert!(p.distance(gate.at) > 5.0, "{p} is near the arch");
        }
        for (center, radius) in [blocks.stack_pool(), blocks.domino_pool()] {
            assert!(center.with_y(0.0).distance(gate.at) - radius > 1.0);
        }
        let pillar = crate::pillar::AT.as_vec3().with_y(0.0);
        assert!(pillar.distance(gate.at) > 6.0);
        // Every other arch, shown or hidden, and their approaches.
        let others = [
            Gate::grid(&layout),
            Gate::everglade(&layout),
            Gate::ritual(&layout),
        ];
        for other in others {
            assert!(other.at.distance(gate.at) > 10.0, "{}", other.at);
            assert!(gate.front().0.distance(other.at) > 8.0);
            assert!(other.front().0.distance(gate.at) > 8.0);
        }
        for wall in crate::world::GymSite::GRID.walls() {
            let nearest = Vec3::new(
                gate.at.x.clamp(wall.min[0], wall.max[0]),
                0.0,
                gate.at.z.clamp(wall.min[1], wall.max[1]),
            );
            assert!(nearest.distance(gate.at) > 14.0, "{wall:?}");
        }
        assert!(gate.at.x.abs() < crate::world::HALF - 5.0);
        assert!(gate.at.z.abs() < crate::world::HALF - 5.0);
    }

    /// The distance from `p` to the segment `a`-`b`, on the ground.
    fn off_line(p: Vec3, a: Vec3, b: Vec3) -> f32 {
        let (p, a, b) = (p.with_y(0.0), a.with_y(0.0), b.with_y(0.0));
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        p.distance(a + ab * t)
    }

    #[test]
    fn the_haunted_house_arch_stands_beside_ice_cream_man_with_both_approaches_clear() {
        let layout = crate::blocks::Layout::grid();
        let haunt = Gate::hosted_at(&layout, GRID_HAUNT_AT);
        let ice = Gate::hosted(&layout);
        let spawn = crate::world::SPAWN;
        for gate in [haunt, ice] {
            // Faces the spawn, lettered side toward it.
            assert!(gate.local(spawn).z < -8.0);
            assert!(gate.local(spawn).x.abs() < 0.01);
            let ball = crate::ball::Ball::new();
            let world = ball.world();
            let blocks = ball.blocks();
            let ball_at = ball.body().pos.as_vec3();
            assert!(ball_at.with_y(0.0).distance(gate.at) - crate::ball::RADIUS as f32 > 6.0);
            for &id in blocks.cubes().iter().chain(blocks.dominoes()) {
                let p = world[id].pos.as_vec3().with_y(0.0);
                assert!(p.distance(gate.at) > 5.0, "{p} is near the arch");
                // Nothing on the straight walk in from the spawn.
                assert!(off_line(p, spawn, gate.at) > 1.0, "{p} is on the way");
            }
            for (center, radius) in [blocks.stack_pool(), blocks.domino_pool()] {
                assert!(center.with_y(0.0).distance(gate.at) - radius > 1.0);
            }
            let pillar = crate::pillar::AT.as_vec3().with_y(0.0);
            assert!(pillar.distance(gate.at) > 6.0);
            // The last 8 m in front of the arch are clear of it.
            let approach = gate.at + (gate.front().0 - gate.at).normalize() * 8.0;
            assert!(off_line(pillar, approach, gate.at) > 4.0);
            for wall in crate::world::GymSite::GRID.walls() {
                let nearest = Vec3::new(
                    gate.at.x.clamp(wall.min[0], wall.max[0]),
                    0.0,
                    gate.at.z.clamp(wall.min[1], wall.max[1]),
                );
                assert!(nearest.distance(gate.at) > 14.0, "{wall:?}");
            }
            for other in [
                Gate::grid(&layout),
                Gate::everglade(&layout),
                Gate::ritual(&layout),
                Gate::lagrange2(&layout),
            ] {
                assert!(other.at.distance(gate.at) > 10.0, "{}", other.at);
                assert!(gate.front().0.distance(other.at) > 8.0);
            }
            assert!(gate.at.x.abs() < crate::world::HALF - 5.0);
            assert!(gate.at.z.abs() < crate::world::HALF - 5.0);
        }
        // Side by side, and each one's walk in from the spawn passes clear
        // of the other's pillars.
        let apart = haunt.at.distance(ice.at);
        assert!((6.0..8.0).contains(&apart), "{apart}");
        let pillars = |g: Gate| {
            let side = g.rotation() * Vec3::X;
            [
                g.at + side * (OPENING_HALF + 0.4),
                g.at - side * (OPENING_HALF + 0.4),
            ]
        };
        for (gate, other) in [(haunt, ice), (ice, haunt)] {
            for pillar in pillars(other) {
                let d = off_line(pillar, spawn, gate.front().0);
                assert!(d > 1.5, "{} m from {pillar}", d);
                assert!(off_line(pillar, gate.front().0, gate.at) > 1.5);
            }
        }
    }

    #[test]
    fn a_page_can_ask_for_several_hosted_arches_as_data() {
        let names = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let (arches, refused) = HostedPortal::list(&names(&["icecream", "haunt"]), &[]);
        assert!(refused.is_empty());
        assert_eq!(arches[0].sign, "ICE CREAM MAN");
        assert_eq!(arches[0].site, GRID_HOSTED_AT);
        assert_eq!(arches[1].sign, "HAUNTED HOUSE");
        assert_eq!(arches[1].site, GRID_HAUNT_AT);
        // One arch, as before.
        let (one, _) = HostedPortal::list(&names(&["icecream"]), &names(&["Ice Cream Man"]));
        assert_eq!(
            one,
            vec![HostedPortal::new("icecream", "ICE CREAM MAN").unwrap()]
        );
        // A new tenant stands outboard of the known ones; duplicates and
        // bad names drop out.
        let (more, refused) =
            HostedPortal::list(&names(&["haunt", "my-game", "haunt", "Bad Name"]), &[]);
        assert_eq!(more.len(), 2);
        assert_eq!(more[1].sign, "MY GAME");
        assert_eq!(
            more[1].site,
            [GRID_HAUNT_AT[0] + HOSTED_SPACING, GRID_HOSTED_AT[1]]
        );
        assert_eq!(refused.len(), 1);
    }

    #[test]
    fn the_arch_is_neutral() {
        let mesh = Gate::grid(&layout()).mesh(ZoneId::Plaza, "LAGRANGE 1", 0.0);
        assert!(!mesh.lines.is_empty());
        assert!(
            mesh.lines
                .iter()
                .chain(&mesh.faces)
                .all(|v| v.color[0] == v.color[1] && v.color[1] == v.color[2])
        );
    }
}
