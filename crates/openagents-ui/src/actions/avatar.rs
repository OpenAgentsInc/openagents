//! Avatar and AvatarGroup, ported from Apps SDK UI `src/components/Avatar`
//! (MIT). Styles: `static/components/avatar.css`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag, safe_url};
use super::{Color, Variant};

/// Avatar diameter in pixels. React takes any number through an inline
/// `--avatar-size`; the site CSP forbids inline styles, so sizes are presets
/// written as `data-avatar-size` and mapped in `avatar.css`. Without one the
/// `--avatar-size` token applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AvatarSize {
    Px16,
    Px20,
    Px24,
    Px28,
    Px32,
    Px36,
    Px40,
    Px48,
    Px56,
    Px64,
    Px80,
    Px96,
}

impl AvatarSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Px16 => "16",
            Self::Px20 => "20",
            Self::Px24 => "24",
            Self::Px28 => "28",
            Self::Px32 => "32",
            Self::Px36 => "36",
            Self::Px40 => "40",
            Self::Px48 => "48",
            Self::Px56 => "56",
            Self::Px64 => "64",
            Self::Px80 => "80",
            Self::Px96 => "96",
        }
    }
}

/// A round avatar showing, in order of preference, an image, an icon, an
/// overflow count ("+12"), or the first initial of `name`.
///
/// When an image is given, the initial is rendered underneath it, so a
/// broken image (empty `alt`) falls back to the initial with no script.
#[derive(Clone, Debug)]
pub struct Avatar {
    name: Option<String>,
    image_url: Option<String>,
    icon: Option<Markup>,
    overflow_count: Option<u64>,
    color: Color,
    variant: Variant,
    size: Option<AvatarSize>,
    interactive: bool,
    attrs: Attrs,
}

impl Default for Avatar {
    fn default() -> Self {
        Self::new()
    }
}

impl Avatar {
    pub fn new() -> Self {
        Self {
            name: None,
            image_url: None,
            icon: None,
            overflow_count: None,
            color: Color::Secondary,
            variant: Variant::Soft,
            size: None,
            interactive: false,
            attrs: Attrs::default(),
        }
    }

    /// Name the initial is taken from.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn image_url(mut self, url: impl Into<String>) -> Self {
        let url = url.into();
        // Gravatar's auth0 fallbacks draw two initials; Apps SDK UI skips them.
        if !url.is_empty() && !(url.contains("gravatar.com") && url.contains("cdn.auth0.com")) {
            self.image_url = Some(url);
        }
        self
    }

    pub fn icon(mut self, icon: impl Render) -> Self {
        self.icon = Some(icon.render());
        self
    }

    /// Shows a compact count such as "+12" or "+2k".
    pub fn overflow_count(mut self, count: u64) -> Self {
        self.overflow_count = Some(count);
        self
    }

    /// Primary, Secondary (default), Success, Info, Discovery or Danger.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Soft (default) or Solid.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Renders a `<button type="button">` instead of a presentational span.
    /// Give it an `aria-label` through `.attr(..)`.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }
}

impl_attrs!(Avatar);

/// `Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 0 })`,
/// lowercased.
pub(crate) fn compact_count(count: u64) -> String {
    const UNITS: [(u64, &str); 4] = [
        (1_000_000_000_000, "t"),
        (1_000_000_000, "b"),
        (1_000_000, "m"),
        (1_000, "k"),
    ];
    if count < 1_000 {
        return count.to_string();
    }
    for (index, (scale, suffix)) in UNITS.iter().enumerate() {
        if count >= *scale {
            let rounded = (count as f64 / *scale as f64).round() as u64;
            // 999,500 rounds to 1000k; promote to the next unit.
            if rounded >= 1_000 && index > 0 {
                let (_, bigger) = UNITS[index - 1];
                return format!("1{bigger}");
            }
            return format!("{rounded}{suffix}");
        }
    }
    count.to_string()
}

impl Render for Avatar {
    fn render(&self) -> Markup {
        let tag = if self.interactive {
            Tag::new("button", "oa-avatar", &self.attrs)
                .attr("data-color", self.color.as_str())
                .attr("data-variant", self.variant.as_str())
                .attr("type", "button")
        } else {
            Tag::new("span", "oa-avatar", &self.attrs)
                .attr("role", "presentation")
                .attr("data-color", self.color.as_str())
                .attr("data-variant", self.variant.as_str())
        };
        let initial: String = self
            .name
            .as_deref()
            .and_then(|name| name.trim().chars().next())
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let fallback = html! {
            @if let Some(icon) = &self.icon {
                span class="oa-avatar-icon" { (icon) }
            } @else if let Some(count) = self.overflow_count.filter(|count| *count > 0) {
                @let formatted = compact_count(count);
                span class="oa-avatar-overflow-count" data-letter-count=(formatted.len()) {
                    span class="oa-avatar-overflow-count-symbol" { "+" }
                    (formatted)
                }
            } @else {
                span class="oa-avatar-initial" { (initial) }
            }
        };
        tag.attr_opt("data-avatar-size", self.size.map(AvatarSize::as_str))
            .extra(&self.attrs)
            .close(html! {
                (fallback)
                @if let Some(url) = &self.image_url {
                    span class="oa-avatar-image-container" {
                        img src=(safe_url(url)) class="oa-avatar-image" data-loaded alt="" role="presentation";
                    }
                }
            })
    }
}

/// Which end of an AvatarGroup sits on top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AvatarStack {
    #[default]
    Start,
    End,
}

impl AvatarStack {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// Overlapping avatars. With `Start` stacking (default) the first avatar
/// is on top: children are written in reverse into a `row-reverse` flex row,
/// as the React component does.
#[derive(Clone, Debug, Default)]
pub struct AvatarGroup {
    avatars: Vec<Avatar>,
    stack: AvatarStack,
    size: Option<AvatarSize>,
    attrs: Attrs,
}

impl AvatarGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn avatar(mut self, avatar: Avatar) -> Self {
        self.avatars.push(avatar);
        self
    }

    pub fn avatars(mut self, avatars: impl IntoIterator<Item = Avatar>) -> Self {
        self.avatars.extend(avatars);
        self
    }

    pub fn stack(mut self, stack: AvatarStack) -> Self {
        self.stack = stack;
        self
    }

    /// Sizes every avatar in the group.
    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = Some(size);
        self
    }
}

impl_attrs!(AvatarGroup);

impl Render for AvatarGroup {
    fn render(&self) -> Markup {
        let ordered: Vec<&Avatar> = match self.stack {
            AvatarStack::Start => self.avatars.iter().rev().collect(),
            AvatarStack::End => self.avatars.iter().collect(),
        };
        Tag::new("div", "oa-avatar-group", &self.attrs)
            .attr("data-stack", self.stack.as_str())
            .attr_opt("data-avatar-size", self.size.map(AvatarSize::as_str))
            .extra(&self.attrs)
            .close(html! {
                @for avatar in ordered { (avatar) }
            })
    }
}
