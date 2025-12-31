//! Skill card molecule for displaying marketplace skills.
//!
//! Shows skill name, description, pricing, and install status.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Skill category
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkillCategory {
    CodeGeneration,
    DataAnalysis,
    WebAutomation,
    FileProcessing,
    ApiIntegration,
    TextProcessing,
    ImageProcessing,
    Other,
}

impl SkillCategory {
    pub fn label(&self) -> &'static str {
        match self {
            SkillCategory::CodeGeneration => "Code Gen",
            SkillCategory::DataAnalysis => "Data Analysis",
            SkillCategory::WebAutomation => "Web Automation",
            SkillCategory::FileProcessing => "File Processing",
            SkillCategory::ApiIntegration => "API Integration",
            SkillCategory::TextProcessing => "Text Processing",
            SkillCategory::ImageProcessing => "Image Processing",
            SkillCategory::Other => "Other",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            SkillCategory::CodeGeneration => Hsla::new(200.0, 0.7, 0.5, 1.0), // Blue
            SkillCategory::DataAnalysis => Hsla::new(270.0, 0.6, 0.55, 1.0),  // Purple
            SkillCategory::WebAutomation => Hsla::new(120.0, 0.6, 0.45, 1.0), // Green
            SkillCategory::FileProcessing => Hsla::new(45.0, 0.7, 0.5, 1.0),  // Yellow
            SkillCategory::ApiIntegration => Hsla::new(180.0, 0.6, 0.5, 1.0), // Cyan
            SkillCategory::TextProcessing => Hsla::new(0.0, 0.6, 0.55, 1.0),  // Red
            SkillCategory::ImageProcessing => Hsla::new(310.0, 0.6, 0.55, 1.0), // Magenta
            SkillCategory::Other => theme::text::MUTED,
        }
    }
}

/// Skill installation status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkillInstallStatus {
    Available,
    Installed,
    UpdateAvailable,
    Installing,
}

impl SkillInstallStatus {
    pub fn label(&self) -> &'static str {
        match self {
            SkillInstallStatus::Available => "Install",
            SkillInstallStatus::Installed => "Installed",
            SkillInstallStatus::UpdateAvailable => "Update",
            SkillInstallStatus::Installing => "Installing...",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            SkillInstallStatus::Available => theme::accent::PRIMARY,
            SkillInstallStatus::Installed => Hsla::new(120.0, 0.6, 0.45, 1.0),
            SkillInstallStatus::UpdateAvailable => Hsla::new(45.0, 0.8, 0.5, 1.0),
            SkillInstallStatus::Installing => Hsla::new(200.0, 0.6, 0.5, 1.0),
        }
    }
}

/// Skill info
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub author: String,
    pub version: String,
    pub install_status: SkillInstallStatus,
    pub price_sats: Option<u64>,
    pub downloads: u32,
    pub rating: f32,
}

impl SkillInfo {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category: SkillCategory::Other,
            author: "unknown".to_string(),
            version: "1.0.0".to_string(),
            install_status: SkillInstallStatus::Available,
            price_sats: None,
            downloads: 0,
            rating: 0.0,
        }
    }

    pub fn category(mut self, category: SkillCategory) -> Self {
        self.category = category;
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn status(mut self, status: SkillInstallStatus) -> Self {
        self.install_status = status;
        self
    }

    pub fn price(mut self, sats: u64) -> Self {
        self.price_sats = Some(sats);
        self
    }

    pub fn downloads(mut self, count: u32) -> Self {
        self.downloads = count;
        self
    }

    pub fn rating(mut self, rating: f32) -> Self {
        self.rating = rating;
        self
    }
}

/// Skill card component
pub struct SkillCard {
    id: Option<ComponentId>,
    skill: SkillInfo,
    hovered: bool,
    install_hovered: bool,
    on_install: Option<Box<dyn FnMut(String)>>,
    on_view: Option<Box<dyn FnMut(String)>>,
}

impl SkillCard {
    pub fn new(skill: SkillInfo) -> Self {
        Self {
            id: None,
            skill,
            hovered: false,
            install_hovered: false,
            on_install: None,
            on_view: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_install<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_install = Some(Box::new(f));
        self
    }

    pub fn on_view<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_view = Some(Box::new(f));
        self
    }

    fn install_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 80.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            75.0,
            24.0,
        )
    }

    fn format_downloads(&self) -> String {
        let d = self.skill.downloads;
        if d >= 1_000_000 {
            format!("{:.1}M", d as f64 / 1_000_000.0)
        } else if d >= 1_000 {
            format!("{:.1}K", d as f64 / 1_000.0)
        } else {
            format!("{}", d)
        }
    }
}

