//! Coder's notifications on macOS (#10061), through the notification
//! center (`UNUserNotificationCenter`).
//!
//! [`openagents_desktop::notices`] decides when a notice shows and what it
//! says; here [`System`] is the real [`Center`] behind
//! [`openagents_desktop::notices::deliver`]: it asks the person for
//! permission on the first notice (never at launch) and posts each notice
//! under its ID, so a newer notice for a chat replaces the older one. A
//! click on a notice's body reaches [`Delegate`], which opens that chat
//! through [`crate::native::open_chat`], the path Linux's clicks take.
//!
//! The notification center speaks for an app bundle: run outside one (as
//! `cargo run` does) there is no center, [`available`] is false, and every
//! notice is dropped instead of crashing on the missing bundle.

use openagents_desktop::notices::{Center, Delivery, Notice, Permission, deliver, mac_click};
use std::sync::Arc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, define_class, msg_send};
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

/// Whether this process is an app bundle the notification center can
/// speak for.
fn available() -> bool {
    crate::mac::bundle_contents().is_some() && NSBundle::mainBundle().bundleIdentifier().is_some()
}

/// The notification center, `None` outside an app bundle.
fn center() -> Option<Retained<UNUserNotificationCenter>> {
    available().then(UNUserNotificationCenter::currentNotificationCenter)
}

/// macOS's notification center as a [`Center`].
struct System;

impl Center for System {
    fn authorize(&self, then: Box<dyn FnOnce(Permission) + Send>) {
        let Some(center) = center() else {
            then(Permission::Unavailable);
            return;
        };
        let then = std::sync::Mutex::new(Some(then));
        let block = RcBlock::new(move |granted: Bool, _: *mut NSError| {
            if let Some(then) = then.lock().ok().and_then(|mut then| then.take()) {
                then(if granted.as_bool() {
                    Permission::Granted
                } else {
                    Permission::Denied
                });
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &block,
        );
    }

    fn post(&self, notice: &Notice, then: Box<dyn FnOnce(Result<(), String>) + Send>) {
        let Some(center) = center() else {
            then(Err("no notification center".into()));
            return;
        };
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notice.title));
        content.setBody(&NSString::from_str(&notice.body));
        if notice.urgent {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&notice.id),
            &content,
            None,
        );
        let then = std::sync::Mutex::new(Some(then));
        let block = RcBlock::new(move |error: *mut NSError| {
            if let Some(then) = then.lock().ok().and_then(|mut then| then.take()) {
                // SAFETY: the center passes a valid NSError or null.
                let error = unsafe { error.as_ref() };
                then(error.map_or(Ok(()), |error| {
                    Err(error.localizedDescription().to_string())
                }));
            }
        });
        center.addNotificationRequest_withCompletionHandler(&request, Some(&block));
    }
}

define_class!(
    /// Hears clicks on this app's notices.
    #[unsafe(super(NSObject))]
    #[name = "OpenAgentsNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        /// A notice arrives only while the window is away, so show it even
        /// if the app happens to be active.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound,));
        }

        /// A click on a notice's body opens its chat.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &block2::DynBlock<dyn Fn()>,
        ) {
            let id = response.notification().request().identifier().to_string();
            let action = response.actionIdentifier().to_string();
            if let Some(chat) = mac_click(&id, &action) {
                crate::native::open_chat(chat.to_owned());
            }
            completion.call(());
        }
    }
);

impl Delegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY: NSObject's `init` on a freshly allocated instance.
        unsafe { msg_send![super(this), init] }
    }
}

/// Shows `notice` on a background queue; nothing waits for it.
pub fn notify(notice: Notice) {
    deliver(Arc::new(System), notice, |_| {});
}

/// Listens for clicks on this app's notices. Call once, on the main thread,
/// as the window starts; asks the person nothing.
pub fn listen_notifications() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let Some(center) = center() else {
            return;
        };
        // The center holds its delegate weakly: this one lives for the
        // app's life.
        let delegate = Box::leak(Box::new(Delegate::new()));
        center.setDelegate(Some(ProtocolObject::from_ref(&**delegate)));
    });
}

/// Shows `notice` and waits (up to two minutes, for the person to answer
/// the permission prompt) to say how it went: `Some` names the service,
/// `None` when nothing was shown and why is printed.
pub fn notify_now(notice: &Notice) -> Option<&'static str> {
    let (tx, rx) = std::sync::mpsc::channel();
    deliver(Arc::new(System), notice.clone(), move |delivery| {
        let _ = tx.send(delivery);
    });
    match rx.recv_timeout(std::time::Duration::from_secs(120)) {
        Ok(Delivery::Shown) => Some("the macOS notification center (UNUserNotificationCenter)"),
        Ok(Delivery::Denied) => {
            eprintln!(
                "notifications are off for this app in System Settings > Notifications \
                 (or this copy is not registered with Launch Services)"
            );
            None
        }
        Ok(Delivery::Unavailable) => {
            eprintln!("not running from an app bundle: macOS shows notifications only for apps");
            None
        }
        Ok(Delivery::Failed(reason)) => {
            eprintln!("the notification center refused the notice: {reason}");
            None
        }
        Err(_) => {
            eprintln!("no answer to the notification permission prompt");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_click_action_is_the_frameworks_default_action() {
        // SAFETY: a constant string the framework exports.
        let framework = unsafe { objc2_user_notifications::UNNotificationDefaultActionIdentifier };
        assert_eq!(
            framework.to_string(),
            openagents_desktop::notices::MAC_DEFAULT_ACTION
        );
    }

    #[test]
    fn outside_an_app_bundle_nothing_is_asked_or_shown() {
        // The test binary is not an app bundle, so the center is never
        // touched (touching it here would abort the process).
        assert!(!super::available());
        super::listen_notifications();
        let notice = openagents_desktop::notices::Notice {
            id: "coder-c1".into(),
            title: "Fix the login bug".into(),
            body: "Coder finished".into(),
            urgent: false,
        };
        assert_eq!(super::notify_now(&notice), None);
    }
}
