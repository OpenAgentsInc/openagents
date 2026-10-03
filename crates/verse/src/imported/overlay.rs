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
    let diagonal = width.hypot(height);
    let plate_scale = diagonal / 1280.0;
    let row_height = 34.0 * plate_scale;
    let small = atlas.layout_at_scale(18.0 / (0.01 * diagonal)).unwrap();
    let atlas_plate = &small;
    let mut occupied: Vec<(f32, f32, f32)> = Vec::new();
    // Keep overhead names clear of the player and target portrait frames.
    if frame
        .actors
        .iter()
        .any(|a| a.visible && a.actor.model == "adventurer")
    {
        let scale = height / 768.0;
        for y in [0.0, row_height, row_height * 2.0] {
            occupied.push((0.0, y, 213.0 * scale));
            occupied.push((250.0 * scale, y, 232.0 * scale));
        }
    }
    for actor in frame
        .actors
        .iter()
        .filter(|a| a.visible && a.actor.nameplate)
    {
        let head = actor.actor.position
            + glam::Vec3::Y * (heights[&actor.actor.model] * actor.actor.scale * 0.9144 + 0.6096);
        let clip = projection * head.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 {
            continue;
        }
        let w = atlas_plate
            .measure(&actor.actor.name)
            .max(128.0 * plate_scale)
            + 8.0;
        let desired_x = ((ndc.x + 1.0) * width * 0.5 - w * 0.5).clamp(4.0, width - w - 4.0);
        let anchor = (1.0 - ndc.y) * height * 0.5;
        let mut selected = (desired_x, (anchor - row_height).max(5.0));
        'search: for row in 0..15 {
            for column in [0, -1, 1, -2, 2, -3, 3] {
                let x = (desired_x + column as f32 * (w + 5.0)).clamp(4.0, width - w - 4.0);
                let y = anchor - row_height - row as f32 * row_height;
                if y < 5.0 {
                    continue;
                }
                if !occupied.iter().any(|(ox, oy, ow)| {
                    x < ox + ow + 4.0 && x + w + 4.0 > *ox && (y - oy).abs() < row_height
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
            [1.0; 4],
        );
        let bx = x + (w - 128.0 * plate_scale) * 0.5;
        ui.rect(
            atlas,
            bx + 3.968 * plate_scale,
            y + 19.008 * plate_scale,
            102.912 * plate_scale,
            8.992 * plate_scale,
            [0.025, 0.005, 0.005, 1.0],
        );
        let fill = 102.912 * plate_scale * actor.health as f32 / actor.actor.health as f32;
        if atlas.sprites.contains_key("status-bar") {
            ui.image_region(
                atlas,
                "status-bar",
                [
                    bx + 3.968 * plate_scale,
                    y + 19.008 * plate_scale,
                    fill,
                    8.992 * plate_scale,
                ],
                [
                    0.0,
                    actor.health as f32 / actor.actor.health as f32,
                    0.0,
                    1.0,
                ],
                [1.0, 0.0, 0.0, 1.0],
            );
        } else {
            ui.rect(
                atlas,
                bx + 3.968 * plate_scale,
                y + 19.008 * plate_scale,
                fill,
                8.992 * plate_scale,
                [1.0, 0.0, 0.0, 1.0],
            );
        }
        ui.image_region(
            atlas,
            "nameplate-border",
            [bx, y, 128.0 * plate_scale, 32.0 * plate_scale],
            [0.0, 1.0, 0.0, 1.0],
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
                .filter(|v| v.color == [1.0, 0.0, 0.0, 1.0])
                .count();
            assert_eq!(red, 13 * 6, "time {time}");
            let bars: Vec<_> = ui
                .vertices
                .iter()
                .filter(|v| v.color == [1.0, 0.0, 0.0, 1.0])
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

/// Classic UI units use a 768-pixel reference height.
fn bar_geometry(width: f32, height: f32) -> (f32, f32, f32) {
    let scale = height / 768.0;
    ((width - 1024.0 * scale) * 0.5, height - 40.0 * scale, scale)
}
/// Classic action-bar hit regions share the drawing geometry with input handling.
pub fn action_at(x: f32, y: f32, width: f32, height: f32) -> Option<super::play::Ability> {
    let (left, top, scale) = bar_geometry(width, height);
    let x = (x - left) / scale - 8.0;
    let y = (y - top) / scale;
    if x < 0.0 || !(0.0..36.0).contains(&y) || x % 42.0 >= 36.0 {
        return None;
    }
    super::play::Ability::ALL
        .get((x / 42.0).floor() as usize)
        .copied()
}
/// Decorative chrome consumes pointer presses instead of selecting the world behind it.
pub fn chrome_at(x: f32, y: f32, width: f32, height: f32) -> bool {
    let (left, _, s) = bar_geometry(width, height);
    (x >= left - 96.0 * s && x <= left + 1120.0 * s && y >= height - 53.0 * s)
        || (y <= 104.0 * s && (x <= 213.0 * s || (250.0 * s..482.0 * s).contains(&x)))
}
fn health_color(fraction: f32) -> [f32; 4] {
    let f = fraction.clamp(0.0, 1.0);
    [(2.0 * (1.0 - f)).min(1.0), (2.0 * f).min(1.0), 0.0, 1.0]
}
fn status(ui: &mut UiBatch, atlas: &Atlas, rect: [f32; 4], fraction: f32, color: [f32; 4]) {
    let f = fraction.clamp(0.0, 1.0);
    ui.rect(
        atlas,
        rect[0],
        rect[1],
        rect[2],
        rect[3],
        [0.0, 0.0, 0.0, 0.5],
    );
    if f > 0.0 {
        ui.image_region(
            atlas,
            "status-bar",
            [rect[0], rect[1], rect[2] * f, rect[3]],
            [0.0, f, 0.0, 1.0],
            color,
        );
    }
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
    let (left, y, s) = bar_geometry(width, height);
    let small = atlas.font("small").layout_at_scale(1.0 / s).unwrap();
    let hotkey = atlas.font("hotkey").layout_at_scale(1.0 / s).unwrap();
    let numbers = atlas.font("numbers").layout_at_scale(1.0 / s).unwrap();
    let image =
        |ui: &mut UiBatch,
         name: &str,
         x: f32,
         y: f32,
         w: f32,
         h: f32,
         uv: [f32; 4],
         color: [f32; 4]| ui.image_region(atlas, name, [x, y, w * s, h * s], uv, color);
    for i in 0..4 {
        let v = 0.83203125 - i as f32 * 0.25;
        image(
            ui,
            "main-bar",
            left + i as f32 * 256.0 * s,
            height - 43.0 * s,
            256.0,
            43.0,
            [0.0, 1.0, v, v + 0.16796875],
            [1.0; 4],
        );
        image(
            ui,
            "main-bar",
            left + i as f32 * 256.0 * s,
            height - 50.0 * s,
            256.0,
            10.0,
            [0.0, 1.0, v - 0.0390625, v],
            [1.0; 4],
        );
    }
    image(
        ui,
        "end-cap",
        left - 96.0 * s,
        height - 128.0 * s,
        128.0,
        128.0,
        [0.0, 1.0, 0.0, 1.0],
        [1.0; 4],
    );
    image(
        ui,
        "end-cap",
        left + 992.0 * s,
        height - 128.0 * s,
        128.0,
        128.0,
        [1.0, 0.0, 0.0, 1.0],
        [1.0; 4],
    );
    for index in 0..12 {
        let x = left + (8.0 + index as f32 * 42.0) * s;
        if let Some(ability) = Ability::ALL.get(index) {
            let (ready, cd, total) = if let Some(spell) = ability.spell() {
                let gate = snapshot.abilities.iter().find(|a| a.id == spell).unwrap();
                (
                    gate.ready,
                    gate.cooldown_remaining,
                    game.cooldown_duration(spell),
                )
            } else if let Some(spell) = ability.utility() {
                let cd = game.controls.cooldown(spell, game.time);
                (
                    cd == 0.0 && snapshot.player.mana >= spell.cost(),
                    cd,
                    spell.cooldown(),
                )
            } else {
                (
                    game.time >= game.bow_ready,
                    (game.bow_ready - game.time).max(0.0),
                    1.0,
                )
            };
            image(
                ui,
                ability.icon(),
                x,
                y,
                36.0,
                36.0,
                [0.0, 1.0, 0.0, 1.0],
                if ready || cd > 0.0 {
                    [1.0; 4]
                } else {
                    [0.5, 0.5, 1.0, 1.0]
                },
            );
            ui.cooldown(atlas, [x, y + s, 36.0 * s], cd / total);
            image(
                ui,
                "action-frame",
                x - 15.0 * s,
                y - 14.0 * s,
                66.0,
                66.0,
                [0.0, 1.0, 0.0, 1.0],
                [1.0; 4],
            );
            let key = if index == 9 {
                "0".into()
            } else {
                (index + 1).to_string()
            };
            outlined(
                ui,
                &hotkey,
                x + 34.0 * s - hotkey.measure(&key),
                y + 2.0 * s,
                &key,
                [0.6, 0.6, 0.6, 1.0],
            );
        } else {
            image(
                ui,
                "empty-slot",
                x,
                y,
                36.0,
                36.0,
                [0.0, 1.0, 0.0, 1.0],
                [1.0; 4],
            );
        }
    }
    image(
        ui,
        "page-up",
        left + 506.0 * s,
        height - 47.0 * s,
        32.0,
        32.0,
        [0.0, 1.0, 0.0, 1.0],
        [1.0; 4],
    );
    image(
        ui,
        "page-down",
        left + 506.0 * s,
        height - 27.0 * s,
        32.0,
        32.0,
        [0.0, 1.0, 0.0, 1.0],
        [1.0; 4],
    );
    outlined(
        ui,
        &small,
        left + 542.0 * s - small.measure("1") * 0.5,
        height - 26.5 * s,
        "1",
        [1.0, 0.82, 0.0, 1.0],
    );
    for (i, name) in [
        "character",
        "spellbook",
        "talents",
        "quest",
        "socials",
        "world",
        "mainmenu",
        "help",
    ]
    .iter()
    .enumerate()
    {
        image(
            ui,
            &format!("micro-{name}"),
            left + (552.0 + i as f32 * 26.0) * s,
            height - 60.0 * s,
            29.0,
            58.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0; 4],
        );
    }
    image(
        ui,
        "portrait-adventurer",
        left + 557.5 * s,
        height - 32.0 * s,
        18.0,
        25.0,
        [0.2, 0.8, 0.0666, 0.9],
        [1.0; 4],
    );
    for i in 0..4 {
        let x = left + (939.0 - i as f32 * 42.0) * s;
        image(
            ui,
            "bag-empty",
            x,
            height - 39.0 * s,
            37.0,
            37.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0; 4],
        );
        image(
            ui,
            "action-frame",
            x - 13.5 * s,
            height - 51.5 * s,
            64.0,
            64.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0; 4],
        );
    }
    image(
        ui,
        "backpack",
        left + 981.0 * s,
        height - 39.0 * s,
        37.0,
        37.0,
        [0.0, 1.0, 0.0, 1.0],
        [1.0; 4],
    );
    image(
        ui,
        "action-frame",
        left + 967.5 * s,
        height - 51.5 * s,
        64.0,
        64.0,
        [0.0, 1.0, 0.0, 1.0],
        [1.0; 4],
    );
    let health = snapshot.player.hp as f32 / snapshot.player.max_hp as f32;
    ui.rect(
        atlas,
        87.0 * s,
        26.0 * s,
        119.0 * s,
        41.0 * s,
        [0.0, 0.0, 0.0, 0.5],
    );
    image(
        ui,
        "portrait-adventurer",
        23.0 * s,
        16.0 * s,
        64.0,
        64.0,
        [0.0, 1.0, 0.0, 1.0],
        [1.0; 4],
    );
    status(
        ui,
        atlas,
        [87.0 * s, 45.0 * s, 119.0 * s, 12.0 * s],
        health,
        health_color(health),
    );
    status(
        ui,
        atlas,
        [87.0 * s, 56.0 * s, 119.0 * s, 12.0 * s],
        snapshot.player.mana as f32 / snapshot.player.max_mana as f32,
        [0.0, 0.0, 1.0, 1.0],
    );
    image(
        ui,
        "unit-frame",
        -19.0 * s,
        4.0 * s,
        232.0,
        100.0,
        [1.0, 0.09375, 0.0, 0.78125],
        [1.0; 4],
    );
    outlined(
        ui,
        &small,
        147.0 * s - small.measure("Adventurer") * 0.5,
        30.0 * s,
        "Adventurer",
        [1.0, 0.82, 0.0, 1.0],
    );
    let mana = format!("{} / {}", snapshot.player.mana, snapshot.player.max_mana);
    outlined(
        ui,
        &numbers,
        147.0 * s - numbers.measure(&mana) * 0.5,
        55.0 * s,
        &mana,
        [1.0; 4],
    );
    if let Some(target) = game
        .frame()
        .actors
        .iter()
        .find(|a| a.actor.id == game.selected)
    {
        let health = target.health as f32 / target.actor.health as f32;
        ui.rect(
            atlas,
            257.0 * s,
            26.0 * s,
            119.0 * s,
            41.0 * s,
            [0.0, 0.0, 0.0, 0.5],
        );
        image(
            ui,
            "unit-name",
            257.0 * s,
            26.0 * s,
            119.0,
            19.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
        );
        image(
            ui,
            &format!("portrait-{}", target.actor.model),
            376.0 * s,
            16.0 * s,
            64.0,
            64.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0; 4],
        );
        status(
            ui,
            atlas,
            [257.0 * s, 45.0 * s, 119.0 * s, 12.0 * s],
            health,
            health_color(health),
        );
        image(
            ui,
            if target.actor.model == "claude" {
                "elite-frame"
            } else {
                "unit-frame"
            },
            250.0 * s,
            4.0 * s,
            232.0,
            100.0,
            [0.09375, 1.0, 0.0, 0.78125],
            [1.0; 4],
        );
        outlined(
            ui,
            &small,
            316.0 * s - small.measure(&target.actor.name) * 0.5,
            30.0 * s,
            &target.actor.name,
            [1.0, 0.82, 0.0, 1.0],
        );
        let hp = format!("{} / {}", target.health, target.actor.health);
        outlined(
            ui,
            &numbers,
            316.0 * s - numbers.measure(&hp) * 0.5,
            44.0 * s,
            &hp,
            [1.0; 4],
        );
        image(
            ui,
            "unit-skull",
            421.0 * s,
            62.0 * s,
            16.0,
            16.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0; 4],
        );
    }
    if let Some(encounter) = &game.encounter {
        let label = if encounter.ended.is_some() {
            if snapshot.player.hp == 0 {
                "Defeat — Claude survives"
            } else {
                "Victory"
            }
        } else if game.agent_controlled {
            "Agent control — F1: manual combat"
        } else {
            "Player control — F2: agent combat"
        };
        outlined(
            ui,
            &small,
            width * 0.5 - small.measure(label) * 0.5,
            12.0 * s,
            label,
            [1.0, 0.82, 0.0, 1.0],
        );
        let hp = format!("{} / {}", snapshot.player.hp, snapshot.player.max_hp);
        outlined(
            ui,
            &numbers,
            147.0 * s - numbers.measure(&hp) * 0.5,
            44.0 * s,
            &hp,
            [1.0; 4],
        );
        if game.controls.shield > 0 && game.time < game.controls.shield_until {
            image(
                ui,
                "shield-icon",
                217.0 * s,
                20.0 * s,
                24.0,
                24.0,
                [0.0, 1.0, 0.0, 1.0],
                [1.0; 4],
            );
            outlined(
                ui,
                &small,
                218.0 * s,
                46.0 * s,
                &format!("{}", game.controls.shield),
                [0.3, 0.8, 1.0, 1.0],
            );
        }
        if encounter.ended.is_some() {
            let outcome = format!(
                "Claude: {} / {} health · {} cultists defeated",
                encounter.boss_remaining, encounter.boss_max, encounter.kills
            );
            outlined(
                ui,
                atlas,
                width * 0.5 - atlas.measure(&outcome) * 0.5,
                height * 0.35,
                &outcome,
                [1.0, 0.82, 0.0, 1.0],
            );
        }
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
    fn classic_hit_regions_keep_six_unit_gaps_at_every_ui_scale() {
        for (w, h) in [(1024.0, 768.0), (1280.0, 720.0), (1920.0, 1080.0)] {
            let (left, top, s) = bar_geometry(w, h);
            assert_eq!(
                action_at(left + 43.9 * s, top + 18.0 * s, w, h),
                Some(super::super::play::Ability::Bow)
            );
            assert_eq!(action_at(left + 45.0 * s, top + 18.0 * s, w, h), None);
            assert_eq!(
                action_at(left + 50.0 * s, top + 18.0 * s, w, h),
                Some(super::super::play::Ability::FireBolt)
            );
            assert!(chrome_at(left + 800.0 * s, h - 20.0 * s, w, h));
            assert!(!chrome_at(w * 0.5, h * 0.5, w, h));
        }
        assert_eq!(health_color(1.0), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(health_color(0.5), [1.0, 1.0, 0.0, 1.0]);
        assert_eq!(health_color(0.0), [1.0, 0.0, 0.0, 1.0]);
    }
    #[test]
    fn clickable_slots_match_hotkeys_and_empty_slots_do_not_cast() {
        let (left, top, s) = bar_geometry(1280.0, 720.0);
        for (i, ability) in super::super::play::Ability::ALL.iter().enumerate() {
            assert_eq!(
                action_at(
                    left + (8.0 + i as f32 * 42.0 + 20.0) * s,
                    top + 20.0 * s,
                    1280.0,
                    720.0
                ),
                Some(*ability)
            );
        }
        assert_eq!(
            action_at(
                left + (8.0 + 10.0 * 42.0) * s,
                top + 20.0 * s,
                1280.0,
                720.0
            ),
            None
        );
        assert_eq!(action_at(left, 600.0, 1280.0, 720.0), None);
    }
}
