//! The window rules, applied.
//!
//! The rules are data in `crates/coder-binds`, the same rows
//! `os/modules/coderos/desktop.nix` writes for Hyprland. The compositor
//! reads the table for a window when it maps and again when its app-id
//! or title changes, and applies what the read changed: a float, a tile,
//! the middle of the screen, a pin, no border. Each tile holds two copies
//! of the effects: what the table said at the last read, so a title
//! change that leaves the rule as it was moves nothing, and what is in
//! force, which the desk protocol's `shape` adds to.
//!
//! A rule that stops matching leaves the window as it is, the way a
//! Hyprland rule applied at map time does. The aspect ratio and the
//! shadow are recorded and not drawn: the layout crate has no ratio to
//! keep, and the compositor draws no shadow.

use coder_binds::Effects;
use coder_wm::{Manager, Rect, WinId};
use smithay::desktop::Window;

use crate::state::Coder;

/// Applies one read of the table to a window the layout holds. `asked` is
/// the rectangle the client asked for, as a fraction of the screen, which
/// an X11 window carries and a toplevel does not.
pub fn apply(manager: &mut Manager, id: WinId, effects: Effects, asked: Option<Rect>) {
    match effects.float {
        Some(true) => {
            manager.set_floating(id, true);
            if let Some(mut rect) = asked.or_else(|| manager.rect_of(id)) {
                if effects.center {
                    rect.x = (1.0 - rect.w).max(0.0) / 2.0;
                    rect.y = (1.0 - rect.h).max(0.0) / 2.0;
                }
                manager.place_float(id, rect);
            }
        }
        Some(false) => {
            manager.set_floating(id, false);
        }
        None => {}
    }
    if effects.pin {
        manager.set_pinned(id, true);
    }
}

impl Coder {
    /// Reads the table for one window and applies what changed since the
    /// last read. Answers whether the layout moved, so the caller arranges
    /// once rather than on every read.
    pub fn read_rules(&mut self, id: WinId, asked: Option<Rect>) -> bool {
        let app_id = self.app_id(id);
        let title = self.title(id);
        let now = coder_binds::matching(&app_id, &title);
        let Some(tile) = self.tiles.iter_mut().find(|tile| tile.id == id) else {
            return false;
        };
        if now == tile.rule {
            return false;
        }
        tile.rule = now;
        tile.effects = tile.effects.merge(now);
        if now != Effects::default() {
            log::info!("the window {app_id:?} titled {title:?} maps under the rule {now:?}");
        }
        apply(&mut self.manager, id, now, asked);
        true
    }

    /// Reads the table for a window whose app-id or title changed, and
    /// redraws the layout when the read moved it.
    pub fn rules_changed(&mut self, window: &Window) {
        let Some(id) = self.tile_of(window).map(|tile| tile.id) else {
            return;
        };
        if self.read_rules(id, None) {
            self.after_layout();
        }
    }

    /// Whether the effects in force on one window keep it out of
    /// fullscreen.
    pub fn suppresses_fullscreen(&self, id: WinId) -> bool {
        self.tile(id)
            .is_some_and(|tile| tile.effects.suppress_fullscreen)
    }

    /// Whether the effects in force on one window draw it with no border.
    pub fn borderless(&self, id: WinId) -> bool {
        self.tile(id)
            .is_some_and(|tile| tile.effects.border == Some(0))
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
