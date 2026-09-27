//! In-world loading controls and a real-time spell hotbar, with matching native semantics.

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
}
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
        let height = 102.0;
        let x = left + (size[0] - left - right - width) * 0.5;
        let y = size[1] - bottom - self.clearance - height - 8.0;
        let visible = enabled && !zone.controls.is_empty() && width >= 240.0 && y > top + 12.0;
        let count = zone.controls.len().max(1) as f32;
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
                                (verse_atlantis::Spell::Firebolt, Intent::Firebolt)
                                    | (verse_atlantis::Spell::MagicMissile, Intent::MagicMissile)
                                    | (verse_atlantis::Spell::Fireball, Intent::Fireball)
                            )
                        })
                    })
                    .map_or(0.0, |a| a.cooldown_remaining),
                frame: [
                    x + 6.0 + i as f32 * (width - 12.0) / count,
                    y + 52.0,
                    (width - 12.0) / count,
                    44.0,
                ],
            })
            .collect();
        Snapshot {
            visible,
            caption: zone
                .error
                .as_deref()
                .unwrap_or(&zone.caption)
                .chars()
                .take(180)
                .collect(),
            frame: [x, y, width, height],
            buttons,
            progress: (zone.state == super::LoadState::Loading).then_some(zone.progress),
            captured_pointers: self.contacts.iter().map(|c| c.id).collect(),
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
        for (row, line) in atlas
            .wrap(&snapshot.caption, w - 16.0)
            .iter()
            .take(2)
            .enumerate()
        {
            ui.text(atlas, x + 8.0, y + 7.0 + row as f32 * 16.0, line, full);
        }
        if let Some(progress) = snapshot.progress {
            ui.rect(
                atlas,
                x + 6.0,
                y + 44.0,
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
            ui.text(
                atlas,
                bx + 4.0,
                by + if b.cooldown > 0.0 { 6.0 } else { 15.0 },
                &b.label.chars().take(limit).collect::<String>(),
                color,
            );
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
