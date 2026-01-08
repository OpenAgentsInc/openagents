//! Text insertion via clipboard and simulated Cmd+V

use arboard::Clipboard;

/// Insert text at the current cursor position in any application
///
/// Works by:
/// 1. Saving current clipboard contents
/// 2. Setting text to clipboard
/// 3. Simulating Cmd+V paste
/// 4. Restoring original clipboard (after a delay)
pub fn insert_text(text: &str) -> Result<(), String> {
    // Get clipboard
    let mut clipboard = Clipboard::new()
        .map_err(|e| format!("Failed to access clipboard: {}", e))?;

    // Save current clipboard content
    let saved = clipboard.get_text().ok();

    // Set our text
    clipboard.set_text(text)
        .map_err(|e| format!("Failed to set clipboard: {}", e))?;

    // Simulate Cmd+V
    simulate_paste()?;

    // Wait for paste to complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Restore original clipboard content (if any)
    if let Some(original) = saved {
        // Delay restoration slightly to ensure paste completed
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if let Ok(mut clip) = Clipboard::new() {
                let _ = clip.set_text(&original);
            }
        });
    }

    Ok(())
}

/// Simulate Cmd+V key press using CGEvent
#[cfg(target_os = "macos")]
fn simulate_paste() -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // V keycode on macOS
    const V_KEYCODE: CGKeyCode = 9;

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create event source")?;

    // Create key down event for V with Command modifier
    let key_down = CGEvent::new_keyboard_event(source.clone(), V_KEYCODE, true)
        .map_err(|_| "Failed to create key down event")?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    // Create key up event
    let key_up = CGEvent::new_keyboard_event(source, V_KEYCODE, false)
        .map_err(|_| "Failed to create key up event")?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // Post events
    key_down.post(core_graphics::event::CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(10));
    key_up.post(core_graphics::event::CGEventTapLocation::HID);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn simulate_paste() -> Result<(), String> {
    Err("Paste simulation only supported on macOS".to_string())
}
