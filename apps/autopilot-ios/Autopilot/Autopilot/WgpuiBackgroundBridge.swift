import Foundation
import Darwin

/// Bridge to WGPUI iOS background renderer (dots grid). Symbols are provided by
/// openagents-client-core when built with wgpui ios feature.
enum WgpuiBackgroundBridge {
    private typealias CreateFn = @convention(c) (
        UnsafeMutableRawPointer?, UInt32, UInt32, Float
    ) -> UnsafeMutableRawPointer?
    private typealias RenderFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    private typealias ResizeFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private static func loadSymbol<T>(_ name: String, as type: T.Type) -> T? {
        guard let handle = dlopen(nil, RTLD_NOW),
              let symbol = dlsym(handle, name) else {
            return nil
        }
        return unsafeBitCast(symbol, to: type)
    }

    private static let createFn = loadSymbol("wgpui_ios_background_create", as: CreateFn.self)
    private static let renderFn = loadSymbol("wgpui_ios_background_render", as: RenderFn.self)
    private static let resizeFn = loadSymbol("wgpui_ios_background_resize", as: ResizeFn.self)
    private static let destroyFn = loadSymbol("wgpui_ios_background_destroy", as: DestroyFn.self)

    static var isAvailable: Bool {
        createFn != nil && renderFn != nil && resizeFn != nil && destroyFn != nil
    }

    /// Create renderer. layerPtr = CAMetalLayer pointer. Returns opaque state or nil.
    static func create(layerPtr: UnsafeMutableRawPointer, width: UInt32, height: UInt32, scale: Float) -> UnsafeMutableRawPointer? {
        guard let create = createFn else { return nil }
        return create(layerPtr, width, height, scale)
    }

    /// Render one frame. Returns true on success.
    static func render(state: UnsafeMutableRawPointer?) -> Bool {
        guard let render = renderFn, let state else { return false }
        return render(state) != 0
    }

    /// Resize surface.
    static func resize(state: UnsafeMutableRawPointer?, width: UInt32, height: UInt32) {
        resizeFn?(state, width, height)
    }

    /// Destroy state and free.
    static func destroy(state: UnsafeMutableRawPointer?) {
        destroyFn?(state)
    }
}
