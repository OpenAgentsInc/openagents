//! Bounds and references for spell state restored from an untrusted checkpoint.
use super::{SpellWorld, Target};
impl SpellWorld {
    pub(super) fn validate_effects(&self) -> Result<(), String> {
        let bodies = self.world.bodies().len();
        let body = |id: physics::BodyId| (id.0 as usize) < bodies;
        let target = |t: Target| match t {
            Target::Actor(id) => id > 0 && id <= u64::from(u32::MAX),
            Target::Prop(i) => i < self.props.len(),
        };
        let bounded = [
            self.levitations.len(),
            self.telekinesis.len(),
            self.gusts.len(),
            self.walls.len(),
            self.wind_walls.len(),
            self.meteors.len(),
            self.tentacles.len(),
            self.reversed.len(),
        ];
        let bad = bounded.into_iter().any(|n| n > 64)
            || self.proxies.len() > 256
            || !self.time.is_finite()
            || self.time < 0.
            || bodies > 8192
            || self.escape_ready.len() > 256
            || self
                .escape_ready
                .values()
                .any(|at| !at.is_finite() || *at < 0.)
            || self.flames.len() > 256
            || self.flames.iter().any(|f| !f.position.is_finite())
            || self.creatures.len() > 256
            || self.damage.len() > 256
            || self.levitations.iter().any(|e| {
                e.cast > self.casts
                    || !e.caster_position.is_finite()
                    || !target(e.target)
                    || !e.state.base.is_finite()
                    || !e.state.rise.is_finite()
                    || !(0. ..=crate::levitate::MAX_RISE).contains(&e.state.rise)
                    || !e.state.cast_at.is_finite()
                    || e.state.commanded_at.is_some_and(|t| !t.is_finite())
            })
            || self.telekinesis.iter().any(|e| {
                e.cast > self.casts
                    || !target(e.target)
                    || !e.aim.is_finite()
                    || !e.caster_position.is_finite()
                    || !body(e.grip.hand)
                    || e.proxy.is_some_and(|id| !body(id))
                    || e.grip.grip.is_some_and(|g| {
                        !body(g.body)
                            || self.world.joint(g.linear).is_none()
                            || self.world.joint(g.angular).is_none()
                            || !g.path.is_finite()
                            || !(0. ..=crate::telekinesis::MOVE_BUDGET + 1e-9).contains(&g.path)
                    })
            })
            || self.gusts.iter().any(|e| {
                e.cast > self.casts
                    || !e.gust.line.origin.is_finite()
                    || !e.gust.line.direction.is_finite()
                    || !e.gust.cast_at.is_finite()
            })
            || self.walls.iter().any(|e| {
                e.cast > self.casts
                    || e.wall.panels.len() > 40
                    || e.wall.panels.iter().any(|p| !body(p.body))
                    || e.wall
                        .debris
                        .iter()
                        .any(|p| !body(p.body) || !p.until.is_finite())
            })
            || self.wind_walls.iter().any(|e| {
                e.cast > self.casts
                    || !e.wall.until.is_finite()
                    || e.wall.wall.path.len() > 64
                    || e.wall.wall.path.iter().any(|p| !p.is_finite())
                    || e.wall.bodies.iter().any(|id| !body(*id))
            })
            || self.meteors.iter().any(|e| {
                e.cast > self.casts
                    || e.swarm.meteors.len() != 4
                    || e.swarm.meteors.iter().any(|m| {
                        !m.point.is_finite()
                            || !m.velocity.is_finite()
                            || !m.start.is_finite()
                            || m.body.is_some_and(|id| !body(id))
                    })
                    || e.objects.iter().any(|o| !body(o.body))
            })
            || self
                .proxies
                .iter()
                .any(|p| !body(p.body) || p.cast > self.casts)
            || self.tentacles.iter().any(|e| {
                e.cast > self.casts
                    || !e.spell.center.is_finite()
                    || e.spell.tentacles.len()
                        != crate::black_tentacles::GRID * crate::black_tentacles::GRID
                    || e.spell.creatures.len() > 256
                    || e.spell.creatures.iter().any(|c| !body(c.body) || !c.hold.is_finite())
                    || e.spell.regrab.iter().any(|(id, _)| !body(*id))
                    || e.spell.tentacles.iter().any(|t| {
                        !body(t.anchor)
                            || t.segments.len() != crate::black_tentacles::SEGMENTS
                            || t.segments.iter().any(|id| !body(*id))
                            || t.mode.body().is_some_and(|id| !body(id))
                            || (!e.spell.ended && t.joints.iter().any(|id| self.world.joint(*id).is_none()))
                            || matches!(t.mode, crate::black_tentacles::Mode::Hold { joint, .. } if self.world.joint(joint).is_none())
                    })
            })
            || self.reversed.iter().any(|e| {
                e.cast > self.casts
                    || !e.spell.gravity.cylinder.base.is_finite()
                    || e.spell.gravity.cylinder.radius != crate::reverse_gravity::RADIUS
                    || e.spell.gravity.cylinder.height != crate::reverse_gravity::HEIGHT
                    || e.spell.falls.iter().any(|(id, _)| !body(*id))
                    || e.spell.holds.iter().any(|h| !body(h.body) || h.joint.is_some_and(|id| self.world.joint(id).is_none()))
            });
        if bad {
            Err("Invalid active spell checkpoint".into())
        } else {
            Ok(())
        }
    }
}
