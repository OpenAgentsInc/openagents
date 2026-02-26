use wgpui::components::hud::{Hotbar, HotbarSlot};
use wgpui::{Bounds, Size};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app_state::RenderState;
use crate::pane_system::{create_empty_pane, create_nostr_identity_pane};

pub const HOTBAR_HEIGHT: f32 = 52.0;
pub const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;
pub const HOTBAR_SLOT_NEW_CHAT: u8 = 1;
pub const HOTBAR_SLOT_NOSTR_IDENTITY: u8 = 2;

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
        if slot == HOTBAR_SLOT_NEW_CHAT || slot == HOTBAR_SLOT_NOSTR_IDENTITY {
            activate_hotbar_slot(state, slot);
            changed = true;
        }
    }
    changed
}

pub fn activate_hotbar_slot(state: &mut RenderState, slot: u8) {
    state.hotbar.flash_slot(slot);
    match slot {
        HOTBAR_SLOT_NEW_CHAT => create_empty_pane(state),
        HOTBAR_SLOT_NOSTR_IDENTITY => create_nostr_identity_pane(state),
        _ => {}
    }
    state.hotbar_flash_was_active = true;
}

pub fn hotbar_slot_for_key(key: PhysicalKey) -> Option<u8> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Numpad1) => Some(HOTBAR_SLOT_NEW_CHAT),
        PhysicalKey::Code(KeyCode::Digit2 | KeyCode::Numpad2) => Some(HOTBAR_SLOT_NOSTR_IDENTITY),
        _ => None,
    }
}

fn hotbar_display_order() -> [u8; 2] {
    [HOTBAR_SLOT_NEW_CHAT, HOTBAR_SLOT_NOSTR_IDENTITY]
}

fn build_hotbar_items() -> Vec<HotbarSlot> {
    hotbar_display_order()
        .into_iter()
        .map(|slot| match slot {
            HOTBAR_SLOT_NEW_CHAT => HotbarSlot::new(slot, "+", "New pane"),
            HOTBAR_SLOT_NOSTR_IDENTITY => HotbarSlot::new(slot, "N", "Nostr keys"),
            _ => HotbarSlot::new(slot, "?", "Unknown"),
        })
        .collect()
}
