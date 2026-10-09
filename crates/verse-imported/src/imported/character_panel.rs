//! Character windows and typed intents over authenticated inventory presentation.
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
    accept: Option<(u64, verse_engine::core::LifeId)>,
    use_item: Option<u64>,
    equip: Option<u64>,
    gear: Option<(verse_world::service::equipment::Slot, u64)>,
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
fn outfit_icon(ui: &mut UiBatch, atlas: &Atlas, x: f32, y: f32) {
    ui.rect(atlas, x, y, 28., 28., [0.026, 0.020, 0.015, 1.]);
    let cloth = [0.13, 0.38, 0.36, 1.];
    let trim = [0.65, 0.52, 0.27, 1.];
    ui.rect(atlas, x + 8., y + 5., 12., 20., cloth);
    ui.rect(atlas, x + 3., y + 6., 6., 10., cloth);
    ui.rect(atlas, x + 19., y + 6., 6., 10., cloth);
    ui.rect(atlas, x + 11., y + 5., 6., 4., [0.026, 0.020, 0.015, 1.]);
    ui.frame(atlas, x + 8., y + 9., 12., 16., 1., trim);
    ui.rect(atlas, x + 8., y + 18., 12., 2., trim);
}
fn gear_icon(
    ui: &mut UiBatch,
    atlas: &Atlas,
    slot: verse_world::service::equipment::Slot,
    x: f32,
    y: f32,
) {
    ui.rect(atlas, x, y, 28., 28., [0.026, 0.020, 0.015, 1.]);
    match slot {
        verse_world::service::equipment::Slot::Head => {
            for i in 0..16 {
                let w = 2. + i as f32;
                ui.rect(
                    atlas,
                    x + 14. - w * 0.5,
                    y + 4. + i as f32,
                    w,
                    1.,
                    [0.5, 0.22, 0.68, 1.],
                );
            }
            ui.line(
                atlas,
                [x + 3., y + 21.],
                [x + 25., y + 21.],
                3.,
                [0.65, 0.32, 0.8, 1.],
            );
            ui.line(
                atlas,
                [x + 7., y + 17.],
                [x + 21., y + 17.],
                2.,
                [0.9, 0.7, 0.3, 1.],
            );
        }
        verse_world::service::equipment::Slot::MainHand => {
            ui.line(
                atlas,
                [x + 6., y + 23.],
                [x + 20., y + 7.],
                3.,
                [0.6, 0.38, 0.15, 1.],
            );
            ui.disc(atlas, x + 20., y + 7., 5., [0.1, 0.6, 0.8, 1.]);
            ui.disc(atlas, x + 19., y + 6., 2., [0.7, 1., 1., 1.]);
        }
    }
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
        self.accept = None;
        self.use_item = None;
        self.equip = None;
        self.gear = None;
    }
    pub fn close(&mut self) -> bool {
        self.claim = None;
        self.accept = None;
        self.use_item = None;
        self.equip = None;
        self.gear = None;
        self.kind.take().is_some()
    }
    pub fn take_accept(&mut self) -> Option<(u64, verse_engine::core::LifeId)> {
        self.accept.take()
    }
    pub fn take_claim(&mut self) -> Option<u64> {
        self.claim.take()
    }
    pub fn take_use(&mut self) -> Option<u64> {
        self.use_item.take()
    }
    pub fn take_equip(&mut self) -> Option<u64> {
        self.equip.take()
    }
    pub fn take_gear(&mut self) -> Option<(verse_world::service::equipment::Slot, u64)> {
        self.gear.take()
    }
    fn row_height(&self, inventory: Option<&Inventory>) -> f32 {
        if (self.kind == Some(Kind::Quests) && inventory.is_some_and(|i| !i.quest_log.is_empty()))
            || (self.kind == Some(Kind::Inventory)
                && inventory.is_some_and(|i| {
                    !i.catalog.items.is_empty()
                        || !i.outfits.outfits.is_empty()
                        || !i.equipment.gear.is_empty()
                }))
        {
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
        } else if self.kind == Some(Kind::Inventory) {
            if let Some(inventory) = inventory {
                for (index, entry) in inventory
                    .items
                    .iter()
                    .skip(self.page * self.row_count(Some(inventory), [x, y, w, h]))
                    .take(self.row_count(Some(inventory), [x, y, w, h]))
                    .enumerate()
                {
                    if let Ok(gear) = inventory.equipment.item(entry.id) {
                        if inside(
                            [
                                x + w - 96.,
                                y + 82. + index as f32 * self.row_height(Some(inventory)) + 23.,
                                80.,
                                23.,
                            ],
                            point,
                        ) {
                            self.gear = Some((
                                gear.slot,
                                if inventory.equipped.get(&gear.slot) == Some(&entry.id) {
                                    0
                                } else {
                                    entry.id
                                },
                            ));
                        }
                    }
                    if inventory.outfits.outfit(entry.id).is_ok()
                        && inside(
                            [
                                x + w - 96.,
                                y + 82. + index as f32 * self.row_height(Some(inventory)) + 23.,
                                80.,
                                23.,
                            ],
                            point,
                        )
                    {
                        self.equip = Some(if inventory.outfit == entry.id {
                            0
                        } else {
                            entry.id
                        });
                    }
                    if inventory.catalog.item(entry.id).is_ok()
                        && inside(
                            [
                                x + w - 96.,
                                y + 82. + index as f32 * self.row_height(Some(inventory)) + 23.,
                                80.,
                                23.,
                            ],
                            point,
                        )
                    {
                        self.use_item = Some(entry.id);
                    }
                }
            }
        } else if self.kind == Some(Kind::Quests) {
            if let Some(inventory) = inventory {
                for (index, quest) in inventory
                    .quest_log
                    .iter()
                    .skip(self.page * self.row_count(Some(inventory), [x, y, w, h]))
                    .take(self.row_count(Some(inventory), [x, y, w, h]))
                    .enumerate()
                {
                    if quest.available
                        && quest.interactable
                        && !quest.claimed
                        && (!quest.accepted || quest.progress == quest.goal)
                        && inside(
                            [x + w - 96., y + 82. + index as f32 * 56. + 23., 80., 23.],
                            point,
                        )
                    {
                        if quest.accepted {
                            self.claim = Some(quest.id);
                        } else if let Some(giver) = quest.giver_life {
                            self.accept = Some((quest.id, giver));
                        }
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
            let ready = quest.available && quest.accepted && quest.progress == quest.goal;
            ui.text(
                small,
                x + 58.,
                row + 24.,
                if quest.claimed {
                    "Completed"
                } else if !quest.available {
                    "Locked: complete prior quests"
                } else if !quest.accepted {
                    "Speak to the quest giver"
                } else if ready && !quest.interactable {
                    "Return to the quest giver"
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
            if quest.available && quest.interactable && !quest.claimed && (!quest.accepted || ready)
            {
                let action = if quest.accepted { "Claim" } else { "Accept" };
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
                    x + w - 56. - font.measure(action) * 0.5,
                    row + 27.,
                    action,
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
                .skip(self.page * self.row_count(Some(inventory), rect))
                .take(self.row_count(Some(inventory), rect))
                .enumerate()
            {
                let row = y + 82. + index as f32 * self.row_height(Some(inventory));
                ui.rect(
                    atlas,
                    x + 12.,
                    row,
                    w - 24.,
                    self.row_height(Some(inventory)) - 4.,
                    [0.075, 0.055, 0.038, 1.],
                );
                ui.frame(
                    atlas,
                    x + 16.,
                    row + 4.,
                    32.,
                    32.,
                    1.,
                    [0.39, 0.30, 0.16, 1.],
                );
                if kind == Kind::Inventory && inventory.equipment.item(entry.id).is_ok() {
                    gear_icon(
                        ui,
                        atlas,
                        inventory.equipment.item(entry.id).unwrap().slot,
                        x + 18.,
                        row + 6.,
                    );
                } else if kind == Kind::Inventory && inventory.outfits.outfit(entry.id).is_ok() {
                    outfit_icon(ui, atlas, x + 18., row + 6.);
                } else {
                    icon(ui, atlas, kind, x + 18., row + 6.);
                }
                let name = if kind == Kind::Inventory && inventory.equipment.item(entry.id).is_ok()
                {
                    inventory.equipment.item(entry.id).unwrap().name.clone()
                } else if kind == Kind::Inventory && inventory.outfits.outfit(entry.id).is_ok() {
                    inventory.outfits.outfit(entry.id).unwrap().name.clone()
                } else {
                    match (kind, entry.id) {
                        (Kind::Inventory, 1) => inventory
                            .catalog
                            .item(1)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|_| "Ritual ember".to_string()),
                        (Kind::Quests, 1) => "Disrupt the summoning".to_string(),
                        (Kind::Inventory, id) => inventory
                            .catalog
                            .item(id)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|_| format!("Item {id}")),
                        (Kind::Quests, id) => format!("Objective {id}"),
                    }
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
                        if inventory.catalog.item(entry.id).is_ok()
                            || inventory.outfits.outfit(entry.id).is_ok()
                            || inventory.equipment.item(entry.id).is_ok()
                        {
                            ""
                        } else {
                            "Collected"
                        }
                    } else {
                        "Progress"
                    },
                    white,
                );
                if kind == Kind::Inventory {
                    if let Ok(item) = inventory.catalog.item(entry.id) {
                        let description = format!("+{} HP / +{} MP", item.health, item.mana);
                        ui.text(small, x + 58., row + 24., &description, white);
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
                            x + w - 56. - font.measure("Use") * 0.5,
                            row + 27.,
                            "Use",
                            gold,
                        );
                    }
                }
                if kind == Kind::Inventory {
                    if let Ok(gear) = inventory.equipment.item(entry.id) {
                        let equipped = inventory.equipped.get(&gear.slot) == Some(&entry.id);
                        let action = if equipped { "Unequip" } else { "Equip" };
                        ui.text(
                            small,
                            x + 58.,
                            row + 24.,
                            &format!("{} +{} HP +{} MP", gear.slot.name(), gear.health, gear.mana),
                            white,
                        );
                        ui.text(
                            small,
                            x + 58.,
                            row + 38.,
                            if equipped {
                                "Equipped"
                            } else {
                                "Owned equipment"
                            },
                            gold,
                        );
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
                            x + w - 56. - font.measure(action) * 0.5,
                            row + 27.,
                            action,
                            gold,
                        );
                    }
                }
                if kind == Kind::Inventory && inventory.outfits.outfit(entry.id).is_ok() {
                    let equipped = inventory.outfit == entry.id;
                    let action = if equipped { "Unequip" } else { "Equip" };
                    ui.text(
                        small,
                        x + 58.,
                        row + 24.,
                        if equipped { "Equipped" } else { "Outfit" },
                        white,
                    );
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
                        x + w - 56. - font.measure(action) * 0.5,
                        row + 27.,
                        action,
                        gold,
                    );
                }
                let count = entry.count.to_string();
                ui.text(
                    font,
                    x + w - 22. - font.measure(&count),
                    row + if kind == Kind::Inventory
                        && (inventory.catalog.item(entry.id).is_ok()
                            || inventory.outfits.outfit(entry.id).is_ok()
                            || inventory.equipment.item(entry.id).is_ok())
                    {
                        5.
                    } else {
                        13.
                    },
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
            catalog: Default::default(),
            outfits: Default::default(),
            outfit: 0,
            equipment: Default::default(),
            equipped: Default::default(),
        }
    }
    #[test]
    fn gear_buttons_emit_owned_slot_changes_without_mutating_inventory() {
        let mut data = inventory();
        data.equipment = verse_world::service::equipment::Catalog {
            version: 1,
            gear: vec![verse_world::service::equipment::Gear {
                id: 1,
                name: "Ritual hat".into(),
                slot: verse_world::service::equipment::Slot::Head,
                model: "gear-hat".into(),
                offset: [0, 0, 230],
                health: 100,
                mana: 5,
            }],
        };
        let mut panel = Panel::default();
        panel.toggle(Kind::Inventory);
        let [x, y, w, _] = geometry(900., 620.).unwrap();
        let point = [x + w - 56., y + 82. + 34.];
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(
            panel.take_gear(),
            Some((verse_world::service::equipment::Slot::Head, 1))
        );
        assert!(data.equipped.is_empty());
        data.equipped
            .insert(verse_world::service::equipment::Slot::Head, 1);
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(
            panel.take_gear(),
            Some((verse_world::service::equipment::Slot::Head, 0))
        );
        assert_eq!(data.items[0].count, 1);
        panel.close();
        assert!(panel.take_gear().is_none());
    }
    #[test]
    fn outfit_buttons_emit_equipping_and_base_selection_without_local_changes() {
        let mut data = inventory();
        data.outfits = verse_world::service::outfits::Catalog {
            version: 1,
            outfits: vec![verse_world::service::outfits::Outfit {
                id: 1,
                name: "Ranger outfit".into(),
                model: "universal-male-ranger".into(),
            }],
        };
        let mut panel = Panel::default();
        panel.toggle(Kind::Inventory);
        let [x, y, w, _] = geometry(900., 620.).unwrap();
        let point = [x + w - 56., y + 116.];
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(panel.take_equip(), Some(1));
        assert_eq!(panel.take_use(), None);
        assert_eq!(data.outfit, 0);
        data.outfit = 1;
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(panel.take_equip(), Some(0));
        assert_eq!(data.outfit, 1);
        assert_eq!(data.items[0].count, 1);
        panel.close();
        assert_eq!(panel.take_equip(), None);
    }
    #[test]
    fn use_buttons_select_only_owned_authored_items_without_spending_locally() {
        let mut data = inventory();
        let mut panel = Panel::default();
        panel.toggle(Kind::Inventory);
        let [x, y, w, _] = geometry(900., 620.).unwrap();
        let point = [x + w - 56., y + 116.];
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(panel.take_use(), None);
        data.catalog = verse_world::service::items::Catalog {
            version: 1,
            items: vec![verse_world::service::items::Item {
                id: 1,
                name: "Recovery ember".into(),
                health: 45,
                mana: 5,
            }],
        };
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(panel.take_use(), Some(1));
        assert_eq!(data.items[0].count, 1);
        panel.page(true, Some(&data), 900., 620.);
        assert!(panel.click(point, Some(&data), 900., 620.));
        assert_eq!(panel.take_use(), None);
        panel.close();
        assert_eq!(panel.take_use(), None);
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
    fn giver_quest_buttons_require_authoritative_acceptance_and_interaction() {
        use verse_engine::core::LifeId;
        let mut data = inventory();
        let giver = LifeId {
            instance: data.life.instance,
            actor: 2,
            generation: 3,
        };
        data.quest_log = vec![verse_world::service::progression::Progress {
            cycle: 0,
            repeatable: false,
            dialogue: None,
            available: true,
            accepted: false,
            giver: Some(2),
            giver_life: Some(giver),
            interactable: false,
            id: 1,
            name: "Disrupt the ritual".into(),
            progress: 0,
            goal: 2,
            claimed: false,
            experience: 75,
            items: vec![],
        }];
        let mut panel = Panel::default();
        panel.toggle(Kind::Quests);
        let [x, y, w, _] = geometry(1280., 720.).unwrap();
        let point = [x + w - 56., y + 82. + 34.];
        panel.click(point, Some(&data), 1280., 720.);
        assert!(panel.take_accept().is_none());
        data.quest_log[0].interactable = true;
        panel.click(point, Some(&data), 1280., 720.);
        assert_eq!(panel.take_accept(), Some((1, giver)));
        assert!(panel.take_claim().is_none());
        assert!(!data.quest_log[0].accepted);
        data.quest_log[0].accepted = true;
        data.quest_log[0].progress = 2;
        data.quest_log[0].interactable = false;
        panel.click(point, Some(&data), 1280., 720.);
        assert!(panel.take_claim().is_none());
        data.quest_log[0].interactable = true;
        panel.click(point, Some(&data), 1280., 720.);
        assert_eq!(panel.take_claim(), Some(1));
    }
    #[test]
    fn quest_buttons_emit_only_ready_unclaimed_intents_without_changing_counters() {
        let mut data = inventory();
        data.quest_log = vec![verse_world::service::progression::Progress {
            cycle: 0,
            repeatable: false,
            dialogue: None,
            accepted: true,
            giver: None,
            giver_life: None,
            interactable: true,
            available: true,
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
        data.quest_log[0].available = false;
        assert!(panel.click(point, Some(&data), 1280., 720.));
        assert!(panel.take_claim().is_none());
        data.quest_log[0].available = true;
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
            data.catalog = verse_world::service::items::Catalog {
                version: 1,
                items: vec![verse_world::service::items::Item {
                    id: 1,
                    name: "Ritual recovery ember".into(),
                    health: 45,
                    mana: 5,
                }],
            };
            data.outfits = verse_world::service::outfits::Catalog {
                version: 1,
                outfits: vec![verse_world::service::outfits::Outfit {
                    id: 2,
                    name: "Ranger outfit".into(),
                    model: "universal-male-ranger".into(),
                }],
            };
            data.outfit = if completed { 2 } else { 0 };
            data.experience = if completed { 120 } else { 45 };
            data.items = vec![Entry {
                id: 1,
                count: if completed { 3 } else { 1 },
            }];
            data.items.push(Entry { id: 2, count: 1 });
            if std::env::var_os("VERSE_EQUIPMENT_PANEL").is_some() {
                data.equipment = verse_world::service::equipment::Catalog {
                    version: 1,
                    gear: vec![
                        verse_world::service::equipment::Gear {
                            id: 3,
                            name: "Ritual hat".into(),
                            slot: verse_world::service::equipment::Slot::Head,
                            model: "gear-hat".into(),
                            offset: [0, 0, 230],
                            health: 100,
                            mana: 0,
                        },
                        verse_world::service::equipment::Gear {
                            id: 4,
                            name: "Ritual wand".into(),
                            slot: verse_world::service::equipment::Slot::MainHand,
                            model: "gear-wand".into(),
                            offset: [0; 3],
                            health: 0,
                            mana: 10,
                        },
                    ],
                };
                data.items
                    .extend([Entry { id: 3, count: 1 }, Entry { id: 4, count: 1 }]);
                if completed {
                    data.equipped
                        .insert(verse_world::service::equipment::Slot::Head, 3);
                    data.equipped
                        .insert(verse_world::service::equipment::Slot::MainHand, 4);
                }
            }
            data.level = verse_world::service::progression::Level {
                level: if completed { 2 } else { 1 },
                start: if completed { 100 } else { 0 },
                next: Some(if completed { 300 } else { 100 }),
            };
            data.quest_log = vec![verse_world::service::progression::Progress {
                cycle: 0,
                repeatable: false,
                dialogue: None,
                accepted: true,
                giver: None,
                giver_life: None,
                interactable: true,
                available: true,
                id: 1,
                name: "Disrupt the summoning".into(),
                progress: 1,
                goal: 1,
                claimed: completed,
                experience: 75,
                items: vec![Entry { id: 1, count: 2 }],
            }];
            if std::env::var_os("VERSE_GIVER_PANEL").is_some() {
                let mut offer = data.quest_log[0].clone();
                offer.giver = Some(2);
                offer.giver_life = Some(verse_engine::core::LifeId {
                    instance: data.life.instance,
                    actor: 2,
                    generation: 0,
                });
                offer.accepted = completed;
                offer.progress = if completed { 2 } else { 0 };
                offer.goal = 2;
                offer.name = "Disrupt the ritual".into();
                let mut active = offer.clone();
                active.id = 2;
                active.name = "Gather ritual embers".into();
                active.accepted = true;
                active.claimed = false;
                active.progress = if completed { 2 } else { 1 };
                let mut return_quest = offer.clone();
                return_quest.id = 3;
                return_quest.name = "Secure the chamber".into();
                return_quest.claimed = false;
                return_quest.accepted = !completed;
                return_quest.progress = if completed { 0 } else { 2 };
                return_quest.interactable = completed;
                let mut locked = offer.clone();
                locked.id = 4;
                locked.name = "Seal the summoning circle".into();
                locked.available = false;
                locked.accepted = false;
                locked.claimed = false;
                locked.progress = 0;
                data.quest_log = vec![offer, active, return_quest, locked];
                data.validate(&Some(verse_world::service::wire::Control {
                    credit_step: 0,
                    world_step: 0,
                    life: data.life,
                    epoch: 1,
                    accepted_sequence: 0,
                    applied_movement: None,
                    dynamic: Vec::new(),
                }))
                .unwrap();
            }
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
