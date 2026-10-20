//! The preferences these views read and change, kept by their host.
//!
//! The desktop keeps them in its saved settings and installs a [`Store`]
//! when it loads them. Without one, as on the web, they live in memory for
//! the session.

use gpui::{App, Global};

/// What the markdown views let a person choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Code fences wrap to the column instead of scrolling sideways.
    pub code_fences_fit_content: bool,
    /// A web link opens in the app's own browser rather than the system's.
    pub open_web_links_in_openagents: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            code_fences_fit_content: false,
            open_web_links_in_openagents: true,
        }
    }
}

/// Where the host keeps [`Prefs`]: `read` returns them, `write` saves a
/// changed copy.
#[derive(Clone, Copy)]
pub struct Store {
    pub read: fn(&App) -> Prefs,
    pub write: fn(Prefs, &mut App),
}

impl Global for Store {}

#[derive(Default)]
struct Session(Prefs);

impl Global for Session {}

/// Use `store` for every later read and change.
pub fn install(store: Store, cx: &mut App) {
    cx.set_global(store);
}

pub fn current(cx: &App) -> Prefs {
    if let Some(store) = cx.try_global::<Store>() {
        return (store.read)(cx);
    }
    cx.try_global::<Session>()
        .map(|session| session.0)
        .unwrap_or_default()
}

pub fn update(cx: &mut App, change: impl FnOnce(&mut Prefs)) {
    let mut prefs = current(cx);
    change(&mut prefs);
    if let Some(store) = cx.try_global::<Store>().copied() {
        (store.write)(prefs, cx);
    } else {
        cx.set_global(Session(prefs));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn session_prefs_change_without_a_store(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            assert_eq!(current(cx), Prefs::default());
            update(cx, |prefs| prefs.code_fences_fit_content = true);
            assert!(current(cx).code_fences_fit_content);
            assert!(current(cx).open_web_links_in_openagents);
        });
    }
}
