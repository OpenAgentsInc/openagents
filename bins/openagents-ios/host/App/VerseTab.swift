// The Verse tab: Verse's bare world, the plaza grid in white light with
// Coder's player controls and the other players in it. UIKit owns the Metal
// layer, the display clock, touches, the motion sensor, and the world key in
// Keychain; Rust owns the world, the player, the camera, the movement stick,
// world presence on the relay, and every frame.
import QuartzCore
import SwiftUI
import UIKit

/// The fields of Coder's native Verse packet (`coder.verse.v1`) this tab reads.
struct WorldPacket: Decodable {
    let schema: String
    let error: String?
    /// The renderer draws to an extended-range surface.
    let hdr_output: Bool?
    let frames_presented: UInt64
    let position: [Double]
    let camera_mode: String
    let camera_yaw: Double
    let camera_pitch: Double
    let camera_distance: Double
    /// Zoomed all the way in: the camera is at the player's head.
    let camera_first_person: Bool?
    /// The pointer holding the movement stick, which pinch never claims.
    let stick_pointer: UInt64?
    let motion_needed: Bool
    /// Other players' avatars with a recent pose.
    let live_remote_entities: UInt64?
    /// The world identity's public key, hex.
    let world_public_key: String?
    let connection: WorldConnectionPacket?
    /// The bare world's ball, for diagnostics.
    let ball: BallPacket?

    struct BallPacket: Decodable {
        let position: [Double]
        let speed: Double
        let asleep: Bool
        let step_ms: Double
    }

    var valid: Bool {
        schema == "coder.verse.v1" && position.count == 3 && position.allSatisfy(\.isFinite)
            && ["touch", "motion"].contains(camera_mode) && camera_yaw.isFinite
            && camera_pitch.isFinite && camera_distance.isFinite && camera_distance > 0
    }
}

/// The world connection's state: offline, paused, connecting, connected, or
/// retrying.
struct WorldConnectionPacket: Decodable {
    let state: String
}

/// The tab's native state: camera mode, errors, and the motion sensor.
@MainActor
final class VerseWorld: ObservableObject {
    @Published private(set) var cameraMode = "touch"
    @Published private(set) var error: String?
    @Published private(set) var motionError: String?
    let motionDriver = DeviceMotionDriver(source: CoreMotionSource())
    var motionAvailable: Bool { motionDriver.available }
    fileprivate weak var surface: VerseWorldView?

    var motionLook: Bool { cameraMode == "motion" }

    func toggleCameraMode() {
        if motionLook {
            motionError = nil
            surface?.send(["action": "camera_mode", "mode": "touch"])
        } else if motionAvailable {
            motionError = nil
            surface?.send(["action": "camera_mode", "mode": "motion"])
        } else {
            motionError = DeviceMotionFailure.unavailable.localizedDescription
        }
    }

    func recenter() { surface?.send(["action": "recenter_camera"]) }

    func retry() { surface?.recreate() }

    fileprivate func receive(_ packet: WorldPacket) {
        if cameraMode != packet.camera_mode { cameraMode = packet.camera_mode }
        if error != packet.error { error = packet.error }
    }

    fileprivate func fail(_ message: String) {
        if error != message { error = message }
    }

    fileprivate func clearError() { error = nil }

    fileprivate func reportMotionFailure(_ failure: Error) {
        motionError = failure.localizedDescription
    }
}

struct VerseTab: View {
    /// The Verse tab is selected.
    let selected: Bool
    @StateObject private var world = VerseWorld()
    @Environment(\.scenePhase) private var phase

    private var active: Bool { selected && phase == .active }

    var body: some View {
        GeometryReader { outer in
            let safe = outer.safeAreaInsets
            ZStack(alignment: .bottomTrailing) {
                VerseWorldSurface(world: world, active: active, insets: safe)
                    .ignoresSafeArea()
                controls
                    .padding(.trailing, safe.trailing + 16)
                    .padding(.bottom, 12)
            }
            .overlay(alignment: .top) {
                if let error = world.error {
                    VStack(spacing: 8) {
                        Text(error).font(.callout).textSelection(.enabled)
                            .accessibilityIdentifier("verse-error")
                        Button("Retry world renderer") { world.retry() }
                    }
                    .padding(.top, 12)
                    .padding(.horizontal, 16)
                }
            }
        }
        .background(Color.black.ignoresSafeArea())
    }

