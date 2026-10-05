//! Head-anchored NPC health bars and scripted cinematic dialogue.
use crate::ui::{Atlas, UiBatch};
use glam::Mat4;
use std::collections::BTreeMap;
use verse_engine::director::{Action, Frame};

pub fn cinematic(
    atlas: &Atlas,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
) -> UiBatch {
    cinematic_with_markers(
        atlas,
        frame,
        heights,
        projection,
        width,
        height,
        &BTreeMap::new(),
    )
}

pub fn cinematic_with_markers(
    atlas: &Atlas,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
    markers: &BTreeMap<verse_engine::core::LifeId, verse_world::service::progression::Marker>,
) -> UiBatch {
    cinematic_with_policy(
        atlas,
        frame,
        heights,
        projection,
        width,
        height,
        markers,
        frame.target,
        None,
        false,
    )
}

pub fn cinematic_with_focus(
    atlas: &Atlas,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
    markers: &BTreeMap<verse_engine::core::LifeId, verse_world::service::progression::Marker>,
    focus: glam::Vec3,
    target: Option<verse_engine::core::LifeId>,
) -> UiBatch {
    cinematic_with_policy(
        atlas, frame, heights, projection, width, height, markers, focus, target, true,
    )
}

fn cinematic_with_policy(
    atlas: &Atlas,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
    markers: &BTreeMap<verse_engine::core::LifeId, verse_world::service::progression::Marker>,
    focus: glam::Vec3,
    target: Option<verse_engine::core::LifeId>,
    selected_only: bool,
) -> UiBatch {
    let mut ui = UiBatch::default();
    let diagonal = width.hypot(height);
    let plate_scale = diagonal / 1280.0;
    let row_height = if markers.is_empty() { 34.0 } else { 58.0 } * plate_scale;
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
    let mut candidates: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| a.visible && a.actor.nameplate && a.health > 0)
        .filter(|a| !selected_only || (target.is_some() && a.life == target))
        .filter(|a| {
            (target.is_some() && a.life == target)
                || a.actor.position.distance_squared(focus) <= 18. * 18.
        })
        .collect();
    candidates.sort_by(|a, b| {
        let priority = |a: &&verse_engine::director::ActorFrame| {
            (
                !(target.is_some() && a.life == target),
                a.actor.model != "claude",
            )
        };
        priority(&a)
            .cmp(&priority(&b))
            .then_with(|| {
                a.actor
                    .position
                    .distance_squared(focus)
                    .total_cmp(&b.actor.position.distance_squared(focus))
            })
            .then_with(|| a.actor.id.cmp(&b.actor.id))
    });
    let mut shown = 0;
    for actor in candidates {
        if shown == 6 {
            break;
        }
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
        let x = desired_x;
        let mut y = (anchor - row_height).max(5.0);
        // Keep each plate at its character's head. Crowded plates are omitted
        // instead of being moved onto unrelated characters or empty floor.
        if y + row_height > height - 5.0
            || occupied.iter().any(|(ox, oy, ow)| {
                x < ox + ow + 4.0 && x + w + 4.0 > *ox && (y - oy).abs() < row_height
            })
        {
            continue;
        }
        shown += 1;
        occupied.push((x, y, w));
        if actor.actor.friendly
            && let Some(marker) = actor.life.and_then(|life| markers.get(&life))
        {
            use verse_world::service::progression::Marker;
            let (text, color) = match marker {
                Marker::Available => ("!", [1.0, 0.82, 0.1, 1.0]),
                Marker::Active => ("?", [0.65, 0.65, 0.65, 1.0]),
                Marker::TurnIn => ("?", [1.0, 0.82, 0.1, 1.0]),
            };
            outlined(
                &mut ui,
                atlas_plate,
                x + (w - atlas_plate.measure(text)) * 0.5,
                y,
                text,
                color,
            );
            y += 24.0 * plate_scale;
        }
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
        let bar_color = if actor.actor.friendly {
            [0.15, 0.85, 0.25, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };
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
                bar_color,
            );
        } else {
            ui.rect(
                atlas,
                bx + 3.968 * plate_scale,
                y + 19.008 * plate_scale,
                fill,
                8.992 * plate_scale,
                bar_color,
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

/// Floating combat text reflects admitted health loss, including lethal hits.
pub fn damage_numbers(
    ui: &mut UiBatch,
    atlas: &Atlas,
    game: &super::play::Game,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
) {
    damage_numbers_from_values(
        ui,
        atlas,
        &game.damage_numbers,
        game.time,
        frame,
        heights,
        projection,
        width,
        height,
    );
}
fn combat_bursts(
    numbers: &[verse_world::play::DamageNumber],
    time: f32,
) -> Vec<(&verse_world::play::DamageNumber, i64)> {
    let mut bursts: BTreeMap<(u64, bool), (&verse_world::play::DamageNumber, i64)> =
        BTreeMap::new();
    for number in numbers {
        if !(0.0..1.35).contains(&(time - number.at)) {
            continue;
        }
        let burst = bursts
            .entry((number.actor, number.incoming))
            .or_insert((number, 0));
        burst.1 += i64::from(number.amount);
        if number.at > burst.0.at {
            burst.0 = number;
        }
    }
    bursts.into_values().collect()
}

/// Shares native floating text rendering with admitted remote damage values.
pub fn damage_numbers_from_values(
    ui: &mut UiBatch,
    atlas: &Atlas,
    numbers: &[verse_world::play::DamageNumber],
    time: f32,
    frame: &Frame,
    heights: &BTreeMap<String, f32>,
    projection: Mat4,
    width: f32,
    height: f32,
) {
    let font = atlas
        .font("combat")
        .layout_at_scale(720.0 / height)
        .unwrap();
    let mut bursts = combat_bursts(numbers, time);
    bursts.sort_by(|(a, _), (b, _)| {
        let distance = |number: &verse_world::play::DamageNumber| {
            frame
                .actors
                .iter()
                .find(|a| a.actor.id == number.actor)
                .map_or(number.position, |a| a.actor.position)
                .distance_squared(frame.target)
        };
        (!a.incoming)
            .cmp(&(!b.incoming))
            .then_with(|| distance(a).total_cmp(&distance(b)))
            .then_with(|| a.actor.cmp(&b.actor))
    });
    let mut occupied: Vec<[f32; 4]> = Vec::new();
    for (number, amount) in bursts {
        if occupied.len() == 8 {
            break;
        }
        let age = time - number.at;
        if !(0.0..1.35).contains(&age) {
            continue;
        }
        let actor = frame.actors.iter().find(|a| a.actor.id == number.actor);
        let head_height = actor.map_or(2.0, |a| {
            heights.get(&a.actor.model).copied().unwrap_or(2.0) * a.actor.scale * 0.9144
        });
        let anchor = actor.map_or(number.position, |a| a.actor.position)
            + glam::Vec3::Y * (head_height + 0.8);
        let clip = projection * anchor.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.1 || ndc.y.abs() > 1.1 {
            continue;
        }
        let alpha = ((1.35 - age) / 0.35).clamp(0.0, 1.0);
        let text = format!("-{amount}");
        let text_width = font.measure(&text);
        let x = (ndc.x * 0.5 + 0.5) * width - text_width * 0.5;
        let y = (0.5 - ndc.y * 0.5) * height - age * 55.0;
        let bounds = [x - 4., y - 4., x + text_width + 4., y + 32.];
        if occupied.iter().any(|other| {
            bounds[0] < other[2]
                && bounds[2] > other[0]
                && bounds[1] < other[3]
                && bounds[3] > other[1]
        }) {
            continue;
        }
        occupied.push(bounds);
        for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)] {
            ui.text(&font, x + dx, y + dy, &text, [0.0, 0.0, 0.0, alpha]);
        }
        let color = if number.incoming {
            [1.0, 0.15, 0.08, alpha]
        } else {
            [1.0, 0.88, 0.15, alpha]
        };
        ui.text(&font, x, y, &text, color);
    }
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
    fn shared_damage_text_matches_local_rendering_and_preserves_colors_and_expiry() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = verse_world::play::Game::new(scene).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        let mut frame = game.frame();
        let actor = frame.actors[0].actor.id;
        frame.actors[0].actor.position = glam::Vec3::ZERO;
        let heights = frame
            .actors
            .iter()
            .map(|a| (a.actor.model.clone(), 0.))
            .collect();
        game.damage_numbers = vec![verse_world::play::DamageNumber {
            actor,
            amount: 45,
            at: game.time - 0.2,
            position: glam::Vec3::ZERO,
            incoming: false,
            serial: 1,
        }];
        let atlas = Atlas::new(16.);
        let mut local = UiBatch::default();
        let mut remote = UiBatch::default();
        damage_numbers(
            &mut local,
            &atlas,
            &game,
            &frame,
            &heights,
            Mat4::IDENTITY,
            1280.,
            720.,
        );
        damage_numbers_from_values(
            &mut remote,
            &atlas,
            &game.damage_numbers,
            game.time,
            &frame,
            &heights,
            Mat4::IDENTITY,
            1280.,
            720.,
        );
        assert!(!local.vertices.is_empty());
        assert_eq!(
            format!("{:?}", local.vertices),
            format!("{:?}", remote.vertices)
        );
        assert!(remote.vertices.iter().any(|v| v.color[1] == 0.88));
        game.damage_numbers[0].incoming = true;
        let mut incoming = UiBatch::default();
        damage_numbers_from_values(
            &mut incoming,
            &atlas,
            &game.damage_numbers,
            game.time,
            &frame,
            &heights,
            Mat4::IDENTITY,
            1280.,
            720.,
        );
        assert!(incoming.vertices.iter().any(|v| v.color[1] == 0.15));
        let mut expired = UiBatch::default();
        damage_numbers_from_values(
            &mut expired,
            &atlas,
            &game.damage_numbers,
            game.time + 2.,
            &frame,
            &heights,
            Mat4::IDENTITY,
            1280.,
            720.,
        );
        assert!(expired.vertices.is_empty());
    }
    #[test]
    fn combat_bursts_sum_health_loss_without_mixing_incoming_or_expired_hits() {
        let number = |actor, amount, at, incoming| verse_world::play::DamageNumber {
            actor,
            amount,
            at,
            incoming,
            position: glam::Vec3::ZERO,
            serial: 0,
        };
        let values = [
            number(1, 45, 4.8, false),
            number(1, 45, 4.9, false),
            number(1, 20, 4.9, true),
            number(1, 900, 2., false),
            number(1, 900, 6., false),
            number(2, 13, 4.9, false),
        ];
        let bursts = combat_bursts(&values, 5.);
        assert_eq!(bursts.len(), 3);
        assert!(bursts.iter().any(|(number, total)| number.actor == 1
            && !number.incoming
            && *total == 90
            && number.at == 4.9));
        assert!(
            bursts
                .iter()
                .any(|(number, total)| number.actor == 1 && number.incoming && *total == 20)
        );
        assert!(combat_bursts(&values, 8.).is_empty());
    }

    #[test]
    fn crowded_plates_stay_anchored_and_selected_target_has_priority() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut frame = scene.frame(scene.cut_at + 1.);
        frame.yell = None;
        let prototype = frame
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .clone();
        frame.actors = (0..40)
            .map(|id| {
                let mut actor = prototype.clone();
                actor.actor.id = id + 100;
                actor.actor.position = glam::Vec3::ZERO;
                actor.actor.friendly = id == 39;
                actor.life = Some(verse_engine::core::LifeId {
                    instance: 1,
                    actor: id + 100,
                    generation: 0,
                });
                actor
            })
            .collect();
        let target = frame.actors.last().unwrap().life;
        let heights = frame
            .actors
            .iter()
            .map(|a| (a.actor.model.clone(), 2.))
            .collect();
        let atlas = Atlas::new(16.);
        let projection = Mat4::perspective_rh(1., 16. / 9., 0.1, 100.)
            * Mat4::look_at_rh(
                glam::Vec3::new(0., 3., -8.),
                glam::Vec3::Y * 2.,
                glam::Vec3::Y,
            );
        let draw = |frame: &Frame| {
            cinematic_with_focus(
                &atlas,
                frame,
                &heights,
                projection,
                1280.,
                720.,
                &BTreeMap::new(),
                glam::Vec3::ZERO,
                target,
            )
        };
        for distance in [0., 40.] {
            for actor in &mut frame.actors {
                actor.actor.position.z = distance;
            }
            let ui = draw(&frame);
            assert_eq!(
                ui.vertices
                    .iter()
                    .filter(|v| v.color == [0.15, 0.85, 0.25, 1.])
                    .count(),
                6
            );
            assert!(!ui.vertices.iter().any(|v| v.color == [1., 0., 0., 1.]));
        }
    }

    #[test]
    fn friendly_nameplates_are_green_and_dead_plates_are_hidden() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut frame = scene.frame(scene.cut_at + 1.);
        let heights = frame
            .actors
            .iter()
            .map(|a| (a.actor.model.clone(), 2.))
            .collect();
        let giver = frame.actors.iter_mut().find(|a| a.actor.id == 2).unwrap();
        giver.actor.friendly = true;
        giver.actor.position = glam::Vec3::ZERO;
        frame.actors.retain(|a| a.actor.id == 2);
        let atlas = Atlas::new(16.);
        let projection = Mat4::perspective_rh(1., 16. / 9., 0.1, 100.)
            * Mat4::look_at_rh(
                glam::Vec3::new(0., 3., -8.),
                glam::Vec3::Y * 2.,
                glam::Vec3::Y,
            );
        let draw = |frame: &Frame| cinematic(&atlas, frame, &heights, projection, 1280., 720.);
        let green = [0.15, 0.85, 0.25, 1.];
        let ui = draw(&frame);
        assert_eq!(ui.vertices.iter().filter(|v| v.color == green).count(), 6);
        assert!(!ui.vertices.iter().any(|v| v.color == [1., 0., 0., 1.]));
        frame.actors[0].health = 0;
        assert!(!draw(&frame).vertices.iter().any(|v| v.color == green));
        frame.actors[0].health = 100;
        frame.actors[0].actor.friendly = false;
        assert_eq!(
            draw(&frame)
                .vertices
                .iter()
                .filter(|v| v.color == [1., 0., 0., 1.])
                .count(),
            6
        );
    }

    #[test]
    fn quest_marker_draws_only_for_the_matching_living_friendly_life() {
        use verse_world::service::progression::Marker;
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual-quests.json"
        ))
        .unwrap();
        let mut frame = scene.frame(20.8);
        frame.actors.retain(|a| a.actor.id == 1_000_000);
        frame.actors[0].actor.position = glam::Vec3::ZERO;
        let life = verse_engine::core::LifeId {
            instance: 1,
            actor: 1_000_000,
            generation: 2,
        };
        frame.actors[0].life = Some(life);
        let heights = BTreeMap::from([("cultist".into(), 2.)]);
        let projection = Mat4::perspective_rh(1., 16. / 9., 0.1, 100.)
            * Mat4::look_at_rh(
                glam::Vec3::new(0., 3., -8.),
                glam::Vec3::Y * 2.,
                glam::Vec3::Y,
            );
        let atlas = Atlas::new(16.);
        let mut markers = BTreeMap::from([(life, Marker::Available)]);
        let draw = |frame: &Frame, markers: &BTreeMap<_, _>| {
            cinematic_with_markers(&atlas, frame, &heights, projection, 1280., 720., markers)
        };
        let gold = [1., 0.82, 0.1, 1.];
        assert!(
            draw(&frame, &markers)
                .vertices
                .iter()
                .any(|v| v.color == gold)
        );
        markers.insert(life, Marker::Active);
        assert!(
            draw(&frame, &markers)
                .vertices
                .iter()
                .any(|v| v.color == [0.65, 0.65, 0.65, 1.])
        );
        markers.insert(life, Marker::TurnIn);
        assert!(
            draw(&frame, &markers)
                .vertices
                .iter()
                .any(|v| v.color == gold)
        );
        frame.actors[0].life.as_mut().unwrap().generation += 1;
        assert!(
            !draw(&frame, &markers)
                .vertices
                .iter()
                .any(|v| v.color == gold)
        );
        frame.actors[0].life = Some(life);
        frame.actors[0].health = 0;
        assert!(
            !draw(&frame, &markers)
                .vertices
                .iter()
                .any(|v| v.color == gold)
        );
    }

    #[test]
    fn ritual_limits_hostile_bars_without_overlap_in_both_camera_shots() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/anthropic.json"
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
            assert!(red <= 6 * 6, "Too many nameplates at time {time}");
            assert_eq!(red % 6, 0, "Incomplete nameplate at time {time}");
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
    ((width - 430.0 * scale) * 0.5, height - 52.0 * scale, scale)
}
/// Classic action-bar hit regions share the drawing geometry with input handling.
pub fn action_at(x: f32, y: f32, width: f32, height: f32) -> Option<super::play::Ability> {
    let (left, top, scale) = bar_geometry(width, height);
    let x = (x - left) / scale - 8.0;
    let y = (y - top) / scale;
    if x < 0.0 || x % 42.0 >= 36.0 {
        return None;
    }
    let row = if (0.0..36.0).contains(&y) {
        &super::play::Ability::ALL
    } else if (-ROW_TWO_RISE..36.0 - ROW_TWO_RISE).contains(&y) {
        &super::play::Ability::ROW_TWO
    } else {
        return None;
    };
    row.get((x / 42.0).floor() as usize).copied()
}
/// The second row sits this many reference pixels above the first.
const ROW_TWO_RISE: f32 = 50.0;
/// Decorative chrome consumes pointer presses instead of selecting the world behind it.
pub fn chrome_at(x: f32, y: f32, width: f32, height: f32) -> bool {
    let (left, _, s) = bar_geometry(width, height);
    (x >= left
        && x <= left + 430.0 * s
        && (height - (60.0 + ROW_TWO_RISE) * s..height - 8.0 * s).contains(&y))
        || (y <= 104.0 * s && (x <= 213.0 * s || (250.0 * s..482.0 * s).contains(&x)))
}
fn respawn_rect(width: f32, height: f32) -> [f32; 4] {
    let s = height / 768.;
    [width * 0.5 - 90. * s, height * 0.43, 180. * s, 40. * s]
}
/// Shares the visible death button's bounds with pointer admission.
pub fn respawn_at(x: f32, y: f32, width: f32, height: f32) -> bool {
    let [left, top, w, h] = respawn_rect(width, height);
    x >= left && x < left + w && y >= top && y < top + h
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
fn shared_action_row(
    ui: &mut UiBatch,
    atlas: &Atlas,
    slots: &[verse_world::hud::Slot],
    width: f32,
    height: f32,
) {
    use super::play::Ability;
    let (left, y, s) = bar_geometry(width, height);
    let hotkey = atlas.font("hotkey").layout_at_scale(1.0 / s).unwrap();
    let image =
        |ui: &mut UiBatch,
         name: &str,
         x: f32,
         y: f32,
         w: f32,
         h: f32,
         uv: [f32; 4],
         color: [f32; 4]| ui.image_region(atlas, name, [x, y, w * s, h * s], uv, color);
    // A compact beveled tray holds only the chamber abilities.
    for (inset, color) in [
        (0.0, [0.08, 0.07, 0.06, 0.96]),
        (1.0, [0.46, 0.39, 0.24, 1.0]),
        (2.0, [0.19, 0.17, 0.13, 1.0]),
        (4.0, [0.035, 0.03, 0.025, 0.96]),
    ] {
        ui.rect(
            atlas,
            left + inset * s,
            y + (inset - 8.0) * s,
            (430.0 - inset * 2.0) * s,
            (52.0 - inset * 2.0) * s,
            color,
        );
    }
    for index in 0..Ability::ALL.len() {
        let x = left + (8.0 + index as f32 * 42.0) * s;
        if let Some(ability) = Ability::ALL.get(index) {
            let slot = &slots[index];
            let (ready, cd, total) = (slot.ready, slot.remaining, slot.duration);
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
        }
    }
}
fn shared_resources(
    ui: &mut UiBatch,
    atlas: &Atlas,
    resources: &verse_world::rules::Player,
    height: f32,
) {
    let s = height / 768.;
    let small = atlas.font("small").layout_at_scale(1.0 / s).unwrap();
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
    let health = resources.hp as f32 / resources.max_hp as f32;
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
        resources.mana as f32 / resources.max_mana as f32,
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
    let mana = format!("{} / {}", resources.mana, resources.max_mana);
    outlined(
        ui,
        &numbers,
        147.0 * s - numbers.measure(&mana) * 0.5,
        55.0 * s,
        &mana,
        [1.0; 4],
    );
    let hp = format!("{} / {}", resources.hp, resources.max_hp);
    outlined(
        ui,
        &numbers,
        147. * s - numbers.measure(&hp) * 0.5,
        44. * s,
        &hp,
        [1.; 4],
    );
}
fn shared_respawn(ui: &mut UiBatch, atlas: &Atlas, width: f32, height: f32) {
    let [x, y, w, h] = respawn_rect(width, height);
    ui.rect(
        atlas,
        x - 2.,
        y - 2.,
        w + 4.,
        h + 4.,
        [0.65, 0.48, 0.19, 1.],
    );
    ui.rect(atlas, x, y, w, h, [0.34, 0.035, 0.025, 0.98]);
    ui.rect(atlas, x + 2., y + 2., w - 4., 2., [0.7, 0.19, 0.12, 1.]);
    outlined(
        ui,
        atlas,
        width * 0.5 - atlas.measure("Respawn") * 0.5,
        y + (h - 18.0) * 0.5,
        "Respawn",
        [1., 0.82, 0.3, 1.],
    );
}
fn shared_cast(
    ui: &mut UiBatch,
    atlas: &Atlas,
    cast: &verse_world::play::Casting,
    time: f32,
    width: f32,
    height: f32,
) {
    let (_, y, s) = bar_geometry(width, height);
    let small = atlas.font("small").layout_at_scale(1.0 / s).unwrap();
    let progress = ((time - cast.started) / (cast.ends - cast.started).max(0.001)).clamp(0.0, 1.0);
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
}
/// Limits remote pointer actions to displayed shared-kit slots.
pub fn owned_action_at(
    hud: &verse_world::hud::Own,
    unlocked: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<super::play::Ability> {
    if !unlocked || hud.resources.hp == 0 {
        return None;
    }
    let ability = action_at(x, y, width, height)?;
    hud.slots
        .iter()
        .any(|s| s.ability == ability)
        .then_some(ability)
}
pub fn owned_respawn_at(
    hud: &verse_world::hud::Own,
    unlocked: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> bool {
    unlocked && hud.resources.hp == 0 && respawn_at(x, y, width, height)
}
/// Draws authenticated owned HUD data through the same native primitives as local play.
pub fn owned_hud(
    ui: &mut UiBatch,
    atlas: &Atlas,
    hud: &verse_world::hud::Own,
    frame: &Frame,
    unlocked: bool,
    width: f32,
    height: f32,
) -> Result<(), String> {
    hud.validate(hud.life.instance)?;
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.
        || height < 1.
        || !frame
            .actors
            .iter()
            .any(|p| p.life == Some(hud.life) && p.actor.model == "adventurer")
    {
        return Err("Owned HUD view or actor mismatch".into());
    }
    if !unlocked {
        return Ok(());
    }
    shared_action_row(ui, atlas, &hud.slots, width, height);
    shared_resources(ui, atlas, &hud.resources, height);
    if hud.resources.hp == 0 {
        shared_respawn(ui, atlas, width, height);
    }
    if let Some(cast) = &hud.casting {
        shared_cast(ui, atlas, cast, hud.time, width, height);
    }
    Ok(())
}
fn shared_target_frame(
    ui: &mut UiBatch,
    atlas: &Atlas,
    target: &verse_engine::director::ActorFrame,
    height: f32,
) {
    let s = height / 768.;
    let small = atlas.font("small").layout_at_scale(1.0 / s).unwrap();
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
/// Draws the same target frame for an admitted exact life; dead targets are hidden.
pub fn target_hud(
    ui: &mut UiBatch,
    atlas: &Atlas,
    frame: &Frame,
    life: verse_engine::core::LifeId,
    width: f32,
    height: f32,
) -> Result<(), String> {
    if !width.is_finite() || !height.is_finite() || width < 1. || height < 1. {
        return Err("Invalid target HUD viewport".into());
    }
    let target = frame
        .actors
        .iter()
        .find(|p| p.life == Some(life))
        .ok_or("Target HUD life is stale")?;
    if !target.visible || target.health == 0 {
        return Ok(());
    }
    if target.actor.health == 0 || target.health > target.actor.health {
        return Err("Invalid target HUD health".into());
    }
    shared_target_frame(ui, atlas, target, height);
    Ok(())
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
    let slots: Vec<_> = Ability::ALL
        .into_iter()
        .map(|ability| {
            let (ready, remaining, duration) = if let Some(spell) = ability.spell() {
                let gate = snapshot.abilities.iter().find(|a| a.id == spell).unwrap();
                (
                    gate.ready,
                    gate.cooldown_remaining,
                    game.cooldown_duration(spell),
                )
            } else if let Some(spell) = ability.utility() {
                let cd = game.controls.cooldown(spell, game.time);
                (
                    cd == 0. && snapshot.player.mana >= spell.cost(),
                    cd,
                    spell.cooldown(),
                )
            } else {
                (
                    game.time >= game.bow_ready,
                    (game.bow_ready - game.time).max(0.),
                    1.,
                )
            };
            verse_world::hud::Slot {
                ability,
                ready: ready && snapshot.player.hp > 0 && game.casting.is_none(),
                remaining,
                duration,
            }
        })
        .collect();
    shared_action_row(ui, atlas, &slots, width, height);
    // Row two: spells from the spell catalog, hotkeys Shift+1 to Shift+0.
    let row = y - ROW_TWO_RISE * s;
    for (inset, color) in [
        (0.0, [0.08, 0.07, 0.06, 0.9]),
        (1.0, [0.36, 0.31, 0.2, 1.0]),
        (3.0, [0.035, 0.03, 0.025, 0.9]),
    ] {
        ui.rect(
            atlas,
            left + inset * s,
            row + (inset - 6.0) * s,
            (430.0 - inset * 2.0) * s,
            (48.0 - inset * 2.0) * s,
            color,
        );
    }
    if verse_world::spells::feather_fall::reaction_available(game) {
        ui.text(
            atlas,
            left,
            row - 24. * s,
            "Feather Fall reaction: Shift+4",
            [0.7, 0.9, 1., 1.],
        );
    }
    for (index, ability) in Ability::ROW_TWO.iter().enumerate() {
        let x = left + (8.0 + index as f32 * 42.0) * s;
        let spell = ability.catalog();
        let (ready, cd, total) = match (ability, spell) {
            (Ability::Spell(slot), Some(spell)) => {
                let cd = game
                    .spells
                    .ready
                    .get(slot)
                    .map_or(0.0, |at| (at - game.time).max(0.0));
                (
                    cd == 0.0 && snapshot.player.mana >= spell.cost,
                    cd,
                    spell.cooldown.max(0.01),
                )
            }
            _ => (false, 0.0, 1.0),
        };
        image(
            ui,
            ability.icon(),
            x,
            row,
            36.0,
            36.0,
            [0.0, 1.0, 0.0, 1.0],
            if spell.is_none() {
                [0.35, 0.35, 0.35, 0.8]
            } else if ready || cd > 0.0 {
                [1.0; 4]
            } else {
                [0.5, 0.5, 1.0, 1.0]
            },
        );
        ui.cooldown(atlas, [x, row + s, 36.0 * s], cd / total);
        image(
            ui,
            "action-frame",
            x - 15.0 * s,
            row - 14.0 * s,
            66.0,
            66.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0; 4],
        );
        let key = format!("s{}", (index + 1) % 10);
        outlined(
            ui,
            &hotkey,
            x + 34.0 * s - hotkey.measure(&key),
            row + 2.0 * s,
            &key,
            [0.6, 0.6, 0.6, 1.0],
        );
    }
    shared_resources(ui, atlas, &snapshot.player, height);
    if let Some(target) = game
        .frame()
        .actors
        .iter()
        .find(|p| p.actor.id == game.selected && p.visible && p.health > 0)
    {
        shared_target_frame(ui, atlas, target, height);
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
    if snapshot.player.hp == 0 {
        shared_respawn(ui, atlas, width, height);
    }
    if let Some(cast) = &game.casting {
        shared_cast(ui, atlas, cast, game.time, width, height);
    } else if let Some(ability) = hover {
        let cost = ability
            .spell()
            .and_then(|spell| snapshot.abilities.iter().find(|a| a.id == spell))
            .map_or_else(
                || {
                    ability
                        .utility()
                        .map(|s| s.cost())
                        .or(ability.catalog().map(|s| s.cost))
                        .unwrap_or(0)
                },
                |a| a.cost,
            );
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
    fn exact_target_frame_reuses_portraits_health_and_hides_dead_or_stale_lives() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = verse_world::play::Game::new(scene).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        let mut frame = game.frame();
        let index = frame
            .actors
            .iter()
            .position(|p| p.actor.model == "claude")
            .unwrap();
        let life = frame.actors[index].life.unwrap();
        let atlas = super::super::original::atlas().unwrap();
        for height in [720., 1080.] {
            frame.actors[index].health = 150000;
            let mut shared = UiBatch::default();
            shared_target_frame(&mut shared, &atlas, &frame.actors[index], height);
            let mut remote = UiBatch::default();
            target_hud(&mut remote, &atlas, &frame, life, 1920., height).unwrap();
            assert!(!remote.vertices.is_empty());
            assert_eq!(
                format!("{:?}", shared.vertices),
                format!("{:?}", remote.vertices)
            );
            assert!(
                remote
                    .vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|p| p.is_finite()))
            );
            frame.actors[index].health = 0;
            let mut dead = UiBatch::default();
            target_hud(&mut dead, &atlas, &frame, life, 1920., height).unwrap();
            assert!(dead.vertices.is_empty());
            let mut stale = UiBatch::default();
            assert!(
                target_hud(
                    &mut stale,
                    &atlas,
                    &frame,
                    verse_engine::core::LifeId {
                        generation: life.generation + 1,
                        ..life
                    },
                    1920.,
                    height
                )
                .is_err()
            );
            assert!(stale.vertices.is_empty());
        }
    }

    #[test]
    fn owned_hud_uses_shared_icons_resources_cast_and_respawn_geometry() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = verse_world::play::Game::new(scene).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        let hud = game.player_hud(game.player_life()).unwrap();
        let frame = game.frame();
        let atlas = super::super::original::atlas().unwrap();
        for (width, height) in [(1280., 720.), (1920., 1080.)] {
            let mut row = UiBatch::default();
            shared_action_row(&mut row, &atlas, &hud.slots, width, height);
            let mut ui = UiBatch::default();
            owned_hud(&mut ui, &atlas, &hud, &frame, true, width, height).unwrap();
            assert_eq!(
                format!("{:?}", row.vertices),
                format!("{:?}", &ui.vertices[..row.vertices.len()])
            );
            assert!(ui.vertices.len() > row.vertices.len());
            assert!(
                ui.vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|p| p.is_finite()))
            );
            let (left, top, scale) = bar_geometry(width, height);
            assert_eq!(
                owned_action_at(
                    &hud,
                    true,
                    left + 20. * scale,
                    top + 15. * scale,
                    width,
                    height
                ),
                Some(super::super::play::Ability::Bow)
            );
            assert!(
                owned_action_at(
                    &hud,
                    true,
                    left + 20. * scale,
                    top - ROW_TWO_RISE * scale + 15. * scale,
                    width,
                    height
                )
                .is_none()
            );
            let mut hidden = UiBatch::default();
            owned_hud(&mut hidden, &atlas, &hud, &frame, false, width, height).unwrap();
            assert!(hidden.vertices.is_empty());
            let mut dead = hud.clone();
            dead.resources.hp = 0;
            for s in &mut dead.slots {
                s.ready = false;
            }
            let [x, y, w, h] = respawn_rect(width, height);
            assert!(owned_respawn_at(
                &dead,
                true,
                x + w * 0.5,
                y + h * 0.5,
                width,
                height
            ));
            assert!(!owned_respawn_at(
                &hud,
                true,
                x + w * 0.5,
                y + h * 0.5,
                width,
                height
            ));
            assert!(
                owned_action_at(
                    &dead,
                    true,
                    left + 20. * scale,
                    top + 15. * scale,
                    width,
                    height
                )
                .is_none()
            );
            let mut death = UiBatch::default();
            owned_hud(&mut death, &atlas, &dead, &frame, true, width, height).unwrap();
            assert!(
                death
                    .vertices
                    .iter()
                    .any(|v| v.color == [0.34, 0.035, 0.025, 0.98])
            );
            let mut casting = hud.clone();
            for s in &mut casting.slots {
                s.ready = false;
            }
            casting.casting = Some(verse_world::play::Casting {
                target_life: hud.life,
                aim: glam::Vec3::Z,
                ability: super::super::play::Ability::Fireball,
                started: hud.time - 1.,
                ends: hud.time + 2.,
                origin: glam::Vec3::ZERO,
                direction: glam::Vec3::Z,
            });
            let mut cast = UiBatch::default();
            owned_hud(&mut cast, &atlas, &casting, &frame, true, width, height).unwrap();
            assert!(
                cast.vertices
                    .iter()
                    .any(|v| v.color == [1., 0.65, 0.04, 1.])
            );
            let mut bad = hud.clone();
            bad.life.generation += 1;
            let mut output = UiBatch::default();
            assert!(owned_hud(&mut output, &atlas, &bad, &frame, true, width, height).is_err());
            assert!(output.vertices.is_empty());
        }
    }

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
            assert!(chrome_at(left + 215.0 * s, h - 20.0 * s, w, h));
            assert!(!chrome_at(left + 800.0 * s, h - 20.0 * s, w, h));
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
        for (i, ability) in super::super::play::Ability::ROW_TWO.iter().enumerate() {
            assert_eq!(
                action_at(
                    left + (8.0 + i as f32 * 42.0 + 20.0) * s,
                    top + (20.0 - ROW_TWO_RISE) * s,
                    1280.0,
                    720.0
                ),
                Some(*ability)
            );
        }
    }
}

