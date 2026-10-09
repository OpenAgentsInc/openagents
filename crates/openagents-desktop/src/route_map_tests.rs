use super::*;
use openagents_chat_app::route_map::{GapKind, Kind};
use rust_native::Element;

const W: f32 = 800.0;
const H: f32 = 600.0;

/// A page painted once at 800 by 600 points, so it knows its size and is
/// fitted.
fn page(reduce: bool) -> (MapPage, Instant) {
    let mut page = MapPage::new(build(Local::default()), reduce);
    let mut frame = Frame::new(W as usize, H as usize, Color::rgb(0, 0, 0));
    page.paint(
        &mut frame,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: W,
            h: H,
        },
    );
    (page, Instant::now())
}

fn node(page: &MapPage, id: &str) -> usize {
    page.map().find(id).unwrap_or_else(|| panic!("{id}"))
}

fn screen(page: &MapPage, index: usize) -> Point {
    page.camera().to_screen(page.layout.positions[index], W, H)
}

fn click(page: &mut MapPage, at: Point, now: Instant) {
    page.input(
        SurfaceInput::Down {
            x: at.x,
            y: at.y,
            shift: false,
        },
        now,
    );
    page.input(SurfaceInput::Up { x: at.x, y: at.y }, now);
}

/// The first paint fits the whole map; a click on a node selects it and
/// opens the inspector; a click on empty space clears the selection.
#[test]
fn a_click_selects_the_node_under_it() {
    let (mut page, now) = page(false);
    let bounds = page.layout.bounds();
    for corner in [bounds.0, bounds.1] {
        let p = page.camera().to_screen(corner, W, H);
        assert!(p.x >= 23.0 && p.x <= W - 23.0 && p.y >= 23.0 && p.y <= H - 23.0);
    }
    let front = node(&page, "front");
    let at = screen(&page, front);
    click(&mut page, at, now);
    assert_eq!(page.selected(), Some(front));
    assert_eq!(page.panel(), Panel::Inspector);
    click(&mut page, Point::new(2.0, 2.0), now + DOUBLE * 2);
    assert_eq!(page.selected(), None);
}

/// A drag pans by the pointer's motion and selects nothing; a wheel pans;
/// a pinch zooms about the pointer.
#[test]
fn drag_wheel_and_pinch_move_the_camera() {
    let (mut page, now) = page(false);
    let before = page.camera();
    page.input(
        SurfaceInput::Down {
            x: 400.0,
            y: 300.0,
            shift: false,
        },
        now,
    );
    page.input(SurfaceInput::Move { x: 430.0, y: 320.0 }, now);
    page.input(SurfaceInput::Up { x: 430.0, y: 320.0 }, now);
    let moved = page.camera();
    assert!((moved.center.x - (before.center.x - 30.0 / before.zoom)).abs() < 1e-3);
    assert!((moved.center.y - (before.center.y - 20.0 / before.zoom)).abs() < 1e-3);
    assert_eq!(page.selected(), None, "a drag is not a click");
    page.input(
        SurfaceInput::Wheel {
            x: 1.0,
            y: 1.0,
            dx: 0.0,
            dy: 40.0,
        },
        now,
    );
    assert!((page.camera().center.y - (moved.center.y - 40.0 / moved.zoom)).abs() < 1e-3);
    let anchor = Point::new(200.0, 150.0);
    let under = page.camera().to_world(anchor, W, H);
    page.input(
        SurfaceInput::Zoom {
            x: anchor.x,
            y: anchor.y,
            factor: 1.5,
        },
        now,
    );
    assert!((page.camera().zoom - moved.zoom * 1.5).abs() < 1e-4);
    assert!(page.camera().to_world(anchor, W, H).distance(under) < 1e-2);
}

/// A double-click zooms into the node: it and its members fill the view.
#[test]
fn a_double_click_zooms_into_a_node() {
    let (mut page, now) = page(true);
    let coder = node(&page, "coder");
    let at = screen(&page, coder);
    let zoom = page.camera().zoom;
    click(&mut page, at, now);
    click(&mut page, at, now + Duration::from_millis(150));
    assert!(page.camera().zoom > zoom, "zoomed in");
    let p = screen(&page, coder);
    assert!(
        p.x > 0.0 && p.x < W && p.y > 0.0 && p.y < H,
        "Coder stays in view"
    );
    for child in page.map().children(coder) {
        let c = screen(&page, child);
        assert!(
            c.x > -1.0 && c.x < W + 1.0 && c.y > -1.0 && c.y < H + 1.0,
            "members in view"
        );
    }
}

