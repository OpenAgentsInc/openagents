//! In-world loading controls, a real-time spell hotbar, and the Physics Lab
//! knobs, with matching native semantics.
//!
//! The panel holds a caption of up to four lines (split on `\n`) above one
//! row of controls, or two rows when there are more than five.

use super::Intent;
use crate::ui::{self, Atlas, UiBatch};
use coder_ui::theme::Intensity;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Button {
    pub id: &'static str,
    pub label: String,
    pub action: Intent,
    pub enabled: bool,
    pub frame: [f32; 4],
    pub cooldown: f32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub visible: bool,
    pub caption: String,
    pub frame: [f32; 4],
    pub buttons: Vec<Button>,
    pub progress: Option<f32>,
    pub captured_pointers: Vec<u64>,
    /// Caption lines the panel has room for.
    #[serde(skip)]
    pub lines: usize,
}
/// Most controls in one row.
const ROW: usize = 5;
/// Most caption lines.
const LINES: usize = 4;
struct Contact {
    id: u64,
    origin: [f32; 2],
    valid: bool,
    action: Option<Intent>,
}
pub struct Hud {
    insets: [f32; 4],
    clearance: f32,
    contacts: Vec<Contact>,
}
impl Default for Hud {
    fn default() -> Self {
        Self {
            insets: [0.0; 4],
            clearance: 86.0,
            contacts: Vec::new(),
        }
    }
}
impl Hud {
    pub fn set_insets(&mut self, values: [f32; 4]) -> Result<(), String> {
        if values
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=256.0).contains(x))
        {
            return Err("Zone safe area exceeds its bounds".into());
        }
        if self.insets != values {
            self.clear_contacts();
            self.insets = values;
        }
        Ok(())
    }
    pub fn set_bottom_clearance(&mut self, value: f32) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=2048.0).contains(&value) {
            return Err("Zone control clearance exceeds its bounds".into());
        }
        if self.clearance != value {
            self.clear_contacts();
            self.clearance = value;
        }
        Ok(())
    }
    pub fn snapshot(&self, size: [f32; 2], zone: &super::Snapshot, enabled: bool) -> Snapshot {
        let [top, right, bottom, left] = self.insets;
        let width = (size[0] - left - right - 24.0).clamp(1.0, 420.0);
        let compact =
            zone.id == super::ZoneId::Everglade && zone.controls.len() == 5 && zone.error.is_none();
        let caption = if compact {
            ""
        } else {
            zone.error.as_deref().unwrap_or(&zone.caption)
        };
        let lines = caption.split('\n').count().clamp(2, LINES);
        let rows = if zone.controls.len() > ROW { 2 } else { 1 };
        let per_row = zone.controls.len().div_ceil(rows).max(1);
        // Two lines and one row keep the original 102-point panel.
        let caption_height = if compact {
            0.0
        } else {
            12.0 + 16.0 * lines as f32
        };
        let height = caption_height + 8.0 + 50.0 * rows as f32;
        let x = left + (size[0] - left - right - width) * 0.5;
        let y = size[1] - bottom - self.clearance - height - 8.0;
        let visible = enabled && !zone.controls.is_empty() && width >= 240.0 && y > top + 12.0;
        let count = per_row as f32;
        let buttons = zone
            .controls
            .iter()
            .enumerate()
            .map(|(i, c)| Button {
                id: c.id,
                label: c.label.clone(),
                action: c.action,
                enabled: visible && c.enabled,
                cooldown: zone
                    .combat
                    .as_ref()
                    .and_then(|combat| {
                        combat.abilities.iter().find(|a| {
                            matches!(
                                (a.id, c.action),
                                (verse_ruins::Spell::Firebolt, Intent::Firebolt)
                                    | (verse_ruins::Spell::MagicMissile, Intent::MagicMissile)
                                    | (verse_ruins::Spell::Fireball, Intent::Fireball)
                            )
                        })
                    })
                    .map_or(0.0, |a| a.cooldown_remaining),
                frame: [
                    x + 6.0 + (i % per_row) as f32 * (width - 12.0) / count,
                    y + caption_height + 8.0 + (i / per_row) as f32 * 50.0,
                    (width - 12.0) / count,
                    44.0,
                ],
            })
            .collect();
        Snapshot {
            visible,
            caption: caption
                .chars()
                .take(if zone.error.is_some() { 180 } else { 360 })
                .collect(),
            frame: [x, y, width, height],
            buttons,
            progress: (zone.state == super::LoadState::Loading).then_some(zone.progress),
            captured_pointers: self.contacts.iter().map(|c| c.id).collect(),
            lines,
        }
    }
    pub fn captured(&self, id: u64) -> bool {
        self.contacts.iter().any(|c| c.id == id)
    }
    pub fn clear_contacts(&mut self) {
        self.contacts.clear();
    }
    pub fn down(&mut self, id: u64, at: [f32; 2], snapshot: &Snapshot) -> bool {
        if !snapshot.visible || !inside(snapshot.frame, at) {
            return false;
        }
        if self.contacts.len() < 8 && !self.captured(id) {
            let action = snapshot
                .buttons
                .iter()
                .find(|b| b.enabled && inside(b.frame, at))
                .map(|b| b.action);
            self.contacts.push(Contact {
                id,
                origin: at,
                valid: self.contacts.is_empty(),
                action,
            });
            if self.contacts.len() > 1 {
                for c in &mut self.contacts {
                    c.valid = false;
                }
            }
        }
        true
    }
    pub fn moved(&mut self, id: u64, at: [f32; 2]) {
        if let Some(c) = self.contacts.iter_mut().find(|c| c.id == id) {
            c.valid &= (at[0] - c.origin[0]).hypot(at[1] - c.origin[1]) <= 10.0;
        }
    }
    pub fn up(&mut self, id: u64, at: [f32; 2], cancelled: bool) -> Option<Intent> {
        self.moved(id, at);
        let index = self.contacts.iter().position(|c| c.id == id)?;
        let c = self.contacts.remove(index);
        (!cancelled && c.valid).then_some(c.action).flatten()
    }
    pub fn draw(&self, atlas: &Atlas, snapshot: &Snapshot, scale: f32) -> UiBatch {
        let mut ui = UiBatch::default();
        if !snapshot.visible {
            return ui;
        }
        let [x, y, w, h] = snapshot.frame;
        let full = ui::amber(Intensity::Full, 1.0);
        let half = ui::amber(Intensity::Half, 1.0);
        ui.rect(atlas, x, y, w, h, ui::field(0.95));
        ui.frame(atlas, x, y, w, h, 1.0, half);
        for (row, line) in snapshot
            .caption
            .split('\n')
            .flat_map(|part| atlas.wrap(part, w - 16.0))
            .take(snapshot.lines.max(2))
            .enumerate()
        {
            ui.text(atlas, x + 8.0, y + 7.0 + row as f32 * 16.0, &line, full);
        }
        if let Some(progress) = snapshot.progress {
            ui.rect(
                atlas,
                x + 6.0,
                y + 12.0 + 16.0 * snapshot.lines.max(2) as f32,
                (w - 12.0) * progress.clamp(0.0, 1.0),
                2.0,
                full,
            );
        }
        for b in &snapshot.buttons {
            let [bx, by, bw, bh] = b.frame;
            let color = if b.enabled { full } else { half };
            if b.cooldown > 0.0 {
                let duration = match b.action {
                    Intent::Fireball => 2.0,
                    Intent::MagicMissile => 1.5,
                    _ => 0.5,
                };
                ui.rect(
                    atlas,
                    bx + 1.0,
                    by + 1.0,
                    (bw - 4.0) * (b.cooldown / duration).clamp(0.0, 1.0),
                    bh - 2.0,
                    ui::amber(Intensity::Half, 0.25),
                );
            }
            ui.frame(atlas, bx, by, bw - 2.0, bh, 1.0, color);
            let limit = ((bw - 8.0) / atlas.advance).floor().max(1.0) as usize;
            let start = ui.vertices.len();
            let text = if matches!(
                b.action,
                Intent::Jump | Intent::Sprint | Intent::Levitate | Intent::Rise | Intent::Lower
            ) {
                b.label.clone()
            } else {
                b.label.chars().take(limit).collect::<String>()
            };
            ui.text(
                atlas,
                bx + 4.0,
                by + if b.cooldown > 0.0 { 6.0 } else { 15.0 },
                &text,
                color,
            );
            let fit = ((bw - 8.0) / (text.chars().count().max(1) as f32 * atlas.advance)).min(1.0);
            for vertex in &mut ui.vertices[start..] {
                vertex.pos[0] = bx + 4.0 + (vertex.pos[0] - bx - 4.0) * fit;
            }
            if b.cooldown > 0.0 {
                ui.text(
                    atlas,
                    bx + 4.0,
                    by + 25.0,
                    &format!("{:.1}s", b.cooldown),
                    half,
                );
            }
        }
        for v in &mut ui.vertices {
            v.pos[0] *= scale;
            v.pos[1] *= scale;
        }
        ui
    }
}
fn inside([x, y, w, h]: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= x && p[0] <= x + w && p[1] >= y && p[1] <= y + h
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_frame_layout_keeps_a_pressed_button() {
        let mut world = super::super::Snapshot::default();
        world.controls.push(super::super::Control {
            id: "enter",
            label: "Enter".into(),
            action: Intent::Enter,
            enabled: true,
        });
        let mut hud = Hud::default();
        hud.set_bottom_clearance(24.0).unwrap();
        let snapshot = hud.snapshot([800.0, 600.0], &world, true);
        let [x, y, _, _] = snapshot.buttons[0].frame;
        let at = [x + 12.0, y + 12.0];
        assert!(hud.down(1, at, &snapshot));
        hud.set_bottom_clearance(24.0).unwrap();
        assert_eq!(hud.up(1, at, false), Some(Intent::Enter));
        assert!(hud.down(2, at, &snapshot));
        hud.set_bottom_clearance(30.0).unwrap();
        assert_eq!(hud.up(2, at, false), None);
    }

    #[test]
    fn cancelled_or_multitouch_buttons_do_not_activate() {
        let s = Snapshot {
            visible: true,
            caption: String::new(),
            frame: [0.0, 0.0, 300.0, 100.0],
            buttons: vec![Button {
                id: "enter",
                label: "Enter".into(),
                action: Intent::Enter,
                enabled: true,
                frame: [0.0, 0.0, 100.0, 50.0],
                cooldown: 0.0,
            }],
            progress: None,
            captured_pointers: vec![],
            lines: 2,
        };
        let mut h = Hud::default();
        h.down(1, [10.0, 10.0], &s);
        h.down(2, [20.0, 10.0], &s);
        assert_eq!(h.up(1, [10.0, 10.0], false), None);
        assert_eq!(h.up(2, [20.0, 10.0], false), None);
        h.down(3, [10.0, 10.0], &s);
        assert_eq!(h.up(3, [10.0, 10.0], true), None);
        h.down(4, [10.0, 10.0], &s);
        assert_eq!(h.up(4, [10.0, 10.0], false), Some(Intent::Enter));
    }
}