#[cfg(test)]
mod respawn_tests {
    use super::*;
    #[test]
    fn respawn_hit_bounds_scale_with_the_visible_button() {
        for (w, h) in [(1280., 720.), (1920., 1080.), (800., 600.)] {
            let [x, y, width, height] = respawn_rect(w, h);
            assert!(respawn_at(x + width * 0.5, y + height * 0.5, w, h));
            assert!(!respawn_at(x - 1., y, w, h));
            assert!(!respawn_at(x + width, y, w, h));
            assert!(!respawn_at(x, y + height, w, h));
        }
    }
}

/// Projects a world point to overlay pixels; `None` behind the camera.
fn project(view_proj: Mat4, p: glam::Vec3, width: f32, height: f32) -> Option<[f32; 2]> {
    let clip = view_proj * p.extend(1.0);
    if clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some([(ndc.x + 1.0) * 0.5 * width, (1.0 - ndc.y) * 0.5 * height])
}

/// The spell playground's overlay: the spell's title, SRD line, save rolls,
/// measured displacements, displacement traces on the floor, and the
/// physics world's contact and joint debug lines.
#[allow(clippy::too_many_arguments)]
pub fn spell_panel(
    ui: &mut UiBatch,
    atlas: &Atlas,
    lines: &[String],
    game: &super::play::Game,
    view_proj: Mat4,
    width: f32,
    height: f32,
    replaying: bool,
) {
    use verse_world::playground::measure;
    for track in &game.spells.tracks {
        let Some((now, _)) = measure(game, track) else {
            continue;
        };
        let lift = glam::DVec3::Y * 0.03;
        let (Some(a), Some(b)) = (
            project(
                view_proj,
                (glam::DVec3::new(track.start.x, track.start.y.min(now.y), track.start.z) + lift)
                    .as_vec3(),
                width,
                height,
            ),
            project(
                view_proj,
                (glam::DVec3::new(now.x, track.start.y.min(now.y), now.z) + lift).as_vec3(),
                width,
                height,
            ),
        ) else {
            continue;
        };
        let color = if track.requested > 0.0 {
            [1.0, 0.85, 0.2, 0.95]
        } else {
            [0.4, 1.0, 0.5, 0.95]
        };
        ui.line(atlas, a, b, 3.0, color);
        ui.disc(atlas, a[0], a[1], 4.0, color);
    }
    for line in game.spells.world.debug_lines() {
        let color = match line.kind {
            physics::DebugKind::ContactImpulse => [1.0, 0.45, 0.1, 0.9],
            physics::DebugKind::Joint => [0.9, 0.3, 1.0, 0.9],
            // A joint at its limit, about to break.
            physics::DebugKind::Strained => [1.0, 0.15, 0.15, 1.0],
            _ => [0.3, 0.85, 1.0, 0.7],
        };
        if let (Some(a), Some(b)) = (
            project(view_proj, line.from.as_vec3(), width, height),
            project(view_proj, line.to.as_vec3(), width, height),
        ) {
            ui.line(atlas, a, b, 1.5, color);
        }
    }
    for (from, to, color) in verse_world::spells::wind_wall::guide_lines(game)
        .into_iter()
        .chain(verse_world::spells::reverse_gravity::guide_lines(game))
    {
        if let (Some(a), Some(b)) = (
            project(view_proj, from, width, height),
            project(view_proj, to, width, height),
        ) {
            ui.line(atlas, a, b, 2.0, color);
        }
    }
    let font = atlas.font("numbers");
    let (x, mut y) = (16.0, 112.0);
    let panel_width = lines.iter().map(|l| font.measure(l)).fold(
        atlas.measure(lines.first().map_or("", |s| s.as_str())),
        f32::max,
    ) + 20.0;
    let panel_height = 30.0 + lines.len() as f32 * 17.0;
    ui.rect(
        atlas,
        x - 8.0,
        y - 6.0,
        panel_width,
        panel_height,
        [0.0, 0.0, 0.0, 0.45],
    );
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            outlined(ui, atlas, x, y, line, [1.0, 0.82, 0.3, 1.0]);
            y += 26.0;
            continue;
        }
        let color = if i == 2 && replaying {
            [1.0, 0.4, 0.3, 1.0]
        } else if i == 1 {
            [0.85, 0.85, 1.0, 1.0]
        } else if line.contains("succeeds") || line.contains("no push") {
            [0.55, 1.0, 0.6, 1.0]
        } else if line.contains("fails") || line.contains("pushed") {
            [1.0, 0.85, 0.45, 1.0]
        } else {
            [0.92, 0.92, 0.92, 1.0]
        };
        outlined(ui, font, x, y, line, color);
        y += 17.0;
    }
    if replaying {
        let banner = "SLOW MOTION 0.25x";
        outlined(
            ui,
            atlas.font("combat"),
            width * 0.5 - atlas.font("combat").measure(banner) * 0.5,
            40.0,
            banner,
            [1.0, 0.45, 0.3, 1.0],
        );
    }
}