/// Without Reduce motion the camera eases over frames and asks for them;
/// with it, it moves at once and asks for none.
#[test]
fn reduce_motion_moves_the_camera_at_once() {
    let (mut eased, now) = page(false);
    let start = eased.camera();
    eased.act(Action::ZoomIn, now);
    assert!(eased.animating());
    assert_eq!(eased.next_wake(now), Some(now + FRAME));
    eased.tick(now + EASE / 2);
    let mid = eased.camera().zoom;
    assert!(mid > start.zoom && mid < start.zoom * layout::ZOOM_STEP);
    eased.tick(now + EASE);
    assert!(!eased.animating());
    assert!((eased.camera().zoom - start.zoom * layout::ZOOM_STEP).abs() < 1e-4);

    let (mut still, now) = page(true);
    let start = still.camera();
    still.act(Action::ZoomIn, now);
    assert!(!still.animating());
    assert_eq!(still.next_wake(now), None);
    assert!((still.camera().zoom - start.zoom * layout::ZOOM_STEP).abs() < 1e-4);
    still.act(Action::Fit, now);
    assert_eq!(still.camera(), start, "Fit returns to the fitted view");
}

/// Keys: Cmd + and − zoom and Cmd 0 fits whenever the page shows; arrows
/// move to a neighbor, Tab steps in the outline's order, and Esc steps out
/// to the parent, then gives the keys back.
#[test]
fn keyboard_navigation_between_nodes() {
    let (mut page, now) = page(true);
    let zoom = page.camera().zoom;
    assert!(page.key("=", true, false, now));
    assert!(page.camera().zoom > zoom);
    assert!(page.key("0", true, false, now));
    assert!((page.camera().zoom - zoom).abs() < 1e-4);
    assert!(page.key("Tab", false, false, now));
    let outline = page.map().outline();
    let first = page.selected().expect("Tab selects");
    assert_eq!(
        first, outline[1],
        "after the front, the outline's next node"
    );
    assert!(page.key("Tab", false, true, now));
    assert_eq!(page.selected(), Some(outline[0]));
    assert!(page.key("ArrowRight", false, false, now));
    let right = page.selected().unwrap();
    assert!(page.layout.positions[right].x > page.layout.positions[outline[0]].x);
    let coder = node(&page, "coder");
    page.select(Some(coder));
    assert!(page.key("Escape", false, false, now));
    assert_eq!(page.selected(), page.map().nodes[coder].parent);
    while page.selected().is_some() {
        page.key("Escape", false, false, now);
    }
    assert!(page.focused());
    page.key("Escape", false, false, now);
    assert!(
        !page.focused(),
        "Esc with nothing selected gives the keys back"
    );
    assert!(
        !page.key("Tab", false, false, now),
        "Tab then moves the window's focus"
    );
}

/// Each next step becomes an effect the window carries out after the tap:
/// a chat draft, a copied command, a page, or Settings.
#[test]
fn next_steps_become_effects() {
    let (mut page, now) = page(true);
    let missing = page
        .map()
        .gaps
        .iter()
        .position(|g| g.kind == GapKind::NoPlugin)
        .expect("capability.missing is a gap");
    page.act(Action::GapStep { gap: missing }, now);
    assert_eq!(
        page.take_effects(),
        vec![Effect::Chat("Help me make a plugin that ".into())]
    );
    let unanswered = page
        .map()
        .gaps
        .iter()
        .position(|g| g.kind == GapKind::UnansweredQuestions)
        .unwrap();
    page.act(Action::GapStep { gap: unanswered }, now);
    let effects = page.take_effects();
    assert!(matches!(&effects[..], [Effect::Copy(c)] if c.starts_with("microcoder kb add")));
    assert!(page.view_keys().iter().any(|k| k == "map-notice"));
    let weak = page
        .map()
        .gaps
        .iter()
        .position(|g| g.kind == GapKind::WeakRoute)
        .unwrap();
    page.act(Action::GapStep { gap: weak }, now);
    assert!(
        matches!(&page.take_effects()[..], [Effect::Open(url)] if url.starts_with("https://github.com/OpenAgentsInc/openagents/issues/new?"))
    );
    let product = node(&page, "route:product.kb");
    let field = page
        .map()
        .inspect(product)
        .fields
        .iter()
        .position(|f| f.link.is_some())
        .unwrap();
    page.act(
        Action::Link {
            node: product,
            field,
        },
        now,
    );
    assert!(matches!(&page.take_effects()[..], [Effect::Open(url)] if url.contains("/blob/main/")));
}

