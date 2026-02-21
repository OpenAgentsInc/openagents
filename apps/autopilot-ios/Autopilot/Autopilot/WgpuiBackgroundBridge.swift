import Foundation
import Darwin
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

/// Bridge to WGPUI iOS background renderer (dots grid). Symbols are provided by
/// openagents-client-core when built with wgpui ios feature.
enum WgpuiBackgroundBridge {
    private typealias CreateFn = @convention(c) (
        UnsafeMutableRawPointer?, UInt32, UInt32, Float
    ) -> UnsafeMutableRawPointer?
    private typealias RenderFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    private typealias ResizeFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias HandleTapFn = @convention(c) (UnsafeMutableRawPointer?, Float, Float) -> Void
    private typealias LoginSubmitRequestedFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    private typealias ConsumeSubmitRequestedFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    private typealias EmailFocusedFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    private typealias SetEmailFocusedFn = @convention(c) (UnsafeMutableRawPointer?, Int32) -> Void
    private typealias SetLoginEmailFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, Int) -> Void

    /// Load symbol from handle. Returns nil if handle is nil or symbol not found.
    private static func loadSymbol<T>(_ name: String, from handle: UnsafeMutableRawPointer?, as type: T.Type) -> T? {
        guard let handle else { return nil }
        guard let symbol = dlsym(handle, name) else { return nil }
        return unsafeBitCast(symbol, to: type)
    }

    /// Resolved handle for WGPUI symbols: either main executable (nil → RTLD_DEFAULT) or app's debug dylib.
    private static let wgpuiHandle: UnsafeMutableRawPointer? = {
        let mainHandle = dlopen(nil, RTLD_NOW)
        if mainHandle != nil, loadSymbol("wgpui_ios_background_create", from: mainHandle, as: CreateFn.self) != nil {
            return mainHandle
        }
        let dylibPath = Bundle.main.bundlePath + "/Autopilot.debug.dylib"
        guard let dylibHandle = dlopen(dylibPath, RTLD_NOW) else {
            print("[WGPUI] dlopen(dylib) failed: \(dylibPath)")
            return nil
        }
        if loadSymbol("wgpui_ios_background_create", from: dylibHandle, as: CreateFn.self) != nil {
            print("[WGPUI] loaded symbols from Autopilot.debug.dylib")
            return dylibHandle
        }
        dlclose(dylibHandle)
        return nil
    }()

    private static let createFn = loadSymbol("wgpui_ios_background_create", from: wgpuiHandle, as: CreateFn.self)
    private static let renderFn = loadSymbol("wgpui_ios_background_render", from: wgpuiHandle, as: RenderFn.self)
    private static let resizeFn = loadSymbol("wgpui_ios_background_resize", from: wgpuiHandle, as: ResizeFn.self)
    private static let destroyFn = loadSymbol("wgpui_ios_background_destroy", from: wgpuiHandle, as: DestroyFn.self)
    private static let handleTapFn = loadSymbol("wgpui_ios_background_handle_tap", from: wgpuiHandle, as: HandleTapFn.self)
    private static let loginSubmitRequestedFn = loadSymbol("wgpui_ios_background_login_submit_requested", from: wgpuiHandle, as: LoginSubmitRequestedFn.self)
    private static let consumeSubmitRequestedFn = loadSymbol("wgpui_ios_background_consume_submit_requested", from: wgpuiHandle, as: ConsumeSubmitRequestedFn.self)
    private static let emailFocusedFn = loadSymbol("wgpui_ios_background_email_focused", from: wgpuiHandle, as: EmailFocusedFn.self)
    private static let setEmailFocusedFn = loadSymbol("wgpui_ios_background_set_email_focused", from: wgpuiHandle, as: SetEmailFocusedFn.self)
    private static let setLoginEmailFn = loadSymbol("wgpui_ios_background_set_login_email", from: wgpuiHandle, as: SetLoginEmailFn.self)

    /// Log which symbols are present/missing and return availability.
    static func logAvailability() -> Bool {
        let names = [
            ("wgpui_ios_background_create", createFn != nil),
            ("wgpui_ios_background_render", renderFn != nil),
            ("wgpui_ios_background_resize", resizeFn != nil),
            ("wgpui_ios_background_destroy", destroyFn != nil),
        ]
        for (name, ok) in names {
            let msg = "[WGPUI] symbol \(name): \(ok ? "OK" : "MISSING")"
            os_log("%{public}@", log: wgpuiLog, type: .default, msg)
            print(msg)
        }
        let available = createFn != nil && renderFn != nil && resizeFn != nil && destroyFn != nil
        let msg = "[WGPUI] bridge isAvailable=\(available)"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return available
    }

    static var isAvailable: Bool {
        createFn != nil && renderFn != nil && resizeFn != nil && destroyFn != nil
    }

    /// Create renderer. layerPtr = CAMetalLayer pointer. Returns opaque state or nil.
    static func create(layerPtr: UnsafeMutableRawPointer, width: UInt32, height: UInt32, scale: Float) -> UnsafeMutableRawPointer? {
        guard let create = createFn else {
            os_log("[WGPUI] create skipped: no createFn", log: wgpuiLog, type: .error)
            return nil
        }
        let result = create(layerPtr, width, height, scale)
        let msg = "[WGPUI] create width=\(width) height=\(height) scale=\(scale) -> \(result != nil ? "OK" : "NULL (Rust failed)")"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return result
    }

    /// Render one frame. Returns true on success. Pass logFirstFrame: true to log once.
    static func render(state: UnsafeMutableRawPointer?, logFirstFrame: Bool = false) -> Bool {
        guard let render = renderFn, let state else { return false }
        let ok = render(state) != 0
        if logFirstFrame {
            let msg = "[WGPUI] first render -> \(ok ? "OK" : "FAIL")"
            os_log("%{public}@", log: wgpuiLog, type: .default, msg)
            print(msg)
        }
        return ok
    }

    /// Resize surface.
    static func resize(state: UnsafeMutableRawPointer?, width: UInt32, height: UInt32) {
        resizeFn?(state, width, height)
    }

    /// Destroy state and free.
    static func destroy(state: UnsafeMutableRawPointer?) {
        print("[WGPUI] destroy state=\(state != nil ? "non-nil" : "nil")")
        destroyFn?(state)
    }

    /// Handle tap at logical pixel coordinates (same as Rust: origin top-left, physical pixels).
    static func handleTap(state: UnsafeMutableRawPointer?, x: Float, y: Float) {
        handleTapFn?(state, x, y)
    }

    /// Returns true if user tapped submit and it has not been consumed yet.
    static func loginSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        loginSubmitRequestedFn?(state) != 0
    }

    /// Consume submit-requested flag. Returns true if it was set.
    static func consumeSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        consumeSubmitRequestedFn?(state) != 0
    }

    /// Returns true if email field is focused (user tapped it).
    static func emailFocused(state: UnsafeMutableRawPointer?) -> Bool {
        emailFocusedFn?(state) != 0
    }

    /// Set email field focused state (e.g. false after dismissing native keyboard).
    static func setEmailFocused(state: UnsafeMutableRawPointer?, focused: Bool) {
        setEmailFocusedFn?(state, focused ? 1 : 0)
    }

    /// Set login email from UTF-8 string. Copies into Rust state.
    static func setLoginEmail(state: UnsafeMutableRawPointer?, _ string: String) {
        guard let setLoginEmailFn else { return }
        let utf8 = Array(string.utf8)
        utf8.withUnsafeBufferPointer { buf in
            buf.baseAddress?.withMemoryRebound(to: CChar.self, capacity: buf.count) { ptr in
                setLoginEmailFn(state, ptr, buf.count)
            }
        }
    }
}
