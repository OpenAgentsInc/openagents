use crate::components::atoms::PermissionAction;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Button, ButtonVariant, Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionType {
    FileRead(String),
    FileWrite(String),
    Execute(String),
    Network(String),
    Custom(String),
}

impl PermissionType {
    pub fn description(&self) -> String {
        match self {
            PermissionType::FileRead(path) => format!("Read file: {}", path),
            PermissionType::FileWrite(path) => format!("Write file: {}", path),
            PermissionType::Execute(cmd) => format!("Execute: {}", cmd),
            PermissionType::Network(url) => format!("Network access: {}", url),
            PermissionType::Custom(desc) => desc.clone(),
        }
    }
}

pub struct PermissionDialog {
    id: Option<ComponentId>,
    permission: PermissionType,
    open: bool,
    on_action: Option<Box<dyn FnMut(PermissionAction)>>,
}

impl PermissionDialog {
    pub fn new(permission: PermissionType) -> Self {
        Self {
            id: None,
            permission,
            open: true,
            on_action: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: FnMut(PermissionAction) + 'static,
    {
        self.on_action = Some(Box::new(f));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn show(&mut self) {
        self.open = true;
    }

    pub fn hide(&mut self) {
        self.open = false;
    }

    pub fn permission(&self) -> &PermissionType {
        &self.permission
    }
}

impl Default for PermissionDialog {
    fn default() -> Self {
        Self::new(PermissionType::Custom("Permission required".to_string()))
    }
}

impl Component for PermissionDialog {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.open {
            return;
        }

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::APP.with_alpha(0.7)));

        let dialog_width = 400.0;
        let dialog_height = 200.0;
        let dialog_x = bounds.origin.x + (bounds.size.width - dialog_width) / 2.0;
        let dialog_y = bounds.origin.y + (bounds.size.height - dialog_height) / 2.0;
        let dialog_bounds = Bounds::new(dialog_x, dialog_y, dialog_width, dialog_height);

        cx.scene.draw_quad(
            Quad::new(dialog_bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::status::WARNING, 2.0),
        );

        let padding = theme::spacing::MD;

        let mut title = Text::new("Permission Required")
            .font_size(theme::font_size::LG)
            .color(theme::text::PRIMARY);
        title.paint(
            Bounds::new(
                dialog_x + padding,
                dialog_y + padding,
                dialog_width - padding * 2.0,
                24.0,
            ),
            cx,
        );

        let description = self.permission.description();
        let mut desc_text = Text::new(&description)
            .font_size(theme::font_size::BASE)
            .color(theme::text::SECONDARY);
        desc_text.paint(
            Bounds::new(
                dialog_x + padding,
                dialog_y + padding + 36.0,
                dialog_width - padding * 2.0,
                60.0,
            ),
            cx,
        );

        let btn_y = dialog_y + dialog_height - padding - 36.0;
        let btn_width = 100.0;
        let btn_height = 32.0;
        let gap = theme::spacing::SM;

        let mut deny_btn = Button::new("Deny").variant(ButtonVariant::Secondary);
        deny_btn.paint(
            Bounds::new(
                dialog_x + dialog_width - padding - btn_width * 3.0 - gap * 2.0,
                btn_y,
                btn_width,
                btn_height,
            ),
            cx,
        );

        let mut allow_once_btn = Button::new("Allow Once").variant(ButtonVariant::Secondary);
        allow_once_btn.paint(
            Bounds::new(
                dialog_x + dialog_width - padding - btn_width * 2.0 - gap,
                btn_y,
                btn_width,
                btn_height,
            ),
            cx,
        );

        let mut allow_btn = Button::new("Allow").variant(ButtonVariant::Primary);
        allow_btn.paint(
            Bounds::new(
                dialog_x + dialog_width - padding - btn_width,
                btn_y,
                btn_width,
                btn_height,
            ),
            cx,
        );
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if !self.open {
            return EventResult::Ignored;
        }

        let dialog_width = 400.0;
        let dialog_height = 200.0;
        let dialog_x = bounds.origin.x + (bounds.size.width - dialog_width) / 2.0;
        let dialog_y = bounds.origin.y + (bounds.size.height - dialog_height) / 2.0;

        let padding = theme::spacing::MD;
        let btn_y = dialog_y + dialog_height - padding - 36.0;
        let btn_width = 100.0;
        let btn_height = 32.0;
        let gap = theme::spacing::SM;

        if let InputEvent::MouseUp { x, y, .. } = event {
            let deny_bounds = Bounds::new(
                dialog_x + dialog_width - padding - btn_width * 3.0 - gap * 2.0,
                btn_y,
                btn_width,
                btn_height,
            );

            let allow_once_bounds = Bounds::new(
                dialog_x + dialog_width - padding - btn_width * 2.0 - gap,
                btn_y,
                btn_width,
                btn_height,
            );

            let allow_bounds = Bounds::new(
                dialog_x + dialog_width - padding - btn_width,
                btn_y,
                btn_width,
                btn_height,
            );

            if deny_bounds.contains(Point::new(*x, *y)) {
                if let Some(callback) = &mut self.on_action {
                    callback(PermissionAction::Deny);
                }
                self.open = false;
                return EventResult::Handled;
            }

            if allow_once_bounds.contains(Point::new(*x, *y)) {
                if let Some(callback) = &mut self.on_action {
                    callback(PermissionAction::AllowOnce);
                }
                self.open = false;
                return EventResult::Handled;
            }

            if allow_bounds.contains(Point::new(*x, *y)) {
                if let Some(callback) = &mut self.on_action {
                    callback(PermissionAction::Allow);
                }
                self.open = false;
                return EventResult::Handled;
            }
        }

        EventResult::Handled
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_dialog_new() {
        let dialog = PermissionDialog::new(PermissionType::FileRead("/etc/passwd".to_string()));
        assert!(dialog.is_open());
        assert_eq!(dialog.permission().description(), "Read file: /etc/passwd");
    }

    #[test]
    fn test_permission_dialog_builder() {
        let dialog = PermissionDialog::new(PermissionType::Execute("rm -rf".to_string()))
            .with_id(1)
            .open(false);

        assert_eq!(dialog.id, Some(1));
        assert!(!dialog.is_open());
    }

    #[test]
    fn test_show_hide() {
        let mut dialog =
            PermissionDialog::new(PermissionType::Network("https://api.com".to_string()));
        assert!(dialog.is_open());
        dialog.hide();
        assert!(!dialog.is_open());
        dialog.show();
        assert!(dialog.is_open());
    }

    #[test]
    fn test_permission_types() {
        assert_eq!(
            PermissionType::FileWrite("test.txt".to_string()).description(),
            "Write file: test.txt"
        );
        assert_eq!(
            PermissionType::Custom("Special permission".to_string()).description(),
            "Special permission"
        );
    }
}
