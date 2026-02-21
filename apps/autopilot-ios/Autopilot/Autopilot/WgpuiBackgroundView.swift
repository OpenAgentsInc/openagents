import SwiftUI
import UIKit
import QuartzCore
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

/// UIView that uses a CAMetalLayer so WGPUI can render the dots grid into it.
private final class WgpuiBackgroundUIView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    private var statePtr: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private var lastBounds: CGRect = .zero
    private var renderTickCount: Int = 0

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        backgroundColor = .black
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTapGesture(_:)))
        addGestureRecognizer(tap)
        isUserInteractionEnabled = true
        os_log("[WGPUI] WgpuiBackgroundUIView init frame=%@", log: wgpuiLog, type: .default, String(describing: frame))
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        isOpaque = true
        backgroundColor = .black
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTapGesture(_:)))
        addGestureRecognizer(tap)
        isUserInteractionEnabled = true
        os_log("[WGPUI] WgpuiBackgroundUIView init(coder)", log: wgpuiLog, type: .default)
    }

    @objc private func handleTapGesture(_ recognizer: UITapGestureRecognizer) {
        guard let statePtr else { return }
        let location = recognizer.location(in: self)
        let scale = Float(layer.contentsScale)
        let x = Float(location.x) * scale
        let y = Float(location.y) * scale
        WgpuiBackgroundBridge.handleTap(state: statePtr, x: x, y: y)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let bounds = self.bounds
        let scale = Float(layer.contentsScale)
        let w = UInt32(bounds.width * CGFloat(scale))
        let h = UInt32(bounds.height * CGFloat(scale))
        if w == 0 || h == 0 {
            print("[WGPUI] layoutSubviews skip zero size bounds=\(bounds) w=\(w) h=\(h)")
            return
        }

        if statePtr == nil {
            if !WgpuiBackgroundBridge.isAvailable {
                print("[WGPUI] layoutSubviews bridge not available")
                os_log("[WGPUI] layoutSubviews bridge not available", log: wgpuiLog, type: .error)
                return
            }
            guard let metalLayer = layer as? CAMetalLayer else {
                print("[WGPUI] layoutSubviews layer is not CAMetalLayer (got \(type(of: layer)))")
                os_log("[WGPUI] layoutSubviews layer is not CAMetalLayer", log: wgpuiLog, type: .error)
                return
            }
            print("[WGPUI] layoutSubviews creating state bounds=\(bounds) scale=\(scale) w=\(w) h=\(h)")
            statePtr = WgpuiBackgroundBridge.create(
                layerPtr: Unmanaged.passUnretained(metalLayer).toOpaque(),
                width: w,
                height: h,
                scale: scale
            )
            if statePtr != nil {
                displayLink = CADisplayLink(target: self, selector: #selector(tick))
                displayLink?.add(to: .main, forMode: .common)
                print("[WGPUI] layoutSubviews state created, displayLink started")
            } else {
                print("[WGPUI] layoutSubviews create returned nil (Rust wgpui_ios_background_create failed)")
            }
        } else {
            WgpuiBackgroundBridge.resize(state: statePtr, width: w, height: h)
        }
        lastBounds = bounds
    }

    @objc private func tick() {
        guard let statePtr else { return }
        let isFirst = (renderTickCount == 0)
        _ = WgpuiBackgroundBridge.render(state: statePtr, logFirstFrame: isFirst)
        renderTickCount += 1
        if renderTickCount == 1 || renderTickCount == 60 {
            print("[WGPUI] tick count=\(renderTickCount)")
        }
        if WgpuiBackgroundBridge.consumeSubmitRequested(state: statePtr) {
            DispatchQueue.main.async { [weak self] in
                self?.showSubmitAlert()
            }
        }
        if WgpuiBackgroundBridge.emailFocused(state: statePtr) {
            WgpuiBackgroundBridge.setEmailFocused(state: statePtr, focused: false)
            DispatchQueue.main.async { [weak self] in
                self?.showEmailAlert()
            }
        }
    }

    private func showSubmitAlert() {
        guard let vc = window?.rootViewController else { return }
        let alert = UIAlertController(title: "Log in", message: "Submit tapped.", preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        vc.present(alert, animated: true)
    }

    private func showEmailAlert() {
        guard let vc = window?.rootViewController,
              let statePtr else { return }
        let alert = UIAlertController(title: "Email", message: "Enter your email", preferredStyle: .alert)
        alert.addTextField { tf in
            tf.placeholder = "you@example.com"
            tf.keyboardType = .emailAddress
            tf.autocapitalizationType = .none
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "OK", style: .default) { [weak self] _ in
            let text = alert.textFields?.first?.text ?? ""
            WgpuiBackgroundBridge.setLoginEmail(state: statePtr, text)
        })
        vc.present(alert, animated: true)
    }

    deinit {
        os_log("[WGPUI] WgpuiBackgroundUIView deinit", log: wgpuiLog, type: .default)
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
        Group {
            if WgpuiBackgroundBridge.isAvailable {
                Representable()
                    .ignoresSafeArea()
            } else {
                Color.black
                    .ignoresSafeArea()
            }
        }
        .onAppear {
            let available = WgpuiBackgroundBridge.logAvailability()
            let msg = "[WGPUI] WgpuiBackgroundView onAppear -> using \(available ? "WGPUI renderer" : "fallback Color.black")"
            os_log("%{public}@", log: wgpuiLog, type: .default, msg)
            print(msg)
        }
    }

    private struct Representable: UIViewRepresentable {
        func makeUIView(context: Context) -> WgpuiBackgroundUIView {
            print("[WGPUI] Representable makeUIView")
            return WgpuiBackgroundUIView()
        }

        func updateUIView(_ uiView: WgpuiBackgroundUIView, context: Context) {}
    }
}
