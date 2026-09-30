//! Coder's notifications on Windows (#10062), as WinRT toasts
//! (`Windows.UI.Notifications`).
//!
//! [`openagents_desktop::notices`] decides when a notice shows and what it
//! says; here [`System`] is the real [`Center`] behind
//! [`openagents_desktop::notices::deliver`]. Windows never asks the person:
//! [`Center::authorize`] reads whether toasts are on for this app, and
//! [`Center::post`] shows the toast XML ([`windows_toast_xml`]: the chat's
//! title and the status, nothing else) tagged with the notice's ID
//! ([`windows_tag`]), so a newer notice for a chat replaces the older one.
//! A click on a toast while the app runs raises the toast's `Activated`
//! event with the toast's `launch` (the notice's ID), and the chat opens
//! through [`crate::native::open_chat`], the path Linux's and macOS's
//! clicks take.
//!
//! Toasts speak for an AppUserModelID ([`WINDOWS_APP_ID`]): the MSI puts it
//! on the Start menu shortcut and registers it under
//! `HKCU\Software\Classes\AppUserModelId`, and [`claim_app_id`] makes this
//! process that app at startup. A copy the MSI did not install (a `.zip`,
//! `cargo run`) has no registration: [`available`] is false and every notice
//! is dropped. There is no COM activator (`ToastActivatorCLSID`), so a click
//! from Action Center after the app quit opens nothing, as a click that
//! launches a quit app does on macOS.

use openagents_desktop::notices::{
    Center, Delivery, Notice, Permission, WINDOWS_APP_ID, WINDOWS_GROUP, deliver, windows_click,
    windows_permission, windows_tag, windows_toast_xml,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager, ToastNotifier,
};
use windows::core::{HSTRING, IInspectable, Interface};

/// The registration the MSI writes for [`WINDOWS_APP_ID`], under
/// `HKEY_CURRENT_USER`.
const REGISTRATION_KEY: &str = r"Software\Classes\AppUserModelId\OpenAgents.Desktop";

/// Makes this process [`WINDOWS_APP_ID`], the app the Start menu shortcut
/// names, so its toasts and taskbar button are that app's. Call once at
/// startup, before any window.
pub fn claim_app_id() {
    // SAFETY: a NUL-terminated wide string that outlives the call.
    let _ = unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(
            WINDOWS_APP_ID,
        ))
    };
}

/// Whether the MSI registered [`WINDOWS_APP_ID`] for this user, so Windows
/// will show this app's toasts.
fn available() -> bool {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() };
    let (key, value) = (wide(REGISTRATION_KEY), wide("DisplayName"));
    let mut bytes = 0u32;
    // SAFETY: a size query with no buffer, on NUL-terminated wide strings.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    status == ERROR_SUCCESS && bytes > 2
}

/// Joins this thread to the multithreaded apartment WinRT needs. A thread
/// already in an apartment keeps it.
fn enter_apartment() {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    // SAFETY: no reserved pointer; S_FALSE and RPC_E_CHANGED_MODE both
    // leave the thread in an apartment WinRT accepts.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

fn notifier() -> windows::core::Result<ToastNotifier> {
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(WINDOWS_APP_ID))
}

/// The toasts shown, by tag: a toast's `Activated` handler fires only while
/// the toast lives, so each is kept until a newer one for its chat
/// replaces it.
static SHOWN: Mutex<Option<HashMap<String, ToastNotification>>> = Mutex::new(None);

/// Opens the chat a click on a toast names.
fn activated(args: windows::core::Ref<IInspectable>) -> windows::core::Result<()> {
    let arguments = args
        .as_ref()
        .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
        .and_then(|args| args.Arguments().ok())
        .map(|arguments| arguments.to_string_lossy())
        .unwrap_or_default();
    if let Some(chat) = windows_click(&arguments) {
        crate::native::open_chat(chat.to_owned());
    }
    Ok(())
}

fn show(notice: &Notice) -> windows::core::Result<()> {
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(windows_toast_xml(notice)))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let tag = windows_tag(&notice.id);
    toast.SetTag(&HSTRING::from(&tag))?;
    toast.SetGroup(&HSTRING::from(WINDOWS_GROUP))?;
    toast.Activated(&TypedEventHandler::new(
        |_: windows::core::Ref<ToastNotification>, args| activated(args),
    ))?;
    notifier()?.Show(&toast)?;
    if let Ok(mut shown) = SHOWN.lock() {
        shown.get_or_insert_with(HashMap::new).insert(tag, toast);
    }
    Ok(())
}

/// Windows's toasts as a [`Center`]. Both calls answer on the calling
/// thread, which must be in an apartment ([`enter_apartment`]).
struct System;

impl Center for System {
    fn authorize(&self, then: Box<dyn FnOnce(Permission) + Send>) {
        if !available() {
            then(Permission::Unavailable);
            return;
        }
        then(match notifier().and_then(|notifier| notifier.Setting()) {
            Ok(setting) => windows_permission(setting.0),
            Err(_) => Permission::Unavailable,
        });
    }

    fn post(&self, notice: &Notice, then: Box<dyn FnOnce(Result<(), String>) + Send>) {
        then(show(notice).map_err(|error| error.message()));
    }
}

/// Shows `notice` on a thread of its own; nothing waits for it.
pub fn notify(notice: Notice) {
    let _ = std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || {
            enter_apartment();
            deliver(Arc::new(System), notice, |_| {});
        });
}

/// Nothing to start: each toast carries its own click handler.
pub fn listen_notifications() {}

/// Shows `notice` and says how it went: `Some` names the service, `None`
/// when nothing was shown and why is printed.
pub fn notify_now(notice: &Notice) -> Option<&'static str> {
    enter_apartment();
    let (tx, rx) = std::sync::mpsc::channel();
    deliver(Arc::new(System), notice.clone(), move |delivery| {
        let _ = tx.send(delivery);
    });
    match rx.recv() {
        Ok(Delivery::Shown) => Some("Windows toast notifications (ToastNotificationManager)"),
        Ok(Delivery::Denied) => {
            eprintln!(
                "notifications are off for OpenAgents in Settings > System > Notifications \
                 (or for this user, or by group policy)"
            );
            None
        }
        Ok(Delivery::Unavailable) => {
            eprintln!(
                "OpenAgents is not installed from its MSI (no AppUserModelID {WINDOWS_APP_ID} \
                 registered): Windows shows toasts only for an installed app"
            );
            None
        }
        Ok(Delivery::Failed(reason)) => {
            eprintln!("Windows refused the toast: {reason}");
            None
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_registration_key_names_the_app_id() {
        assert!(super::REGISTRATION_KEY.ends_with(openagents_desktop::notices::WINDOWS_APP_ID));
    }

    #[test]
    fn the_toast_xml_parses_as_windows_reads_it() {
        super::enter_apartment();
        let notice = openagents_desktop::notices::Notice {
            id: "coder-c1".into(),
            title: "Fix <login> & more".into(),
            body: "Coder finished".into(),
            urgent: false,
        };
        let Ok(xml) = super::XmlDocument::new() else {
            // No WinRT here (Wine): nothing to parse with.
            return;
        };
        xml.LoadXml(&super::HSTRING::from(
            openagents_desktop::notices::windows_toast_xml(&notice),
        ))
        .unwrap();
    }
}
