//! What one screen draws, as the render elements both backends hand their
//! renderer.
//!
//! A frame is, from the front: the pointer and the icon a drag carries, the
//! tracked hand when hands drive the desk, the notice bar, the windows and
//! the layer surfaces the space holds, and each tile's border behind them. The nested backend draws the list into its
//! window, the hardware backend hands it to the DRM compositor for each
//! connector, and a screen copy draws the same list into the client's
//! buffer, so the three agree on what the screen shows.
//!
//! The layout places everything in logical pixels in the space every
//! screen shares. An element is placed in the physical pixels of the one
//! screen it draws on, so each rectangle is moved to that screen's corner
//! and multiplied by its scale here.

use std::sync::{Arc, Mutex};

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Id, Kind};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer};
use smithay::desktop::space::SpaceRenderElements;
use smithay::desktop::{PopupManager, Space, Window, WindowSurface, layer_map_for_output};
use smithay::input::pointer::{CursorImageAttributes, CursorImageStatus};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::render_elements;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Transform};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;

use crate::cursor::Cursor;
use crate::hands_overlay::Picture;
use crate::layout::{self, Placed};

// One element of a frame: a window or a layer surface as the space places
// it, a window's surface cut to its tile, a surface the compositor places
// itself (a client's cursor, the icon a drag carries, or a popup), a border
// bar or the notice bar, or the pointer the compositor draws.
render_elements! {
    pub Elements<R, E> where R: ImportAll + ImportMem;
    Space=SpaceRenderElements<R, E>,
    Tile=CropRenderElement<WaylandSurfaceRenderElement<R>>,
    Surface=WaylandSurfaceRenderElement<R>,
    Solid=SolidColorRenderElement,
    Memory=MemoryRenderBufferRenderElement<R>,
}

/// One element of a frame, with the element a window draws as.
pub type Element<R> = Elements<R, WaylandSurfaceRenderElement<R>>;

/// What a frame draws over the space: the borders, the notice, and the
/// pointer. The state builds it before a backend borrows its renderer.
#[derive(Clone, Debug)]
pub struct Overlay {
    /// Each border bar, in logical pixels in the shared space, with its
    /// color.
    pub bars: Vec<(Placed, [f32; 4])>,
    /// Whether a notice is showing.
    pub notice: bool,
    /// Where the pointer sits in the shared space, or nothing when the
    /// compositor draws no pointer.
    pub pointer: Option<Point<f64, Logical>>,
    /// The cursor a client asked for.
    pub cursor: CursorImageStatus,
    /// The icon a drag in progress carries.
    pub drag_icon: Option<WlSurface>,
    /// How long the compositor has run, which an animated cursor steps by.
    pub millis: u32,
    /// The tracked hand and its caption, in logical pixels in the shared
    /// space, empty when hands are off. It is a handle rather than a copy:
    /// a frame reads the rasters the gesture module last drew.
    pub hands: Arc<Vec<Picture>>,
    /// Each window a screen shows with the tile it is configured to, in
    /// logical pixels in the shared space. A window draws inside its tile
    /// only, so a client that commits a buffer larger than the size it was
    /// configured to, such as a terminal that keeps its rows and columns
    /// when its text grows, does not draw over its neighbours.
    pub clips: Vec<(Window, Placed)>,
}

/// One solid rectangle the compositor keeps drawing, with the identity the
/// damage tracker follows it by.
struct Paint {
    id: Id,
    color: [f32; 4],
    commit: CommitCounter,
}

impl Paint {
    fn new() -> Paint {
        Paint {
            id: Id::new(),
            color: [0.0; 4],
            commit: CommitCounter::default(),
        }
    }

    fn element(&mut self, geometry: Placed, color: [f32; 4]) -> SolidColorRenderElement {
        if color != self.color {
            self.color = color;
            self.commit.increment();
        }
        SolidColorRenderElement::new(
            self.id.clone(),
            Rectangle::new(
                (geometry.x, geometry.y).into(),
                (geometry.width, geometry.height).into(),
            ),
            self.commit,
            Color32F::from(color),
            Kind::Unspecified,
        )
    }
}

