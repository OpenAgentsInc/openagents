//! Agent profile card molecule for displaying sovereign agent identities.
//!
//! Shows agent name, type, status, and capabilities.

use crate::components::atoms::{AgentStatus, AgentType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Agent profile information
#[derive(Debug, Clone)]
pub struct AgentProfileInfo {
    pub id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub npub: Option<String>,
    pub description: Option<String>,
    pub capabilities: Vec<String>,
    pub created_at: String,
    pub last_active: Option<String>,
}

impl AgentProfileInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>, agent_type: AgentType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            agent_type,
            status: AgentStatus::Idle,
            npub: None,
            description: None,
            capabilities: Vec::new(),
            created_at: "Unknown".to_string(),
            last_active: None,
        }
    }

    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn npub(mut self, npub: impl Into<String>) -> Self {
        self.npub = Some(npub.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn capabilities(mut self, caps: Vec<String>) -> Self {
        self.capabilities = caps;
        self
    }

    pub fn created_at(mut self, ts: impl Into<String>) -> Self {
        self.created_at = ts.into();
        self
    }

    pub fn last_active(mut self, ts: impl Into<String>) -> Self {
        self.last_active = Some(ts.into());
        self
    }

    fn short_npub(&self) -> Option<String> {
        self.npub.as_ref().map(|n| {
            if n.len() > 20 {
                format!("{}...{}", &n[..12], &n[n.len() - 8..])
            } else {
                n.clone()
            }
        })
    }
}

/// Agent profile card component
pub struct AgentProfileCard {
    id: Option<ComponentId>,
    profile: AgentProfileInfo,
    hovered: bool,
    action_hovered: bool,
    on_view: Option<Box<dyn FnMut(String)>>,
    on_action: Option<Box<dyn FnMut(String)>>,
}

impl AgentProfileCard {
    pub fn new(profile: AgentProfileInfo) -> Self {
        Self {
            id: None,
            profile,
            hovered: false,
            action_hovered: false,
            on_view: None,
            on_action: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_view<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_view = Some(Box::new(f));
        self
    }

    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_action = Some(Box::new(f));
        self
    }

    fn action_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 80.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            70.0,
            24.0,
        )
    }
}

