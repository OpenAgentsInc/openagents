//! The engine Grid, gold value, and white chain links on the noir canvas.

use glam::{Mat4, Quat, Vec3};
use std::path::Path;
use verse::imported::{Renderer, flat, lighting::Lighting};
use verse::render::View;
use verse::ui::{Atlas, UiBatch};
use verse_engine::assets::{Pack, Surface, Topology, Vertex};
use verse_engine::presentation::Instance;

use super::{Set, model::Node};

pub const GOLD: [f32; 3] = [1.0, 206.0 / 255.0, 84.0 / 255.0];
pub const WHITE: [f32; 3] = [236.0 / 255.0, 240.0 / 255.0, 1.0];
pub const CANVAS: [f32; 3] = [10.0 / 255.0; 3];

fn instance(model: &str, transform: Mat4) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform,
        animation: 0.into(),
        time: 0.0,
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

fn vertex(p: Vec3) -> Vertex {
    Vertex {
        position: p.to_array(),
        normal: [0.0, 1.0, 0.0],
        uv: [0.5, 0.5],
        joints: [0; 4],
        weights: [1.0, 0.0, 0.0, 0.0],
    }
}

fn triangle(surface: &mut Surface, a: Vec3, b: Vec3, c: Vec3) {
    let base = surface.vertices.len() as u32;
    surface.vertices.extend([vertex(a), vertex(b), vertex(c)]);
    surface.indices.extend([base, base + 1, base + 2]);
}

// Invert the imported renderer's grade so its background reaches the
// product's 10/255 sRGB canvas after tone mapping.
fn canvas_tint() -> [f32; 3] {
    let target = CANVAS[0] / 12.92;
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..24 {
        let middle = (low + high) * 0.5;
        if verse_engine::lighting::Grade::CHAMBER
            .display(Vec3::splat(middle))
            .x
            < target
        {
            low = middle;
        } else {
            high = middle;
        }
    }
    [(low + high) * 0.5; 3]
}

fn pack() -> Result<Pack, String> {
    let mut pack = verse::grid_pack::load_pinned()?;
    pack.inventory = None;
    pack.placements.clear();
    pack.models
        .retain(|name, _| name == verse::grid_pack::FLOOR);
    for model in pack.models.values_mut() {
        for surface in &mut model.surfaces {
            surface.tint = surface.tint.map(|c| c * 0.15);
        }
    }
    pack.source_revision = "layertree-events-v1".into();
    for (name, tint) in [
        ("value", GOLD),
        ("signature", WHITE),
        ("faint", [0.18; 3]),
        ("danger", [1.0, 77.0 / 255.0, 66.0 / 255.0]),
    ] {
        let mut sphere = flat::surface(0, tint, Topology::Triangles);
        let ring = [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z];
        for i in 0..4 {
            triangle(&mut sphere, Vec3::Y, ring[(i + 1) % 4], ring[i]);
            triangle(&mut sphere, -Vec3::Y, ring[i], ring[(i + 1) % 4]);
        }
        pack.models
            .insert(name.into(), flat::model(name, vec![sphere], 2.0));
        let mut wire = flat::surface(0, tint.map(|c| c * 0.42), Topology::Lines);
        flat::line(&mut wire, Vec3::ZERO, Vec3::Y);
        pack.models.insert(
            format!("{name}-link"),
            flat::model(&format!("{name}-link"), vec![wire], 1.0),
        );
    }
    let mut canvas = flat::surface(0, canvas_tint(), Topology::Triangles);
    let ring = [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z];
    for i in 0..4 {
        triangle(&mut canvas, Vec3::Y, ring[i], ring[(i + 1) % 4]);
        triangle(&mut canvas, -Vec3::Y, ring[(i + 1) % 4], ring[i]);
    }
    pack.models
        .insert("canvas".into(), flat::model("canvas", vec![canvas], 2.0));
    pack.validate()?;
    Ok(pack)
}

pub fn renderer(size: [u32; 2], atlas: &Atlas) -> Result<Renderer, String> {
    Renderer::new(
        pack()?,
        &verse::grid_pack::pinned_dir(),
        size[0],
        size[1],
        atlas,
        &[
            instance("canvas", Mat4::from_scale(Vec3::splat(1500.0))),
            instance(verse::grid_pack::FLOOR, Mat4::IDENTITY),
        ],
    )
}

pub fn lighting() -> Lighting {
    Lighting {
        ambient: Vec3::ZERO,
        fog: Vec3::from_array(CANVAS),
        density: 0.0,
        shadowed: 0,
        ..Default::default()
    }
}

fn node(model: &str, at: Vec3, radius: f32) -> Instance {
    instance(
        model,
        Mat4::from_scale_rotation_translation(Vec3::splat(radius), Quat::IDENTITY, at),
    )
}

