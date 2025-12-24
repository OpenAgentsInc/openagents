mod command_palette;
mod notifications;
mod status_bar;

pub use command_palette::{Command, CommandPalette};
pub use notifications::{Notification, NotificationLevel, NotificationPosition, Notifications};
pub use status_bar::{
    StatusBar, StatusBarPosition, StatusItem, StatusItemAlignment, StatusItemContent,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::atoms::{Mode, Model, Status};

    #[test]
    fn test_hud_exports() {
        let _palette = CommandPalette::new();
        let _bar = StatusBar::new();
        let _notifs = Notifications::new();
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
}
