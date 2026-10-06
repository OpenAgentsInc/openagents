//! The owner's house: the first Greco-futurism building
//! (`docs/verse/greco-futurism.md`), a two-storey estate on a podium at the
//! east end of Library Way, on open ground near the clearing's edge with
//! the east woods behind it and Observatory Hill to the south.
//!
//! `scripts/blender/greco_futurism.py` builds it as one model,
//! `generated/greco_house`, with a far level of detail. It stands on open
//! ground like the boathouse ([`super::city::GROUNDS`]): its walls,
//! columns, planter walls, and furniture block walking by the boxes of its
//! `greco_house.footprint.json`, its flat roof is a surface to land on, and
//! its bronze doors stand open, so a walk leads from Library Way up the
//! stair, between the round columns, and into the great room. Like every
//! generated model, it collides by its own triangles and breaks
//! (`demolition::carve`). It doesn't smoke: its chimneys are plain blocks
//! on a quiet house.

use super::generated::{GableRoof, Instance, Model};
use std::f32::consts::FRAC_PI_2;

/// The owner's house, in its glTF frame: +z out of the front, the origin
/// on the ground at the center of the lowest step's front edge.
pub const GRECO_HOUSE: Model = Model {
    name: "generated/greco_house",
    blocks: &[
        // The hedged planter walls beside the lower and upper flights.
        [-10.4, -4.5, -1.68, 0.0, 1.2],
        [4.5, 10.4, -1.68, 0.0, 1.2],
        [-7.2, -4.4, -7.88, -5.6, 2.7],
        [4.4, 7.2, -7.88, -5.6, 2.7],
        // The portico's square piers and round columns.
        [-9.95, -8.85, -9.15, -8.05, 9.8],
        [-7.15, -6.05, -9.15, -8.05, 9.8],
        [6.05, 7.15, -9.15, -8.05, 9.8],
        [8.85, 9.95, -9.15, -8.05, 9.8],
        [-2.67, -1.53, -9.17, -8.03, 9.8],
        [1.53, 2.67, -9.17, -8.03, 9.8],
        // The side walls, the facade on each side of the door, and the
        // back wall.
        [-10.05, -9.55, -25.6, -11.6, 9.8],
        [9.55, 10.05, -25.6, -11.6, 9.8],
        [-10.0, -1.35, -12.0, -11.6, 9.8],
        [1.35, 10.0, -12.0, -11.6, 9.8],
        [-10.0, 10.0, -25.6, -25.2, 9.8],
        // The great room's desk and sofa.
        [-1.4, 1.4, -22.1, -21.1, 2.38],
        [-2.2, 2.2, -14.9, -13.9, 2.44],
    ],
    roofs: &[GableRoof {
        center: [0.0, -17.1],
        slopes_z: true,
        half: [8.5, 9.5],
        eave: 12.4,
        ridge: 12.41,
    }],
    front: [0.0, 1.0],
    inside: Some([0.0, -12.9]),
};

/// Where the house stands: its stair's foot at Library Way's east end,
/// facing west down the street.
pub const OWNERS_HOUSE: Instance =
    Instance::new("owner's house", &GRECO_HOUSE, [109.0, -29.0], -FRAC_PI_2);

/// The walk from Library Way's end to the foot of the stair, as a road:
/// from, to, and half width, m.
pub const WALK: ([f32; 2], [f32; 2], f32) = ([104.0, -29.0], [108.0, -29.0], 1.4);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_house_stands_on_flat_ground_off_every_road_with_its_door_open() {
        let front = OWNERS_HOUSE.front();
        // The walk ends at the front step, and the house faces down it.
        assert!((front[0] - WALK.1[0]).abs() < 0.01 && (front[1] - WALK.1[1]).abs() < 0.01);
        let out = OWNERS_HOUSE.outward();
        assert!(out[0] < -0.99, "{out:?}");
        // Its front is inside the flat clearing, and its back stands where
        // the ground has risen less than the podium.
        let at = OWNERS_HOUSE.at;
        assert!(at[0].hypot(at[1]) < verse_world::social::everglade::CLEARING_RADIUS);
        for corner in [[-10.9, -26.1], [10.9, -26.1]] {
            let [x, z] = OWNERS_HOUSE.world(corner);
            assert!(super::super::height(x, z) < 1.0, "{x}, {z}");
        }
        // The doorway leads well inside, past the portico and the facade.
        let inside = OWNERS_HOUSE.world(GRECO_HOUSE.inside.unwrap());
        assert!(inside[0] > front[0] + 12.0);
    }

    #[test]
    fn nothing_else_stands_on_the_lot() {
        // Every other placement keeps off the house's footprint, so none
        // pokes through its floors or closes its doorway.
        let ([cx, cz], [hx, hz]) = (
            [OWNERS_HOUSE.at[0] + 13.05, OWNERS_HOUSE.at[1]],
            [13.05, 10.9],
        );
        for p in super::super::placements() {
            let [x, z] = p.at;
            let on = (x - cx).abs() < hx && (z - cz).abs() < hz;
            assert!(
                !on || p.model == GRECO_HOUSE.name,
                "{} at {x}, {z}",
                p.model
            );
        }
    }
}