/// The views: a toolbar, the surface beside the panel, the inspector's
/// fields and steps for a selection, a paged Gaps panel, and an outline
/// whose rows are named for screen readers.
#[test]
fn the_page_views_carry_the_inspector_gaps_and_outline() {
    let (mut page, now) = page(true);
    let keys = page.view_keys();
    for key in [
        "map-toolbar",
        "map-fit",
        "map-families",
        "map-kinds",
        "map-gaps-only",
        "route-map-surface",
        "map-side",
        "map-tab-gaps",
    ] {
        assert!(keys.iter().any(|k| k == key), "{key}");
    }
    assert!(
        keys.iter().any(|k| k == "map-gaps-next"),
        "the gaps are paged"
    );
    let coder = node(&page, "coder");
    page.act(Action::Select { node: coder }, now);
    let labels = page.view_labels();
    assert!(labels.iter().any(|l| l == "Coder"));
    // The sample plugins are never on the map, so nothing reads adopted.
    assert!(
        !labels
            .iter()
            .any(|l| l.starts_with("Adopted into everyone's Coder"))
    );
    assert!(labels.iter().any(|l| l.contains("Engine")));
    page.act(
        Action::Panel {
            panel: Panel::Outline,
        },
        now,
    );
    let labels = page.view_labels();
    assert!(
        labels
            .iter()
            .any(|l| l.trim_start() == "Router: OpenAgents")
    );
    assert!(
        labels
            .iter()
            .any(|l| l.trim_start().starts_with("Route family: Answers"))
    );
    // Selecting Coder opened its ancestors, so its members are listed.
    assert!(
        labels
            .iter()
            .any(|l| l.trim_start().starts_with("Plugin: Outline, Not packaged"))
    );
}

/// Filters: the family cycle, the kind cycle, gaps only, and not measured
/// only narrow what's drawn bright and what the Gaps panel lists.
#[test]
fn filters_cycle_and_narrow() {
    let (mut page, now) = page(true);
    page.act(Action::Families, now);
    assert_eq!(page.filter().family.as_deref(), Some("answers"));
    for _ in 0..5 {
        page.act(Action::Families, now);
    }
    assert_eq!(page.filter().family, None, "back to every family");
    page.act(Action::Kinds, now);
    assert_eq!(page.filter().kinds, vec![Kind::Answer]);
    page.act(Action::Kinds, now);
    page.act(Action::Kinds, now);
    assert_eq!(page.filter().kinds, vec![Kind::Plugin]);
    let plugins = page.map().gaps_where(page.filter());
    assert!(
        plugins
            .iter()
            .all(|&g| page.map().nodes[page.map().gaps[g].node].kind == Kind::Plugin)
    );
    for _ in 0..3 {
        page.act(Action::Kinds, now);
    }
    assert!(page.filter().kinds.is_empty());
    page.act(Action::GapsOnly, now);
    assert!(page.filter().gaps_only);
    let front = node(&page, "front");
    assert!(!page.visible(front), "the front has no gap");
}

