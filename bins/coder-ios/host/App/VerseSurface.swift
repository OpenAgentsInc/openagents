// UIKit owns the Metal layer and display clock. Rust owns every frame and
// interprets bounded pointer events; no game physics or geometry lives here.
import QuartzCore
import SwiftUI
import UIKit

struct VerseSurface: UIViewRepresentable {
    let bridge: VerseBridge
    let active: Bool
    let label: String
    let safeInsets: EdgeInsets

    func makeUIView(context: Context) -> VerseMetalView {
        let view = VerseMetalView(bridge: bridge)
        view.accessibilityLabel = label
        view.setHudInsets(safeInsets)
        return view
    }

    func updateUIView(_ uiView: VerseMetalView, context: Context) {
        uiView.setHudInsets(safeInsets)
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
    private var computerAccessible = false
    private var hudPointers = Set<UInt64>()
    private var hudInsets = EdgeInsets()
    private var mapState: VerseMap?
    private var accessibilityActionState = ""
    private var highestObservedY = 0.0
    private var pinchAdmission = PinchAdmission()

    init(bridge: VerseBridge) {
        self.bridge = bridge
        super.init(frame: .zero)
        isMultipleTouchEnabled = true
        isOpaque = true
        isAccessibilityElement = true
        accessibilityIdentifier = "verse-surface"
        accessibilityHint = "Drag on the left to move and on the right to look around. Double-tap to jump and pinch with two fingers to zoom. Walk to the computer and tap its screen to open your chats."
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
                if let relay = bridge.storedWorldRelay() { configuration["world_relay"] = relay }
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
                sendHudInsets()
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

    func setHudInsets(_ insets: EdgeInsets) {
        guard insets.top.isFinite, insets.trailing.isFinite, insets.bottom.isFinite, insets.leading.isFinite,
              insets.top >= 0, insets.trailing >= 0, insets.bottom >= 0, insets.leading >= 0 else { return }
        guard hudInsets != insets else { return }
        hudInsets = insets
        cancelPointers()
        sendHudInsets()
    }

    private func sendHudInsets() {
        send(["action": "hud_insets", "top": Double(hudInsets.top), "right": Double(hudInsets.trailing),
              "bottom": Double(hudInsets.bottom), "left": Double(hudInsets.leading)],
             forcePublish: true, deferred: true)
    }

    func setActive(_ active: Bool) {
        wantedActive = active
        updateActivity()
    }

    private func updateActivity() {
        let active = wantedActive && window != nil && handle != nil
        guard active != running else { return }
        running = active
        if !active {
            pinchAdmission.reset()
            stopMotion()
            cancelPointers()
        }
        send(["action": "active", "active": active], forcePublish: true, deferred: true)
        if active, displayLink == nil {
            let link = CADisplayLink(target: displayTarget, selector: #selector(VerseDisplayTarget.frame(_:)))
            link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)
            link.add(to: .main, forMode: .common)
            displayLink = link
        }
        displayLink?.isPaused = !active
    }

    func frame(_ link: CADisplayLink) {
        guard running, window != nil else { return }
        autoreleasepool {
            pollMotion(now: CACurrentMediaTime())
            send(["action": "frame", "timestamp": link.timestamp], forcePublish: false)
        }
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
        if case let .success(packet) = result {
            syncMotion(packet)
            syncAccessibility(packet)
        }
        if deferred {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.bridge.receive(result, from: self, force: forcePublish)
            }
        } else { bridge.receive(result, from: self, force: forcePublish) }
        return result
    }

    private func syncAccessibility(_ packet: VersePacket) {
        let available = running && packet.computer.near && packet.computer.visible
            && !packet.computer_open && !packet.gym_open
        computerAccessible = available
        mapState = packet.map
        let map = packet.map
        let mapAvailable = running && map.visible
        let state = "\(available):\(mapAvailable):\(map.expanded):\(map.destination != nil):" + map.landmarks.map(\.id).joined(separator: ",")
        if state != accessibilityActionState {
            accessibilityActionState = state
            var actions: [UIAccessibilityCustomAction] = []
            if available {
                actions.append(UIAccessibilityCustomAction(name: "Use computer", target: self, selector: #selector(useComputerAccessibly)))
            }
            if mapAvailable {
                actions.append(UIAccessibilityCustomAction(name: map.expanded ? "Close map" : "Open map", target: self, selector: #selector(toggleMapAccessibly)))
                if map.destination != nil {
                    actions.append(UIAccessibilityCustomAction(name: "Cancel walk", target: self, selector: #selector(cancelMapWalkAccessibly)))
                }
                for landmark in map.landmarks {
                    actions.append(UIAccessibilityCustomAction(name: "Walk to \(landmark.label)") { [weak self] _ in
                        guard let self, self.running, self.mapState?.visible == true,
                              let current = self.mapState?.landmarks.first(where: { $0.id == landmark.id }) else { return false }
                        guard case let .success(result)? = self.send(["action": "map_walk", "x": current.x, "z": current.z], forcePublish: true) else { return false }
                        return result.error == nil
                    })
                }
            }
            accessibilityCustomActions = actions
        }
        if bridge.synthetic {
            // Keep test observations in accessibility metadata, not in the HUD.
            highestObservedY = max(highestObservedY, packet.position[1])
            let metadata: [String: Any] = [
                "frames": packet.frames_presented,
                "position": packet.position,
                "highest_observed_y": highestObservedY,
                "camera": [packet.camera_yaw, packet.camera_pitch],
                "camera_distance": packet.camera_distance,
                "motion_needed": packet.motion_needed,
                "gym_active": packet.gym_active,
                "computer_ready": available,
                "computer_target": [packet.computer.screen_x, packet.computer.screen_y],
                "map": packet.map.observation,
            ]
            accessibilityValue = (try? JSONSerialization.data(withJSONObject: metadata))
                .flatMap { String(data: $0, encoding: .utf8) }
        } else {
            accessibilityValue = available ? "Computer in reach" : "Exploring Verse"
        }
    }

    @objc private func useComputerAccessibly() -> Bool {
        guard running, computerAccessible else { return false }
        guard let result = send(["action": "interact_computer"], forcePublish: true) else { return false }
        if case let .success(packet) = result { return packet.computer_open }
        return false
    }

    @objc private func toggleMapAccessibly() -> Bool {
        guard running, mapState?.visible == true else { return false }
        guard case let .success(result)? = send(["action": "map_toggle"], forcePublish: true) else { return false }
        return result.error == nil
    }

    @objc private func cancelMapWalkAccessibly() -> Bool {
        guard running, mapState?.visible == true, mapState?.destination != nil else { return false }
        guard case let .success(result)? = send(["action": "map_cancel"], forcePublish: true) else { return false }
        return result.error == nil
    }

    private func syncMotion(_ packet: VersePacket) {
        do {
            let changed = try bridge.motionDriver.setNeeded(running && window != nil && packet.motion_needed,
                                                             now: CACurrentMediaTime())
            if changed {
                send(["action": "reset_motion"], forcePublish: false, deferred: true)
            }
        } catch { failMotion(error) }
    }

    private func pollMotion(now: TimeInterval) {
        do {
            if let sample = try bridge.motionDriver.poll(now: now) {
                let result = send(["action": "device_motion", "quaternion": sample.quaternion,
                                   "timestamp": sample.timestamp, "received_at": now], forcePublish: false)
                // A packet can retain an unrelated world error. Rust refuses
                // invalid camera input; native sensor failures have their own
                // availability, freshness, and decoder path.
                if case let .failure(error) = result { failMotion(error) }
            }
        } catch { failMotion(error) }
    }

    private func stopMotion() {
        _ = try? bridge.motionDriver.setNeeded(false, now: CACurrentMediaTime())
    }

    private func failMotion(_ error: Error) {
        stopMotion()
        send(["action": "camera_mode", "mode": "touch"], forcePublish: true, deferred: true)
        DispatchQueue.main.async { [weak self] in self?.bridge.reportMotionFailure(error) }
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard running else { return }
        // UIKit delivers a set, so admit the oldest contact first.
        for touch in touches.sorted(by: { $0.timestamp < $1.timestamp }) {
            guard pointers.count < 8 else { continue }
            let id = nextPointer
            nextPointer &+= 1
            pointers[ObjectIdentifier(touch)] = id
            let at = touch.location(in: self)
            pointer(touch, id: id, phase: "down")
            if mapState?.captured_pointers.contains(id) == true {
                hudPointers.insert(id)
            } else {
                pinchAdmission.down(id, x: Double(at.x), y: Double(at.y), time: touch.timestamp)
            }
        }
        if pinchAdmission.reserved { cancelRustPointers(keepingHud: true) }
        _ = pinchAdmission.scale()
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            if let id = pointers[ObjectIdentifier(touch)] {
                let at = touch.location(in: self)
                if hudPointers.contains(id) {
                    pointer(touch, id: id, phase: "move")
                } else {
                    pinchAdmission.move(id, x: Double(at.x), y: Double(at.y))
                    if !pinchAdmission.reserved { pointer(touch, id: id, phase: "move") }
                }
            }
        }
        if running, hudPointers.isEmpty, let scale = pinchAdmission.scale(), scale.isFinite, scale > 0 {
            send(["action": "pinch_zoom", "scale": scale], forcePublish: true)
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { finish(touches, phase: "up") }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { finish(touches, phase: "cancel") }

    private func finish(_ touches: Set<UITouch>, phase: String) {
        for touch in touches {
            if let id = pointers.removeValue(forKey: ObjectIdentifier(touch)) {
                if hudPointers.remove(id) != nil || !pinchAdmission.reserved { pointer(touch, id: id, phase: phase) }
                pinchAdmission.up(id)
            }
        }
    }

    private func pointer(_ touch: UITouch, id: UInt64, phase: String) {
        let at = touch.location(in: self)
        guard at.x.isFinite, at.y.isFinite else { return }
        send(["action": "pointer", "id": id, "phase": phase,
              "x": Double(at.x), "y": Double(at.y)], forcePublish: phase != "move")
    }

    private func cancelRustPointers(keepingHud: Bool = false) {
        for id in pointers.values where !keepingHud || !hudPointers.contains(id) {
            send(["action": "pointer", "id": id, "phase": "cancel", "x": 0, "y": 0],
                 forcePublish: false, deferred: true)
        }
    }

    private func cancelPointers() {
        cancelRustPointers()
        pointers.removeAll()
        hudPointers.removeAll()
        pinchAdmission.reset()
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
        running = false
        pinchAdmission.reset()
        stopMotion()
        cancelPointers()
        if let handle {
            send(["action": "active", "active": false], forcePublish: false, deferred: true)
            coder_verse_destroy(handle)
            self.handle = nil
        }
        running = false
        extent = nil
        highestObservedY = 0
    }
}