impl Component for SkillCard {
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

        let mut y = bounds.origin.y + padding;

        // Skill name
        let name_run = cx.text.layout(
            &self.skill.name,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Category badge
        let cat_x = bounds.origin.x + padding + 150.0;
        let cat_w = (self.skill.category.label().len() as f32 * 6.0) + 12.0;
        let cat_bounds = Bounds::new(cat_x, y - 2.0, cat_w, 18.0);
        cx.scene.draw_quad(
            Quad::new(cat_bounds)
                .with_background(self.skill.category.color().with_alpha(0.2))
                .with_border(self.skill.category.color(), 1.0),
        );
        let cat_run = cx.text.layout(
            self.skill.category.label(),
            Point::new(cat_x + 4.0, y),
            theme::font_size::XS,
            self.skill.category.color(),
        );
        cx.scene.draw_text(cat_run);

        y += 22.0;

        // Description
        let desc = if self.skill.description.len() > 60 {
            format!("{}...", &self.skill.description[..57])
        } else {
            self.skill.description.clone()
        };
        let desc_run = cx.text.layout(
            &desc,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(desc_run);

        y += 20.0;

        // Author and version
        let author_text = format!("by {} | v{}", self.skill.author, self.skill.version);
        let author_run = cx.text.layout(
            &author_text,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(author_run);

        y += 18.0;

        // Stats row: downloads, rating, price
        let downloads_text = format!("\u{2B07} {}", self.format_downloads());
        let downloads_run = cx.text.layout(
            &downloads_text,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(downloads_run);

        let rating_text = format!("\u{2605} {:.1}", self.skill.rating);
        let rating_run = cx.text.layout(
            &rating_text,
            Point::new(bounds.origin.x + padding + 70.0, y),
            theme::font_size::XS,
            Hsla::new(45.0, 0.8, 0.5, 1.0),
        );
        cx.scene.draw_text(rating_run);

        if let Some(price) = self.skill.price_sats {
            let price_text = format!("{} sats", price);
            let price_run = cx.text.layout(
                &price_text,
                Point::new(bounds.origin.x + padding + 130.0, y),
                theme::font_size::XS,
                Hsla::new(35.0, 0.9, 0.5, 1.0),
            );
            cx.scene.draw_text(price_run);
        } else {
            let free_run = cx.text.layout(
                "Free",
                Point::new(bounds.origin.x + padding + 130.0, y),
                theme::font_size::XS,
                Hsla::new(120.0, 0.6, 0.5, 1.0),
            );
            cx.scene.draw_text(free_run);
        }

        // Install button
        let install_bounds = self.install_button_bounds(&bounds);
        let install_bg = if self.install_hovered {
            self.skill.install_status.color().with_alpha(0.3)
        } else {
            self.skill.install_status.color().with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(install_bounds)
                .with_background(install_bg)
                .with_border(self.skill.install_status.color(), 1.0),
        );
        let install_run = cx.text.layout(
            self.skill.install_status.label(),
            Point::new(install_bounds.origin.x + 8.0, install_bounds.origin.y + 5.0),
            theme::font_size::XS,
            self.skill.install_status.color(),
        );
        cx.scene.draw_text(install_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let install_bounds = self.install_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_install = self.install_hovered;

                self.hovered = bounds.contains(point);
                self.install_hovered = install_bounds.contains(point);

                if was_hovered != self.hovered || was_install != self.install_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if install_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_install {
                            callback(self.skill.id.clone());
                        }
                        return EventResult::Handled;
                    }

                    if bounds.contains(point) {
                        if let Some(callback) = &mut self.on_view {
                            callback(self.skill.id.clone());
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
        (None, Some(110.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_info() {
        let skill = SkillInfo::new("s1", "Code Review", "AI-powered code review assistant")
            .category(SkillCategory::CodeGeneration)
            .author("alice")
            .downloads(12500)
            .rating(4.7);

        assert_eq!(skill.name, "Code Review");
        assert_eq!(skill.category, SkillCategory::CodeGeneration);
    }

    #[test]
    fn test_category_labels() {
        assert_eq!(SkillCategory::CodeGeneration.label(), "Code Gen");
        assert_eq!(SkillCategory::DataAnalysis.label(), "Data Analysis");
    }

    #[test]
    fn test_format_downloads() {
        let skill = SkillInfo::new("s1", "Test", "Test skill").downloads(1500000);
        let card = SkillCard::new(skill);
        assert_eq!(card.format_downloads(), "1.5M");
    }
}
