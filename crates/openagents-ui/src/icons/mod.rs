//! Apps SDK UI icons as a typed Rust enum.
//!
//! `Icon` has one variant per upstream icon, so a misspelled icon is a compile
//! error rather than a missing image at runtime:
//!
//! ```compile_fail
//! let _ = openagents_ui::icons::Icon::AddMembr;
//! ```
//!
//! Rendering emits inline SVG with `class="oa-icon"`, `aria-hidden="true"`,
//! `focusable="false"` and `currentColor`, so an icon takes the text color of
//! its parent. Multi-color brand icons (for example `Zendesk`) keep their own
//! fills. Without a size the icon is `1em`; [`Icon::size`] sets `data-size`,
//! which `static/components/icon.css` maps to the `--control-icon-size-*`
//! tokens.
//!
//! ```
//! use openagents_ui::icons::{Icon, IconSize};
//! let markup = maud::html! { (Icon::Check) (Icon::Search.size(IconSize::Sm)) };
//! assert!(markup.into_string().contains(r#"data-size="sm""#));
//! ```
//!
//! `generated.rs` is produced by `scripts/generate-openagents-ui-icons.py`
//! from `apps-sdk-ui/src/components/Icon/svg` (MIT). Do not edit it by hand.

#[rustfmt::skip]
mod generated;

pub use generated::Icon;

/// Icon sizes, matching the Apps SDK UI `icon-*` utilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconSize {
    /// 14px (`--control-icon-size-xs`).
    Xs,
    /// 16px (`--control-icon-size-sm`).
    Sm,
    /// 18px (`--control-icon-size-md`).
    Md,
    /// 20px (`--control-icon-size-lg`).
    Lg,
    /// 22px (`--control-icon-size-xl`).
    Xl,
    /// 24px (`--control-icon-size-2xl`).
    Xxl,
}

impl IconSize {
    /// The `data-size` attribute value.
    pub const fn as_str(self) -> &'static str {
        match self {
            IconSize::Xs => "xs",
            IconSize::Sm => "sm",
            IconSize::Md => "md",
            IconSize::Lg => "lg",
            IconSize::Xl => "xl",
            IconSize::Xxl => "2xl",
        }
    }
}

impl Icon {
    /// This icon at a fixed size.
    pub const fn size(self, size: IconSize) -> SizedIcon {
        SizedIcon { icon: self, size }
    }

    /// Look up an icon by its upstream name, for catalogs and tooling.
    /// Application code should name the variant directly.
    pub fn from_name(name: &str) -> Option<Icon> {
        Icon::ALL.into_iter().find(|icon| icon.name() == name)
    }

    fn write_svg(self, size: Option<IconSize>, buffer: &mut String) {
        let (attrs, body) = self.parts();
        buffer.push_str(r#"<svg class="oa-icon""#);
        if let Some(size) = size {
            buffer.push_str(r#" data-size=""#);
            buffer.push_str(size.as_str());
            buffer.push('"');
        }
        buffer.push_str(r#" aria-hidden="true" focusable="false" width="1em" height="1em""#);
        buffer.push_str(attrs);
        buffer.push('>');
        buffer.push_str(body);
        buffer.push_str("</svg>");
    }
}

impl maud::Render for Icon {
    fn render_to(&self, buffer: &mut String) {
        self.write_svg(None, buffer);
    }
}

/// An [`Icon`] with an explicit [`IconSize`], built by [`Icon::size`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizedIcon {
    pub icon: Icon,
    pub size: IconSize,
}

impl maud::Render for SizedIcon {
    fn render_to(&self, buffer: &mut String) {
        self.icon.write_svg(Some(self.size), buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maud::Render;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_complete_and_unique() {
        assert_eq!(Icon::ALL.len(), 755);
        let names: HashSet<_> = Icon::ALL.iter().map(|icon| icon.name()).collect();
        assert_eq!(names.len(), Icon::ALL.len());
        assert_eq!(Icon::from_name("AddMember"), Some(Icon::AddMember));
        assert_eq!(Icon::from_name("NoSuchIcon"), None);
    }

    #[test]
    fn plain_icon_uses_current_color() {
        let html = Icon::AddMember.render().into_string();
        assert!(html.starts_with(
            r#"<svg class="oa-icon" aria-hidden="true" focusable="false" width="1em" height="1em" viewBox="0 0 24 24" fill="currentColor">"#
        ));
        assert!(html.contains(r#"<path d="M11.4998 7.5C"#));
        assert!(html.ends_with("</svg>"));
        assert!(
            !html.contains('#'),
            "plain icon should have no fixed colors"
        );
    }

    #[test]
    fn brand_icon_keeps_fills_and_scopes_ids() {
        let html = Icon::Zendesk.render().into_string();
        assert!(html.contains(r##"fill="#16140C""##));
        assert!(html.contains(r##"clip-path="url(#oa-icon-Zendesk-clip0_8150_7114)""##));
        assert!(html.contains(r#"<clipPath id="oa-icon-Zendesk-clip0_8150_7114">"#));
    }

    #[test]
    fn sized_icon_sets_data_size() {
        let html = Icon::Dot.size(IconSize::Xxl).render().into_string();
        assert!(html.starts_with(r#"<svg class="oa-icon" data-size="2xl" aria-hidden="true""#));
        assert!(html.contains(r#"stroke-width="2""#));
    }

    #[test]
    fn every_icon_renders_well_formed_svg() {
        for icon in Icon::ALL {
            let html = icon.render().into_string();
            assert!(html.starts_with("<svg "), "{}", icon.name());
            assert!(html.ends_with("</svg>"), "{}", icon.name());
            assert_eq!(html.matches("<svg").count(), 1, "{}", icon.name());
            assert!(
                !html.contains('{') && !html.contains("props"),
                "{}",
                icon.name()
            );
            assert!(
                !html.contains("Rule=") && !html.contains("strokeW"),
                "{}",
                icon.name()
            );
        }
    }
}
