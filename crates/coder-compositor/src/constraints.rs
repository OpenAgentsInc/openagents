//! `zwp_pointer_constraints_v1` and `zwp_relative_pointer_manager_v1`: the
//! pointer a client holds while you look around.
//!
//! A client that hides the cursor and steers by motion asks the compositor to
//! keep the pointer where it is and to report movement as a delta. World of
//! Warcraft does it while you hold the right button, and Xwayland asks on the
//! X11 client's behalf.
//!
//! Without the two globals the request reaches nothing. The pointer keeps
//! walking across the shared space, leaves the window, and enters whatever
//! pane sits beside it, while the game reads motion that no longer matches
//! the mouse and spins the view, which a game on the owner's desk did in
//! September 2026.
//!
//! Code owns the policy:
//!
//! - A constraint activates only while its surface is the one under the
//!   pointer, so a background window cannot take the mouse.
//! - A locked pointer holds its position. The client reads the delta, and the
//!   compositor sends no absolute motion, so the pointer cannot leave the
//!   pane and the focus cannot follow it out.
//! - A confined pointer moves inside its region, clamped to the region's
//!   bounding box in the shared space.

use smithay::input::pointer::PointerHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::compositor::{RectangleKind, RegionAttributes};
use smithay::wayland::pointer_constraints::{
    PointerConstraint, PointerConstraintsHandler, with_pointer_constraint,
};
use smithay::{delegate_pointer_constraints, delegate_relative_pointer};

use crate::state::Coder;

/// What the surface under the pointer asks of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Held {
    /// Nothing. The pointer moves as it always does.
    Free,
    /// The pointer holds its position and the client reads the delta.
    Locked,
    /// The pointer stays inside this rectangle of the shared space.
    Confined(Rectangle<f64, Logical>),
}

/// What the pointer's own surface asks of it now.
///
/// The surface is the one under the pointer, so a constraint a background
/// window holds answers [`Held::Free`].
pub fn held(state: &Coder) -> Held {
    let Some((surface, location)) = state.surface_under() else {
        return Held::Free;
    };
    let pointer = state.pointer.clone();
    with_pointer_constraint(&surface, &pointer, |constraint| {
        let Some(constraint) = constraint else {
            return Held::Free;
        };
        if !constraint.is_active() {
            return Held::Free;
        }
        match &*constraint {
            PointerConstraint::Locked(_) => Held::Locked,
            PointerConstraint::Confined(confined) => match bounds(confined.region(), location) {
                // A confinement with no region confines to the surface, and
                // the surface's own extent is not known here. Holding the
                // pointer still keeps it inside, which is what the client
                // asked for.
                None => Held::Locked,
                Some(rectangle) => Held::Confined(rectangle),
            },
        }
    })
}

/// The bounding box of a region, in the shared space, given where its surface
/// sits. A region with no additive rectangle has no bounds.
fn bounds(
    region: Option<&RegionAttributes>,
    location: Point<f64, Logical>,
) -> Option<Rectangle<f64, Logical>> {
    let region = region?;
    let mut left = f64::MAX;
    let mut top = f64::MAX;
    let mut right = f64::MIN;
    let mut bottom = f64::MIN;
    for (kind, rectangle) in &region.rects {
        if !matches!(kind, RectangleKind::Add) {
            continue;
        }
        let x = f64::from(rectangle.loc.x);
        let y = f64::from(rectangle.loc.y);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + f64::from(rectangle.size.w));
        bottom = bottom.max(y + f64::from(rectangle.size.h));
    }
    if left > right || top > bottom {
        return None;
    }
    Some(Rectangle::new(
        (left + location.x, top + location.y).into(),
        (right - left, bottom - top).into(),
    ))
}

/// Hold a point inside a rectangle, on both axes.
#[must_use]
pub fn clamp(at: Point<f64, Logical>, to: Rectangle<f64, Logical>) -> Point<f64, Logical> {
    // A rectangle of no width or height would put the pointer on its edge,
    // which is where the clamp already lands.
    let right = to.loc.x + to.size.w;
    let bottom = to.loc.y + to.size.h;
    (
        at.x.clamp(to.loc.x, right.max(to.loc.x)),
        at.y.clamp(to.loc.y, bottom.max(to.loc.y)),
    )
        .into()
}

impl PointerConstraintsHandler for Coder {
    /// A client asked for the pointer. It is granted only while the pointer is
    /// already over that surface, which is the moment a press in the window
    /// starts a mouse look.
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        let over = self
            .surface_under()
            .map(|(under, _)| under == *surface)
            .unwrap_or(false);
        if !over {
            return;
        }
        with_pointer_constraint(surface, pointer, |constraint| {
            if let Some(constraint) = constraint {
                constraint.activate();
            }
        });
    }

    /// Where the client would like the cursor left when the lock ends. The
    /// compositor draws no cursor for a locked pointer, and the pointer has
    /// not moved, so there is nothing to place.
    fn cursor_position_hint(
        &mut self,
        _surface: &WlSurface,
        _pointer: &PointerHandle<Self>,
        _location: Point<f64, Logical>,
    ) {
    }
}

delegate_pointer_constraints!(Coder);
delegate_relative_pointer!(Coder);

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn a_point_inside_the_rectangle_does_not_move() {
        let at = Point::from((120.0, 90.0));
        assert_eq!(clamp(at, rect(100.0, 80.0, 200.0, 150.0)), at);
    }

    #[test]
    fn a_point_past_an_edge_lands_on_it() {
        let held = clamp(Point::from((500.0, 400.0)), rect(100.0, 80.0, 200.0, 150.0));
        assert_eq!(held, Point::from((300.0, 230.0)));
    }

    #[test]
    fn a_point_before_an_edge_lands_on_it() {
        let held = clamp(Point::from((10.0, 10.0)), rect(100.0, 80.0, 200.0, 150.0));
        assert_eq!(held, Point::from((100.0, 80.0)));
    }

    #[test]
    fn a_region_of_one_rectangle_bounds_where_its_surface_sits() {
        let region = RegionAttributes {
            rects: vec![(
                RectangleKind::Add,
                Rectangle::new((0, 0).into(), (640, 480).into()),
            )],
        };
        let bounds = bounds(Some(&region), Point::from((100.0, 50.0)));
        assert_eq!(bounds, Some(rect(100.0, 50.0, 640.0, 480.0)));
    }

    #[test]
    fn a_region_of_several_rectangles_bounds_all_of_them() {
        let region = RegionAttributes {
            rects: vec![
                (
                    RectangleKind::Add,
                    Rectangle::new((0, 0).into(), (100, 100).into()),
                ),
                (
                    RectangleKind::Add,
                    Rectangle::new((200, 50).into(), (100, 100).into()),
                ),
            ],
        };
        let bounds = bounds(Some(&region), Point::from((0.0, 0.0)));
        assert_eq!(bounds, Some(rect(0.0, 0.0, 300.0, 150.0)));
    }

    #[test]
    fn a_region_that_only_subtracts_has_no_bounds() {
        let region = RegionAttributes {
            rects: vec![(
                RectangleKind::Subtract,
                Rectangle::new((0, 0).into(), (100, 100).into()),
            )],
        };
        assert_eq!(bounds(Some(&region), Point::from((0.0, 0.0))), None);
    }

    #[test]
    fn no_region_has_no_bounds() {
        assert_eq!(bounds(None, Point::from((0.0, 0.0))), None);
    }
}