/// What the compositor draws of its own: the pointer and the solid bars.
pub struct Decor {
    /// The pointer's images.
    pub cursor: Cursor,
    bars: Vec<Paint>,
    notice: Paint,
}

impl Decor {
    /// The decor a compositor starts with, with the session's cursor.
    pub fn load() -> Decor {
        Decor {
            cursor: Cursor::load(),
            bars: Vec::new(),
            notice: Paint::new(),
        }
    }
}

/// One rectangle in the shared space, in the physical pixels of a screen
/// whose corner is `at` and whose scale is `scale`. The edges round
/// separately, so two rectangles that touch in logical pixels touch on the
/// screen, and nothing narrower than a pixel disappears.
pub fn physical(rect: Placed, at: (i32, i32), scale: f64) -> Placed {
    let edge = |value: i32, origin: i32| (f64::from(value - origin) * scale).round() as i32;
    let left = edge(rect.x, at.0);
    let top = edge(rect.y, at.1);
    let right = edge(rect.x + rect.width, at.0);
    let bottom = edge(rect.y + rect.height, at.1);
    Placed {
        x: left,
        y: top,
        width: (right - left).max(1),
        height: (bottom - top).max(1),
    }
}

/// Every element one screen draws, front to back.
pub fn output_elements<R>(
    renderer: &mut R,
    space: &Space<Window>,
    output: &Output,
    overlay: &Overlay,
    decor: &mut Decor,
) -> Result<Vec<Element<R>>, String>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let Some(geometry) = space.output_geometry(output) else {
        return Ok(Vec::new());
    };
    let at = (geometry.loc.x, geometry.loc.y);
    let scale = output.current_scale().fractional_scale();
    let mut elements: Vec<Element<R>> = Vec::new();

    if let Some(pointer) = overlay.pointer.filter(|at| geometry.to_f64().contains(*at)) {
        let local = pointer - geometry.loc.to_f64();
        cursor_elements(renderer, local, scale, overlay, decor, &mut elements);
    }

    for picture in overlay.hands.iter() {
        let rect = Placed {
            x: picture.x,
            y: picture.y,
            width: picture.width,
            height: picture.height,
        };
        if !touches(rect, geometry) {
            continue;
        }
        hand_element(renderer, picture, at, scale, &mut elements);
    }

    if overlay.notice {
        let bar = Placed {
            x: 0,
            y: 0,
            width: geometry.size.w,
            height: layout::NOTICE_BAR,
        };
        let placed = physical(bar, (0, 0), scale);
        elements.push(Elements::Solid(
            decor.notice.element(placed, layout::BORDER_ACTIVE),
        ));
    }

    space_elements(renderer, space, output, overlay, &mut elements);

    let bars: Vec<(Placed, [f32; 4])> = overlay
        .bars
        .iter()
        .filter(|(bar, _)| touches(*bar, geometry))
        .copied()
        .collect();
    while decor.bars.len() < bars.len() {
        decor.bars.push(Paint::new());
    }
    for ((bar, color), paint) in bars.into_iter().zip(decor.bars.iter_mut()) {
        let placed = physical(bar, at, scale);
        elements.push(Elements::Solid(paint.element(placed, color)));
    }
    Ok(elements)
}

