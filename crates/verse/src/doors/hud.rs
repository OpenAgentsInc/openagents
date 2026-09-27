//! Contextual demo-item controls shared by desktop and native world surfaces.

use serde::Serialize;

use super::{DemoItem, DoorId, DoorIntent, Doors};
use crate::ui::{self, Atlas, UiBatch};
use coder_ui::theme::Intensity;

#[derive(Clone, Debug, Serialize)]
pub struct Button {
    pub id: &'static str,
    pub label: &'static str,
    pub selected: bool,
    pub frame: [f32; 4],
    pub action: DoorIntent,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub visible: bool,
    pub door: Option<DoorId>,
    pub held: DemoItem,
    pub caption: String,
    pub frame: [f32; 4],
    pub buttons: Vec<Button>,
    pub captured_pointers: Vec<u64>,
}

struct Contact {
    id: u64,
    origin: [f32; 2],
    valid: bool,
    action: Option<DoorIntent>,
}

pub struct DoorHud {
    insets: [f32; 4],
    bottom_clearance: f32,
    contacts: Vec<Contact>,
}

impl Default for DoorHud {
    fn default() -> Self {
        Self {
            insets: [0.0; 4],
            bottom_clearance: 86.0,
            contacts: Vec::new(),
        }
    }
}

impl DoorHud {
    pub fn set_insets(&mut self, insets: [f32; 4]) -> Result<(), String> {
        if insets
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=256.0).contains(v))
        {
            return Err("Door safe area exceeds its bounds".into());
        }
        if self.insets != insets {
            self.clear_contacts();
            self.insets = insets;
        }
        Ok(())
    }

    /// Reserve space for native camera controls or the desktop chat area.
    pub fn set_bottom_clearance(&mut self, points: f32) -> Result<(), String> {
        if !points.is_finite() || !(0.0..=2048.0).contains(&points) {
            return Err("Door control clearance exceeds its bounds".into());
        }
        if self.bottom_clearance != points {
            self.clear_contacts();
            self.bottom_clearance = points;
        }
        Ok(())
    }

    pub fn snapshot(
        &self,
        size: [f32; 2],
        door: Option<DoorId>,
        doors: &Doors,
        visible: bool,
        notice: Option<&str>,
    ) -> Snapshot {
        let [top, right, bottom, left] = self.insets;
        let available = size[0] - left - right - 24.0;
        let width = available.clamp(1.0, 360.0);
        let height = 78.0;
        let x = left + (size[0] - left - right - width) * 0.5;
        let y = size[1] - bottom - self.bottom_clearance - height - 8.0;
        let visible = visible && door.is_some() && available >= 252.0 && y >= top + 12.0;
        let caption = notice.map_or_else(
            || {
                door.map_or(String::new(), |id| {
                    format!("{} · {}", id.label(), doors.state(id).caption())
                })
            },
            str::to_owned,
        );
        let actions = DemoItem::ALL
            .into_iter()
            .map(DoorIntent::Hold)
            .chain(door.map(DoorIntent::Reset));
        let buttons = actions
            .enumerate()
            .map(|(index, action)| {
                let (id, label, selected) = match action {
                    DoorIntent::Hold(item) => (item.as_str(), item.label(), doors.held() == item),
                    DoorIntent::Reset(_) => ("reset", "Reset", false),
                    DoorIntent::Tap(_) => unreachable!("the item strip has no door-tap button"),
                };
                Button {
                    id,
                    label,
                    selected,
                    action,
                    enabled: visible,
                    frame: [
                        x + 6.0 + index as f32 * (width - 12.0) / 5.0,
                        y + 28.0,
                        (width - 12.0) / 5.0,
                        44.0,
                    ],
                }
            })
            .collect();
        Snapshot {
            visible,
            door,
            held: doors.held(),
            caption: caption.chars().take(100).collect(),
            frame: [x, y, width, height],
            buttons,
            captured_pointers: self.contacts.iter().map(|contact| contact.id).collect(),
        }
    }

    pub fn captured(&self, id: u64) -> bool {
        self.contacts.iter().any(|contact| contact.id == id)
    }

    pub fn clear_contacts(&mut self) {
        self.contacts.clear();
    }

    pub fn down(&mut self, id: u64, point: [f32; 2], snapshot: &Snapshot) -> bool {
        if !snapshot.visible || !inside(snapshot.frame, point) {
            return false;
        }
        if self.contacts.len() < 8 && !self.captured(id) {
            let action = snapshot
                .buttons
                .iter()
                .find(|button| button.enabled && inside(button.frame, point))
                .map(|button| button.action);
            self.contacts.push(Contact {
                id,
                origin: point,
                valid: self.contacts.is_empty(),
                action,
            });
            if self.contacts.len() > 1 {
                for contact in &mut self.contacts {
                    contact.valid = false;
                }
            }
        }
        true
    }

    pub fn moved(&mut self, id: u64, point: [f32; 2]) {
        if let Some(contact) = self.contacts.iter_mut().find(|contact| contact.id == id) {
            contact.valid &=
                (point[0] - contact.origin[0]).hypot(point[1] - contact.origin[1]) <= 10.0;
        }
    }

    pub fn up(&mut self, id: u64, point: [f32; 2], cancelled: bool) -> Option<DoorIntent> {
        self.moved(id, point);
        let index = self.contacts.iter().position(|contact| contact.id == id)?;
        let contact = self.contacts.remove(index);
        if !cancelled && contact.valid {
            contact.action
        } else {
            None
        }
    }

    pub fn draw(&self, atlas: &Atlas, snapshot: &Snapshot, scale: f32) -> UiBatch {
        let mut ui = UiBatch::default();
        if !snapshot.visible {
            return ui;
        }
        let [x, y, w, h] = snapshot.frame;
        let half = ui::amber(Intensity::Half, 1.0);
        let full = ui::amber(Intensity::Full, 1.0);
        ui.rect(atlas, x, y, w, h, ui::field(0.985));
        ui.frame(atlas, x, y, w, h, 1.0, half);
        let limit = ((w - 16.0) / atlas.advance).max(1.0) as usize;
        let caption: String = snapshot.caption.chars().take(limit).collect();
        ui.text(atlas, x + 8.0, y + 7.0, &caption, full);
        for button in &snapshot.buttons {
            let [bx, by, bw, bh] = button.frame;
            let color = if button.selected { full } else { half };
            ui.frame(
                atlas,
                bx + 1.0,
                by,
                bw - 2.0,
                bh,
                if button.selected { 1.5 } else { 0.7 },
                color,
            );
            let limit = ((bw - 8.0) / atlas.advance).max(1.0) as usize;
            let label: String = button.label.chars().take(limit).collect();
            ui.text(
                atlas,
                bx + (bw - label.chars().count() as f32 * atlas.advance) * 0.5,
                by + 25.0,
                &label,
                color,
            );
            let cx = bx + bw * 0.5;
            let cy = by + 12.0;
            let points: Vec<[f32; 2]> = match button.action {
                DoorIntent::Hold(DemoItem::Prism) => vec![
                    [cx, cy - 7.0],
                    [cx + 7.0, cy],
                    [cx, cy + 7.0],
                    [cx - 7.0, cy],
                    [cx, cy - 7.0],
                ],
                DoorIntent::Hold(DemoItem::Ring) => (0..=12)
                    .map(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 12.0;
                        [cx + 7.0 * a.cos(), cy + 7.0 * a.sin()]
                    })
                    .collect(),
                DoorIntent::Hold(DemoItem::Bolt) => vec![
                    [cx + 4.0, cy - 8.0],
                    [cx - 4.0, cy],
                    [cx + 3.0, cy],
                    [cx - 4.0, cy + 8.0],
                ],
                DoorIntent::Hold(DemoItem::Empty) => vec![[cx - 6.0, cy], [cx + 6.0, cy]],
                DoorIntent::Tap(_) => Vec::new(),
                DoorIntent::Reset(_) => vec![
                    [cx + 6.0, cy + 5.0],
                    [cx - 6.0, cy + 5.0],
                    [cx - 6.0, cy - 5.0],
                    [cx + 4.0, cy - 5.0],
                    [cx, cy - 8.0],
                    [cx + 4.0, cy - 5.0],
                    [cx, cy - 2.0],
                ],
            };
            for pair in points.windows(2) {
                line(&mut ui, atlas, pair[0], pair[1], color);
            }
        }
        for vertex in &mut ui.vertices {
            vertex.pos[0] *= scale;
            vertex.pos[1] *= scale;
        }
        ui
    }
}

fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    point.iter().all(|v| v.is_finite())
        && point[0] >= rect[0]
        && point[0] <= rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] <= rect[1] + rect[3]
}

fn line(ui: &mut UiBatch, atlas: &Atlas, a: [f32; 2], b: [f32; 2], color: [f32; 4]) {
    let [dx, dy] = [b[0] - a[0], b[1] - a[1]];
    let length = dx.hypot(dy);
    if length < 0.001 {
        return;
    }
    let offset = ui.vertices.len();
    ui.rect(atlas, 0.0, -0.6, length, 1.2, color);
    for vertex in &mut ui.vertices[offset..] {
        let [x, y] = vertex.pos;
        vertex.pos = [
            a[0] + (x * dx - y * dy) / length,
            a[1] + (x * dy + y * dx) / length,
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_strip_respects_native_controls_and_capture_cancellation() {
        let doors = Doors::default();
        let mut hud = DoorHud::default();
        hud.set_insets([62.0, 0.0, 34.0, 0.0]).unwrap();
        let snapshot = hud.snapshot([393.0, 852.0], Some(DoorId::Spark), &doors, true, None);
        assert!(snapshot.visible);
        assert!(snapshot.frame[1] + snapshot.frame[3] <= 852.0 - 34.0 - 86.0);
        assert_eq!(snapshot.buttons.len(), 5);
        assert_eq!(
            serde_json::to_value(snapshot.buttons[1].action).unwrap(),
            serde_json::json!({"action":"door_hold","item":"ring"})
        );
        assert_eq!(
            serde_json::to_value(snapshot.buttons[4].action).unwrap(),
            serde_json::json!({"action":"door_reset","door":"spark"})
        );
        assert!(
            snapshot
                .buttons
                .iter()
                .all(|b| b.frame[2] >= 44.0 && b.frame[3] >= 44.0)
        );
        let button = &snapshot.buttons[1];
        let at = [button.frame[0] + 20.0, button.frame[1] + 20.0];
        assert!(hud.down(1, at, &snapshot));
        assert_eq!(hud.up(1, at, false), Some(button.action));
        hud.down(2, at, &snapshot);
        hud.moved(2, [at[0] + 20.0, at[1]]);
        assert_eq!(hud.up(2, at, false), None);
        hud.down(3, at, &snapshot);
        assert_eq!(hud.up(3, at, true), None);
        hud.down(4, at, &snapshot);
        hud.down(5, at, &snapshot);
        assert_eq!(hud.up(4, at, false), None);
        assert_eq!(hud.up(5, at, false), None);
        let hidden = hud.snapshot([393.0, 852.0], Some(DoorId::Spark), &doors, false, None);
        assert!(!hud.down(6, at, &hidden));
        assert!(
            !hud.snapshot([200.0, 852.0], Some(DoorId::Spark), &doors, true, None)
                .visible
        );
        assert!(
            !hud.snapshot([393.0, 852.0], None, &doors, true, None)
                .visible
        );
    }
}
