use wgpui::components::hud::{Hotbar, HotbarSlot};
use wgpui::{Bounds, Size};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app_state::RenderState;
use crate::pane_system::PaneController;
use crate::spark_wallet::SparkWalletCommand;

pub const HOTBAR_HEIGHT: f32 = 52.0;
pub const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;
pub const HOTBAR_SLOT_NEW_CHAT: u8 = 1;
pub const HOTBAR_SLOT_NOSTR_IDENTITY: u8 = 2;
pub const HOTBAR_SLOT_SPARK_WALLET: u8 = 3;
pub const HOTBAR_SLOT_COMMAND_PALETTE: u8 = 4;

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
        if slot == HOTBAR_SLOT_NEW_CHAT
            || slot == HOTBAR_SLOT_NOSTR_IDENTITY
            || slot == HOTBAR_SLOT_SPARK_WALLET
            || slot == HOTBAR_SLOT_COMMAND_PALETTE
        {
            activate_hotbar_slot(state, slot);
            changed = true;
        }
    }
    changed
}

pub fn activate_hotbar_slot(state: &mut RenderState, slot: u8) {
    state.hotbar.flash_slot(slot);
    match slot {
        HOTBAR_SLOT_NEW_CHAT => PaneController::create_empty(state),
        HOTBAR_SLOT_NOSTR_IDENTITY => PaneController::create_nostr_identity(state),
        HOTBAR_SLOT_SPARK_WALLET => {
            let was_open = state
                .panes
                .iter()
                .any(|pane| pane.kind == crate::app_state::PaneKind::SparkWallet);
            PaneController::create_spark_wallet(state);
            if !was_open && let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::Refresh)
            {
                state.spark_wallet.last_error = Some(error);
            }
        }
        HOTBAR_SLOT_COMMAND_PALETTE => {
            state.command_palette.open();
        }
        _ => {}
    }
    state.hotbar_flash_was_active = true;
}

pub fn hotbar_slot_for_key(key: PhysicalKey) -> Option<u8> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Numpad1) => Some(HOTBAR_SLOT_NEW_CHAT),
        PhysicalKey::Code(KeyCode::Digit2 | KeyCode::Numpad2) => Some(HOTBAR_SLOT_NOSTR_IDENTITY),
        PhysicalKey::Code(KeyCode::Digit3 | KeyCode::Numpad3) => Some(HOTBAR_SLOT_SPARK_WALLET),
        PhysicalKey::Code(KeyCode::Digit4 | KeyCode::Numpad4) => Some(HOTBAR_SLOT_COMMAND_PALETTE),
        _ => None,
    }
}

fn hotbar_display_order() -> [u8; 4] {
    [
        HOTBAR_SLOT_NEW_CHAT,
        HOTBAR_SLOT_NOSTR_IDENTITY,
        HOTBAR_SLOT_SPARK_WALLET,
        HOTBAR_SLOT_COMMAND_PALETTE,
    ]
}

fn build_hotbar_items() -> Vec<HotbarSlot> {
    hotbar_display_order()
        .into_iter()
        .map(|slot| match slot {
            HOTBAR_SLOT_NEW_CHAT => HotbarSlot::new(slot, "+", "New pane"),
            HOTBAR_SLOT_NOSTR_IDENTITY => HotbarSlot::new(slot, "N", "Nostr keys"),
            HOTBAR_SLOT_SPARK_WALLET => HotbarSlot::new(slot, "S", "Spark wallet"),
            HOTBAR_SLOT_COMMAND_PALETTE => HotbarSlot::new(slot, "K", "Command palette"),
            _ => HotbarSlot::new(slot, "?", "Unknown"),
        })
        .collect()
}
