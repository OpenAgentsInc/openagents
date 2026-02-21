import SwiftUI
import UIKit
import QuartzCore

/// UIView that uses a CAMetalLayer so WGPUI can render the dots grid into it.
private final class WgpuiBackgroundUIView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    private var statePtr: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private var lastBounds: CGRect = .zero

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        backgroundColor = .black
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        isOpaque = true
        backgroundColor = .black
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let bounds = self.bounds
        let scale = Float(layer.contentsScale)
        let w = UInt32(bounds.width * CGFloat(scale))
        let h = UInt32(bounds.height * CGFloat(scale))
        guard w > 0, h > 0 else { return }

        if statePtr == nil {
            guard WgpuiBackgroundBridge.isAvailable,
                  let metalLayer = layer as? CAMetalLayer else { return }
            statePtr = WgpuiBackgroundBridge.create(
                layerPtr: Unmanaged.passUnretained(metalLayer).toOpaque(),
                width: w,
                height: h,
                scale: scale
            )
            if statePtr != nil {
                displayLink = CADisplayLink(target: self, selector: #selector(tick))
                displayLink?.add(to: .main, forMode: .common)
            }
        } else {
            WgpuiBackgroundBridge.resize(state: statePtr, width: w, height: h)
        }
        lastBounds = bounds
    }

    @objc private func tick() {
        guard let statePtr else { return }
        _ = WgpuiBackgroundBridge.render(state: statePtr)
    }

    deinit {
        displayLink?.invalidate()
        displayLink = nil
        if let statePtr {
            WgpuiBackgroundBridge.destroy(state: statePtr)
        }
    }
}

/// SwiftUI view that shows the WGPUI dots grid background. Falls back to solid black if WGPUI symbols are unavailable.
struct WgpuiBackgroundView: View {
    var body: some View {
        if WgpuiBackgroundBridge.isAvailable {
            Representable()
                .ignoresSafeArea()
        } else {
            Color.black
                .ignoresSafeArea()
        }
    }

    private struct Representable: UIViewRepresentable {
        func makeUIView(context: Context) -> WgpuiBackgroundUIView {
            WgpuiBackgroundUIView()
        }

        func updateUIView(_ uiView: WgpuiBackgroundUIView, context: Context) {}
    }
}