fn link(model: &str, a: Vec3, b: Vec3) -> Instance {
    let delta = b - a;
    instance(
        model,
        Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, delta.length(), 1.0),
            Quat::from_rotation_arc(Vec3::Y, delta.normalize_or_zero()),
            a,
        ),
    )
}

pub fn frame(set: &Set) -> Vec<Instance> {
    let layout = &set.layout;
    let mut out = Vec::new();
    for item in layout.visible() {
        let (position, radius) = animated_node(set, item);
        out.push(node("value", position, radius));
        if item.height < layout.depth {
            let parent = &layout.nodes[&(item.height + 1, item.index / 2)];
            out.push(link("value-link", item.position, parent.position));
            for dot in 0..2 {
                let progress =
                    (set.time * 0.45 + dot as f32 * 0.5 + item.index as f32 * 0.09).fract();
                out.push(node(
                    "value",
                    item.position.lerp(parent.position, progress),
                    0.055,
                ));
            }
        }
        if item.height > layout.cutoff {
            for side in 0..2 {
                if !layout
                    .nodes
                    .contains_key(&(item.height - 1, item.index * 2 + side))
                {
                    let end =
                        item.position + Vec3::new(if side == 0 { -0.75 } else { 0.75 }, -0.8, 0.0);
                    out.push(link("faint-link", item.position, end));
                    out.push(node("faint", end, 0.07));
                }
            }
        }
    }
    let coin = layout.coin();
    let heartbeat = set.event().data.get("op").and_then(|v| v.as_str()) == Some("refresh");
    let coin_at = coin
        - Vec3::X
            * if heartbeat {
                (1.0 - set.phase) * 1.2
            } else {
                0.0
            };
    out.push(node("signature", coin_at, 0.65));
    out.push(link(
        "signature-link",
        coin + Vec3::new(-2.0, -0.6, 0.0),
        coin + Vec3::new(2.0, -0.6, 0.0),
    ));
    if let Some(root) = layout.nodes.get(&(layout.depth, 0)) {
        out.push(link("signature-link", root.position, coin));
    }
    transition_frame(set, &mut out);
    out
}

#[allow(dead_code)] // The viewer uses this camera; film shots use their rig.
pub fn camera(set: &Set, aspect: f32, yaw: f32, zoom: f32) -> View {
    let distance = (set.layout.width * 0.85).max(28.0) * zoom;
    let target = Vec3::new(1.0, 2.0 + set.state.depth as f32 * 0.75, 0.0);
    let eye = target + Vec3::new(yaw.sin() * distance, distance * 0.34, yaw.cos() * distance);
    View {
        eye,
        view_proj: Mat4::perspective_rh(46f32.to_radians(), aspect, 0.1, 2000.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y),
    }
}