impl Component for AgentProfileCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Type indicator stripe
        let stripe_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, 4.0, bounds.size.height);
        cx.scene
            .draw_quad(Quad::new(stripe_bounds).with_background(self.profile.agent_type.color()));

        let mut y = bounds.origin.y + padding;

        // Agent icon (placeholder)
        let icon_size = 36.0;
        let icon_bounds = Bounds::new(bounds.origin.x + padding + 6.0, y, icon_size, icon_size);
        cx.scene.draw_quad(
            Quad::new(icon_bounds)
                .with_background(self.profile.agent_type.color().with_alpha(0.2))
                .with_border(self.profile.agent_type.color(), 1.0),
        );

        // Icon symbol based on type
        let icon = self.profile.agent_type.icon();
        let icon_run = cx.text.layout_mono(
            icon,
            Point::new(bounds.origin.x + padding + 14.0, y + 10.0),
            theme::font_size::SM,
            self.profile.agent_type.color(),
        );
        cx.scene.draw_text(icon_run);

        // Name and type
        let name_x = bounds.origin.x + padding + icon_size + 16.0;
        let name_run = cx.text.layout_mono(
            &self.profile.name,
            Point::new(name_x, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Type badge
        let type_w = (self.profile.agent_type.label().len() as f32 * 6.0) + 12.0;
        let type_x = name_x + (self.profile.name.len() as f32 * 7.0) + 10.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(type_x, y - 1.0, type_w, 16.0))
                .with_background(self.profile.agent_type.color().with_alpha(0.2))
                .with_border(self.profile.agent_type.color(), 1.0),
        );
        let type_run = cx.text.layout_mono(
            self.profile.agent_type.label(),
            Point::new(type_x + 4.0, y),
            theme::font_size::XS,
            self.profile.agent_type.color(),
        );
        cx.scene.draw_text(type_run);

        // Status badge
        let status_w = (self.profile.status.label().len() as f32 * 6.0) + 12.0;
        let status_x = bounds.origin.x + bounds.size.width - padding - status_w;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(status_x, y - 1.0, status_w, 16.0))
                .with_background(self.profile.status.color().with_alpha(0.2))
                .with_border(self.profile.status.color(), 1.0),
        );
        let status_run = cx.text.layout_mono(
            self.profile.status.label(),
            Point::new(status_x + 4.0, y),
            theme::font_size::XS,
            self.profile.status.color(),
        );
        cx.scene.draw_text(status_run);

        y += 18.0;

        // npub or ID
        if let Some(npub) = self.profile.short_npub() {
            let npub_run = cx.text.layout_mono(
                &npub,
                Point::new(name_x, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(npub_run);
        }

        y += 16.0;

        // Description
        if let Some(desc) = &self.profile.description {
            let desc_truncated = if desc.len() > 50 {
                format!("{}...", &desc[..47])
            } else {
                desc.clone()
            };
            let desc_run = cx.text.layout_mono(
                &desc_truncated,
                Point::new(name_x, y),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(desc_run);
        }

        y = bounds.origin.y + bounds.size.height - padding - 22.0;

        // Capabilities as small badges
        if !self.profile.capabilities.is_empty() {
            let mut cap_x = bounds.origin.x + padding + 6.0;
            for cap in self.profile.capabilities.iter().take(3) {
                let cap_w = (cap.len() as f32 * 5.5) + 8.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cap_x, y, cap_w, 14.0))
                        .with_background(theme::accent::PRIMARY.with_alpha(0.15))
                        .with_border(theme::accent::PRIMARY.with_alpha(0.4), 1.0),
                );
                let cap_run = cx.text.layout_mono(
                    cap,
                    Point::new(cap_x + 3.0, y + 2.0),
                    10.0,
                    theme::accent::PRIMARY,
                );
                cx.scene.draw_text(cap_run);
                cap_x += cap_w + 6.0;
            }
            if self.profile.capabilities.len() > 3 {
                let more = format!("+{}", self.profile.capabilities.len() - 3);
                let more_run = cx.text.layout_mono(
                    &more,
                    Point::new(cap_x + 2.0, y + 2.0),
                    10.0,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(more_run);
            }
        }

        // Action button
        let action_bounds = self.action_button_bounds(&bounds);
        let action_label = match self.profile.status {
            AgentStatus::Online | AgentStatus::Busy => "Stop",
            AgentStatus::Idle | AgentStatus::Offline => "Start",
            AgentStatus::Error => "Restart",
        };
        let action_bg = if self.action_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(action_bounds)
                .with_background(action_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let action_run = cx.text.layout_mono(
            action_label,
            Point::new(action_bounds.origin.x + 12.0, action_bounds.origin.y + 6.0),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(action_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let action_bounds = self.action_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_action = self.action_hovered;

                self.hovered = bounds.contains(point);
                self.action_hovered = action_bounds.contains(point);

                if was_hovered != self.hovered || was_action != self.action_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if action_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_action {
                            callback(self.profile.id.clone());
                        }
                        return EventResult::Handled;
                    }

                    if bounds.contains(point) {
                        if let Some(callback) = &mut self.on_view {
                            callback(self.profile.id.clone());
                        }
                        return EventResult::Handled;
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
        (None, Some(100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_profile_info() {
        let profile = AgentProfileInfo::new("a1", "CodeReviewer", AgentType::Sovereign)
            .status(AgentStatus::Busy)
            .npub("npub1abc...")
            .description("Reviews code and suggests improvements")
            .capabilities(vec!["code_review".to_string(), "testing".to_string()]);

        assert_eq!(profile.name, "CodeReviewer");
        assert_eq!(profile.agent_type, AgentType::Sovereign);
    }

    #[test]
    fn test_short_npub() {
        let profile = AgentProfileInfo::new("a1", "Test", AgentType::Sovereign)
            .npub("npub1qwertyuiopasdfghjklzxcvbnm123456789");
        assert!(profile.short_npub().unwrap().contains("..."));
    }
}