    /// The same camera controls as Coder's world: the touch or motion look
    /// toggle and Recenter.
    private var controls: some View {
        VStack(alignment: .trailing, spacing: 4) {
            if let error = world.motionError {
                Text(error).font(.caption).multilineTextAlignment(.trailing)
                    .accessibilityIdentifier("verse-motion-error")
            }
            HStack(spacing: 16) {
                Button { world.toggleCameraMode() } label: {
                    Image(systemName: world.motionLook ? "gyroscope" : "hand.draw")
                        .frame(width: 44, height: 44)
                }
                .disabled(!active || (!world.motionAvailable && !world.motionLook))
                .accessibilityLabel(world.motionLook ? "Motion look" : "Touch look")
                .accessibilityIdentifier("verse-camera-mode")
                .accessibilityHint(world.motionAvailable
                    ? "Switches between dragging and phone orientation for camera control."
                    : "Motion look is unavailable on this device. Drag the world to look around.")
                Button { world.recenter() } label: {
                    Image(systemName: "scope").frame(width: 44, height: 44)
                }
                .disabled(!active)
                .accessibilityLabel("Recenter camera")
                .accessibilityIdentifier("verse-motion-recenter")
            }
        }
    }
}

private struct VerseWorldSurface: UIViewRepresentable {
    let world: VerseWorld
    let active: Bool
    /// Safe-area insets of the tab's content, including the tab bar.
    let insets: EdgeInsets

    func makeUIView(context: Context) -> VerseWorldView { VerseWorldView(world: world) }

    func updateUIView(_ uiView: VerseWorldView, context: Context) {
        uiView.setInsets(insets)
        uiView.setActive(active)
    }

    static func dismantleUIView(_ uiView: VerseWorldView, coordinator: ()) { uiView.detach() }
}

@MainActor
private final class VerseWorldDisplayTarget: NSObject {
    weak var view: VerseWorldView?
    @objc func frame(_ link: CADisplayLink) { view?.frame(link) }
}

