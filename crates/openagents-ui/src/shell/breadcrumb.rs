//! The header breadcrumb and the account button at the bottom of the left
//! panel.

use maud::{Markup, Render, html};

use crate::actions::{Avatar, AvatarSize};
use crate::overlays::{Menu, MenuItem, Side};

/// The `id` of the shell's breadcrumb, so an HTMX response can replace it
/// out of band ([`Breadcrumb::swap_oob`]) when it swaps the page content.
pub const BREADCRUMB_ID: &str = "oa-breadcrumb";

/// The page's place in the header row, on the same line as the header
/// actions: optional linked ancestors, then the current page (a chat's
/// title), truncated with an ellipsis and shown whole in its `title`.
#[derive(Clone, Debug, Default)]
pub struct Breadcrumb {
    ancestors: Vec<(String, String)>,
    current: Option<String>,
    id: Option<String>,
    oob: bool,
}

impl Breadcrumb {
    /// A breadcrumb whose current page is `current`.
    #[must_use]
    pub fn new(current: impl Into<String>) -> Self {
        Self {
            current: Some(current.into()),
            ..Self::default()
        }
    }

    /// A linked ancestor before the current page, such as a section.
    #[must_use]
    pub fn crumb(mut self, label: impl Into<String>, href: impl Into<String>) -> Self {
        self.ancestors.push((label.into(), href.into()));
        self
    }

    /// Its `id`, [`BREADCRUMB_ID`] by default (one per page).
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Marks it to replace the page's breadcrumb out of band (HTMX).
    #[must_use]
    pub fn swap_oob(mut self, oob: bool) -> Self {
        self.oob = oob;
        self
    }
}

impl Render for Breadcrumb {
    fn render(&self) -> Markup {
        html! {
            nav id=(self.id.as_deref().unwrap_or(BREADCRUMB_ID)) class="oa-breadcrumb" aria-label="Breadcrumb"
                hx-swap-oob=[self.oob.then_some("outerHTML")] {
                ol class="oa-breadcrumb-list" role="list" {
                    @for (label, href) in &self.ancestors {
                        li class="oa-breadcrumb-item" {
                            a class="oa-breadcrumb-link" href=(href) { (label) }
                        }
                    }
                    @if let Some(current) = &self.current {
                        li class="oa-breadcrumb-item" {
                            span class="oa-breadcrumb-current" aria-current="page" title=(current) {
                                (current)
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The signed-in account at the bottom of the left panel, as in ChatGPT:
/// an avatar and the account's name; a click opens a menu above it
/// (settings, billing, sign out). It is a [`Menu`], so it opens without
/// JavaScript too.
#[derive(Clone, Debug)]
pub struct AccountMenu {
    id: String,
    name: String,
    picture: Option<String>,
    items: Vec<MenuItem>,
}

impl AccountMenu {
    /// The account called `name` (a label or an email).
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: "oa-account-menu".to_owned(),
            name: name.into(),
            picture: None,
            items: Vec::new(),
        }
    }

    /// The menu list's `id` (`oa-account-menu` by default).
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// The account's picture, served from this site; the initial shows
    /// without one.
    #[must_use]
    pub fn picture(mut self, url: impl Into<String>) -> Self {
        self.picture = Some(url.into());
        self
    }

    /// A menu entry.
    #[must_use]
    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }
}

impl Render for AccountMenu {
    fn render(&self) -> Markup {
        let trigger = html! {
            (match &self.picture {
                Some(url) => Avatar::new().name(&self.name).image_url(url).size(AvatarSize::Px28),
                None => Avatar::new().name(&self.name).size(AvatarSize::Px28),
            })
            span class="oa-account-name" { (self.name) }
        };
        html! {
            div class="oa-account" {
                (Menu::new(self.id.clone(), trigger)
                    .label("Account")
                    .side(Side::Top)
                    .trigger_class("oa-account-trigger")
                    .trigger_label(format!("Account: {}", self.name))
                    .items(self.items.iter().cloned()))
            }
        }
    }
}
