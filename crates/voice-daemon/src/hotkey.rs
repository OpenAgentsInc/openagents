//! Global hotkey listener using CGEventTap (macOS)

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // FFI declaration for getting modifier flags state
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceFlagsState(stateID: i32) -> u64;
    }

    /// Combined session state
    const COMBINED_SESSION_STATE: i32 = 0;

    /// Global hotkey listener
    pub struct HotkeyListener {
        _marker: std::marker::PhantomData<()>,
    }

    impl HotkeyListener {
        /// Create a new hotkey listener
        ///
        /// Requires Accessibility permissions for full functionality.
        pub fn new() -> Result<Self, String> {
            // Check accessibility permissions
            if !check_accessibility() {
                return Err(
                    "Accessibility permission required. Please grant access in System Settings > Privacy & Security > Accessibility".to_string()
                );
            }

            Ok(Self {
                _marker: std::marker::PhantomData,
            })
        }

        /// Start listening for Right Command key
        ///
        /// Calls `on_press` when key is pressed, `on_release` when released.
        /// Note: This detects ANY Command key press, not specifically Right Command.
        pub fn listen_right_command<F1, F2>(
            &mut self,
            on_press: F1,
            on_release: F2,
        ) -> Result<(), String>
        where
            F1: Fn() + Send + Sync + 'static,
            F2: Fn() + Send + Sync + 'static,
        {
            let on_press = Arc::new(on_press);
            let on_release = Arc::new(on_release);

            // Track command state
            let was_pressed = Arc::new(AtomicBool::new(false));

            // Spawn thread to poll modifier state
            std::thread::spawn(move || {
                tracing::info!("Hotkey polling thread started");

                loop {
                    // Get current modifier flags
                    let flags = unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) };

                    // Check if Command flag is set (bit 20)
                    let cmd_pressed = (flags & 0x100000) != 0;
                    let was = was_pressed.load(Ordering::SeqCst);

                    if cmd_pressed && !was {
                        was_pressed.store(true, Ordering::SeqCst);
                        tracing::debug!("Command key pressed");
                        on_press();
                    } else if !cmd_pressed && was {
                        was_pressed.store(false, Ordering::SeqCst);
                        tracing::debug!("Command key released");
                        on_release();
                    }

                    // Poll every 10ms for responsive feel
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            });

            tracing::info!("Hotkey listener started (polling mode)");
            Ok(())
        }
    }

    /// Check if we have accessibility permissions, prompting user if not
    fn check_accessibility() -> bool {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::string::CFString;

        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
        }

        // Create options dictionary with kAXTrustedCheckOptionPrompt = true
        // This will show the system prompt asking user to grant accessibility
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();

        // Create CFDictionary manually using Core Foundation
        extern "C" {
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
