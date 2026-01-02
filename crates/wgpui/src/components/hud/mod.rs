mod backgrounds;
mod command_palette;
mod context_menu;
mod dots_grid;
mod frame;
mod frame_clips;
mod heatmap;
mod notifications;
mod resizable_pane;
mod reticle;
mod ring_gauge;
mod scanlines;
mod signal_meter;
mod status_bar;
mod tooltip;

pub use backgrounds::{GridLinesBackground, LineDirection, MovingLinesBackground, PuffsBackground};
pub use command_palette::{Command, CommandPalette};
pub use context_menu::{ContextMenu, MenuItem};
pub use dots_grid::{DotShape, DotsGrid, DotsOrigin};
pub use frame::{CornerConfig, DrawDirection, Frame, FrameAnimation, FrameAnimator, FrameStyle};
pub use frame_clips::{
    CssSize, StyleFrameClipKranoxProps, StyleFrameClipOctagonProps, style_frame_clip_kranox,
    style_frame_clip_octagon,
};
pub use heatmap::Heatmap;
pub use notifications::{Notification, NotificationLevel, NotificationPosition, Notifications};
pub use resizable_pane::{ResizablePane, ResizeEdge};
pub use reticle::Reticle;
pub use ring_gauge::RingGauge;
pub use scanlines::Scanlines;
pub use signal_meter::SignalMeter;
pub use status_bar::{
    StatusBar, StatusBarPosition, StatusItem, StatusItemAlignment, StatusItemContent,
};
pub use tooltip::{Tooltip, TooltipPosition};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::atoms::{Mode, Model, Status};
    use crate::{Bounds, Point};

    #[test]
    fn test_hud_exports() {
        let _palette = CommandPalette::new();
        let _bar = StatusBar::new();
        let _notifs = Notifications::new();
        let _tooltip = Tooltip::new("Test");
        let _menu = ContextMenu::new();
        let _frame = Frame::new();
        let _grid = GridLinesBackground::new();
        let _moving = MovingLinesBackground::new();
        let _puffs = PuffsBackground::new();
        let _scanlines = Scanlines::new();
        let _meter = SignalMeter::new();
        let _reticle = Reticle::new();
        let _resizable = ResizablePane::new();
        let _heatmap = Heatmap::new();
        let _ring = RingGauge::new();
    }

    #[test]
    fn test_resizable_pane_workflow() {
        let pane = ResizablePane::new()
            .resizable(true)
            .show_handles(true)
            .min_size(100.0, 100.0)
            .max_size(800.0, 600.0)
            .size(400.0, 300.0);

        assert!(pane.current_size().is_some());
        let size = pane.current_size().unwrap();
        assert_eq!(size.width, 400.0);
        assert_eq!(size.height, 300.0);

        // Test non-resizable mode
        let fixed = ResizablePane::new().resizable(false);
        assert_eq!(fixed.hovered_edge(), ResizeEdge::None);
    }

    #[test]
    fn test_command_palette_workflow() {
        let mut palette = CommandPalette::new().commands(vec![
            Command::new("file.new", "New File").keybinding("Cmd+N"),
            Command::new("file.open", "Open File").keybinding("Cmd+O"),
            Command::new("file.save", "Save").keybinding("Cmd+S"),
        ]);

        palette.open();
        assert!(palette.is_open());

        palette.move_selection_down();
        palette.move_selection_down();

        palette.close();
        assert!(!palette.is_open());
    }

    #[test]
    fn test_status_bar_layout() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusItem::mode("mode", Mode::Plan).left());
        bar.add_item(StatusItem::text("file", "main.rs").center());
        bar.add_item(StatusItem::model("model", Model::Claude).right());
        bar.add_item(StatusItem::status("status", Status::Online).right());

        bar.remove_item("nonexistent");
        bar.update_item("mode", StatusItemContent::Mode(Mode::Act));
    }

    #[test]
    fn test_notifications_lifecycle() {
        let mut notifs = Notifications::new()
            .position(NotificationPosition::TopRight)
            .max_visible(3);

        notifs.success("Build complete");
        notifs.warning("Deprecated API");
        notifs.error("Connection failed");

        assert_eq!(notifs.count(), 3);

        notifs.clear();
        assert_eq!(notifs.count(), 0);
    }

    #[test]
    fn test_tooltip_workflow() {
        let mut tooltip = Tooltip::new("Helpful hint")
            .position(TooltipPosition::Top)
            .target(Bounds::new(100.0, 100.0, 50.0, 30.0))
            .delay(5);

        assert!(!tooltip.is_visible());

        for _ in 0..5 {
            tooltip.update_hover(true);
        }
        assert!(tooltip.is_visible());

        tooltip.hide();
        assert!(!tooltip.is_visible());
    }

    #[test]
    fn test_context_menu_workflow() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("cut", "Cut").shortcut("Cmd+X"),
            MenuItem::separator(),
            MenuItem::new("copy", "Copy").shortcut("Cmd+C"),
            MenuItem::new("paste", "Paste")
                .shortcut("Cmd+V")
                .disabled(true),
        ]);

        assert!(!menu.is_open());

        menu.open(Point::new(200.0, 150.0));
        assert!(menu.is_open());

        menu.select_next();
        menu.select_next();

        let selected = menu.confirm();
        assert_eq!(selected, Some("copy".to_string()));
        assert!(!menu.is_open());
    }
}
