import Foundation
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

enum WgpuiCodexRole: UInt8 {
    case user = 0
    case assistant = 1
    case reasoning = 2
    case tool = 3
    case system = 4
    case error = 5
}

// Direct symbol bindings to avoid runtime dlsym/dead-strip issues.
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

@_silgen_name("wgpui_ios_background_clear_codex_messages")
private func wgpui_ios_background_clear_codex_messages(_ state: UnsafeMutableRawPointer?)

@_silgen_name("wgpui_ios_background_push_codex_message")
private func wgpui_ios_background_push_codex_message(
    _ state: UnsafeMutableRawPointer?,
    _ role: UInt8,
    _ string: UnsafePointer<CChar>?,
    _ length: Int,
    _ streaming: Int32
)

@_silgen_name("wgpui_ios_background_set_codex_context")
private func wgpui_ios_background_set_codex_context(
    _ state: UnsafeMutableRawPointer?,
    _ threadPtr: UnsafePointer<CChar>?,
    _ threadLen: Int,
    _ turnPtr: UnsafePointer<CChar>?,
    _ turnLen: Int,
    _ modelPtr: UnsafePointer<CChar>?,
    _ modelLen: Int,
    _ reasoningPtr: UnsafePointer<CChar>?,
    _ reasoningLen: Int
)

@_silgen_name("wgpui_ios_background_set_empty_state")
private func wgpui_ios_background_set_empty_state(
    _ state: UnsafeMutableRawPointer?,
    _ titlePtr: UnsafePointer<CChar>?,
    _ titleLen: Int,
    _ detailPtr: UnsafePointer<CChar>?,
    _ detailLen: Int
)

@_silgen_name("wgpui_ios_background_set_composer_text")
private func wgpui_ios_background_set_composer_text(
    _ state: UnsafeMutableRawPointer?,
    _ string: UnsafePointer<CChar>?,
    _ length: Int
)

@_silgen_name("wgpui_ios_background_composer_focused")
private func wgpui_ios_background_composer_focused(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_set_composer_focused")
private func wgpui_ios_background_set_composer_focused(_ state: UnsafeMutableRawPointer?, _ focused: Int32)

@_silgen_name("wgpui_ios_background_consume_send_requested")
private func wgpui_ios_background_consume_send_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_new_thread_requested")
private func wgpui_ios_background_consume_new_thread_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_interrupt_requested")
private func wgpui_ios_background_consume_interrupt_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_model_cycle_requested")
private func wgpui_ios_background_consume_model_cycle_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_reasoning_cycle_requested")
private func wgpui_ios_background_consume_reasoning_cycle_requested(_ state: UnsafeMutableRawPointer?) -> Int32

// Backward-compatible alias symbols.
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

/// Bridge to WGPUI iOS Codex renderer.
enum WgpuiBackgroundBridge {
    static func logAvailability() -> Bool {
        let msg = "[WGPUI] bridge symbols linked directly (no runtime dlsym)"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return true
    }

    static var isAvailable: Bool { true }

    static func create(layerPtr: UnsafeMutableRawPointer, width: UInt32, height: UInt32, scale: Float) -> UnsafeMutableRawPointer? {
        let result = wgpui_ios_background_create(layerPtr, width, height, scale)
        let msg = "[WGPUI] create width=\(width) height=\(height) scale=\(scale) -> \(result != nil ? "OK" : "NULL (Rust failed)")"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return result
    }

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

    static func resize(state: UnsafeMutableRawPointer?, width: UInt32, height: UInt32) {
        wgpui_ios_background_resize(state, width, height)
    }

    static func destroy(state: UnsafeMutableRawPointer?) {
        print("[WGPUI] destroy state=\(state != nil ? "non-nil" : "nil")")
        wgpui_ios_background_destroy(state)
    }

    static func handleTap(state: UnsafeMutableRawPointer?, x: Float, y: Float) {
        wgpui_ios_background_handle_tap(state, x, y)
    }

    static func clearCodexMessages(state: UnsafeMutableRawPointer?) {
        wgpui_ios_background_clear_codex_messages(state)
    }

    static func pushCodexMessage(
        state: UnsafeMutableRawPointer?,
        role: WgpuiCodexRole,
        text: String,
        streaming: Bool
    ) {
        let utf8 = Array(text.utf8)
        utf8.withUnsafeBufferPointer { buf in
            buf.baseAddress?.withMemoryRebound(to: CChar.self, capacity: buf.count) { ptr in
                wgpui_ios_background_push_codex_message(state, role.rawValue, ptr, buf.count, streaming ? 1 : 0)
            }
        }
    }

    static func setCodexContext(
        state: UnsafeMutableRawPointer?,
        thread: String,
        turn: String,
        model: String,
        reasoning: String
    ) {
        withCStringBytes(thread) { threadPtr, threadLen in
            withCStringBytes(turn) { turnPtr, turnLen in
                withCStringBytes(model) { modelPtr, modelLen in
                    withCStringBytes(reasoning) { reasoningPtr, reasoningLen in
                        wgpui_ios_background_set_codex_context(
                            state,
                            threadPtr,
                            threadLen,
                            turnPtr,
                            turnLen,
                            modelPtr,
                            modelLen,
                            reasoningPtr,
                            reasoningLen
                        )
                    }
                }
            }
        }
    }

    static func setComposerText(state: UnsafeMutableRawPointer?, _ text: String) {
        withCStringBytes(text) { ptr, len in
            wgpui_ios_background_set_composer_text(state, ptr, len)
        }
    }

    static func setEmptyState(state: UnsafeMutableRawPointer?, title: String, detail: String) {
        withCStringBytes(title) { titlePtr, titleLen in
            withCStringBytes(detail) { detailPtr, detailLen in
                wgpui_ios_background_set_empty_state(state, titlePtr, titleLen, detailPtr, detailLen)
            }
        }
    }

    static func composerFocused(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_composer_focused(state) != 0
    }

    static func setComposerFocused(state: UnsafeMutableRawPointer?, focused: Bool) {
        wgpui_ios_background_set_composer_focused(state, focused ? 1 : 0)
    }

    static func consumeSendRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_send_requested(state) != 0
    }

    static func consumeNewThreadRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_new_thread_requested(state) != 0
    }

    static func consumeInterruptRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_interrupt_requested(state) != 0
    }

    static func consumeModelCycleRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_model_cycle_requested(state) != 0
    }

    static func consumeReasoningCycleRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_reasoning_cycle_requested(state) != 0
    }

    // Compatibility wrappers (legacy login naming).
    static func loginSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_login_submit_requested(state) != 0
    }

    static func consumeSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_submit_requested(state) != 0
    }

    static func emailFocused(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_email_focused(state) != 0
    }

    static func setEmailFocused(state: UnsafeMutableRawPointer?, focused: Bool) {
        wgpui_ios_background_set_email_focused(state, focused ? 1 : 0)
    }

    static func setLoginEmail(state: UnsafeMutableRawPointer?, _ string: String) {
        withCStringBytes(string) { ptr, len in
            wgpui_ios_background_set_login_email(state, ptr, len)
        }
    }

    private static func withCStringBytes(
        _ string: String,
        _ body: (_ ptr: UnsafePointer<CChar>?, _ len: Int) -> Void
    ) {
        let utf8 = Array(string.utf8)
        utf8.withUnsafeBufferPointer { buf in
            if let baseAddress = buf.baseAddress {
                baseAddress.withMemoryRebound(to: CChar.self, capacity: buf.count) { ptr in
                    body(ptr, buf.count)
                }
            } else {
                body(nil, 0)
            }
        }
    }
}
