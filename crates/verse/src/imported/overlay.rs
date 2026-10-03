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
            let height = if frame
                .actors
                .iter()
                .any(|a| a.actor.model == "adventurer" && a.visible)
            {
                height - 85.0
            } else {
                height
            };
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

pub(crate) fn outlined(
    ui: &mut UiBatch,
    atlas: &Atlas,
    x: f32,
    y: f32,
    text: &str,
    color: [f32; 4],
) {
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

/// Classic action-bar hit regions share the drawing geometry with input handling.
pub fn action_at(x: f32, y: f32, width: f32, height: f32) -> Option<super::play::Ability> {
    let left = (width - 12.0 * 46.0) * 0.5;
    if y < height - 64.0 || y > height - 14.0 || x < left {
        return None;
    }
    let index = ((x - left) / 46.0).floor() as usize;
    super::play::Ability::ALL.get(index).copied()
}
pub fn action_bar(
    ui: &mut UiBatch,
    atlas: &Atlas,
    game: &super::play::Game,
    width: f32,
    height: f32,
    hover: Option<super::play::Ability>,
) {
    if !game.unlocked() {
        return;
    }
    use super::play::Ability;
    let snapshot = game.snapshot();
    let small = atlas.layout_at_scale(1.35).unwrap();
    let left = (width - 12.0 * 46.0) * 0.5;
    let y = height - 62.0;
    ui.rect(
        atlas,
        left - 12.0,
        y - 7.0,
        576.0,
        57.0,
        [0.02, 0.018, 0.015, 0.95],
    );
    for index in 0..12 {
        let x = left + index as f32 * 46.0;
        ui.rect(
            atlas,
            x + 3.0,
            y + 3.0,
            39.0,
            39.0,
            [0.025, 0.025, 0.025, 1.0],
        );
        if let Some(ability) = Ability::ALL.get(index) {
            ui.image(
                atlas,
                ability.icon(),
                x + 4.0,
                y + 4.0,
                37.0,
                37.0,
                [1.0; 4],
            );
            let (ready, cd) = if let Some(spell) = ability.spell() {
                let gate = snapshot.abilities.iter().find(|a| a.id == spell).unwrap();
                (gate.ready, gate.cooldown_remaining)
            } else if let Some(spell) = ability.utility() {
                let cd = game.controls.cooldown(spell, game.time);
                (cd == 0.0 && snapshot.player.mana >= spell.cost(), cd)
            } else {
                (
                    game.time >= game.bow_ready,
                    (game.bow_ready - game.time).max(0.0),
                )
            };
            if !ready {
                ui.rect(atlas, x + 4.0, y + 4.0, 37.0, 37.0, [0.0, 0.0, 0.0, 0.65]);
            }
            if cd > 0.0 {
                outlined(
                    ui,
                    atlas,
                    x + 13.0,
                    y + 14.0,
                    &format!("{:.0}", cd.ceil()),
                    [1.0; 4],
                );
            }
            outlined(
                ui,
                &small,
                x + 30.0,
                y + 1.0,
                &(index + 1).to_string(),
                [1.0; 4],
            );
        }
        ui.image(
            atlas,
            "action-frame",
            x - 6.0,
            y - 6.0,
            58.0,
            58.0,
            [1.0; 4],
        );
    }
    outlined(
        ui,
        &small,
        left,
        y + 48.0,
        "1-9 Cast   Tab Target   WASD Move   Right drag Turn",
        [0.9, 0.8, 0.6, 1.0],
    );
    let health = snapshot.player.hp as f32 / snapshot.player.max_hp as f32;
    outlined(ui, atlas, 28.0, 20.0, "Adventurer", [1.0, 0.85, 0.5, 1.0]);
    ui.rect(atlas, 26.0, 46.0, 204.0, 32.0, [0.025, 0.025, 0.025, 0.9]);
    ui.image(
        atlas,
        "status-bar",
        28.0,
        48.0,
        200.0 * health,
        12.0,
        [0.015, 0.6, 0.035, 1.0],
    );
    ui.image(
        atlas,
        "status-bar",
        28.0,
        63.0,
        200.0 * snapshot.player.mana as f32 / snapshot.player.max_mana as f32,
        12.0,
        [0.02, 0.15, 0.9, 1.0],
    );
    outlined(
        ui,
        &small,
        78.0,
        62.0,
        &format!("{} / {}", snapshot.player.mana, snapshot.player.max_mana),
        [1.0; 4],
    );
    if let Some(target) = game
        .frame()
        .actors
        .iter()
        .find(|a| a.actor.id == game.selected)
    {
        outlined(
            ui,
            atlas,
            280.0,
            20.0,
            &target.actor.name,
            [1.0, 0.25, 0.16, 1.0],
        );
        ui.rect(atlas, 278.0, 46.0, 204.0, 17.0, [0.025, 0.025, 0.025, 0.9]);
        ui.image(
            atlas,
            "status-bar",
            280.0,
            48.0,
            200.0 * target.health as f32 / target.actor.health as f32,
            13.0,
            [0.8, 0.015, 0.01, 1.0],
        );
        outlined(
            ui,
            &small,
            332.0,
            47.0,
            &format!("{} / {}", target.health, target.actor.health),
            [1.0; 4],
        );
    }
    if let Some(cast) = &game.casting {
        let progress = ((game.time - cast.started) / (cast.ends - cast.started)).clamp(0.0, 1.0);
        ui.rect(
            atlas,
            width * 0.5 - 152.0,
            y - 48.0,
            304.0,
            22.0,
            [0.06, 0.04, 0.01, 0.95],
        );
        ui.image(
            atlas,
            "status-bar",
            width * 0.5 - 150.0,
            y - 46.0,
            300.0 * progress,
            18.0,
            [1.0, 0.65, 0.04, 1.0],
        );
        outlined(
            ui,
            &small,
            width * 0.5 - atlas.measure(cast.ability.label()) * 0.4,
            y - 45.0,
            cast.ability.label(),
            [1.0; 4],
        );
    } else if let Some(ability) = hover {
        let cost = ability
            .spell()
            .and_then(|spell| snapshot.abilities.iter().find(|a| a.id == spell))
            .map_or_else(|| ability.utility().map_or(0, |s| s.cost()), |a| a.cost);
        let text = format!(
            "{} · {} mana · {}",
            ability.label(),
            cost,
            ability.description()
        );
        outlined(
            ui,
            atlas,
            width * 0.5 - atlas.measure(&text) * 0.5,
            y - 35.0,
            &text,
            [1.0, 0.85, 0.55, 1.0],
        );
    }
    if !game.message.is_empty() {
        outlined(
            ui,
            &small,
            width * 0.5 - small.measure(&game.message) * 0.5,
            100.0,
            &game.message,
            [1.0, 0.8, 0.4, 1.0],
        );
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;
    #[test]
    fn clickable_slots_match_hotkeys_and_empty_slots_do_not_cast() {
        let left = (1280.0 - 12.0 * 46.0) * 0.5;
        for (i, ability) in super::super::play::Ability::ALL.iter().enumerate() {
            assert_eq!(
                action_at(left + i as f32 * 46.0 + 20.0, 680.0, 1280.0, 720.0),
                Some(*ability)
            );
        }
        assert_eq!(action_at(left + 10.0 * 46.0, 680.0, 1280.0, 720.0), None);
        assert_eq!(action_at(left, 600.0, 1280.0, 720.0), None);
    }
}
