//! UIKit feasibility shell implemented in Rust against the public Objective-C ABI.

use objc2::runtime::{AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{CGRect, NSString};
use std::io::Write;

#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {
    fn UIApplicationMain(
        argc: i32,
        argv: *mut *mut std::ffi::c_char,
        principal: *const NSString,
        delegate: *const NSString,
    ) -> i32;
}

/// Start UIKit on the process's main thread.
pub fn run() {
    let mut delegate = ClassBuilder::new("CoderProbeDelegate", class!(NSObject))
        .expect("the probe delegate is registered once");
    // SAFETY: These Rust callbacks have the Objective-C signatures of the
    // corresponding UIApplicationDelegate selectors and live for the process.
    unsafe {
        delegate.add_method(
            sel!(application:didFinishLaunchingWithOptions:),
            launch as unsafe extern "C" fn(_, _, _, _) -> _,
        );
        delegate.add_method(
            sel!(applicationDidEnterBackground:),
            background as unsafe extern "C" fn(_, _, _),
        );
        delegate.add_method(
            sel!(applicationDidBecomeActive:),
            active as unsafe extern "C" fn(_, _, _),
        );
    }
    delegate.register();
    let name = NSString::from_str("CoderProbeDelegate");
    // SAFETY: This is the process entry on the main thread. UIKit accepts a
    // zero-length argument vector; the delegate class remains registered.
    unsafe { UIApplicationMain(0, std::ptr::null_mut(), std::ptr::null(), &*name) };
}

unsafe extern "C" fn launch(
    _delegate: &AnyObject,
    _selector: Sel,
    _application: *mut AnyObject,
    _options: *mut AnyObject,
) -> Bool {
    // SAFETY: UIKit invokes this delegate on its main thread. All selectors,
    // argument encodings, and return types below follow UIKit's public API.
    // Views retain their children; the window intentionally lives until exit.
    unsafe {
        let screen: *mut AnyObject = msg_send![class!(UIScreen), mainScreen];
        let bounds: CGRect = msg_send![screen, bounds];
        let allocated: *mut AnyObject = msg_send![class!(UIWindow), alloc];
        let window: *mut AnyObject = msg_send![allocated, initWithFrame: bounds];
        let controller: *mut AnyObject = msg_send![class!(UIViewController), new];
        let view: *mut AnyObject = msg_send![controller, view];
        let background: *mut AnyObject = msg_send![class!(UIColor), systemBackgroundColor];
        let foreground: *mut AnyObject = msg_send![class!(UIColor), labelColor];
        let _: () = msg_send![view, setBackgroundColor: background];

        let title_frame = CGRect::new(
            objc2_foundation::CGPoint::new(16.0, 60.0),
            objc2_foundation::CGSize::new(bounds.size.width - 32.0, 44.0),
        );
        let title_alloc: *mut AnyObject = msg_send![class!(UILabel), alloc];
        let title: *mut AnyObject = msg_send![title_alloc, initWithFrame: title_frame];
        let heading = NSString::from_str("Coder probe · synthetic · read-only");
        let _: () = msg_send![title, setText: &*heading];
        let _: () = msg_send![title, setTextColor: foreground];
        let _: () = msg_send![title, setNumberOfLines: 2isize];
        let _: () = msg_send![title, setAutoresizingMask: 2usize];
        let _: () = msg_send![view, addSubview: title];

        let input_frame = CGRect::new(
            objc2_foundation::CGPoint::new(16.0, 112.0),
            objc2_foundation::CGSize::new(bounds.size.width - 32.0, 48.0),
        );
        let input_alloc: *mut AnyObject = msg_send![class!(UITextField), alloc];
        let input: *mut AnyObject = msg_send![input_alloc, initWithFrame: input_frame];
        let label = NSString::from_str("Input and IME probe. This field sends nothing.");
        let placeholder = NSString::from_str("日本語 · café · 👩🏽‍💻");
        let _: () = msg_send![input, setAccessibilityLabel: &*label];
        let _: () = msg_send![input, setPlaceholder: &*placeholder];
        let _: () = msg_send![input, setBorderStyle: 3isize];
        let _: () = msg_send![input, setAutoresizingMask: 2usize];
        let _: () = msg_send![view, addSubview: input];

        let trace_frame = CGRect::new(
            objc2_foundation::CGPoint::new(12.0, 174.0),
            objc2_foundation::CGSize::new(bounds.size.width - 24.0, bounds.size.height - 196.0),
        );
        let trace_alloc: *mut AnyObject = msg_send![class!(UITextView), alloc];
        let trace: *mut AnyObject = msg_send![trace_alloc, initWithFrame: trace_frame];
        let text = NSString::from_str(&crate::transcript(&crate::fixture()));
        let identifier = NSString::from_str("complete-synthetic-transcript");
        let _: () = msg_send![trace, setText: &*text];
        let _: () = msg_send![trace, setEditable: false];
        let _: () = msg_send![trace, setSelectable: true];
        let _: () = msg_send![trace, setAccessibilityIdentifier: &*identifier];
        let _: () = msg_send![trace, setAutoresizingMask: 2usize | 16usize];
        let _: () = msg_send![view, addSubview: trace];
        let _: () = msg_send![window, setRootViewController: controller];
        let _: () = msg_send![window, makeKeyAndVisible];
        let _: () = msg_send![title, release];
        let _: () = msg_send![input, release];
        let _: () = msg_send![trace, release];
        let _: () = msg_send![controller, release];
    }
    record("launch");
    record_value(crate::ios_keychain::probe());
    Bool::YES
}

unsafe extern "C" fn background(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    record("background");
}

unsafe extern "C" fn active(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    record("active");
}

fn record(event: &str) {
    record_value(serde_json::json!({"event":event,"at_ms":atif::now_ms(),"synthetic":true}));
}

fn record_value(value: serde_json::Value) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let directory = std::path::PathBuf::from(home).join("Documents");
    let _ = std::fs::create_dir_all(&directory);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("probe-events.jsonl"))
    {
        let _ = writeln!(file, "{value}");
    }
}
