//! Head-anchored hostile health bars and scripted cinematic dialogue.
use crate::ui::{Atlas, UiBatch};
use glam::Mat4;
use std::collections::BTreeMap;
use verse_wow::director::{Action, Frame};

pub fn cinematic(
    atlas: &Atlas,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
) -> UiBatch {
    let mut ui = UiBatch::default();
    let small = atlas.layout_at_scale(1.285714).unwrap();
    let atlas_plate = &small;
    let mut occupied: Vec<(f32, f32, f32)> = Vec::new();
    for actor in frame
        .actors
        .iter()
        .filter(|a| a.visible && a.actor.nameplate)
    {
        let head = actor.actor.position
            + glam::Vec3::Y * (heights[&actor.actor.model] * actor.actor.scale * 0.9144 + 0.3);
        let clip = projection * head.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 {
            continue;
        }
        let w = atlas_plate.measure(&actor.actor.name).max(128.0) + 8.0;
        let desired_x = ((ndc.x + 1.0) * width * 0.5 - w * 0.5).clamp(4.0, width - w - 4.0);
        let anchor = (1.0 - ndc.y) * height * 0.5;
        let mut selected = (desired_x, (anchor - 34.0).max(5.0));
        'search: for row in 0..15 {
            for column in [0, -1, 1, -2, 2, -3, 3] {
                let x = (desired_x + column as f32 * (w + 5.0)).clamp(4.0, width - w - 4.0);
                let y = anchor - 34.0 - row as f32 * 34.0;
                if y < 5.0 {
                    continue;
                }
                if !occupied.iter().any(|(ox, oy, ow)| {
                    x < ox + ow + 4.0 && x + w + 4.0 > *ox && (y - oy).abs() < 34.0
                }) {
                    selected = (x, y);
                    break 'search;
                }
            }
        }
        let (x, y) = selected;
        occupied.push((x, y, w));
        outlined(
            &mut ui,
            atlas_plate,
            x + (w - atlas_plate.measure(&actor.actor.name)) * 0.5,
            y,
            &actor.actor.name,
            [1.0, 1.0, 0.82, 1.0],
        );
        let bx = x + (w - 128.0) * 0.5;
        ui.rect(
            atlas,
            bx + 4.0,
            y + 18.0,
            103.0,
            10.0,
            [0.025, 0.005, 0.005, 1.0],
        );
        let fill = 103.0 * actor.health as f32 / actor.actor.health as f32;
        if atlas.sprites.contains_key("status-bar") {
            ui.image(
                atlas,
                "status-bar",
                bx + 4.0,
                y + 18.0,
                fill,
                10.0,
                [0.8, 0.015, 0.01, 1.0],
            );
        } else {
            ui.rect(
                atlas,
                bx + 4.0,
                y + 18.0,
                fill,
                10.0,
                [0.8, 0.015, 0.01, 1.0],
            );
        }
        ui.image(
            atlas,
            "nameplate-border",
            bx,
            y + 16.0,
            128.0,
            16.0,
            [1.0; 4],
        );
    }
    if let Some(cue) = &frame.yell {
        if let Action::Yell { text, .. } = &cue.action {
            let speaker = &frame
                .actors
                .iter()
                .find(|a| a.actor.id == cue.actor)
                .unwrap()
                .actor
                .name;
            let label = format!("{speaker} yells:");
            outlined(
                &mut ui,
                atlas,
                (width - atlas.measure(&label)) * 0.5,
                height - 92.0,
                &label,
                [1.0, 0.16, 0.12, 1.0],
            );
            let lines = atlas.wrap(text, width - 180.0);
            for (i, line) in lines.iter().enumerate() {
                outlined(
                    &mut ui,
                    atlas,
                    (width - atlas.measure(line)) * 0.5,
                    height - 64.0 + i as f32 * atlas.line,
                    line,
                    [1.0, 0.86, 0.55, 1.0],
                );
            }
        }
    }
    ui
}

fn outlined(ui: &mut UiBatch, atlas: &Atlas, x: f32, y: f32, text: &str, color: [f32; 4]) {
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        ui.text(atlas, x + dx, y + dy, text, [0.0, 0.0, 0.0, 1.0]);
    }
    ui.text(atlas, x, y, text, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ritual_keeps_all_thirteen_hostile_bars_in_both_camera_shots() {
        let scene = verse_wow::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let atlas = Atlas::new(16.0);
        let heights = BTreeMap::from([
            ("claude".into(), 3.8556),
            ("cultist".into(), 2.333),
            ("adventurer".into(), 2.0153),
        ]);
        for index in 0..2160 {
            let time = index as f32 / 30.0;
            let frame = scene.frame(time);
            let ui = cinematic(
                &atlas,
                &frame,
                &heights,
                frame.view_projection(1280.0 / 720.0),
                1280.0,
                720.0,
            );
            let red = ui
                .vertices
                .iter()
                .filter(|v| v.color == [0.8, 0.015, 0.01, 1.0])
                .count();
            assert_eq!(red, 13 * 6, "time {time}");
            let bars: Vec<_> = ui
                .vertices
                .iter()
                .filter(|v| v.color == [0.8, 0.015, 0.01, 1.0])
                .collect();
            let bounds: Vec<_> = bars
                .chunks(6)
                .map(|b| {
                    (
                        b.iter().map(|v| v.pos[0]).fold(f32::INFINITY, f32::min),
                        b.iter().map(|v| v.pos[1]).fold(f32::INFINITY, f32::min),
                        b.iter().map(|v| v.pos[0]).fold(f32::NEG_INFINITY, f32::max),
                        b.iter().map(|v| v.pos[1]).fold(f32::NEG_INFINITY, f32::max),
                    )
                })
                .collect();
            for (i, a) in bounds.iter().enumerate() {
                for b in &bounds[i + 1..] {
                    assert!(
                        !(a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1),
                        "Overlapping bars at {time}"
                    );
                }
            }

            assert!(
                ui.vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|x| x.is_finite()))
            );
        }
    }
}