pub fn amount(value: u64) -> String {
    let source = value.to_string();
    let mut out = String::new();
    for (i, c) in source.chars().enumerate() {
        if i > 0 && (source.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn project(view: View, position: Vec3, size: [f32; 2]) -> Option<[f32; 2]> {
    let p = view.view_proj * position.extend(1.0);
    if p.w <= 0.0 {
        return None;
    }
    let n = p.truncate() / p.w;
    if n.x.abs() > 1.0 || n.y.abs() > 1.0 {
        return None;
    }
    Some([(n.x + 1.0) * size[0] * 0.5, (1.0 - n.y) * size[1] * 0.5])
}

fn label(item: &Node) -> String {
    if item.height == 0 {
        format!("{} sat", amount(item.sum))
    } else {
        amount(item.sum)
    }
}

pub fn overlay(
    set: &Set,
    view: View,
    size: [f32; 2],
    atlas: &Atlas,
    cursor: Option<[f32; 2]>,
    controls: bool,
) -> UiBatch {
    let mut ui = UiBatch::default();
    let white = [WHITE[0], WHITE[1], WHITE[2], 1.0];
    let gold = [GOLD[0], GOLD[1], GOLD[2], 1.0];
    let dim = [0.5, 0.52, 0.56, 1.0];
    ui.text(atlas, 26.0, 24.0, "LayerTree", white);
    ui.text(
        atlas,
        26.0,
        49.0,
        &format!(
            "{}  /  event {}  /  block {}",
            set.state.network,
            set.state.seq,
            set.state.height.map_or("-".into(), |h| h.to_string())
        ),
        dim,
    );
    ui.text(
        atlas,
        26.0,
        78.0,
        &format!(
            "{} members  /  {} sat",
            set.state.leaves.len(),
            amount(set.state.output_amount)
        ),
        gold,
    );
    let mut occupied: Vec<(f32, f32, f32)> = Vec::new();
    for item in set.layout.visible() {
        let Some([x, y]) = project(view, animated_node(set, item).0, size) else {
            continue;
        };
        let hovered = cursor.is_some_and(|p| (p[0] - x).hypot(p[1] - y) < 24.0);
        if item.height == 0
            || item.height == set.layout.depth
            || set.layout.nodes.len() < 120
            || hovered
        {
            let text = label(item);
            let mut label_y = if item.height == 0 {
                y + 18.0 + (item.index % 2) as f32 * 21.0
            } else {
                y - 22.0
            };
            let width = atlas.measure(&text);
            let left = x - width * 0.5;
            for _ in 0..12 {
                if !occupied.iter().any(|&(a, b, y)| {
                    left < b + 5.0 && left + width > a - 5.0 && (label_y - y).abs() < 17.0
                }) {
                    break;
                }
                label_y -= 18.0;
            }
            occupied.push((left, left + width, label_y));
            ui.text(atlas, left, label_y, &text, gold);
        }
        if hovered && item.height == 0 {
            if let Some(leaf) = set.state.leaves.get(&item.index) {
                ui.text(
                    atlas,
                    x - 54.0,
                    y - 48.0,
                    &format!("leaf {} / {}", item.index, &leaf.exit_key[..12]),
                    white,
                );
            }
        }
    }
    if let Some([x, y]) = project(view, set.layout.coin(), size) {
        ui.text(atlas, x - 50.0, y + 20.0, "Bitcoin output", white);
        ui.text(
            atlas,
            x - 50.0,
            y + 39.0,
            &format!("{} sat", amount(set.state.output_amount)),
            gold,
        );
        let short = set.state.outpoint.chars().take(14).collect::<String>();
        ui.text(atlas, x - 50.0, y + 58.0, &format!("{short}.."), dim);
    }
    transition_labels(set, view, size, atlas, &mut ui);
    let source = Path::new(&set.run.source)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&set.run.source);
    ui.text(
        atlas,
        26.0,
        size[1] - 70.0,
        &format!("Source: {source}"),
        dim,
    );
    ui.text(
        atlas,
        26.0,
        size[1] - 48.0,
        "Gold: value    White: Bitcoin output    Faint stubs: empty subtrees",
        dim,
    );
    if controls {
        ui.text(
            atlas,
            26.0,
            size[1] - 27.0,
            "LEFT / RIGHT orbit    UP / DOWN zoom    ESC close",
            dim,
        );
    }
    ui
}

#[allow(dead_code)] // The viewer writes PNGs; film shots use the film encoder.
pub fn write_png(path: &Path, size: [u32; 2], pixels: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut png = png::Encoder::new(file, size[0], size[1]);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}

// Balances always name the checked destination state. Geometry moves between
// those states without inventing fractional satoshi balances.
fn animated_node(set: &Set, item: &Node) -> (Vec3, f32) {
    let Some((_, previous)) = &set.previous else {
        return (item.position, item.radius);
    };
    let t = set.phase * set.phase * (3.0 - 2.0 * set.phase);
    if let Some(old) = previous.nodes.get(&(item.height, item.index)) {
        (
            old.position.lerp(item.position, t),
            old.radius + (item.radius - old.radius) * t,
        )
    } else {
        (
            item.position + Vec3::Y * (1.0 - t) * 3.0,
            item.radius * (0.2 + 0.8 * t),
        )
    }
}

fn transition_frame(set: &Set, out: &mut Vec<Instance>) {
    let event = set.event();
    let op = event.data.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let t = set.phase;
    let coin = set.layout.coin();
    if matches!(op, "refresh" | "migrate") {
        out.push(node(
            "signature",
            coin + Vec3::X * t * 1.2,
            0.7 + (t * std::f32::consts::PI).sin() * 0.25,
        ));
    }
    if event.kind == "rejected" {
        let pos = event.data.get("pos").and_then(|v| v.as_u64()).unwrap_or(0);
        let at = set
            .layout
            .nodes
            .get(&(0, pos))
            .map_or(Vec3::new(0.0, 2.0, 0.0), |n| n.position);
        out.push(node(
            "danger",
            at,
            0.4 + (set.time * 8.0).sin().abs() * 0.18,
        ));
        return;
    }
    let Some((_, previous)) = &set.previous else {
        return;
    };
    let changes: Vec<super::model::Change> = event.field("changes").unwrap_or_default();
    for change in &changes {
        let old = previous.nodes.get(&(0, change.pos));
        let new = set.layout.nodes.get(&(0, change.pos));
        if matches!(op, "exit" | "evict" | "detach") && new.is_none() {
            if let Some(old) = old {
                let progress = if op == "evict" {
                    ((t - 0.4) / 0.6).max(0.0)
                } else {
                    t
                };
                let end = coin + Vec3::new(0.0, 0.0, 2.0 + change.pos as f32 * 0.15);
                let at = old.position.lerp(end, progress)
                    + Vec3::Y * (progress * std::f32::consts::PI).sin() * 3.0;
                out.push(node("value", at, old.radius));
                out.push(node("signature", end, 0.16));
                out.push(link("value-link", old.position, end));
            }
        }
        if op == "update" {
            if let Some(new) = new {
                for side in [-1.0, 1.0] {
                    let start = new.position + Vec3::new(side * 2.0, 2.0, 0.0);
                    out.push(link("signature-link", start, new.position));
                    out.push(node("signature", start.lerp(new.position, t), 0.11));
                }
            }
        }
    }
    if matches!(op, "split" | "merge" | "rewrite") {
        let affected: Vec<_> = changes
            .iter()
            .filter_map(|c| {
                previous
                    .nodes
                    .get(&(0, c.pos))
                    .or_else(|| set.layout.nodes.get(&(0, c.pos)))
            })
            .collect();
        for pair in affected.windows(2) {
            out.push(link("value-link", pair[0].position, pair[1].position));
            out.push(node(
                "value",
                pair[0].position.lerp(pair[1].position, t)
                    + Vec3::Y * (t * std::f32::consts::PI).sin(),
                0.15,
            ));
        }
        for item in affected {
            out.push(node(
                "signature",
                item.position + Vec3::Y * (1.0 - t) * 2.0,
                0.12,
            ));
        }
    }
    if op == "claim" {
        let source = changes.iter().find(|c| {
            c.old
                .as_ref()
                .is_some_and(|old| c.new.as_ref().map_or(0, |n| n.amount) < old.amount)
        });
        let target = changes.iter().find(|c| {
            c.new
                .as_ref()
                .is_some_and(|new| c.old.as_ref().map_or(0, |o| o.amount) < new.amount)
        });
        if let (Some(a), Some(b)) = (
            source.and_then(|c| previous.nodes.get(&(0, c.pos))),
            target.and_then(|c| set.layout.nodes.get(&(0, c.pos))),
        ) {
            let arc = |p: f32| {
                a.position.lerp(b.position, p) + Vec3::Y * (p * std::f32::consts::PI).sin() * 3.0
            };
            for i in 0..24 {
                out.push(link(
                    "value-link",
                    arc(i as f32 / 24.0),
                    arc((i + 1) as f32 / 24.0),
                ));
            }
            out.push(node("value", arc(t), 0.18));
        }
    }
}

fn transition_labels(set: &Set, view: View, size: [f32; 2], atlas: &Atlas, ui: &mut UiBatch) {
    let event = set.event();
    let simulated = event
        .data
        .get("synthetic")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || set.run.events[0]
            .data
            .get("synthetic")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let op = event.data.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let text = if event.kind == "rejected" {
        format!(
            "{} refused: {}",
            op,
            event
                .data
                .get("reject")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown reason")
        )
    } else if op == "evict" && set.phase < 0.4 {
        "evict / lease expires".into()
    } else {
        op.to_string()
    };
    let color = if event.kind == "rejected" {
        [1.0, 77.0 / 255.0, 66.0 / 255.0, 1.0]
    } else {
        [WHITE[0], WHITE[1], WHITE[2], 1.0]
    };
    ui.text(
        atlas,
        26.0,
        105.0,
        &format!("{text}{}", if simulated { "  /  simulated" } else { "" }),
        color,
    );
    if matches!(op, "exit" | "evict" | "detach") {
        let changes: Vec<super::model::Change> = event.field("changes").unwrap_or_default();
        for (i, change) in changes.iter().enumerate() {
            if let Some(leaf) = &change.old {
                if change.new.is_none() {
                    if let Some([x, y]) =
                        project(view, set.layout.coin() + Vec3::new(0.0, 1.0, 2.0), size)
                    {
                        ui.text(
                            atlas,
                            x,
                            y - 20.0 - i as f32 * 20.0,
                            &format!("{} sat to {}..", amount(leaf.amount), &leaf.exit_key[..8]),
                            [GOLD[0], GOLD[1], GOLD[2], 1.0],
                        );
                    }
                }
            }
        }
    }
    if op == "claim" {
        let changes: Vec<super::model::Change> = event.field("changes").unwrap_or_default();
        for change in changes {
            if let Some(leaf) = change.new {
                if change.old.is_some_and(|old| old.seq != leaf.seq) {
                    if let Some(node) = set.layout.nodes.get(&(0, change.pos)) {
                        if let Some([x, y]) = project(view, node.position + Vec3::Y * 1.0, size) {
                            ui.text(atlas, x, y, &format!("voucher seq {}", leaf.seq), color);
                        }
                    }
                }
            }
        }
    }
}
