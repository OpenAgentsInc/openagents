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
    let small = atlas.layout_at_scale(1.333333).unwrap();
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
        let w = atlas_plate.measure(&actor.actor.name).max(120.0) + 8.0;
        let desired_x = ((ndc.x + 1.0) * width * 0.5 - w * 0.5).clamp(4.0, width - w - 4.0);
        let anchor = (1.0 - ndc.y) * height * 0.5;
        let mut selected = (desired_x, (anchor - 28.0).max(5.0));
        'search: for row in 0..15 {
            for column in [0, -1, 1, -2, 2, -3, 3] {
                let x = (desired_x + column as f32 * (w + 5.0)).clamp(4.0, width - w - 4.0);
                let y = anchor - 28.0 - row as f32 * 28.0;
                if y < 5.0 {
                    continue;
                }
                if !occupied.iter().any(|(ox, oy, ow)| {
                    x < ox + ow + 4.0 && x + w + 4.0 > *ox && (y - oy).abs() < 28.0
                }) {
                    selected = (x, y);
                    break 'search;
                }
            }
        }
        let (x, y) = selected;
        occupied.push((x, y, w));
        ui.rect(
            atlas,
            x + w * 0.5,
            y + 25.0,
            1.0,
            (anchor - y - 25.0).max(0.0),
            [0.7, 0.5, 0.3, 0.6],
        );
        ui.rect(atlas, x, y, w, 25.0, [0.015, 0.01, 0.01, 0.9]);
        ui.text(
            atlas_plate,
            x + 4.0,
            y + 1.0,
            &actor.actor.name,
            [1.0, 0.9, 0.75, 1.0],
        );
        ui.rect(
            atlas,
            x + 3.0,
            y + 16.0,
            w - 6.0,
            8.0,
            [0.35, 0.28, 0.2, 1.0],
        );
        ui.rect(
            atlas,
            x + 4.0,
            y + 17.0,
            (w - 8.0) * actor.health as f32 / actor.actor.health as f32,
            6.0,
            [0.8, 0.015, 0.01, 1.0],
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
            ui.rect(
                atlas,
                70.0,
                height - 100.0,
                width - 140.0,
                75.0,
                [0.008, 0.008, 0.008, 0.85],
            );
            ui.text(
                atlas,
                90.0,
                height - 90.0,
                &format!("{speaker} yells:"),
                [1.0, 0.15, 0.1, 1.0],
            );
            ui.text(
                atlas,
                (width - atlas.measure(text)) * 0.5,
                height - 60.0,
                text,
                [1.0, 0.92, 0.8, 1.0],
            );
        }
    }
    ui
}
