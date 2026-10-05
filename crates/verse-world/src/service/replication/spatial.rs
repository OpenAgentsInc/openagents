use super::Scope;
use crate::service::wire::{Control, State};
use glam::{DVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
const CELL: f32 = 32.;
fn cell(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}
fn near(p: Vec3, center: Vec3, radius: f32) -> bool {
    p.distance_squared(center) <= radius * radius
}
/// Reusable instance-local cell lists, rebuilt once when shared extraction changes.
pub(crate) struct Index {
    cells: BTreeMap<(i32, i32), Vec<(u32, Vec3)>>,
}
impl Index {
    pub(crate) fn new(state: &State) -> Self {
        let mut cells: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for actor in &state.snapshot.actors {
            cells
                .entry(cell(actor.pos.into()))
                .or_default()
                .push((actor.id, actor.pos.into()));
        }
        Self { cells }
    }
    fn sources(&self, center: Vec3) -> BTreeSet<u32> {
        let (cx, cz) = cell(center);
        let mut sources = BTreeSet::new();
        for x in cx - 2..=cx + 2 {
            for z in cz - 2..=cz + 2 {
                if let Some(actors) = self.cells.get(&(x, z)) {
                    sources.extend(
                        actors
                            .iter()
                            .filter(|(_, pos)| near(*pos, center, 64.))
                            .map(|(id, _)| *id),
                    );
                }
            }
        }
        sources
    }
}
/// Sphere bounds retain rotated and large geometry conservatively.
fn shape_near(shape: &physics::queries::ShapeSnapshot, center: DVec3) -> bool {
    use physics::queries::GeometrySnapshot;
    let (local, radius) = match &shape.geometry {
        GeometrySnapshot::Box { min, max } => ((*min + *max) * 0.5, (*max - *min).length() * 0.5),
        GeometrySnapshot::Capsule { a, b, radius } => {
            ((*a + *b) * 0.5, (*b - *a).length() * 0.5 + radius)
        }
        GeometrySnapshot::Triangles { triangles } => {
            let mut min = DVec3::splat(f64::INFINITY);
            let mut max = DVec3::splat(f64::NEG_INFINITY);
            for triangle in triangles {
                for point in triangle.0 {
                    min = min.min(point);
                    max = max.max(point);
                }
            }
            ((min + max) * 0.5, (max - min).length() * 0.5)
        }
    };
    shape.pose.point(local).distance(center) <= 80. + radius
}
#[cfg(test)]
pub(crate) fn scope(
    state: State,
    control: &Option<Control>,
    previous: Option<&State>,
    tick: u64,
) -> Result<State, String> {
    let index = Index::new(&state);
    scoped(state, control, previous, tick % 6 == 0, &index)
}
fn center(state: &State, control: &Option<Control>) -> Vec3 {
    control
        .as_ref()
        .and_then(|c| state.presentation.actors.iter().find(|a| a.life == c.life))
        .or_else(|| {
            state
                .presentation
                .actors
                .iter()
                .find(|a| a.actor.model == "adventurer")
        })
        .map(|a| a.actor.position)
        .unwrap_or(Vec3::ZERO)
}
/// Previous transforms are needed only for actors outside the near update band.
pub(super) fn needs_outer_history(state: &State, control: &Option<Control>) -> bool {
    let center = center(state, control);
    state.presentation.actors.iter().any(|pose| {
        !control.as_ref().is_some_and(|c| c.life == pose.life)
            && !near(pose.actor.position, center, 32.)
    })
}
pub(crate) fn scoped(
    mut state: State,
    control: &Option<Control>,
    previous: Option<&State>,
    refresh_outer: bool,
    index: &Index,
) -> Result<State, String> {
    let center = center(&state, control);
    let mut sources = index.sources(center);
    if let Some(control) = control {
        sources.extend(
            state
                .actors
                .iter()
                .filter(|a| a.life == control.life)
                .map(|a| a.source),
        );
    }
    if let Some(target) = state
        .hud
        .as_ref()
        .and_then(|h| h.casting.as_ref())
        .map(|c| c.target_life)
    {
        sources.extend(
            state
                .actors
                .iter()
                .filter(|a| verse_engine::core::LifeId::from(a.life) == target)
                .map(|a| a.source),
        );
    }
    state
        .snapshot
        .projectiles
        .retain(|p| near(p.pos.into(), center, 64.));
    sources.extend(state.snapshot.projectiles.iter().map(|p| p.caster));
    // Telegraph endpoints form an inseparable presentation record.
    for cast in &state.presentation.hostile_casts {
        if near(cast.origin.into(), center, 64.)
            || near(cast.target.into(), center, 64.)
            || cast.position.is_some_and(|p| near(p.into(), center, 64.))
        {
            sources.extend(
                state
                    .actors
                    .iter()
                    .filter(|a| a.life == cast.caster || a.life == cast.target_life)
                    .map(|a| a.source),
            );
        }
    }
    state.snapshot.actors.retain(|a| sources.contains(&a.id));
    state.actors.retain(|a| sources.contains(&a.source));
    let lives: BTreeSet<_> = state
        .actors
        .iter()
        .map(|a| verse_engine::core::LifeId::from(a.life))
        .collect();
    state
        .presentation
        .actors
        .retain(|a| lives.contains(&a.life.into()));
    state
        .presentation
        .effects
        .retain(|a| lives.contains(&a.life.into()));
    state
        .presentation
        .hostile_casts
        .retain(|a| lives.contains(&a.caster.into()) && lives.contains(&a.target_life.into()));
    state
        .presentation
        .corpses
        .retain(|a| near(a.actor.position, center, 64.));
    state
        .presentation
        .impacts
        .retain(|a| near(a.position.into(), center, 64.));
    state
        .presentation
        .flames
        .retain(|a| near(a.position.as_vec3(), center, 64.));
    state
        .snapshot
        .effects
        .retain(|a| near(a.pos.into(), center, 64.));
    state
        .presentation
        .props
        .retain(|p| near(p.center, center, 64. + p.dimensions.length() * 0.5));
    state.presentation.blockers.retain(|p| {
        ((p.min + p.max) * 0.5).distance(center.as_dvec3()) <= 80. + (p.max - p.min).length() * 0.5
    });
    if let Some(collision) = &mut state.collision {
        collision
            .colliders
            .retain(|shape| shape_near(shape, center.as_dvec3()));
    }
    if control.is_none() {
        state.snapshot.player = crate::rules::Player {
            hp: 0,
            max_hp: 0,
            mana: 0,
            max_mana: 0,
        };
        state.snapshot.abilities.clear();
    }
    // Outer-band transforms refresh at 5 Hz. Health, outfit, life, and teleport changes bypass it.
    if !refresh_outer {
        if let Some(previous) = previous {
            for pose in &mut state.presentation.actors {
                if control.as_ref().is_some_and(|c| c.life == pose.life)
                    || near(pose.actor.position, center, 32.)
                {
                    continue;
                }
                if let Some(old) = previous.presentation.actors.iter().find(|p| {
                    p.life == pose.life
                        && p.health == pose.health
                        && p.teleport_stamp == pose.teleport_stamp
                        && p.outfit_model == pose.outfit_model
                        && serde_json::to_value(&p.equipment).ok()
                            == serde_json::to_value(&pose.equipment).ok()
                }) {
                    pose.actor.position = old.actor.position;
                    pose.actor.yaw = old.actor.yaw;
                    pose.animation = old.animation;
                    pose.animation_time = old.animation_time;
                    if let Some(binding) = state.actors.iter().find(|a| a.life == pose.life) {
                        if let Some(actor) = state
                            .snapshot
                            .actors
                            .iter_mut()
                            .find(|a| a.id == binding.source)
                        {
                            actor.pos = old.actor.position.to_array();
                            actor.yaw = old.actor.yaw;
                        }
                    }
                    if let Some(effect) = state
                        .presentation
                        .effects
                        .iter_mut()
                        .find(|e| e.life == pose.life)
                    {
                        effect.position = old.actor.position.to_array();
                    }
                }
            }
        }
    }
    state.scope = Some(Scope {
        center: center.to_array(),
        radius: 64.,
        collision_radius: 80.,
    });
    Ok(state)
}
