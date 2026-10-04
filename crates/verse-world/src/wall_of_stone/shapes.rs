//! Panel layouts an authoring command can ask for. Each returns placements
//! only; [`super::validate`] decides whether the wall may stand.

use glam::{DMat3, DQuat, DVec3};

use super::{Form, Placement};

/// Rotation whose x axis is `along` and z axis is `normal`; both unit and
/// perpendicular.
fn frame(along: DVec3, normal: DVec3) -> DQuat {
    let across = normal.cross(along);
    DQuat::from_mat3(&DMat3::from_cols(along, across, normal)).normalize()
}

fn horizontal(v: DVec3) -> DVec3 {
    DVec3::new(v.x, 0.0, v.z).normalize_or_zero()
}

/// An upright wall of `count` panels standing on `start`, running along the
/// horizontal `direction`.
#[must_use]
pub fn straight(start: DVec3, direction: DVec3, count: usize, form: Form) -> Vec<Placement> {
    let along = horizontal(direction);
    let size = form.size();
    let orientation = upright(along);
    (0..count)
        .map(|i| Placement {
            form,
            center: start + along * (size.x * (i as f64 + 0.5)) + DVec3::Y * (size.y * 0.5),
            orientation,
        })
        .collect()
}

/// Rotation for an upright panel running along the horizontal unit `along`:
/// x along, y up, z the horizontal face normal.
fn upright(along: DVec3) -> DQuat {
    DQuat::from_mat3(&DMat3::from_cols(along, DVec3::Y, along.cross(DVec3::Y))).normalize()
}

/// A deck from `from` to `to` whose underside lies on the line between them,
/// `lanes` panels wide. The panel count along is the fewest that reach, and
/// the deck is centered on the line, so both ends overhang equally.
#[must_use]
pub fn bridge(from: DVec3, to: DVec3, lanes: usize, form: Form) -> Vec<Placement> {
    let along = (to - from).normalize();
    let normal = (DVec3::Y - along * along.y).normalize();
    let orientation = frame(along, normal);
    let across = normal.cross(along);
    let size = form.size();
    let length = from.distance(to);
    let count = ((length / size.x) - 1e-9).ceil().max(1.0) as usize;
    let middle = (from + to) * 0.5 + normal * (size.z * 0.5);
    let mut panels = Vec::new();
    for lane in 0..lanes {
        let side = (lane as f64 + 0.5 - lanes as f64 * 0.5) * size.y;
        for i in 0..count {
            let s = (i as f64 + 0.5 - count as f64 * 0.5) * size.x;
            panels.push(Placement {
                form,
                center: middle + along * s + across * side,
                orientation,
            });
        }
    }
    panels
}

/// A ramp deck from `bottom` (on the floor) to `top` (a ledge's edge),
/// `lanes` panels wide. Half-size panels also get upright supports standing
/// on the floor under the deck, one stack per panel height of rise, each
/// stack just clear of the sloping underside.
#[must_use]
pub fn ramp(bottom: DVec3, top: DVec3, lanes: usize, form: Form) -> Vec<Placement> {
    let mut panels = bridge(bottom, top, lanes, form);
    let length = bottom.distance(top);
    let count = ((length / form.size().x) - 1e-9).ceil().max(1.0);
    // `bridge` centers the deck on the line; shift it so it starts at
    // `bottom` and ends at or past `top`.
    let along = (top - bottom).normalize();
    let shift = along * ((count * form.size().x - length) * 0.5);
    for panel in &mut panels {
        panel.center += shift;
    }
    if form != Form::Half {
        return panels;
    }
    let run = horizontal(top - bottom);
    let rise = top.y - bottom.y;
    let reach = DVec3::new(top.x - bottom.x, 0.0, top.z - bottom.z).length();
    if rise <= 0.0 || reach <= 0.0 {
        return panels;
    }
    let slope = rise / reach;
    let size = form.size();
    let normal = (DVec3::Y - along * along.y).normalize();
    let across = normal.cross(along);
    let support = upright(across);
    let mut level = 1;
    while (level as f64) * size.y < rise - size.y * 0.5 {
        let height = level as f64 * size.y;
        // The stack's downhill top corner meets the underside.
        let u = height / slope + size.z * 0.5;
        for lane in 0..lanes {
            let side = (lane as f64 + 0.5 - lanes as f64 * 0.5) * size.y;
            for j in 0..level {
                panels.push(Placement {
                    form,
                    center: bottom
                        + run * u
                        + across * side
                        + DVec3::Y * (size.y * (j as f64 + 0.5)),
                    orientation: support,
                });
            }
        }
        level += 1;
    }
    panels
}

/// Four upright panels around `base` (the floor point at the middle), the
/// ends of each against the face of the next, `levels` high, with a roof
/// panel on top. One level is an enclosure.
#[must_use]
pub fn tower(base: DVec3, levels: usize, form: Form) -> Vec<Placement> {
    let size = form.size();
    let outer = size.x + size.z;
    let mut panels = Vec::new();
    for level in 0..levels {
        for side in 0..4 {
            let turn = DQuat::from_rotation_y(-std::f64::consts::FRAC_PI_2 * side as f64);
            let offset = turn * DVec3::new(-size.z * 0.5, 0.0, -(outer - size.z) * 0.5);
            panels.push(Placement {
                form,
                center: base + offset + DVec3::Y * (size.y * (level as f64 + 0.5)),
                orientation: turn,
            });
        }
    }
    panels.push(Placement {
        form,
        center: base + DVec3::Y * (size.y * levels as f64 + size.z * 0.5),
        orientation: frame(DVec3::X, DVec3::Y),
    });
    panels
}

/// A one-level [`tower`]: walls and a roof around a creature.
#[must_use]
pub fn enclosure(base: DVec3, form: Form) -> Vec<Placement> {
    tower(base, 1, form)
}
