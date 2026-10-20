//! The motion catalog is `openagents_chat_view::motion`, shared with the web.
//! This module adds what only the desktop has: reading the OS reduced-motion
//! setting and saving the user's choices to `ui-settings.json`.

use gpui::App;

pub use openagents_chat_view::motion::*;

/// Install the motion state, read the OS setting, and set the flag before the
/// first window opens, so boot animations already honor it.
pub fn init(preference: ReduceMotion, pause_in_background: bool, cx: &mut App) {
    openagents_chat_view::motion::init(preference, pause_in_background, cx);
    refresh_system(cx);
}

pub fn set_preference(preference: ReduceMotion, cx: &mut App) {
    if !has_state(cx) || openagents_chat_view::motion::preference(cx) == preference {
        return;
    }
    openagents_chat_view::motion::set_preference(preference, cx);
    crate::settings::update(crate::settings::SavePolicy::Immediate, cx, |settings| {
        settings.reduce_motion = preference;
    });
}

pub fn set_pause_in_background(enabled: bool, cx: &mut App) {
    if !has_state(cx) || openagents_chat_view::motion::pause_in_background(cx) == enabled {
        return;
    }
    openagents_chat_view::motion::set_pause_in_background(enabled, cx);
    crate::settings::update(crate::settings::SavePolicy::Immediate, cx, |settings| {
        settings.pause_animations_in_background = enabled;
    });
}

/// Main-window focus moved. Regaining focus also re-reads the OS setting:
/// changing it means visiting System Settings, so coming back is the moment
/// it can have moved, without holding a platform notification observer.
pub fn window_activation_changed(active: bool, cx: &mut App) {
    // Only the app installs motion state. Test windows activate too, and the
    // Linux portal read would wake the deterministic scheduler from a D-Bus
    // thread.
    if !has_state(cx) {
        return;
    }
    if active {
        refresh_system(cx);
    }
    openagents_chat_view::motion::window_activation_changed(active, cx);
}

#[cfg(target_os = "macos")]
fn refresh_system(cx: &mut App) {
    use objc::runtime::{NO, Object};
    use objc::{class, msg_send, sel, sel_impl};
    let reduce = unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduce: objc::runtime::BOOL =
            msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce != NO
    };
    set_system(reduce, cx);
}

/// "Show animations in Windows" (Settings → Accessibility → Visual effects).
#[cfg(windows)]
fn refresh_system(cx: &mut App) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut enabled: windows_sys::core::BOOL = 1;
    let read = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0)
    };
    set_system(read != 0 && enabled == 0, cx);
}

/// The desktop portal's `reduced-motion` key, which GNOME and KDE map from
/// their own animation settings. A D-Bus round trip, so it stays off the
/// foreground; portals without the key read as no preference.
#[cfg(target_os = "linux")]
fn refresh_system(cx: &mut App) {
    use ashpd::desktop::settings::{ReducedMotion, Settings};
    use gpui::AppContext as _;
    let read = cx.background_spawn(async {
        let settings = Settings::new().await.ok()?;
        let motion = settings.reduced_motion().await.ok()?;
        Some(motion == ReducedMotion::ReducedMotion)
    });
    cx.spawn(async move |cx| {
        let reduce = read.await.unwrap_or(false);
        cx.update(|cx| set_system(reduce, cx));
    })
    .detach();
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn refresh_system(_cx: &mut App) {}
