//! Nearby descriptions for the Grid's gateways.

/// How near, in meters, a visitor stands before the description shows.
pub const LINE_RANGE: f32 = 9.0;

/// A place to go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// What the page calls the place.
    pub id: &'static str,
    /// The line shown within [`LINE_RANGE`].
    pub line: &'static str,
}

pub const GAME_LAB: Mark = Mark {
    id: "game-lab",
    line: "Game Lab: change a game by chatting",
};
pub const EVERGLADE: Mark = Mark {
    id: "everglade",
    line: "Everglade: walk a town by the water",
};
pub const LAGRANGE2: Mark = Mark {
    id: "lagrange-2",
    line: "Lagrange 2: build spaceships in orbit",
};
pub const ICE_CREAM_MAN: Mark = Mark {
    id: "icecream",
    line: "The Ice Cream Man: a horror game to play",
};
pub const HAUNTED_HOUSE: Mark = Mark {
    id: "haunt",
    line: "The Haunted House: a horror game to play",
};
pub const GYM: Mark = Mark {
    id: "gym",
    line: "The Gym: see how agents score",
};

/// Every mark.
#[cfg(test)]
pub const ALL: [Mark; 6] = [
    GAME_LAB,
    EVERGLADE,
    LAGRANGE2,
    ICE_CREAM_MAN,
    HAUNTED_HOUSE,
    GYM,
];

/// The mark of a hosted arch's tenant, when the Grid knows it.
#[must_use]
pub fn hosted(tenant: &str) -> Option<Mark> {
    [ICE_CREAM_MAN, HAUNTED_HOUSE]
        .into_iter()
        .find(|m| m.id == tenant)
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    use super::{LINE_RANGE, Mark};
    use glam::{Mat4, Quat, Vec3};
    use verse::runtime::WorldRuntime;
    use verse::ui::{Atlas, UiBatch};

    const LINE_PAD: [f32; 2] = [10.0, 6.0];
    const LINE_AT: f32 = 4.1;

    /// Where each mark stands on the Grid now: its arch's foot and turn.
    fn placed(runtime: &WorldRuntime) -> Vec<(Mark, Vec3, f32)> {
        let mut out = vec![(super::GAME_LAB, verse::zones::game_place::POSITION, 0.0)];
        if let Some(gate) = runtime.everglade_gate() {
            out.push((super::EVERGLADE, gate.at, gate.yaw));
        }
        if let Some(gate) = runtime.lagrange2_gate() {
            out.push((super::LAGRANGE2, gate.at, gate.yaw));
        }
        for (portal, gate) in runtime.hosted_gates() {
            if let Some(mark) = super::hosted(&portal.tenant) {
                out.push((mark, gate.at, gate.yaw));
            }
        }
        let door = verse::world::GymSite::GRID.point(verse::world::GYM_ENTRANCE);
        out.push((super::GYM, Vec3::new(door.x, 0.0, door.z), 0.0));
        out
    }

    /// The description of the nearest gateway within [`LINE_RANGE`].
    pub fn draw(
        ui: &mut UiBatch,
        atlas: &Atlas,
        size: [f32; 2],
        view_proj: Mat4,
        runtime: &WorldRuntime,
    ) {
        let player = runtime.player.pos;
        let near = placed(runtime)
            .into_iter()
            .map(|(mark, at, yaw)| {
                let front = at + Quat::from_rotation_y(yaw) * Vec3::new(0.0, 0.0, -2.0);
                (
                    mark,
                    at,
                    front.distance(Vec3::new(player.x, at.y, player.z)),
                )
            })
            .filter(|(_, _, d)| *d <= LINE_RANGE)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((mark, at, _)) = near else {
            return;
        };
        let Some([x, y]) = verse::hud::project(view_proj, size, at + Vec3::Y * LINE_AT) else {
            return;
        };
        let w = atlas.measure(mark.line) + LINE_PAD[0] * 2.0;
        let h = atlas.line + LINE_PAD[1] * 2.0;
        let (left, top) = (x - w / 2.0, y);
        ui.rect(
            atlas,
            left,
            top,
            w,
            h,
            crate::theme::linear(coder_ui::coder_noir::SURFACE, 0.92),
        );
        ui.text(
            atlas,
            left + LINE_PAD[0],
            top + LINE_PAD[1],
            mark.line,
            crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_known_hosted_arches_have_marks() {
        assert_eq!(hosted("icecream"), Some(ICE_CREAM_MAN));
        assert_eq!(hosted("haunt"), Some(HAUNTED_HOUSE));
        assert_eq!(hosted("someone-else"), None);
    }

    #[test]
    fn the_lines_are_plain_and_short() {
        for mark in ALL {
            assert!(
                oa_copy::violations(mark.line, &[]).is_empty(),
                "{}",
                mark.line
            );
            assert!(mark.line.len() <= 44, "{}", mark.line);
            assert!(mark.line.is_ascii());
        }
    }
}
