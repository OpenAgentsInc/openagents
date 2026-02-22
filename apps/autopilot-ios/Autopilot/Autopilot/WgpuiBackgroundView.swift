import SwiftUI
import UIKit
import QuartzCore
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

/// UIView that uses a CAMetalLayer so WGPUI can render the dots grid into it.
private final class WgpuiBackgroundUIView: UIView, UITextFieldDelegate {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    private var statePtr: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private var lastBounds: CGRect = .zero
    private var renderTickCount: Int = 0
    private var emailDraft: String = ""
    private let keyboardProxyField = UITextField(frame: .zero)

    private var effectiveScale: CGFloat {
        window?.screen.nativeScale ?? UIScreen.main.nativeScale
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        configureView()
        os_log("[WGPUI] WgpuiBackgroundUIView init frame=%@", log: wgpuiLog, type: .default, String(describing: frame))
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        configureView()
        os_log("[WGPUI] WgpuiBackgroundUIView init(coder)", log: wgpuiLog, type: .default)
    }

    private func configureView() {
        isOpaque = true
        backgroundColor = .black
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTapGesture(_:)))
        addGestureRecognizer(tap)
        isUserInteractionEnabled = true
        configureKeyboardProxyField()
    }

    private func configureKeyboardProxyField() {
        keyboardProxyField.translatesAutoresizingMaskIntoConstraints = false
        keyboardProxyField.autocapitalizationType = .none
        keyboardProxyField.autocorrectionType = .no
        keyboardProxyField.spellCheckingType = .no
        keyboardProxyField.keyboardType = .emailAddress
        keyboardProxyField.returnKeyType = .done
        keyboardProxyField.textContentType = .username
        keyboardProxyField.textColor = .clear
        keyboardProxyField.tintColor = .clear
        keyboardProxyField.backgroundColor = .clear
        keyboardProxyField.alpha = 0.01
        keyboardProxyField.delegate = self
        keyboardProxyField.addTarget(self, action: #selector(keyboardProxyChanged(_:)), for: .editingChanged)
        addSubview(keyboardProxyField)
        NSLayoutConstraint.activate([
            keyboardProxyField.widthAnchor.constraint(equalToConstant: 1),
            keyboardProxyField.heightAnchor.constraint(equalToConstant: 1),
            keyboardProxyField.leadingAnchor.constraint(equalTo: leadingAnchor),
            keyboardProxyField.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    @objc private func handleTapGesture(_ recognizer: UITapGestureRecognizer) {
        guard let statePtr else { return }
        let location = recognizer.location(in: self)
        // Rust iOS bridge now consumes logical points for interaction coordinates.
        let x = Float(location.x)
        let y = Float(location.y)
        WgpuiBackgroundBridge.handleTap(state: statePtr, x: x, y: y)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let bounds = self.bounds
        let scale = effectiveScale
        guard let metalLayer = layer as? CAMetalLayer else {
            print("[WGPUI] layoutSubviews layer is not CAMetalLayer (got \(type(of: layer)))")
            os_log("[WGPUI] layoutSubviews layer is not CAMetalLayer", log: wgpuiLog, type: .error)
            return
        }
        // Make CAMetalLayer explicit about HiDPI backing to avoid implicit 1x drawables.
        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )

        let logicalW = UInt32(max(1.0, bounds.width.rounded(.toNearestOrAwayFromZero)))
        let logicalH = UInt32(max(1.0, bounds.height.rounded(.toNearestOrAwayFromZero)))
        if logicalW == 0 || logicalH == 0 {
            print("[WGPUI] layoutSubviews skip zero size bounds=\(bounds) logicalW=\(logicalW) logicalH=\(logicalH)")
            return
        }

        if statePtr == nil {
            if !WgpuiBackgroundBridge.isAvailable {
                print("[WGPUI] layoutSubviews bridge not available")
                os_log("[WGPUI] layoutSubviews bridge not available", log: wgpuiLog, type: .error)
                return
            }
            print("[WGPUI] layoutSubviews creating state bounds=\(bounds) scale=\(scale) logicalW=\(logicalW) logicalH=\(logicalH)")
            statePtr = WgpuiBackgroundBridge.create(
                layerPtr: Unmanaged.passUnretained(metalLayer).toOpaque(),
                width: logicalW,
                height: logicalH,
                scale: Float(scale)
            )
            if statePtr != nil {
                displayLink = CADisplayLink(target: self, selector: #selector(tick))
                displayLink?.add(to: .main, forMode: .common)
                print("[WGPUI] layoutSubviews state created, displayLink started")
            } else {
                print("[WGPUI] layoutSubviews create returned nil (Rust wgpui_ios_background_create failed)")
            }
        } else {
            WgpuiBackgroundBridge.resize(state: statePtr, width: logicalW, height: logicalH)
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
                self?.keyboardProxyField.resignFirstResponder()
            }
        }
        if WgpuiBackgroundBridge.emailFocused(state: statePtr) {
            WgpuiBackgroundBridge.setEmailFocused(state: statePtr, focused: false)
            DispatchQueue.main.async { [weak self] in
                self?.beginEmailEditing()
            }
        }
    }

    private func beginEmailEditing() {
        guard statePtr != nil else { return }
        keyboardProxyField.text = emailDraft
        if !keyboardProxyField.isFirstResponder {
            keyboardProxyField.becomeFirstResponder()
        }
    }

    @objc private func keyboardProxyChanged(_ textField: UITextField) {
        emailDraft = textField.text ?? ""
        WgpuiBackgroundBridge.setLoginEmail(state: statePtr, emailDraft)
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        textField.resignFirstResponder()
        return false
    }

    func textFieldDidEndEditing(_ textField: UITextField) {
        WgpuiBackgroundBridge.setEmailFocused(state: statePtr, focused: false)
    }

    deinit {
        os_log("[WGPUI] WgpuiBackgroundUIView deinit", log: wgpuiLog, type: .default)
        keyboardProxyField.delegate = nil
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