@MainActor
final class VerseWorldView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    private let world: VerseWorld
    private var handle: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private let displayTarget = VerseWorldDisplayTarget()
    private var wantedActive = false
    private var running = false
    private var creationFailed = false
    private var extent: (width: UInt32, height: UInt32, scale: CGFloat)?
    private var insets = EdgeInsets()
    private var pointers: [ObjectIdentifier: UInt64] = [:]
    private var nextPointer: UInt64 = 1
    private var pinchAdmission = PinchAdmission()
    /// Pointers Rust took for the movement stick. They stay out of pinch
    /// arbitration and always reach Rust, so walking continues while the
    /// other hand pinches.
    private var stickPointers: Set<UInt64> = []
    private var wantsHDR = false
    private lazy var script = VerseWorldScript.fromLaunchArguments()

    init(world: VerseWorld) {
        self.world = world
        super.init(frame: .zero)
        isMultipleTouchEnabled = true
        isOpaque = true
        backgroundColor = .black
        isAccessibilityElement = true
        accessibilityLabel = "Verse world"
        accessibilityIdentifier = "verse-surface"
        accessibilityHint = "Drag anywhere to look around and use the stick at the bottom left to move. Double-tap to jump and pinch with two fingers to zoom."
        accessibilityTraits = [.allowsDirectInteraction]
        world.surface = self
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

    func setInsets(_ value: EdgeInsets) {
        guard value != insets, value.top.isFinite, value.bottom.isFinite,
              value.leading.isFinite, value.trailing.isFinite else { return }
        insets = value
        cancelPointers()
        sendInsets()
    }

    /// The world runs behind the status bar and the tab bar; the insets keep
    /// the stick above the tab bar.
    private func sendInsets() {
        send(["action": "hud_insets", "top": Double(max(0, insets.top)),
              "right": Double(max(0, insets.trailing)), "bottom": Double(max(0, insets.bottom)),
              "left": Double(max(0, insets.leading))])
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
            configureDynamicRange(metal)
            var configuration: [String: Any] = ["width": width, "height": height,
                                                "scale": Double(scale), "hdr": wantsHDR]
            // World presence signs with its own key, never the device key. If
            // Keychain cannot provide it, the world stays offline.
            if let secret = try? DeviceKey.loadOrCreateVerse() {
                configuration["world_secret_hex"] = secret.map { String(format: "%02x", $0) }.joined()
            }
            guard let data = try? JSONSerialization.data(withJSONObject: configuration) else { return }
            handle = data.withUnsafeBytes {
                openagents_verse_create(Unmanaged.passUnretained(metal).toOpaque(),
                                        $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
            }
            guard handle != nil else {
                creationFailed = true
                let reason = Self.text(openagents_verse_create_error())
                world.fail(reason.isEmpty ? "The Metal world could not start on this device." : reason)
                return
            }
            sendInsets()
            send(["action": "snapshot"])
            updateActivity()
        } else if changed, handle != nil {
            send(["action": "resize", "width": width, "height": height, "scale": Double(scale)])
        }
    }

    /// On a screen with EDR headroom, tag the layer as extended linear sRGB so
    /// the world's bright line cores reach the display as highlights.
    private func configureDynamicRange(_ metal: CAMetalLayer) {
        let screen = window?.windowScene?.screen ?? UIScreen.main
        wantsHDR = screen.potentialEDRHeadroom > 1.01
            && !ProcessInfo.processInfo.arguments.contains("--sdr")
        metal.wantsExtendedDynamicRangeContent = wantsHDR
        metal.colorspace = wantsHDR ? CGColorSpace(name: CGColorSpace.extendedLinearSRGB) : nil
    }

    private func currentHeadroom() -> Double {
        guard wantsHDR else { return 1.0 }
        let screen = window?.windowScene?.screen ?? UIScreen.main
        return Double(max(1.0, screen.currentEDRHeadroom))
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
        send(["action": "active", "active": active])
        if active, displayLink == nil {
            let link = CADisplayLink(target: displayTarget, selector: #selector(VerseWorldDisplayTarget.frame(_:)))
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
            script?.step(self, bounds: bounds.size, insets: insets)
            send(["action": "frame", "timestamp": link.timestamp, "headroom": currentHeadroom()])
        }
    }

    @discardableResult
    func send(_ request: [String: Any]) -> WorldPacket? {
        guard let handle,
              let input = try? JSONSerialization.data(withJSONObject: request),
              input.count <= 4096 else { return nil }
        let output = input.withUnsafeBytes {
            openagents_verse_call(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
        }
        defer { openagents_mobile_buffer_free(output) }
        guard let data = output.data, output.len > 0, output.len <= 1_048_576,
              let packet = try? JSONDecoder().decode(WorldPacket.self, from: Data(bytes: data, count: output.len)),
              packet.valid else {
            world.fail("The world returned an invalid packet.")
            return nil
        }
        if wantsHDR, packet.hdr_output == false, let metal = layer as? CAMetalLayer {
            // The surface stayed in standard range: undo the layer setup.
            wantsHDR = false
            metal.wantsExtendedDynamicRangeContent = false
            metal.colorspace = nil
        }
        world.receive(packet)
        syncMotion(packet)
        observe(packet)
        return packet
    }

    /// Debug and simulator builds expose position and camera to UI checks.
    private func observe(_ packet: WorldPacket) {
        #if DEBUG || targetEnvironment(simulator)
        let value = String(format: "frames %llu position %.2f %.2f %.2f yaw %.2f pitch %.2f zoom %.2f%@ %@ world %@ players %llu key %@",
                           packet.frames_presented, packet.position[0], packet.position[1], packet.position[2],
                           packet.camera_yaw, packet.camera_pitch, packet.camera_distance,
                           packet.camera_first_person == true ? " first-person" : "", packet.camera_mode,
                           packet.connection?.state ?? "unknown", packet.live_remote_entities ?? 0,
                           packet.world_public_key ?? "")
        if accessibilityValue != value { accessibilityValue = value }
        if script != nil, packet.frames_presented % 30 == 0 {
            if let ball = packet.ball, ball.position.count == 3 {
                NSLog("verse-world %@ ball %.2f %.2f %.2f speed %.2f %@ step %.3f ms", value,
                      ball.position[0], ball.position[1], ball.position[2], ball.speed,
                      ball.asleep ? "asleep" : "awake", ball.step_ms)
            } else {
                NSLog("verse-world %@", value)
            }
        }
        #endif
    }

    private func syncMotion(_ packet: WorldPacket) {
        do {
            if try world.motionDriver.setNeeded(running && window != nil && packet.motion_needed,
                                                now: CACurrentMediaTime()) {
                DispatchQueue.main.async { [weak self] in self?.send(["action": "reset_motion"]) }
            }
        } catch { failMotion(error) }
    }

    private func pollMotion(now: TimeInterval) {
        do {
            if let sample = try world.motionDriver.poll(now: now) {
                send(["action": "device_motion", "quaternion": sample.quaternion,
                      "timestamp": sample.timestamp, "received_at": now])
            }
        } catch { failMotion(error) }
    }

    private func stopMotion() {
        _ = try? world.motionDriver.setNeeded(false, now: CACurrentMediaTime())
    }

    private func failMotion(_ error: Error) {
        stopMotion()
        DispatchQueue.main.async { [weak self] in
            self?.send(["action": "camera_mode", "mode": "touch"])
            self?.world.reportMotionFailure(error)
        }
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
            if pointer(id, phase: "down", at: at)?.stick_pointer == id {
                stickPointers.insert(id)
            } else {
                pinchAdmission.down(id, x: Double(at.x), y: Double(at.y), time: touch.timestamp)
            }
        }
        if pinchAdmission.reserved { cancelRustPointers(keepingStick: true) }
        _ = pinchAdmission.scale()
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            guard let id = pointers[ObjectIdentifier(touch)] else { continue }
            let at = touch.location(in: self)
            if stickPointers.contains(id) {
                pointer(id, phase: "move", at: at)
                continue
            }
            pinchAdmission.move(id, x: Double(at.x), y: Double(at.y))
            if !pinchAdmission.reserved { pointer(id, phase: "move", at: at) }
        }
        if running, let scale = pinchAdmission.scale(), scale.isFinite, scale > 0 {
            send(["action": "pinch_zoom", "scale": scale])
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { finish(touches, phase: "up") }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { finish(touches, phase: "cancel") }

    private func finish(_ touches: Set<UITouch>, phase: String) {
        for touch in touches {
            guard let id = pointers.removeValue(forKey: ObjectIdentifier(touch)) else { continue }
            if stickPointers.remove(id) != nil {
                pointer(id, phase: phase, at: touch.location(in: self))
                continue
            }
            if !pinchAdmission.reserved { pointer(id, phase: phase, at: touch.location(in: self)) }
            pinchAdmission.up(id)
        }
    }

    @discardableResult
    fileprivate func pointer(_ id: UInt64, phase: String, at: CGPoint) -> WorldPacket? {
        guard at.x.isFinite, at.y.isFinite else { return nil }
        return send(["action": "pointer", "id": id, "phase": phase, "x": Double(at.x), "y": Double(at.y)])
    }

    /// Cancels every pointer in Rust; a pinch keeps the stick's.
    private func cancelRustPointers(keepingStick: Bool = false) {
        for id in pointers.values where !(keepingStick && stickPointers.contains(id)) {
            send(["action": "pointer", "id": id, "phase": "cancel", "x": 0, "y": 0])
        }
    }

    private func cancelPointers() {
        cancelRustPointers()
        pointers.removeAll()
        stickPointers.removeAll()
        pinchAdmission.reset()
    }

    func recreate() {
        releaseSurface()
        creationFailed = false
        world.clearError()
        layoutSurface()
    }

    func detach() {
        releaseSurface()
        if world.surface === self { world.surface = nil }
    }

    private func releaseSurface() {
        displayLink?.invalidate()
        displayLink = nil
        running = false
        pinchAdmission.reset()
        stopMotion()
        cancelPointers()
        if let handle {
            send(["action": "active", "active": false])
            openagents_verse_destroy(handle)
            self.handle = nil
        }
        extent = nil
    }

    private static func text(_ buffer: OpenAgentsMobileBuffer) -> String {
        defer { openagents_mobile_buffer_free(buffer) }
        guard let data = buffer.data, buffer.len > 0 else { return "" }
        return String(decoding: UnsafeBufferPointer(start: data, count: buffer.len), as: UTF8.self)
    }
}

/// A developer launch argument, `--verse-script look,walk,jump,zoom`, that
/// drives the world through the same pointer path as a finger, so the
/// controls can be checked on a simulator without touching the screen.
/// `push` holds the stick forward for the whole step, walking into the ball
/// ahead of the spawn, `closer` pinches in past the nearest orbit into first
/// person, `walkpinch` holds the stick forward while pinching in, and `wait`
/// does nothing for a step.
/// Debug and simulator builds only.
@MainActor
private final class VerseWorldScript {
    private var steps: [String]
    private var frame = 0
    private let pointer: UInt64 = 1_000_000

    private init(steps: [String]) { self.steps = steps }

    static func fromLaunchArguments() -> VerseWorldScript? {
        #if DEBUG || targetEnvironment(simulator)
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: "--verse-script"), index + 1 < arguments.count else {
            return nil
        }
        let steps = arguments[index + 1].split(separator: ",").map(String.init)
        return steps.isEmpty ? nil : VerseWorldScript(steps: steps)
        #else
        return nil
        #endif
    }

    /// Each step runs for 90 frames after a one-second settle.
    func step(_ view: VerseWorldView, bounds: CGSize, insets: EdgeInsets) {
        frame += 1
        guard frame > 60, let current = steps.first else { return }
        let t = frame - 61
        // The stick's center, as Rust places it in the bare world: centered
        // between the safe-area sides, 24 + 56 points above the bottom inset.
        let stick = CGPoint(x: insets.leading + (bounds.width - insets.leading - insets.trailing) / 2,
                            y: bounds.height - insets.bottom - 80)
        let center = CGPoint(x: bounds.width * 0.5, y: bounds.height * 0.4)
        switch (current, t) {
        case ("look", 0): view.pointer(pointer, phase: "down", at: center)
        case ("look", 1..<40):
            view.pointer(pointer, phase: "move", at: CGPoint(x: center.x + CGFloat(t) * 3, y: center.y + CGFloat(t)))
        case ("look", 40): view.pointer(pointer, phase: "up", at: CGPoint(x: center.x + 117, y: center.y + 39))
        case ("walk", 0): view.pointer(pointer, phase: "down", at: stick)
        case ("walk", 1): view.pointer(pointer, phase: "move", at: CGPoint(x: stick.x, y: stick.y - 56))
        case ("walk", 80): view.pointer(pointer, phase: "up", at: CGPoint(x: stick.x, y: stick.y - 56))
        case ("push", 0): view.pointer(pointer, phase: "down", at: stick)
        case ("push", 1): view.pointer(pointer, phase: "move", at: CGPoint(x: stick.x, y: stick.y - 56))
        case ("push", 89): view.pointer(pointer, phase: "up", at: CGPoint(x: stick.x, y: stick.y - 56))
        case ("jump", 0), ("jump", 6): view.pointer(pointer + UInt64(t), phase: "down", at: center)
        case ("jump", 2), ("jump", 8): view.pointer(pointer + UInt64(t - 2), phase: "up", at: center)
        case ("zoom", 0..<20): view.send(["action": "pinch_zoom", "scale": 0.97])
        case ("closer", 0..<40): view.send(["action": "pinch_zoom", "scale": 1.1])
        // Hold the stick forward and pinch in at the same time.
        case ("walkpinch", 0): view.pointer(pointer, phase: "down", at: stick)
        case ("walkpinch", 1): view.pointer(pointer, phase: "move", at: CGPoint(x: stick.x, y: stick.y - 56))
        case ("walkpinch", 2..<60): view.send(["action": "pinch_zoom", "scale": 1.02])
        case ("walkpinch", 89): view.pointer(pointer, phase: "up", at: CGPoint(x: stick.x, y: stick.y - 56))
        case ("recenter", 0): view.send(["action": "recenter_camera"])
        default: break
        }
        if t == 89 {
            steps.removeFirst()
            frame = 60
        }
    }
}
