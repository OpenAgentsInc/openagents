//! Skill license badge for NIP-SA Sovereign Agents.
//!
//! Shows skill license status and ownership.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Skill license status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LicenseStatus {
    #[default]
    Unknown,
    Active,
    Pending,
    Expired,
    Revoked,
}

impl LicenseStatus {
    pub fn label(&self) -> &'static str {
        match self {
            LicenseStatus::Unknown => "Unknown",
            LicenseStatus::Active => "Active",
            LicenseStatus::Pending => "Pending",
            LicenseStatus::Expired => "Expired",
            LicenseStatus::Revoked => "Revoked",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            LicenseStatus::Unknown => "?",
            LicenseStatus::Active => "✓",
            LicenseStatus::Pending => "◐",
            LicenseStatus::Expired => "⏱",
            LicenseStatus::Revoked => "✕",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            LicenseStatus::Unknown => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            LicenseStatus::Active => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            LicenseStatus::Pending => Hsla::new(45.0, 0.8, 0.5, 1.0), // Gold
            LicenseStatus::Expired => Hsla::new(30.0, 0.7, 0.5, 1.0), // Orange
            LicenseStatus::Revoked => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
        }
    }
}

/// Skill type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillType {
    #[default]
    Code,
    Data,
    Model,
    Tool,
}

impl SkillType {
    pub fn icon(&self) -> &'static str {
        match self {
            SkillType::Code => "⟨⟩",
            SkillType::Data => "⬡",
            SkillType::Model => "◆",
            SkillType::Tool => "⚙",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            SkillType::Code => Hsla::new(200.0, 0.7, 0.5, 1.0), // Blue
            SkillType::Data => Hsla::new(280.0, 0.6, 0.5, 1.0), // Purple
            SkillType::Model => Hsla::new(140.0, 0.7, 0.45, 1.0), // Green
            SkillType::Tool => Hsla::new(30.0, 0.7, 0.5, 1.0),  // Orange
        }
    }
}

/// Badge showing skill license
pub struct SkillLicenseBadge {
    id: Option<ComponentId>,
    skill_type: SkillType,
    status: LicenseStatus,
    name: Option<String>,
    compact: bool,
}

impl SkillLicenseBadge {
    pub fn new(skill_type: SkillType, status: LicenseStatus) -> Self {
        Self {
            id: None,
            skill_type,
            status,
            name: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for SkillLicenseBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let status_color = self.status.color();
        let bg = Hsla::new(status_color.h, status_color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(status_color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        if self.compact {
            // Just type icon and status
            let icon = self.skill_type.icon();
            let run = cx.text.layout_mono(
                icon,
                Point::new(x, text_y),
                theme::font_size::SM,
                self.skill_type.color(),
            );
            cx.scene.draw_text(run);
            x += 16.0;
            let status_icon = self.status.icon();
            let run = cx.text.layout_mono(
                status_icon,
                Point::new(x, text_y),
                theme::font_size::SM,
                status_color,
            );
            cx.scene.draw_text(run);
        } else {
            // Type icon
            let type_icon = self.skill_type.icon();
            let type_run = cx.text.layout_mono(
                type_icon,
                Point::new(x, text_y),
                theme::font_size::SM,
                self.skill_type.color(),
            );
            cx.scene.draw_text(type_run);
            x += 18.0;

            // Name (if provided)
            if let Some(ref name) = self.name {
                let name_display: String = if name.len() > 12 {
                    format!("{}…", &name[..11])
                } else {
                    name.clone()
                };
                let name_run = cx.text.layout_mono(
                    &name_display,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(name_run);
                x += name_display.len() as f32 * 6.5 + 8.0;
            }

            // Status icon
            let status_icon = self.status.icon();
            let status_run = cx.text.layout_mono(
                status_icon,
                Point::new(x, text_y),
                theme::font_size::SM,
                status_color,
            );
            cx.scene.draw_text(status_run);
            x += 14.0;

            // Status label
            let label = self.status.label();
            let label_run = cx.text.layout_mono(
                label,
                Point::new(x, text_y),
                theme::font_size::XS,
                status_color,
            );
            cx.scene.draw_text(label_run);
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.compact {
            (Some(40.0), Some(22.0))
        } else {
            let mut width = 12.0 + 18.0 + 14.0 + self.status.label().len() as f32 * 6.5;
            if let Some(ref name) = self.name {
                let name_len = name.len().min(12);
                width += name_len as f32 * 6.5 + 8.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_license() {
        let badge = SkillLicenseBadge::new(SkillType::Code, LicenseStatus::Active);
        assert_eq!(badge.skill_type, SkillType::Code);
        assert_eq!(badge.status, LicenseStatus::Active);
    }
}
