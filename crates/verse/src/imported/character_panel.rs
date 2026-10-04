//! Read-only character windows over authenticated inventory presentation.
use crate::ui::{Atlas, UiBatch};
use verse_world::service::wire::Inventory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Inventory,
    Quests,
}
#[derive(Default)]
pub struct Panel {
    pub kind: Option<Kind>,
    page: usize,
    claim: Option<u64>,
}
fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}
fn geometry(width: f32, height: f32) -> Option<[f32; 4]> {
    if !width.is_finite() || !height.is_finite() || width < 280. || height < 400. {
        return None;
    }
    Some([
        (width - 426.).max(16.),
        140.,
        (width - 32.).min(410.),
        (height - 230.).min(424.),
    ])
}
fn rows(rect: [f32; 4]) -> usize {
    ((rect[3] - 122.) / 44.).floor().max(1.) as usize
}
fn icon(ui: &mut UiBatch, atlas: &Atlas, kind: Kind, x: f32, y: f32) {
    ui.rect(atlas, x, y, 28., 28., [0.026, 0.020, 0.015, 1.]);
    match kind {
        Kind::Inventory => {
            ui.disc(atlas, x + 14., y + 17., 11., [0.28, 0.075, 0.009, 1.]);
            ui.disc(atlas, x + 14., y + 17., 8., [0.88, 0.20, 0.012, 1.]);
            ui.disc(atlas, x + 12., y + 11., 6., [1., 0.42, 0.035, 1.]);
            ui.disc(atlas, x + 14., y + 15., 4., [1., 0.82, 0.24, 1.]);
            ui.disc(atlas, x + 15., y + 15., 2., [1., 0.96, 0.65, 1.]);
            ui.line(
                atlas,
                [x + 8., y + 7.],
                [x + 7., y + 4.],
                1.5,
                [1., 0.62, 0.12, 1.],
            );
            ui.line(
                atlas,
                [x + 20., y + 10.],
                [x + 22., y + 6.],
                1.5,
                [1., 0.62, 0.12, 1.],
            );
        }
        Kind::Quests => {
            ui.rect(atlas, x + 5., y + 3., 18., 22., [0.82, 0.67, 0.43, 1.]);
            ui.frame(atlas, x + 5., y + 3., 18., 22., 1., [0.41, 0.26, 0.12, 1.]);
            ui.rect(atlas, x + 3., y + 3., 22., 3., [0.93, 0.79, 0.52, 1.]);
            ui.rect(atlas, x + 4., y + 23., 21., 3., [0.62, 0.44, 0.25, 1.]);
            for (line, length) in [(0, 10.), (1, 8.), (2, 10.), (3, 6.)] {
                ui.rect(
                    atlas,
                    x + 8.,
                    y + 9. + line as f32 * 3.,
                    length,
                    1.,
                    [0.38, 0.24, 0.12, 1.],
                );
            }
        }
    }
}
impl Panel {
    pub fn toggle(&mut self, kind: Kind) {
        self.kind = if self.kind == Some(kind) {
            None
        } else {
            Some(kind)
        };
        self.page = 0;
        self.claim = None;
    }
    pub fn close(&mut self) -> bool {
        self.claim = None;
        self.kind.take().is_some()
    }
    pub fn take_claim(&mut self) -> Option<u64> {
        self.claim.take()
    }
    fn row_height(&self, inventory: Option<&Inventory>) -> f32 {
        if self.kind == Some(Kind::Quests) && inventory.is_some_and(|i| !i.quest_log.is_empty()) {
            56.
        } else {
            44.
        }
    }
    fn row_count(&self, inventory: Option<&Inventory>, rect: [f32; 4]) -> usize {
        ((rect[3] - 122.) / self.row_height(inventory))
            .floor()
            .max(1.) as usize
    }
    fn pages(&self, inventory: Option<&Inventory>, rect: [f32; 4]) -> usize {
        let count = inventory.map_or(0, |i| {
            if self.kind == Some(Kind::Inventory) {
                i.items.len()
            } else if !i.quest_log.is_empty() {
                i.quest_log.len()
            } else {
                i.quests.len()
            }
        });
        count.div_ceil(self.row_count(inventory, rect)).max(1)
    }
    pub fn contains(&self, point: [f32; 2], width: f32, height: f32) -> bool {
        self.kind.is_some() && geometry(width, height).is_some_and(|rect| inside(rect, point))
    }
    pub fn page(&mut self, next: bool, inventory: Option<&Inventory>, width: f32, height: f32) {
        if let Some(rect) = geometry(width, height) {
            self.page = if next {
                self.page.saturating_add(1)
            } else {
                self.page.saturating_sub(1)
            }
            .min(self.pages(inventory, rect) - 1);
        }
    }
    /// Consumes the whole panel, including blank rows, before world input dispatch.
    pub fn click(
        &mut self,
        point: [f32; 2],
        inventory: Option<&Inventory>,
        width: f32,
        height: f32,
    ) -> bool {
        if !self.contains(point, width, height) {
            return false;
        }
        let [x, y, w, h] = geometry(width, height).unwrap();
        if inside([x + w - 34., y + 10., 24., 24.], point) {
            self.close();
        } else if inside([x + 12., y + h - 36., 28., 24.], point) {
            self.page(false, inventory, width, height);
        } else if inside([x + w - 40., y + h - 36., 28., 24.], point) {
            self.page(true, inventory, width, height);
        } else if self.kind == Some(Kind::Quests) {
            if let Some(inventory) = inventory {
                for (index, quest) in inventory
                    .quest_log
                    .iter()
                    .skip(self.page * self.row_count(Some(inventory), [x, y, w, h]))
                    .take(self.row_count(Some(inventory), [x, y, w, h]))
                    .enumerate()
                {
                    if !quest.claimed
                        && quest.progress == quest.goal
                        && inside(
                            [x + w - 96., y + 82. + index as f32 * 56. + 23., 80., 23.],
                            point,
                        )
                    {
                        self.claim = Some(quest.id);
                        break;
                    }
                }
            }
        }
        true
    }
    fn quest_rows(&self, ui: &mut UiBatch, atlas: &Atlas, inventory: &Inventory, rect: [f32; 4]) {
        let [x, y, w, _] = rect;
        let font = atlas.font("numbers");
        let small = atlas.font("small");
        let gold = [0.86, 0.68, 0.30, 1.];
        for (index, quest) in inventory
            .quest_log
            .iter()
            .skip(self.page * self.row_count(Some(inventory), rect))
            .take(self.row_count(Some(inventory), rect))
            .enumerate()
        {
            let row = y + 82. + index as f32 * 56.;
            ui.rect(atlas, x + 12., row, w - 24., 52., [0.075, 0.055, 0.038, 1.]);
            ui.frame(
                atlas,
                x + 16.,
                row + 4.,
                32.,
                32.,
                1.,
                [0.39, 0.30, 0.16, 1.],
            );
            icon(ui, atlas, Kind::Quests, x + 18., row + 6.);
            let name = font
                .wrap(&quest.name, (w - 142.).max(60.))
                .into_iter()
                .next()
                .unwrap_or_default();
            ui.text(font, x + 58., row + 5., &name, gold);
            let progress = format!("{} / {}", quest.progress, quest.goal);
            ui.text(
                small,
                x + w - 20. - small.measure(&progress),
                row + 7.,
                &progress,
                [0.92, 0.90, 0.82, 1.],
            );
            let ready = quest.progress == quest.goal;
            ui.text(
                small,
                x + 58.,
                row + 24.,
                if quest.claimed {
                    "Completed"
                } else if ready {
                    "Ready to complete"
                } else {
                    "In progress"
                },
                if quest.claimed {
                    [0.5, 0.85, 0.5, 1.]
                } else {
                    gold
                },
            );
            let reward = format!(
                "Reward: {} XP{}",
                quest.experience,
                if quest.items.is_empty() {
                    ""
                } else {
                    " + items"
                }
            );
            ui.text(small, x + 58., row + 38., &reward, [0.92, 0.90, 0.82, 1.]);
            if ready && !quest.claimed {
                ui.rect(
                    atlas,
                    x + w - 96.,
                    row + 23.,
                    80.,
                    23.,
                    [0.24, 0.14, 0.035, 1.],
                );
                ui.frame(atlas, x + w - 96., row + 23., 80., 23., 1., gold);
                ui.text(
                    font,
                    x + w - 56. - font.measure("Claim") * 0.5,
                    row + 27.,
                    "Claim",
                    gold,
                );
            }
        }
    }
    pub fn draw(
        &mut self,
        ui: &mut UiBatch,
        atlas: &Atlas,
        inventory: Option<&Inventory>,
        width: f32,
        height: f32,
    ) {
        let (Some(kind), Some(rect)) = (self.kind, geometry(width, height)) else {
            return;
        };
        let [x, y, w, h] = rect;
        let font = atlas.font("numbers");
        let small = atlas.font("small");
        let gold = [0.86, 0.68, 0.30, 1.];
        let white = [0.92, 0.90, 0.82, 1.];
        ui.rect(atlas, x + 5., y + 7., w, h, [0., 0., 0., 0.55]);
        ui.rect(atlas, x, y, w, h, [0.035, 0.025, 0.019, 0.98]);
        ui.frame(atlas, x, y, w, h, 2., [0.43, 0.31, 0.14, 1.]);
        ui.frame(
            atlas,
            x + 4.,
            y + 4.,
            w - 8.,
            h - 8.,
            1.,
            [0.18, 0.13, 0.065, 1.],
        );
        ui.rect(
            atlas,
            x + 7.,
            y + 7.,
            w - 14.,
            36.,
            [0.13, 0.075, 0.036, 1.],
        );
        ui.text(
            atlas,
            x + 16.,
            y + 16.,
            if kind == Kind::Inventory {
                "Inventory"
            } else {
                "Quest log"
            },
            gold,
        );
        ui.rect(
            atlas,
            x + w - 34.,
            y + 10.,
            24.,
            24.,
            [0.32, 0.045, 0.027, 1.],
        );
        ui.frame(atlas, x + w - 34., y + 10., 24., 24., 1., gold);
        ui.text(font, x + w - 26., y + 15., "X", white);
        let Some(inventory) = inventory else {
            ui.text(font, x + 16., y + 60., "Loading character...", white);
            return;
        };
        ui.text(
            font,
            x + 16.,
            y + 54.,
            &format!("Level {}", inventory.level.level),
            gold,
        );
        let experience = inventory.level.next.map_or_else(
            || format!("Experience: {}", inventory.experience),
            |next| format!("{} / {next} XP", inventory.experience),
        );
        ui.text(
            font,
            x + w - 16. - font.measure(&experience),
            y + 54.,
            &experience,
            gold,
        );
        if let Some(next) = inventory.level.next {
            let fraction = (inventory.experience - inventory.level.start) as f64
                / (next - inventory.level.start) as f64;
            ui.rect(
                atlas,
                x + 16.,
                y + 73.,
                w - 32.,
                4.,
                [0.10, 0.065, 0.13, 1.],
            );
            ui.rect(
                atlas,
                x + 16.,
                y + 73.,
                (w - 32.) * fraction.clamp(0., 1.) as f32,
                4.,
                [0.48, 0.22, 0.68, 1.],
            );
        }
        let pages = self.pages(Some(inventory), rect);
        self.page = self.page.min(pages - 1);
        if kind == Kind::Quests && !inventory.quest_log.is_empty() {
            self.quest_rows(ui, atlas, inventory, rect);
        } else {
            let values = if kind == Kind::Inventory {
                &inventory.items
            } else {
                &inventory.quests
            };

            if values.is_empty() {
                ui.text(
                    font,
                    x + 16.,
                    y + 92.,
                    if kind == Kind::Inventory {
                        "Your inventory is empty."
                    } else {
                        "No recorded quest progress."
                    },
                    white,
                );
            }
            for (index, entry) in values
                .iter()
                .skip(self.page * rows(rect))
                .take(rows(rect))
                .enumerate()
            {
                let row = y + 82. + index as f32 * 44.;
                ui.rect(atlas, x + 12., row, w - 24., 40., [0.075, 0.055, 0.038, 1.]);
                ui.frame(
                    atlas,
                    x + 16.,
                    row + 4.,
                    32.,
                    32.,
                    1.,
                    [0.39, 0.30, 0.16, 1.],
                );
                icon(ui, atlas, kind, x + 18., row + 6.);
                let name = match (kind, entry.id) {
                    (Kind::Inventory, 1) => "Ritual ember".to_string(),
                    (Kind::Quests, 1) => "Disrupt the summoning".to_string(),
                    (Kind::Inventory, id) => format!("Item {id}"),
                    (Kind::Quests, id) => format!("Objective {id}"),
                };
                // Fit unknown content labels within the row without covering the count.
                let name = font
                    .wrap(&name, (w - 132.).max(60.))
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                ui.text(font, x + 58., row + 5., &name, gold);
                ui.text(
                    small,
                    x + 58.,
                    row + 24.,
                    if kind == Kind::Inventory {
                        "Collected"
                    } else {
                        "Progress"
                    },
                    white,
                );
                let count = entry.count.to_string();
                ui.text(
                    font,
                    x + w - 22. - font.measure(&count),
                    row + 13.,
                    &count,
                    white,
                );
            }
        }
        for bx in [x + 12., x + w - 40.] {
            ui.rect(atlas, bx, y + h - 36., 28., 24., [0.16, 0.10, 0.055, 1.]);
            ui.frame(atlas, bx, y + h - 36., 28., 24., 1., gold);
        }
        ui.text(font, x + 21., y + h - 32., "<", gold);
        ui.text(font, x + w - 31., y + h - 32., ">", gold);
        let page = format!("{} / {}", self.page + 1, pages);
        ui.text(
            small,
            x + (w - small.measure(&page)) * 0.5,
            y + h - 31.,
            &page,
            white,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use verse_world::service::{rewards::Entry, wire::Life};
    fn inventory() -> Inventory {
        Inventory {
            life: Life {
                instance: 1,
                actor: 14,
                generation: 0,
            },
            revision: 1,
            experience: 45,
            items: (1..=64).map(|id| Entry { id, count: 1 }).collect(),
            quests: vec![Entry { id: 1, count: 3 }],
            level: verse_world::service::progression::Level {
                level: 1,
                start: 0,
                next: None,
            },
            quest_log: vec![],
        }
    }
    #[test]
    fn panel_consumes_blank_rows_closes_and_pages_all_bounded_entries() {
        let mut panel = Panel::default();
        let inventory = inventory();
        assert!(!panel.click([1100., 200.], Some(&inventory), 1280., 720.));
        panel.toggle(Kind::Inventory);
        for _ in 0..100 {
            panel.page(true, Some(&inventory), 1280., 720.);
        }
        assert_eq!(panel.page, 10);
        assert!(panel.click([1100., 200.], Some(&inventory), 1280., 720.));
        let [x, y, w, _] = geometry(1280., 720.).unwrap();
        assert!(panel.click([x + w - 22., y + 22.], Some(&inventory), 1280., 720.));
        assert_eq!(panel.kind, None);
        panel.toggle(Kind::Quests);
        assert_eq!(panel.page, 0);
        assert!(panel.close());
        assert!(!panel.close());
    }
    #[test]
    fn quest_buttons_emit_only_ready_unclaimed_intents_without_changing_counters() {
        let mut data = inventory();
        data.quest_log = vec![verse_world::service::progression::Progress {
            id: 1,
            name: "Disrupt the summoning".into(),
            progress: 2,
            goal: 3,
            claimed: false,
            experience: 75,
            items: vec![],
        }];
        let mut panel = Panel::default();
        panel.toggle(Kind::Quests);
        let [x, y, w, _] = geometry(1280., 720.).unwrap();
        let point = [x + w - 56., y + 82. + 34.];
        assert!(panel.click(point, Some(&data), 1280., 720.));
        assert!(panel.take_claim().is_none());
        data.quest_log[0].progress = 3;
        assert!(panel.click(point, Some(&data), 1280., 720.));
        assert_eq!(panel.take_claim(), Some(1));
        assert_eq!(data.experience, 45);
        assert!(!data.quest_log[0].claimed);
        data.quest_log[0].claimed = true;
        assert!(panel.click(point, Some(&data), 1280., 720.));
        assert!(panel.take_claim().is_none());
    }
    #[test]
    #[ignore = "Explicit GPU acceptance capture"]
    fn capture_character_panels() {
        let output =
            std::env::var_os("VERSE_CHARACTER_PANEL_CAPTURE").expect("Explicit capture path");
        let dir = tempfile::tempdir().unwrap();
        let pack = super::super::original::generate(dir.path()).unwrap();
        let atlas = super::super::original::atlas().unwrap();
        let mut renderer =
            super::super::Renderer::new(pack, dir.path(), 2700, 1860, &atlas, &[]).unwrap();
        renderer.set_overlay_size(900., 620.);
        let eye = glam::Vec3::new(0., 3., 10.);
        let camera = crate::render::View {
            view_proj: glam::Mat4::perspective_rh(60f32.to_radians(), 900. / 620., 0.1, 100.)
                * glam::Mat4::look_at_rh(eye, glam::Vec3::ZERO, glam::Vec3::Y),
            eye,
        };
        for completed in [false, true] {
            let mut data = inventory();
            data.experience = if completed { 120 } else { 45 };
            data.items = vec![Entry {
                id: 1,
                count: if completed { 3 } else { 1 },
            }];
            data.level = verse_world::service::progression::Level {
                level: if completed { 2 } else { 1 },
                start: if completed { 100 } else { 0 },
                next: Some(if completed { 300 } else { 100 }),
            };
            data.quest_log = vec![verse_world::service::progression::Progress {
                id: 1,
                name: "Disrupt the summoning".into(),
                progress: 1,
                goal: 1,
                claimed: completed,
                experience: 75,
                items: vec![Entry { id: 1, count: 2 }],
            }];
            let mut ui = UiBatch::default();
            let mut panel = Panel::default();
            panel.toggle(Kind::Inventory);
            panel.draw(&mut ui, &atlas, Some(&data), 450., 620.);
            let mut quests = UiBatch::default();
            panel.toggle(Kind::Quests);
            panel.draw(&mut quests, &atlas, Some(&data), 450., 620.);
            for vertex in &mut quests.vertices {
                vertex.pos[0] += 450.;
            }
            ui.vertices.extend(quests.vertices);
            let pixels = renderer
                .draw(
                    camera,
                    &[],
                    &ui,
                    &super::super::chamber::lighting(glam::Vec3::ZERO),
                )
                .unwrap();
            let path = if completed {
                std::path::PathBuf::from(&output)
            } else {
                std::path::PathBuf::from(&output).with_file_name("ready.png")
            };
            let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 2700, 1860);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
    }
}
