//! Contact card molecule for displaying Nostr contacts.
//!
//! Shows contact profile with npub, avatar placeholder, and interaction options.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Contact verification status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactVerification {
    Verified,
    WebOfTrust,
    Unknown,
}

impl ContactVerification {
    pub fn label(&self) -> &'static str {
        match self {
            ContactVerification::Verified => "Verified",
            ContactVerification::WebOfTrust => "WoT",
            ContactVerification::Unknown => "Unknown",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ContactVerification::Verified => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            ContactVerification::WebOfTrust => Hsla::new(200.0, 0.6, 0.5, 1.0), // Blue
            ContactVerification::Unknown => theme::text::MUTED,
        }
    }
}

/// Contact info
#[derive(Debug, Clone)]
pub struct ContactInfo {
    pub npub: String,
    pub display_name: Option<String>,
    pub nip05: Option<String>,
    pub picture_url: Option<String>,
    pub about: Option<String>,
    pub verification: ContactVerification,
    pub following: bool,
    pub mutual: bool,
}

impl ContactInfo {
    pub fn new(npub: impl Into<String>) -> Self {
        Self {
            npub: npub.into(),
            display_name: None,
            nip05: None,
            picture_url: None,
            about: None,
            verification: ContactVerification::Unknown,
            following: false,
            mutual: false,
        }
    }

    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn nip05(mut self, nip05: impl Into<String>) -> Self {
        self.nip05 = Some(nip05.into());
        self
    }

    pub fn picture(mut self, url: impl Into<String>) -> Self {
        self.picture_url = Some(url.into());
        self
    }

    pub fn about(mut self, about: impl Into<String>) -> Self {
        self.about = Some(about.into());
        self
    }

    pub fn verification(mut self, verification: ContactVerification) -> Self {
        self.verification = verification;
        self
    }

    pub fn following(mut self, following: bool) -> Self {
        self.following = following;
        self
    }

    pub fn mutual(mut self, mutual: bool) -> Self {
        self.mutual = mutual;
        self
    }

    fn short_npub(&self) -> String {
        if self.npub.len() > 20 {
            format!(
                "{}...{}",
                &self.npub[..12],
                &self.npub[self.npub.len() - 8..]
            )
        } else {
            self.npub.clone()
        }
    }
}

/// Contact card component
pub struct ContactCard {
    id: Option<ComponentId>,
    contact: ContactInfo,
    hovered: bool,
    follow_hovered: bool,
    dm_hovered: bool,
    on_follow: Option<Box<dyn FnMut(String)>>,
    on_dm: Option<Box<dyn FnMut(String)>>,
    on_view: Option<Box<dyn FnMut(String)>>,
}

impl ContactCard {
    pub fn new(contact: ContactInfo) -> Self {
        Self {
            id: None,
            contact,
            hovered: false,
            follow_hovered: false,
            dm_hovered: false,
            on_follow: None,
            on_dm: None,
            on_view: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_follow<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_follow = Some(Box::new(f));
        self
    }

    pub fn on_dm<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_dm = Some(Box::new(f));
        self
    }

    pub fn on_view<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_view = Some(Box::new(f));
        self
    }

    fn follow_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 150.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            70.0,
            24.0,
        )
    }

    fn dm_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 70.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            60.0,
            24.0,
        )
    }
}

impl Component for ContactCard {
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

        // Avatar placeholder (circle with initial)
        let avatar_size = 40.0;
        let avatar_bounds = Bounds::new(bounds.origin.x + padding, y, avatar_size, avatar_size);
        cx.scene.draw_quad(
            Quad::new(avatar_bounds)
                .with_background(theme::accent::PRIMARY.with_alpha(0.3))
                .with_border(theme::accent::PRIMARY, 1.0),
        );

