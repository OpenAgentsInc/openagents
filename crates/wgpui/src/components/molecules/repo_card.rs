//! Repository card molecule for displaying repository summaries.
//!
//! Shows repository name, description, stats, and status.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Repository visibility
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepoVisibility {
    Public,
    Private,
}

impl RepoVisibility {
    pub fn label(&self) -> &'static str {
        match self {
            RepoVisibility::Public => "Public",
            RepoVisibility::Private => "Private",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            RepoVisibility::Public => Hsla::new(120.0, 0.6, 0.45, 1.0), // Green
            RepoVisibility::Private => Hsla::new(45.0, 0.7, 0.5, 1.0),  // Yellow
        }
    }
}

/// Repository info
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub visibility: RepoVisibility,
    pub stars: u32,
    pub forks: u32,
    pub issues: u32,
    pub language: Option<String>,
    pub updated_at: String,
}

impl RepoInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            visibility: RepoVisibility::Public,
            stars: 0,
            forks: 0,
            issues: 0,
            language: None,
            updated_at: "Just now".to_string(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn visibility(mut self, visibility: RepoVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn stars(mut self, count: u32) -> Self {
        self.stars = count;
        self
    }

    pub fn forks(mut self, count: u32) -> Self {
        self.forks = count;
        self
    }

    pub fn issues(mut self, count: u32) -> Self {
        self.issues = count;
        self
    }

    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    pub fn updated_at(mut self, time: impl Into<String>) -> Self {
        self.updated_at = time.into();
        self
    }
}

/// Repository card component
pub struct RepoCard {
    id: Option<ComponentId>,
    repo: RepoInfo,
    hovered: bool,
    on_click: Option<Box<dyn FnMut(String)>>,
}

impl RepoCard {
    pub fn new(repo: RepoInfo) -> Self {
        Self {
            id: None,
            repo,
            hovered: false,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    fn language_color(&self) -> Hsla {
        match self.repo.language.as_deref() {
            Some("Rust") => Hsla::new(25.0, 0.8, 0.5, 1.0),
            Some("TypeScript") => Hsla::new(200.0, 0.7, 0.5, 1.0),
            Some("JavaScript") => Hsla::new(50.0, 0.8, 0.5, 1.0),
            Some("Python") => Hsla::new(210.0, 0.5, 0.5, 1.0),
            Some("Go") => Hsla::new(190.0, 0.6, 0.5, 1.0),
            _ => theme::text::MUTED,
        }
    }
}

impl Component for RepoCard {
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

        // Repo name
        let name_run = cx.text.layout_mono(
            &self.repo.name,
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Visibility badge
        let vis_x = bounds.origin.x + bounds.size.width - padding - 50.0;
        let vis_bounds = Bounds::new(vis_x, y - 2.0, 50.0, 18.0);
        cx.scene.draw_quad(
            Quad::new(vis_bounds)
                .with_background(self.repo.visibility.color().with_alpha(0.2))
                .with_border(self.repo.visibility.color(), 1.0),
        );
        let vis_label = cx.text.layout_mono(
            self.repo.visibility.label(),
            Point::new(vis_x + 6.0, y),
            theme::font_size::XS,
            self.repo.visibility.color(),
        );
        cx.scene.draw_text(vis_label);

        y += 24.0;

        // Description
        if let Some(desc) = &self.repo.description {
            let desc_truncated = if desc.len() > 60 {
                format!("{}...", &desc[..57])
            } else {
                desc.clone()
            };
            let desc_run = cx.text.layout_mono(
                &desc_truncated,
                Point::new(bounds.origin.x + padding, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc_run);
            y += 20.0;
        }

        y += 8.0;

        // Stats row
        let stats_y = y;

        // Language
        if let Some(lang) = &self.repo.language {
            let dot_bounds = Bounds::new(bounds.origin.x + padding, stats_y + 4.0, 8.0, 8.0);
            cx.scene
                .draw_quad(Quad::new(dot_bounds).with_background(self.language_color()));
            let lang_run = cx.text.layout_mono(
                lang,
                Point::new(bounds.origin.x + padding + 14.0, stats_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(lang_run);
        }

        // Stars
        let stars_x = bounds.origin.x + 120.0;
        let stars_text = format!("\u{2605} {}", self.repo.stars);
        let stars_run = cx.text.layout_mono(
            &stars_text,
            Point::new(stars_x, stats_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(stars_run);

        // Forks
        let forks_x = stars_x + 60.0;
        let forks_text = format!("\u{2442} {}", self.repo.forks);
        let forks_run = cx.text.layout_mono(
            &forks_text,
            Point::new(forks_x, stats_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(forks_run);

        // Issues
        let issues_x = forks_x + 60.0;
        let issues_text = format!("\u{25CB} {}", self.repo.issues);
        let issues_run = cx.text.layout_mono(
            &issues_text,
            Point::new(issues_x, stats_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(issues_run);

        // Updated at
        let updated_run = cx.text.layout_mono(
            &self.repo.updated_at,
            Point::new(
                bounds.origin.x + bounds.size.width - padding - 80.0,
                stats_y,
            ),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(updated_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_click {
                        callback(self.repo.id.clone());
                    }
                    return EventResult::Handled;
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
        let height = if self.repo.description.is_some() {
            100.0
        } else {
            80.0
        };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_info() {
        let repo = RepoInfo::new("1", "openagents")
            .description("An open source AI agent framework")
            .stars(100)
            .forks(25)
            .language("Rust");

        assert_eq!(repo.name, "openagents");
        assert_eq!(repo.stars, 100);
    }

    #[test]
    fn test_visibility() {
        assert_eq!(RepoVisibility::Public.label(), "Public");
        assert_eq!(RepoVisibility::Private.label(), "Private");
    }
}
