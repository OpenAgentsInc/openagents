use wgpui::components::hud::{Hotbar, HotbarSlot};
use wgpui::{Bounds, Size};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app_state::RenderState;
use crate::pane_registry::{
    HOTBAR_COMMAND_PALETTE_ICON, HOTBAR_COMMAND_PALETTE_SHORTCUT, HOTBAR_COMMAND_PALETTE_TOOLTIP,
    pane_kind_for_hotbar_slot, pane_spec_for_hotbar_slot, pane_specs,
};
use crate::pane_system::PaneController;
use crate::spark_wallet::SparkWalletCommand;

pub use crate::pane_registry::{
    HOTBAR_SLOT_COMMAND_PALETTE, HOTBAR_SLOT_NEW_CHAT, HOTBAR_SLOT_NOSTR_IDENTITY,
    HOTBAR_SLOT_SPARK_WALLET,
};

pub const HOTBAR_HEIGHT: f32 = 52.0;
pub const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;

pub fn configure_hotbar(hotbar: &mut Hotbar) {
    hotbar.set_item_size(HOTBAR_ITEM_SIZE);
    hotbar.set_padding(HOTBAR_PADDING);
    hotbar.set_gap(HOTBAR_ITEM_GAP);
    hotbar.set_corner_radius(8.0);
    hotbar.set_font_scale(1.0);
}

pub fn new_hotbar() -> Hotbar {
    let mut hotbar = Hotbar::new()
        .item_size(HOTBAR_ITEM_SIZE)
        .padding(HOTBAR_PADDING)
        .gap(HOTBAR_ITEM_GAP)
        .corner_radius(8.0)
        .font_scale(1.0);
    hotbar.set_items(build_hotbar_items());
    hotbar
}

pub fn hotbar_bounds(size: Size) -> Bounds {
    let slot_count = hotbar_display_order().len();
    let bar_width = HOTBAR_PADDING * 2.0
        + HOTBAR_ITEM_SIZE * slot_count as f32
        + HOTBAR_ITEM_GAP * (slot_count.saturating_sub(1) as f32);
    let bar_x = size.width * 0.5 - bar_width * 0.5;
    let bar_y = size.height - HOTBAR_FLOAT_GAP - HOTBAR_HEIGHT;
    Bounds::new(bar_x, bar_y, bar_width, HOTBAR_HEIGHT)
}

pub fn process_hotbar_clicks(state: &mut RenderState) -> bool {
    let mut changed = false;
    for slot in state.hotbar.take_clicked_slots() {
        if pane_kind_for_hotbar_slot(slot).is_some() || slot == HOTBAR_SLOT_COMMAND_PALETTE {
            activate_hotbar_slot(state, slot);
            changed = true;
        }
    }
    changed
}

pub fn activate_hotbar_slot(state: &mut RenderState, slot: u8) {
    state.hotbar.flash_slot(slot);
    if slot == HOTBAR_SLOT_COMMAND_PALETTE {
        state.command_palette.open();
        state.hotbar_flash_was_active = true;
        return;
    }

    if let Some(kind) = pane_kind_for_hotbar_slot(slot) {
        if kind == crate::app_state::PaneKind::SparkWallet {
            let was_open = state
                .panes
                .iter()
                .any(|pane| pane.kind == crate::app_state::PaneKind::SparkWallet);
            let _ = PaneController::create_for_kind(state, kind);
            if !was_open && let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::Refresh)
            {
                state.spark_wallet.last_error = Some(error);
            }
        } else {
            let _ = PaneController::create_for_kind(state, kind);
        }
    }
    state.hotbar_flash_was_active = true;
}

pub fn hotbar_slot_for_key(key: PhysicalKey) -> Option<u8> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Numpad1) => Some(HOTBAR_SLOT_NEW_CHAT),
        PhysicalKey::Code(KeyCode::Digit2 | KeyCode::Numpad2) => Some(HOTBAR_SLOT_NOSTR_IDENTITY),
        PhysicalKey::Code(KeyCode::Digit3 | KeyCode::Numpad3) => Some(HOTBAR_SLOT_SPARK_WALLET),
        _ => None,
    }
}

fn hotbar_display_order() -> Vec<u8> {
    let mut slots: Vec<u8> = pane_specs()
        .iter()
        .filter_map(|spec| spec.hotbar.map(|hotbar| hotbar.slot))
        .collect();
    slots.sort_unstable();
    slots.push(HOTBAR_SLOT_COMMAND_PALETTE);
    slots
}

fn build_hotbar_items() -> Vec<HotbarSlot> {
    hotbar_display_order()
        .into_iter()
        .map(|slot| {
            if slot == HOTBAR_SLOT_COMMAND_PALETTE {
                return HotbarSlot::new(
                    slot,
                    HOTBAR_COMMAND_PALETTE_ICON,
                    HOTBAR_COMMAND_PALETTE_TOOLTIP,
                )
                .shortcut(HOTBAR_COMMAND_PALETTE_SHORTCUT);
            }

            if let Some(spec) = pane_spec_for_hotbar_slot(slot)
                && let Some(hotbar) = spec.hotbar
            {
                let mut item = HotbarSlot::new(slot, hotbar.icon, hotbar.tooltip);
                if let Some(shortcut) = hotbar.shortcut {
                    item = item.shortcut(shortcut);
                }
                return item;
            }

            HotbarSlot::new(slot, "?", "Unknown")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        HOTBAR_SLOT_NEW_CHAT, HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_SPARK_WALLET,
        hotbar_slot_for_key,
    };
    use winit::keyboard::{KeyCode, PhysicalKey};

    #[test]
    fn numeric_hotbar_shortcuts_only_cover_one_two_three() {
        assert_eq!(
            hotbar_slot_for_key(PhysicalKey::Code(KeyCode::Digit1)),
            Some(HOTBAR_SLOT_NEW_CHAT)
        );
        assert_eq!(
            hotbar_slot_for_key(PhysicalKey::Code(KeyCode::Digit2)),
            Some(HOTBAR_SLOT_NOSTR_IDENTITY)
        );
        assert_eq!(
            hotbar_slot_for_key(PhysicalKey::Code(KeyCode::Digit3)),
            Some(HOTBAR_SLOT_SPARK_WALLET)
        );
        assert_eq!(
            hotbar_slot_for_key(PhysicalKey::Code(KeyCode::Digit4)),
            None
        );
        assert_eq!(
            hotbar_slot_for_key(PhysicalKey::Code(KeyCode::Numpad4)),
            None
        );
    }
}