/// The layer surfaces and the windows of one screen, front to back: the
/// overlay and top layers, the windows from the top of the stack down, then
/// the bottom and background layers. This is the order
/// `space_render_elements` gives, with each window's own surfaces cut to
/// its tile. A window's popups are not cut, so a menu can open past the
/// edge of its tile.
fn space_elements<R>(
    renderer: &mut R,
    space: &Space<Window>,
    output: &Output,
    overlay: &Overlay,
    elements: &mut Vec<Element<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let Some(geometry) = space.output_geometry(output) else {
        return;
    };
    let scale = output.current_scale().fractional_scale();
    let at = (geometry.loc.x, geometry.loc.y);

    let layers: Vec<(Point<i32, Logical>, smithay::desktop::LayerSurface, bool)> = {
        let map = layer_map_for_output(output);
        map.layers()
            .rev()
            .filter_map(|surface| {
                let lower = matches!(surface.layer(), Layer::Background | Layer::Bottom);
                map.layer_geometry(surface)
                    .map(|geo| (geo.loc, surface.clone(), lower))
            })
            .collect()
    };
    let layer = |renderer: &mut R, lower: bool, elements: &mut Vec<Element<R>>| {
        for (loc, surface, _) in layers.iter().filter(|(_, _, held)| *held == lower) {
            let drawn: Vec<WaylandSurfaceRenderElement<R>> = AsRenderElements::<R>::render_elements(
                surface,
                renderer,
                loc.to_physical_precise_round(scale),
                Scale::from(scale),
                1.0,
            );
            elements.extend(drawn.into_iter().map(Elements::Surface));
        }
    };

    layer(renderer, false, elements);

    let windows: Vec<Window> = space.elements().rev().cloned().collect();
    for window in windows {
        let Some(bbox) = space.element_bbox(&window) else {
            continue;
        };
        if !geometry.overlaps(bbox) {
            continue;
        }
        let Some(location) = space.element_location(&window) else {
            continue;
        };
        let origin =
            (location - window.geometry().loc - geometry.loc).to_physical_precise_round(scale);
        let clip = overlay
            .clips
            .iter()
            .find(|(held, _)| *held == window)
            .map(|(_, tile)| {
                let placed = physical(*tile, at, scale);
                Rectangle::<i32, Physical>::new(
                    (placed.x, placed.y).into(),
                    (placed.width, placed.height).into(),
                )
            });
        let Some(clip) = clip else {
            let drawn: Vec<WaylandSurfaceRenderElement<R>> =
                window.render_elements(renderer, origin, Scale::from(scale), 1.0);
            elements.extend(drawn.into_iter().map(Elements::Surface));
            continue;
        };
        let own: Vec<WaylandSurfaceRenderElement<R>> = match window.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                let surface = toplevel.wl_surface();
                for (popup, offset) in PopupManager::popups_for_surface(surface) {
                    let offset = (window.geometry().loc + offset - popup.geometry().loc)
                        .to_physical_precise_round(scale);
                    let drawn: Vec<WaylandSurfaceRenderElement<R>> =
                        render_elements_from_surface_tree(
                            renderer,
                            popup.wl_surface(),
                            origin + offset,
                            scale,
                            1.0,
                            Kind::Unspecified,
                        );
                    elements.extend(drawn.into_iter().map(Elements::Surface));
                }
                render_elements_from_surface_tree(
                    renderer,
                    surface,
                    origin,
                    scale,
                    1.0,
                    Kind::Unspecified,
                )
            }
            WindowSurface::X11(surface) => AsRenderElements::<R>::render_elements(
                surface,
                renderer,
                origin,
                Scale::from(scale),
                1.0,
            ),
        };
        elements.extend(
            own.into_iter()
                .filter_map(|element| CropRenderElement::from_element(element, scale, clip))
                .map(Elements::Tile),
        );
    }

    layer(renderer, true, elements);
}

/// One picture of the hand overlay, drawn from memory at its place on
/// the screen the way the pointer is.
fn hand_element<R>(
    renderer: &mut R,
    picture: &Picture,
    at: (i32, i32),
    scale: f64,
    elements: &mut Vec<Element<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let buffer = MemoryRenderBuffer::from_slice(
        &picture.pixels,
        Fourcc::Argb8888,
        (picture.width, picture.height),
        1,
        Transform::Normal,
        None,
    );
    let local: Point<f64, Logical> =
        (f64::from(picture.x - at.0), f64::from(picture.y - at.1)).into();
    let location: Point<f64, Physical> = local.to_physical(scale);
    match MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        location,
        &buffer,
        None,
        None,
        None,
        Kind::Unspecified,
    ) {
        Ok(element) => elements.push(Elements::Memory(element)),
        Err(err) => log::debug!("the hand overlay did not draw: {err}"),
    }
}

