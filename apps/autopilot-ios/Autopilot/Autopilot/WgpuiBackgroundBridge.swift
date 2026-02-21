import Foundation
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

// Directly bind Rust C ABI symbols so Release dead-strip cannot remove functions
// that are only discovered via dlsym at runtime.
@_silgen_name("wgpui_ios_background_create")
private func wgpui_ios_background_create(
    _ layerPtr: UnsafeMutableRawPointer?,
    _ width: UInt32,
    _ height: UInt32,
    _ scale: Float
) -> UnsafeMutableRawPointer?

@_silgen_name("wgpui_ios_background_render")
private func wgpui_ios_background_render(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_resize")
private func wgpui_ios_background_resize(_ state: UnsafeMutableRawPointer?, _ width: UInt32, _ height: UInt32)

@_silgen_name("wgpui_ios_background_destroy")
private func wgpui_ios_background_destroy(_ state: UnsafeMutableRawPointer?)

@_silgen_name("wgpui_ios_background_handle_tap")
private func wgpui_ios_background_handle_tap(_ state: UnsafeMutableRawPointer?, _ x: Float, _ y: Float)

@_silgen_name("wgpui_ios_background_login_submit_requested")
private func wgpui_ios_background_login_submit_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_submit_requested")
private func wgpui_ios_background_consume_submit_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_email_focused")
private func wgpui_ios_background_email_focused(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_set_email_focused")
private func wgpui_ios_background_set_email_focused(_ state: UnsafeMutableRawPointer?, _ focused: Int32)

@_silgen_name("wgpui_ios_background_set_login_email")
private func wgpui_ios_background_set_login_email(
    _ state: UnsafeMutableRawPointer?,
    _ string: UnsafePointer<CChar>?,
    _ length: Int
)

/// Bridge to WGPUI iOS background renderer (dots grid). Symbols are provided by
/// openagents-client-core when built with wgpui ios feature.
enum WgpuiBackgroundBridge {
    /// Log bridge availability and return true.
    static func logAvailability() -> Bool {
        let msg = "[WGPUI] bridge symbols linked directly (no runtime dlsym)"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return true
    }

    static var isAvailable: Bool {
        true
    }

    /// Create renderer. `width`/`height` are logical points, `scale` is device pixel ratio.
    static func create(layerPtr: UnsafeMutableRawPointer, width: UInt32, height: UInt32, scale: Float) -> UnsafeMutableRawPointer? {
        let result = wgpui_ios_background_create(layerPtr, width, height, scale)
        let msg = "[WGPUI] create width=\(width) height=\(height) scale=\(scale) -> \(result != nil ? "OK" : "NULL (Rust failed)")"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return result
    }

    /// Render one frame. Returns true on success. Pass logFirstFrame: true to log once.
    static func render(state: UnsafeMutableRawPointer?, logFirstFrame: Bool = false) -> Bool {
        guard let state else { return false }
        let ok = wgpui_ios_background_render(state) != 0
        if logFirstFrame {
            let msg = "[WGPUI] first render -> \(ok ? "OK" : "FAIL")"
            os_log("%{public}@", log: wgpuiLog, type: .default, msg)
            print(msg)
        }
        return ok
    }

    /// Resize surface.
    static func resize(state: UnsafeMutableRawPointer?, width: UInt32, height: UInt32) {
        wgpui_ios_background_resize(state, width, height)
    }

    /// Destroy state and free.
    static func destroy(state: UnsafeMutableRawPointer?) {
        print("[WGPUI] destroy state=\(state != nil ? "non-nil" : "nil")")
        wgpui_ios_background_destroy(state)
    }

    /// Handle tap at logical point coordinates (origin top-left).
    static func handleTap(state: UnsafeMutableRawPointer?, x: Float, y: Float) {
        wgpui_ios_background_handle_tap(state, x, y)
    }

    /// Returns true if user tapped submit and it has not been consumed yet.
    static func loginSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_login_submit_requested(state) != 0
    }

    /// Consume submit-requested flag. Returns true if it was set.
    static func consumeSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_submit_requested(state) != 0
    }

    /// Returns true if email field is focused (user tapped it).
    static func emailFocused(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_email_focused(state) != 0
    }

    /// Set email field focused state (e.g. false after dismissing native keyboard).
    static func setEmailFocused(state: UnsafeMutableRawPointer?, focused: Bool) {
        wgpui_ios_background_set_email_focused(state, focused ? 1 : 0)
    }

    /// Set login email from UTF-8 string. Copies into Rust state.
    static func setLoginEmail(state: UnsafeMutableRawPointer?, _ string: String) {
        let utf8 = Array(string.utf8)
        utf8.withUnsafeBufferPointer { buf in
            buf.baseAddress?.withMemoryRebound(to: CChar.self, capacity: buf.count) { ptr in
                wgpui_ios_background_set_login_email(state, ptr, buf.count)
            }
        }
    }
}
