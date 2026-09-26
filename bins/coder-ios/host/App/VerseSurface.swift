// UIKit owns the Metal layer and display clock. Rust owns every frame and
// interprets bounded pointer events; no game physics or geometry lives here.
import QuartzCore
import SwiftUI
import UIKit

struct VerseSurface: UIViewRepresentable {
    let bridge: VerseBridge
    let active: Bool
    let label: String

    func makeUIView(context: Context) -> VerseMetalView {
        let view = VerseMetalView(bridge: bridge)
        view.accessibilityLabel = label
        return view
    }

    func updateUIView(_ uiView: VerseMetalView, context: Context) {
        uiView.setActive(active)
    }

    static func dismantleUIView(_ uiView: VerseMetalView, coordinator: ()) {
        uiView.detach()
    }
}

@MainActor
private final class VerseDisplayTarget: NSObject {
    weak var view: VerseMetalView?
    @objc func frame(_ link: CADisplayLink) { view?.frame(link) }
}

@MainActor
final class VerseMetalView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    private let bridge: VerseBridge
    private var handle: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private let displayTarget = VerseDisplayTarget()
    private var wantedActive = false
    private var running = false
    private var creationFailed = false
    private var extent: (width: UInt32, height: UInt32, scale: CGFloat)?
    private var pointers: [ObjectIdentifier: UInt64] = [:]
    private var nextPointer: UInt64 = 1

    init(bridge: VerseBridge) {
        self.bridge = bridge
        super.init(frame: .zero)
        isMultipleTouchEnabled = true
        isOpaque = true
        isAccessibilityElement = true
        accessibilityIdentifier = "verse-surface"
        accessibilityHint = "Drag on the left to move and on the right to look around."
        accessibilityTraits = [.allowsDirectInteraction]
        bridge.bind(self)
        displayTarget.view = self
    }

    required init?(coder: NSCoder) { nil }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        layoutSurface()
        updateActivity()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        layoutSurface()
    }

    private func layoutSurface() {
        guard window != nil, bounds.width > 0, bounds.height > 0,
              let metal = layer as? CAMetalLayer else { return }
        let scale = max(1, traitCollection.displayScale)
        let width = UInt32(min(4096, max(1, (bounds.width * scale).rounded())))
        let height = UInt32(min(4096, max(1, (bounds.height * scale).rounded())))
        contentScaleFactor = scale
        metal.contentsScale = scale
        metal.drawableSize = CGSize(width: Int(width), height: Int(height))
        metal.framebufferOnly = true
        metal.presentsWithTransaction = false
        let changed = extent?.width != width || extent?.height != height || extent?.scale != scale
        extent = (width, height, scale)
        if handle == nil, !creationFailed {
            do {
                let secret = try DeviceIdentity.loadOrCreateVerse(synthetic: bridge.synthetic)
                var configuration: [String: Any] = [
                    "secret_hex": secret.map { String(format: "%02x", $0) }.joined(),
                    "width": width, "height": height, "scale": Double(scale),
                    "synthetic": bridge.synthetic,
                    "synthetic_gym": bridge.synthetic && ProcessInfo.processInfo.arguments.contains("--gym-preview"),
                ]
                if let code = bridge.storedGymCode() { configuration["gym_code"] = code }
                let config = try JSONSerialization.data(withJSONObject: configuration)
                handle = config.withUnsafeBytes {
                    coder_verse_create(Unmanaged.passUnretained(metal).toOpaque(),
                                       $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
                }
                guard handle != nil else {
                    let failure = try VerseBridge.decode(coder_verse_blueprint())
                    throw ReaderError.message(failure.error ?? "The Metal world could not start on this device.")
                }
                send(["action": "snapshot"], forcePublish: true, deferred: true)
                updateActivity()
            } catch {
                creationFailed = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.bridge.receive(.failure(error), from: self, force: true)
                }
            }
        } else if changed, handle != nil {
            send(["action": "resize", "width": width, "height": height, "scale": Double(scale)],
                 forcePublish: true, deferred: true)
        }
    }

    func setActive(_ active: Bool) {
        wantedActive = active
        updateActivity()
    }

    private func updateActivity() {
        let active = wantedActive && window != nil && handle != nil
        guard active != running else { return }
        if !active { cancelPointers() }
        running = active
        send(["action": "active", "active": active], forcePublish: true, deferred: true)
        if active, displayLink == nil {
            let link = CADisplayLink(target: displayTarget, selector: #selector(VerseDisplayTarget.frame(_:)))
            link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 30, preferred: 30)
            link.add(to: .main, forMode: .common)
            displayLink = link
        }
        displayLink?.isPaused = !active
    }

    func frame(_ link: CADisplayLink) {
        guard running, window != nil else { return }
        _ = autoreleasepool { send(["action": "frame", "timestamp": link.timestamp], forcePublish: false) }
    }

    @discardableResult
    func send(_ request: [String: Any], forcePublish: Bool, deferred: Bool = false) -> Result<VersePacket, Error>? {
        guard let handle else { return nil }
        let result: Result<VersePacket, Error>
        do {
            let input = try JSONSerialization.data(withJSONObject: request)
            let limit = request["action"] as? String == "gym_configure" ? 96 * 1024 : 4_096
            guard input.count <= limit else { throw ReaderError.message("World input exceeds its bound.") }
            let output = input.withUnsafeBytes {
                coder_verse_call(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
            }
            result = .success(try VerseBridge.decode(output))
        } catch { result = .failure(error) }
        if deferred {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.bridge.receive(result, from: self, force: forcePublish)
            }
        } else { bridge.receive(result, from: self, force: forcePublish) }
        return result
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard running else { return }
        for touch in touches {
            guard pointers.count < 8 else { continue }
            let id = nextPointer
            nextPointer &+= 1
            pointers[ObjectIdentifier(touch)] = id
            pointer(touch, id: id, phase: "down")
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            if let id = pointers[ObjectIdentifier(touch)] { pointer(touch, id: id, phase: "move") }
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { finish(touches, phase: "up") }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { finish(touches, phase: "cancel") }

    private func finish(_ touches: Set<UITouch>, phase: String) {
        for touch in touches {
            if let id = pointers.removeValue(forKey: ObjectIdentifier(touch)) { pointer(touch, id: id, phase: phase) }
        }
    }

    private func pointer(_ touch: UITouch, id: UInt64, phase: String) {
        let at = touch.location(in: self)
        guard at.x.isFinite, at.y.isFinite else { return }
        send(["action": "pointer", "id": id, "phase": phase,
              "x": Double(at.x), "y": Double(at.y)], forcePublish: phase != "move")
    }

    private func cancelPointers() {
        for id in pointers.values {
            send(["action": "pointer", "id": id, "phase": "cancel", "x": 0, "y": 0],
                 forcePublish: false, deferred: true)
        }
        pointers.removeAll()
    }

    func recreate() {
        releaseSurface()
        creationFailed = false
        layoutSurface()
    }

    func detach() {
        releaseSurface()
        bridge.unbind(self)
    }

    private func releaseSurface() {
        displayLink?.invalidate()
        displayLink = nil
        cancelPointers()
        if let handle {
            send(["action": "active", "active": false], forcePublish: false, deferred: true)
            coder_verse_destroy(handle)
            self.handle = nil
        }
        running = false
        extent = nil
    }
}