/// Whether a rectangle in the shared space overlaps a screen.
fn touches(rect: Placed, screen: Rectangle<i32, Logical>) -> bool {
    rect.x < screen.loc.x + screen.size.w
        && screen.loc.x < rect.x + rect.width
        && rect.y < screen.loc.y + screen.size.h
        && screen.loc.y < rect.y + rect.height
}

/// The pointer and the drag icon, at a point in the screen's own logical
/// pixels.
fn cursor_elements<R>(
    renderer: &mut R,
    local: Point<f64, Logical>,
    scale: f64,
    overlay: &Overlay,
    decor: &mut Decor,
    elements: &mut Vec<Element<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    match &overlay.cursor {
        CursorImageStatus::Hidden => {}
        CursorImageStatus::Surface(surface) if surface.is_alive() => {
            let hotspot = with_states(surface, |states| {
                states
                    .data_map
                    .get::<Mutex<CursorImageAttributes>>()
                    .and_then(|held| held.lock().ok().map(|attributes| attributes.hotspot))
                    .unwrap_or_default()
            });
            let location = (local - hotspot.to_f64()).to_physical(scale).to_i32_round();
            let drawn: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
                renderer,
                surface,
                location,
                scale,
                1.0,
                Kind::Cursor,
            );
            elements.extend(drawn.into_iter().map(Elements::Surface));
        }
        CursorImageStatus::Surface(_) | CursorImageStatus::Named(_) => {
            let (buffer, hotspot) = decor.cursor.buffer(scale, overlay.millis);
            let location: Point<f64, Physical> = (local - hotspot.to_f64()).to_physical(scale);
            match MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                location,
                &buffer,
                None,
                None,
                None,
                Kind::Cursor,
            ) {
                Ok(element) => elements.push(Elements::Memory(element)),
                Err(err) => log::debug!("the pointer did not draw: {err}"),
            }
        }
    }
    if let Some(icon) = overlay.drag_icon.as_ref().filter(|icon| icon.is_alive()) {
        let location = local.to_physical(scale).to_i32_round();
        let drawn: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
            renderer,
            icon,
            location,
            scale,
            1.0,
            Kind::Unspecified,
        );
        elements.extend(drawn.into_iter().map(Elements::Surface));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_on_an_unscaled_screen_keeps_its_pixels() {
        let rect = Placed {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        assert_eq!(physical(rect, (0, 0), 1.0), rect);
    }

    #[test]
    fn a_rectangle_on_the_right_screen_is_moved_to_that_screens_corner() {
        let rect = Placed {
            x: 2048 + 6,
            y: 6,
            width: 100,
            height: 50,
        };
        let placed = physical(rect, (2048, 0), 1.0);
        assert_eq!((placed.x, placed.y), (6, 6));
    }

    #[test]
    fn a_scaled_rectangle_grows_by_the_scale_and_a_border_stays_visible() {
        let rect = Placed {
            x: 8,
            y: 8,
            width: 800,
            height: 400,
        };
        assert_eq!(
            physical(rect, (0, 0), 1.25),
            Placed {
                x: 10,
                y: 10,
                width: 1000,
                height: 500
            }
        );
        let bar = Placed {
            x: 7,
            y: 8,
            width: 1,
            height: 400,
        };
        assert!(physical(bar, (0, 0), 1.25).width >= 1);
    }

    #[test]
    fn two_bars_that_touch_in_logical_pixels_touch_on_the_screen() {
        let left = Placed {
            x: 0,
            y: 0,
            width: 3,
            height: 10,
        };
        let right = Placed {
            x: 3,
            y: 0,
            width: 3,
            height: 10,
        };
        let a = physical(left, (0, 0), 1.25);
        let b = physical(right, (0, 0), 1.25);
        assert_eq!(a.x + a.width, b.x);
    }

    #[test]
    fn a_bar_on_another_screen_does_not_touch_this_one() {
        let screen = Rectangle::new((0, 0).into(), (2048, 1152).into());
        let there = Placed {
            x: 2100,
            y: 10,
            width: 100,
            height: 1,
        };
        let here = Placed {
            x: 2000,
            y: 10,
            width: 100,
            height: 1,
        };
        assert!(!touches(there, screen));
        assert!(touches(here, screen));
    }
}
