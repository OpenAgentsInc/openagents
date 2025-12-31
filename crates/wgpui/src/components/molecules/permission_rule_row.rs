//! Permission rule row for displaying and managing permission rules.
//!
//! Shows a permission rule with tool type, action, and scope.

use crate::components::atoms::{ToolIcon, ToolType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Permission decision for a rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionDecision {
    #[default]
    Ask,
    AllowAlways,
    AllowOnce,
    Deny,
}

impl PermissionDecision {
    pub fn label(&self) -> &'static str {
        match self {
            PermissionDecision::Ask => "Ask",
            PermissionDecision::AllowAlways => "Allow Always",
            PermissionDecision::AllowOnce => "Allow Once",
            PermissionDecision::Deny => "Deny",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            PermissionDecision::Ask => "Ask",
            PermissionDecision::AllowAlways => "Always",
            PermissionDecision::AllowOnce => "Once",
            PermissionDecision::Deny => "Deny",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            PermissionDecision::Ask => Hsla::new(200.0, 0.6, 0.5, 1.0), // Blue
            PermissionDecision::AllowAlways => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            PermissionDecision::AllowOnce => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            PermissionDecision::Deny => Hsla::new(0.0, 0.8, 0.5, 1.0),  // Red
        }
    }
}

/// Permission rule information
#[derive(Debug, Clone)]
pub struct PermissionRule {
    pub id: String,
    pub tool_type: ToolType,
    pub tool_name: String,
    pub pattern: Option<String>,
    pub decision: PermissionDecision,
    pub scope: PermissionScope,
}

/// Scope of a permission rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionScope {
    #[default]
    Session,
    Project,
    Global,
}

impl PermissionScope {
    pub fn label(&self) -> &'static str {
        match self {
            PermissionScope::Session => "Session",
            PermissionScope::Project => "Project",
            PermissionScope::Global => "Global",
        }
    }
}

impl PermissionRule {
    pub fn new(id: impl Into<String>, tool_type: ToolType, tool_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tool_type,
            tool_name: tool_name.into(),
            pattern: None,
            decision: PermissionDecision::Ask,
            scope: PermissionScope::Session,
        }
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    pub fn decision(mut self, decision: PermissionDecision) -> Self {
        self.decision = decision;
        self
    }

    pub fn scope(mut self, scope: PermissionScope) -> Self {
        self.scope = scope;
        self
    }
}

/// Actions on a permission rule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    Edit,
    Delete,
}

/// A row displaying a permission rule
pub struct PermissionRuleRow {
    id: Option<ComponentId>,
    rule: PermissionRule,
    hovered: bool,
    hovered_action: Option<RuleAction>,
    on_action: Option<Box<dyn FnMut(RuleAction, String)>>,
}

impl PermissionRuleRow {
    pub fn new(rule: PermissionRule) -> Self {
        Self {
            id: None,
            rule,
            hovered: false,
            hovered_action: None,
            on_action: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: FnMut(RuleAction, String) + 'static,
    {
        self.on_action = Some(Box::new(f));
        self
    }

    pub fn rule(&self) -> &PermissionRule {
        &self.rule
    }

    fn action_bounds(&self, bounds: &Bounds) -> Vec<(RuleAction, Bounds)> {
        let btn_width = 40.0;
        let btn_height = 22.0;
        let gap = 6.0;
        let padding = 12.0;
        let y = bounds.origin.y + (bounds.size.height - btn_height) / 2.0;

        vec![
            (
                RuleAction::Edit,
                Bounds::new(
                    bounds.origin.x + bounds.size.width - padding - btn_width * 2.0 - gap,
                    y,
                    btn_width,
                    btn_height,
                ),
            ),
            (
                RuleAction::Delete,
                Bounds::new(
                    bounds.origin.x + bounds.size.width - padding - btn_width,
                    y,
                    btn_width,
                    btn_height,
                ),
            ),
        ]
    }
}

impl Component for PermissionRuleRow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let decision_color = self.rule.decision.color();
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };

        // Row background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 12.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Tool icon
        let mut icon = ToolIcon::new(self.rule.tool_type).size(18.0);
        icon.paint(
            Bounds::new(
                x,
                bounds.origin.y + (bounds.size.height - 18.0) / 2.0,
                18.0,
                18.0,
            ),
            cx,
        );
        x += 26.0;

        // Tool name
        let name_run = cx.text.layout(
            &self.rule.tool_name,
            Point::new(x, text_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);
        x += self.rule.tool_name.len() as f32 * 7.0 + 16.0;

        // Pattern (if any)
        if let Some(pattern) = &self.rule.pattern {
            let pattern_text = format!("[{}]", pattern);
            let pattern_run = cx.text.layout(
                &pattern_text,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(pattern_run);
            x += pattern_text.len() as f32 * 6.0 + 12.0;
        }

        // Decision badge
        let decision_bounds = Bounds::new(
            x,
            bounds.origin.y + (bounds.size.height - 22.0) / 2.0,
            60.0,
            22.0,
        );
        cx.scene.draw_quad(
            Quad::new(decision_bounds)
                .with_background(decision_color.with_alpha(0.2))
                .with_border(decision_color, 1.0),
        );

        let decision_run = cx.text.layout(
            self.rule.decision.short_label(),
            Point::new(
                decision_bounds.origin.x + 6.0,
                decision_bounds.origin.y + 4.0,
            ),
            theme::font_size::XS,
            decision_color,
        );
        cx.scene.draw_text(decision_run);
        x += 72.0;

        // Scope
        let scope_run = cx.text.layout(
            self.rule.scope.label(),
            Point::new(x, text_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(scope_run);

        // Action buttons (only on hover)
        if self.hovered {
            for (action, action_bounds) in self.action_bounds(&bounds) {
                let is_hovered = self.hovered_action == Some(action);
                let btn_bg = if is_hovered {
                    if action == RuleAction::Delete {
                        theme::status::ERROR.with_alpha(0.2)
                    } else {
                        theme::bg::HOVER
                    }
                } else {
                    theme::bg::MUTED
                };

                cx.scene
                    .draw_quad(Quad::new(action_bounds).with_background(btn_bg));

                let label = match action {
                    RuleAction::Edit => "Edit",
                    RuleAction::Delete => "Del",
                };
                let label_color = if action == RuleAction::Delete && is_hovered {
                    theme::status::ERROR
                } else {
                    theme::text::MUTED
                };

                let label_run = cx.text.layout(
                    label,
                    Point::new(action_bounds.origin.x + 6.0, action_bounds.origin.y + 4.0),
                    theme::font_size::XS,
                    label_color,
                );
                cx.scene.draw_text(label_run);
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_action = self.hovered_action;

                self.hovered = bounds.contains(point);
                self.hovered_action = None;

                if self.hovered {
                    for (action, action_bounds) in self.action_bounds(&bounds) {
                        if action_bounds.contains(point) {
                            self.hovered_action = Some(action);
                            break;
                        }
                    }
                }

                if was_hovered != self.hovered || was_action != self.hovered_action {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && self.hovered {
                    let point = Point::new(*x, *y);

                    for (action, action_bounds) in self.action_bounds(&bounds) {
                        if action_bounds.contains(point) {
                            if let Some(callback) = &mut self.on_action {
                                callback(action, self.rule.id.clone());
                            }
                            return EventResult::Handled;
                        }
                    }
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(44.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_decision() {
        assert_eq!(PermissionDecision::AllowAlways.label(), "Allow Always");
        assert_eq!(PermissionDecision::Deny.short_label(), "Deny");
    }

    #[test]
    fn test_permission_rule() {
        let rule = PermissionRule::new("rule-1", ToolType::Bash, "Bash")
            .pattern("*.sh")
            .decision(PermissionDecision::AllowAlways)
            .scope(PermissionScope::Project);

        assert_eq!(rule.tool_name, "Bash");
        assert_eq!(rule.pattern, Some("*.sh".to_string()));
        assert_eq!(rule.decision, PermissionDecision::AllowAlways);
    }

    #[test]
    fn test_permission_rule_row() {
        let rule = PermissionRule::new("1", ToolType::Read, "Read");
        let row = PermissionRuleRow::new(rule);
        assert!(!row.hovered);
    }
}