        // Initial letter
        let initial = self
            .contact
            .display_name
            .as_ref()
            .and_then(|n| n.chars().next())
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        let initial_run = cx.text.layout(
            &initial,
            Point::new(bounds.origin.x + padding + 14.0, y + 10.0),
            theme::font_size::SM,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(initial_run);

        // Name and verification
        let name_x = bounds.origin.x + padding + avatar_size + 12.0;
        let name = self.contact.display_name.as_deref().unwrap_or("Unknown");
        let name_run = cx.text.layout(
            name,
            Point::new(name_x, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Verification badge
        if self.contact.verification != ContactVerification::Unknown {
            let ver_x = name_x + (name.len() as f32 * 7.0) + 8.0;
            let ver_w = (self.contact.verification.label().len() as f32 * 6.0) + 8.0;
            let ver_bounds = Bounds::new(ver_x, y - 1.0, ver_w, 16.0);
            cx.scene.draw_quad(
                Quad::new(ver_bounds)
                    .with_background(self.contact.verification.color().with_alpha(0.2))
                    .with_border(self.contact.verification.color(), 1.0),
            );
            let ver_run = cx.text.layout(
                self.contact.verification.label(),
                Point::new(ver_x + 3.0, y),
                theme::font_size::XS,
                self.contact.verification.color(),
            );
            cx.scene.draw_text(ver_run);
        }

        // Mutual badge
        if self.contact.mutual {
            let mutual_x = bounds.origin.x + bounds.size.width - padding - 60.0;
            let mutual_run = cx.text.layout(
                "Mutual",
                Point::new(mutual_x, y),
                theme::font_size::XS,
                Hsla::new(270.0, 0.6, 0.55, 1.0),
            );
            cx.scene.draw_text(mutual_run);
        }

        y += 18.0;

        // NIP-05 or npub
        let identifier = self
            .contact
            .nip05
            .clone()
            .unwrap_or_else(|| self.contact.short_npub());
        let id_run = cx.text.layout(
            &identifier,
            Point::new(name_x, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(id_run);

        y += 18.0;

        // About (truncated)
        if let Some(about) = &self.contact.about {
            let about_truncated = if about.len() > 50 {
                format!("{}...", &about[..47])
            } else {
                about.clone()
            };
            let about_run = cx.text.layout(
                &about_truncated,
                Point::new(name_x, y),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(about_run);
        }

        // Follow button
        let follow_bounds = self.follow_button_bounds(&bounds);
        let follow_text = if self.contact.following {
            "Following"
        } else {
            "Follow"
        };
        let follow_color = if self.contact.following {
            Hsla::new(120.0, 0.6, 0.45, 1.0)
        } else {
            theme::accent::PRIMARY
        };
        let follow_bg = if self.follow_hovered {
            follow_color.with_alpha(0.3)
        } else {
            follow_color.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(follow_bounds)
                .with_background(follow_bg)
                .with_border(follow_color, 1.0),
        );
        let follow_run = cx.text.layout(
            follow_text,
            Point::new(follow_bounds.origin.x + 8.0, follow_bounds.origin.y + 5.0),
            theme::font_size::XS,
            follow_color,
        );
        cx.scene.draw_text(follow_run);

        // DM button
        let dm_bounds = self.dm_button_bounds(&bounds);
        let dm_bg = if self.dm_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(dm_bounds)
                .with_background(dm_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let dm_run = cx.text.layout(
            "DM",
            Point::new(dm_bounds.origin.x + 18.0, dm_bounds.origin.y + 5.0),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(dm_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let follow_bounds = self.follow_button_bounds(&bounds);
        let dm_bounds = self.dm_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_follow = self.follow_hovered;
                let was_dm = self.dm_hovered;

                self.hovered = bounds.contains(point);
                self.follow_hovered = follow_bounds.contains(point);
                self.dm_hovered = dm_bounds.contains(point);

                if was_hovered != self.hovered
                    || was_follow != self.follow_hovered
                    || was_dm != self.dm_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if follow_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_follow {
                            callback(self.contact.npub.clone());
                        }
                        return EventResult::Handled;
                    }

                    if dm_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_dm {
                            callback(self.contact.npub.clone());
                        }
                        return EventResult::Handled;
                    }

                    if bounds.contains(point) {
                        if let Some(callback) = &mut self.on_view {
                            callback(self.contact.npub.clone());
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
        (None, Some(90.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_info() {
        let contact = ContactInfo::new("npub1abc123...")
            .display_name("Alice")
            .nip05("alice@example.com")
            .verification(ContactVerification::Verified)
            .following(true);

        assert_eq!(contact.display_name, Some("Alice".to_string()));
        assert!(contact.following);
    }

    #[test]
    fn test_verification_labels() {
        assert_eq!(ContactVerification::Verified.label(), "Verified");
        assert_eq!(ContactVerification::WebOfTrust.label(), "WoT");
    }

    #[test]
    fn test_short_npub() {
        let contact = ContactInfo::new("npub1qwertyuiopasdfghjklzxcvbnm123456789");
        assert!(contact.short_npub().contains("..."));
    }
}