/// Screen readers read every node by name, kind, state, and gaps; nodes on
/// screen have bounds a screen reader's click lands in.
#[test]
fn the_surface_is_described_to_screen_readers() {
    let (mut page, now) = page(true);
    let content = page.access_content();
    assert_eq!(content.rows.len(), page.map().nodes.len() + Kind::ALL.len());
    let names: Vec<String> = content
        .rows
        .iter()
        .filter_map(|row| match &row.element {
            Element::Button { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect();
    for index in 0..page.map().nodes.len() {
        assert!(names.contains(&page.map().accessible_name(index)));
    }
    let coder = node(&page, "coder");
    let key = format!("route-map-node-{coder}");
    let rect = content.bounds.get(&key).expect("Coder is on screen");
    click(
        &mut page,
        Point::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0),
        now,
    );
    assert_eq!(
        page.selected(),
        Some(coder),
        "a screen reader's click selects it"
    );
}

/// The paint is dark, each kind in its own color, and a gap is a red dot.
#[test]
fn the_paint_colors_nodes_by_kind() {
    let (mut page, _) = page(true);
    let mut frame = Frame::new(W as usize, H as usize, Color::rgb(255, 255, 255));
    let rect = PxRect {
        x: 0.0,
        y: 0.0,
        w: W,
        h: H,
    };
    page.paint(&mut frame, rect);
    let corner = frame.pixel(W as usize - 2, 2);
    assert!(
        corner.iter().all(|c| *c < 20),
        "the canvas is dark: {corner:?}"
    );
    let near = |a: [u8; 3], b: Color| {
        (i32::from(a[0]) - i32::from(b.red)).abs() < 40
            && (i32::from(a[1]) - i32::from(b.green)).abs() < 40
            && (i32::from(a[2]) - i32::from(b.blue)).abs() < 40
    };
    for id in ["front", "coder", "route:work.dispatch"] {
        let index = node(&page, id);
        let p = screen(&page, index);
        let pixel = frame.pixel(p.x as usize, p.y as usize);
        assert!(
            near(pixel, page.map().nodes[index].kind.color()),
            "{id} is drawn in its kind's color: {pixel:?}"
        );
    }
}

impl MapPage {
    fn view_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        walk(&self.view(), &mut |node| keys.push(node.key.clone()));
        keys
    }

    fn view_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        walk(&self.view(), &mut |node| match &node.element {
            Element::Button { label, .. } => labels.push(label.clone()),
            Element::Text { value, .. } => labels.push(value.clone()),
            _ => {}
        });
        labels
    }
}

fn walk(node: &Node<Intent>, visit: &mut dyn FnMut(&Node<Intent>)) {
    visit(node);
    if let Element::Stack { children, .. } = &node.element {
        for child in children {
            walk(child, visit);
        }
    }
}

/// How long one paint of the whole map takes at the default and minimum
/// window sizes, 1x and 2x, fitted and zoomed in: the frame budget for
/// panning and zooming. Run in release:
/// `cargo test --release -p openagents-desktop --lib paint_timing -- --ignored --nocapture`.
#[test]
#[ignore = "a timing, not a check"]
fn paint_timing() {
    for (w, h) in [(1200.0_f32, 840.0_f32), (760.0, 540.0)] {
        for scale in [1.0_f32, 2.0] {
            for zoom in [None, Some(1.6)] {
                let mut page = MapPage::new(build(Local::default()), true);
                page.set_unit(scale);
                let (sw, sh) = (w - 600.0_f32.min(w * 0.6), h - 140.0);
                let rect = PxRect {
                    x: 0.0,
                    y: 0.0,
                    w: sw * scale,
                    h: sh * scale,
                };
                let mut frame = Frame::new(rect.w as usize, rect.h as usize, Color::rgb(0, 0, 0));
                page.paint(&mut frame, rect);
                if let Some(zoom) = zoom {
                    let coder = node(&page, "coder");
                    page.camera = Camera {
                        center: page.layout.positions[coder],
                        zoom,
                    };
                }
                let frames = 30;
                let started = Instant::now();
                for n in 0..frames {
                    page.camera.pan(3.0, (n % 3) as f32);
                    page.paint(&mut frame, rect);
                }
                let ms = started.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
                println!(
                    "{w}x{h} window, map {sw}x{sh} pt at {scale}x, {}: {ms:.2} ms a frame",
                    zoom.map_or("fitted".to_string(), |z| format!("zoom {z}"))
                );
            }
        }
    }
}
