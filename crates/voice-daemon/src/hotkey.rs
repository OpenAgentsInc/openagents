//! Global hotkey listener using CGEventTap (macOS)

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Wrapper to allow sending raw pointers across threads
    /// SAFETY: The pointer is allocated via Box::into_raw and only accessed
    /// from the CGEventTap callback which runs on the spawned thread's run loop
    /// We use usize to avoid the compiler seeing the raw pointer type
    struct SendPtr(usize);
    unsafe impl Send for SendPtr {}

    impl SendPtr {
        fn new(ptr: *mut std::ffi::c_void) -> Self {
            Self(ptr as usize)
        }

        fn as_ptr(&self) -> *mut std::ffi::c_void {
            self.0 as *mut std::ffi::c_void
        }
    }

    // Right Command keycode on macOS
    const RIGHT_COMMAND_KEYCODE: i64 = 54;

    // CGEventTap constants
    const K_CGEVENT_TAP_LOCATION_HID: u32 = 0;
    const K_CGEVENT_TAP_OPTION_DEFAULT: u32 = 0;
    const K_CGEVENT_TAP_PLACEMENT_HEAD: u32 = 0;
    const K_CGEVENT_FLAGS_CHANGED: u32 = 12;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            eventsOfInterest: u64,
            callback: extern "C" fn(
                proxy: *mut std::ffi::c_void,
                event_type: u32,
                event: *mut std::ffi::c_void,
                user_info: *mut std::ffi::c_void,
            ) -> *mut std::ffi::c_void,
            userInfo: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;

        fn CGEventTapEnable(tap: *mut std::ffi::c_void, enable: bool);
        fn CGEventGetIntegerValueField(event: *mut std::ffi::c_void, field: u32) -> i64;
        fn CGEventGetFlags(event: *mut std::ffi::c_void) -> u64;
        fn CFMachPortCreateRunLoopSource(
            allocator: *const std::ffi::c_void,
            port: *mut std::ffi::c_void,
            order: i64,
        ) -> *mut std::ffi::c_void;
        fn CFRunLoopGetCurrent() -> *mut std::ffi::c_void;
        fn CFRunLoopAddSource(
            rl: *mut std::ffi::c_void,
            source: *mut std::ffi::c_void,
            mode: *const std::ffi::c_void,
        );
        fn CFRunLoopRun();
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopCommonModes: *const std::ffi::c_void;
    }

    // Event field for keycode
    const K_CGKEYBOARD_EVENT_KEYCODE: u32 = 9;

    // Command flag bit
    const K_CGEVENT_FLAG_COMMAND: u64 = 0x100000;

    /// Callback context passed to CGEventTap
    struct CallbackContext {
        on_press: Arc<dyn Fn() + Send + Sync>,
        on_release: Arc<dyn Fn() + Send + Sync>,
        right_cmd_pressed: AtomicBool,
    }

    /// CGEventTap callback - called for every flags changed event
    extern "C" fn event_tap_callback(
        _proxy: *mut std::ffi::c_void,
        _event_type: u32,
        event: *mut std::ffi::c_void,
        user_info: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void {
        unsafe {
            let ctx = &*(user_info as *const CallbackContext);

            // Get the keycode that triggered this flags change
            let keycode = CGEventGetIntegerValueField(event, K_CGKEYBOARD_EVENT_KEYCODE);

            // Only care about Right Command key
            if keycode == RIGHT_COMMAND_KEYCODE {
                let flags = CGEventGetFlags(event);
                let cmd_down = (flags & K_CGEVENT_FLAG_COMMAND) != 0;
                let was_pressed = ctx.right_cmd_pressed.load(Ordering::SeqCst);

                if cmd_down && !was_pressed {
                    ctx.right_cmd_pressed.store(true, Ordering::SeqCst);
                    (ctx.on_press)();
                } else if !cmd_down && was_pressed {
                    ctx.right_cmd_pressed.store(false, Ordering::SeqCst);
                    (ctx.on_release)();
                }
            }
        }

        // Return the event unchanged (don't consume it)
        event
    }

    /// Global hotkey listener
    pub struct HotkeyListener {
        _marker: std::marker::PhantomData<()>,
    }

    impl HotkeyListener {
        /// Create a new hotkey listener
        pub fn new() -> Result<Self, String> {
            if !check_accessibility() {
                return Err(
                    "Accessibility permission required. Please grant access in System Settings > Privacy & Security > Accessibility".to_string()
                );
            }

            Ok(Self {
                _marker: std::marker::PhantomData,
            })
        }

        /// Start listening for Right Command key specifically
        pub fn listen_right_command<F1, F2>(
            &mut self,
            on_press: F1,
            on_release: F2,
        ) -> Result<(), String>
        where
            F1: Fn() + Send + Sync + 'static,
            F2: Fn() + Send + Sync + 'static,
        {
            let ctx = Box::new(CallbackContext {
                on_press: Arc::new(on_press),
                on_release: Arc::new(on_release),
                right_cmd_pressed: AtomicBool::new(false),
            });

            let send_ptr = SendPtr::new(Box::into_raw(ctx) as *mut std::ffi::c_void);

            std::thread::spawn(move || {
                let ctx_ptr = send_ptr.as_ptr();
                unsafe {
                    // Create event tap for flags changed events (modifier keys)
                    let event_mask: u64 = 1 << K_CGEVENT_FLAGS_CHANGED;

                    let tap = CGEventTapCreate(
                        K_CGEVENT_TAP_LOCATION_HID,
                        K_CGEVENT_TAP_PLACEMENT_HEAD,
                        K_CGEVENT_TAP_OPTION_DEFAULT,
                        event_mask,
                        event_tap_callback,
                        ctx_ptr,
                    );

                    if tap.is_null() {
                        tracing::error!("Failed to create event tap - check accessibility permissions");
                        return;
                    }

                    // Create run loop source
                    let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                    if source.is_null() {
                        tracing::error!("Failed to create run loop source");
                        return;
                    }

                    // Add to current run loop
                    let run_loop = CFRunLoopGetCurrent();
                    CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);

                    // Enable the tap
                    CGEventTapEnable(tap, true);

                    tracing::info!("Hotkey listener started - listening for Right Command key");

                    // Run the loop (blocks forever)
                    CFRunLoopRun();
                }
            });

            Ok(())
        }
    }

    /// Check accessibility permissions with prompt
    fn check_accessibility() -> bool {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::string::CFString;

        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
            fn CFDictionaryCreate(
                allocator: *const std::ffi::c_void,
                keys: *const *const std::ffi::c_void,
                values: *const *const std::ffi::c_void,
                numValues: isize,
                keyCallBacks: *const std::ffi::c_void,
                valueCallBacks: *const std::ffi::c_void,
            ) -> *const std::ffi::c_void;
            static kCFTypeDictionaryKeyCallBacks: std::ffi::c_void;
            static kCFTypeDictionaryValueCallBacks: std::ffi::c_void;
        }

        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();

        unsafe {
            let keys = [key.as_concrete_TypeRef() as *const std::ffi::c_void];
            let values = [value.as_concrete_TypeRef() as *const std::ffi::c_void];

            let dict = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );

            AXIsProcessTrustedWithOptions(dict)
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::HotkeyListener;

#[cfg(not(target_os = "macos"))]
pub struct HotkeyListener;

#[cfg(not(target_os = "macos"))]
impl HotkeyListener {
    pub fn new() -> Result<Self, String> {
        Err("Global hotkeys only supported on macOS".to_string())
    }

    pub fn listen_right_command<F1, F2>(
        &mut self,
        _on_press: F1,
        _on_release: F2,
    ) -> Result<(), String>
    where
        F1: Fn() + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        Err("Global hotkeys only supported on macOS".to_string())
    }
}
