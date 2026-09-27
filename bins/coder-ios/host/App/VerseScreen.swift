// Rust draws and hit-tests the computer in the world. Native code lays out
// its reader and pairing panel only after that interaction opens it.
import SwiftUI

struct VerseScreen: View {
    @StateObject private var bridge: VerseBridge
    @ObservedObject var reader: MobileBridge
    @Environment(\.scenePhase) private var phase
    @State private var pairing = false
    let synthetic: Bool

    init(reader: MobileBridge, synthetic: Bool) {
        self.reader = reader
        self.synthetic = synthetic
        _bridge = StateObject(wrappedValue: VerseBridge(synthetic: synthetic))
    }

    private var active: Bool { phase == .active }
    private var computerOpen: Bool { bridge.packet?.computer_open == true }
    private var gymOpen: Bool { bridge.packet?.gym_open == true }
    private var panelOpen: Bool { computerOpen || gymOpen }
    private var motionLook: Bool { bridge.packet?.camera_mode == "motion" }

    var body: some View {
        GeometryReader { safeGeometry in
            // Capture the window's safe areas before extending the actual Metal
            // surface. World projection coordinates use the full canvas size.
            let safe = safeGeometry.safeAreaInsets
            GeometryReader { geometry in
                canvas(size: geometry.size, safe: safe)
            }
            .ignoresSafeArea()
        }
        .onChange(of: active) { _, enabled in reader.setLifecycle(enabled) }
    }

    private func canvas(size: CGSize, safe: EdgeInsets) -> some View {
        ZStack(alignment: .topLeading) {
            if let packet = bridge.packet, let view = packet.view {
                NativeRenderer(node: view.root, revision: view.revision,
                               followTarget: nil, followChanged: nil, surface: mount) { _ in }
                    .frame(width: size.width, height: size.height)
            }
            VStack(alignment: .leading, spacing: 8) {
                if let error = bridge.nativeError ?? bridge.packet?.error {
                    Text(error).font(.callout).textSelection(.enabled).accessibilityIdentifier("verse-error")
                    Button("Retry world renderer") { bridge.retry() }
                }
                Spacer()
                if !panelOpen {
                    HStack {
                        Button {
                            bridge.toggleCameraMode()
                        } label: {
                            Label(motionLook ? "Motion look" : "Touch look", systemImage: motionLook ? "gyroscope" : "hand.draw")
                        }
                        .disabled(!active || (!bridge.motionAvailable && !motionLook))
                        .accessibilityIdentifier("verse-camera-mode")
                        .accessibilityHint(bridge.motionAvailable
                            ? "Switches between dragging and phone orientation for camera control."
                            : "Motion look is unavailable on this device. Drag the world to look around.")
                        .highPriorityGesture(
                            LongPressGesture(minimumDuration: 0.6).onEnded { _ in bridge.previewMotion() },
                            including: bridge.motionSynthetic ? .all : .none)
                        Spacer()
                        if motionLook {
                            Button("Recenter") { bridge.recenterMotion() }
                                .disabled(!active).accessibilityIdentifier("verse-motion-recenter")
                        }
                    }
                    if let error = bridge.motionError {
                        Text(error).font(.caption).accessibilityIdentifier("verse-motion-error")
                    }
                }
            }
            .padding(.top, safe.top + 12)
            .padding(.bottom, safe.bottom + 12)
            .padding(.leading, safe.leading + 16)
            .padding(.trailing, safe.trailing + 16)
            .allowsHitTesting(!panelOpen)
            if let computer = bridge.packet?.computer, !gymOpen, computer.visible || computerOpen {
                let anchor = CGPoint(x: clamped(computer.screen_x, 0, 1) * size.width,
                                     y: clamped(computer.screen_y, 0, 1) * size.height)
                if computerOpen {
                    anchoredPanel(anchor: anchor, size: size, safe: safe)
                }
            }
            if let gym = bridge.packet?.gym, !computerOpen, gymOpen || (gym.inside && gym.visible) {
                let anchor = CGPoint(x: clamped(gym.screen_x, 0, 1) * size.width,
                                     y: clamped(gym.screen_y, 0, 1) * size.height)
                if gymOpen {
                    gymPanel(anchor: anchor, size: size, safe: safe)
                } else {
                    Button { bridge.send(["action": "interact_gym"]) } label: {
                        Label(gym.near ? "Open Gym board" : "Gym board", systemImage: "chart.xyaxis.line")
                            .padding(.horizontal, 12).padding(.vertical, 9)
                            .background(.ultraThinMaterial, in: Capsule())
                    }
                    .disabled(!gym.near || !active)
                    .accessibilityIdentifier("gym-interact")
                    .position(x: clamped(anchor.x, safe.leading + 106, size.width - safe.trailing - 106),
                              y: clamped(anchor.y - 28, safe.top + 70, size.height - safe.bottom - 160))
                }
            }
        }
        .frame(width: size.width, height: size.height)
        .background(Color(red: 0.025, green: 0.02, blue: 0))
    }

    private func anchoredPanel(anchor: CGPoint, size: CGSize, safe: EdgeInsets) -> some View {
        let bounds = panelBounds(size: size, safe: safe)
        let width = min(bounds.width, 540)
        let minimum = min(340, bounds.height)
        let maximum = max(minimum, min(560, bounds.height * 0.75))
        let reading = reader.packet?.reading == true && !pairing
        let height = reading ? bounds.height : clamped(bounds.maxY - anchor.y - 30, minimum, maximum)
        let left = clamped(anchor.x - width / 2, bounds.minX, bounds.maxX - width)
        let below = anchor.y + 30
        let top = reading ? bounds.minY : clamped(below, bounds.minY, bounds.maxY - height)
        return ZStack(alignment: .topLeading) {
            Path { path in
                path.move(to: anchor)
                path.addLine(to: CGPoint(x: clamped(anchor.x, left + 20, left + width - 20), y: top))
            }.stroke(.tint.opacity(0.8), lineWidth: 2).allowsHitTesting(false)
            Circle().fill(.tint).frame(width: 7, height: 7).position(anchor).allowsHitTesting(false)
            ComputerPanel(reader: reader, active: active, close: {
                pairing = false
                bridge.send(["action": "close_computer"])
            }, worldAction: { bridge.send($0) }, pairing: $pairing)
            .frame(width: width, height: height)
            .position(x: left + width / 2, y: top + height / 2)
        }
    }

    private func gymPanel(anchor: CGPoint, size: CGSize, safe: EdgeInsets) -> some View {
        let bounds = panelBounds(size: size, safe: safe)
        let width = min(bounds.width, 540)
        let height = bounds.height
        let left = clamped(anchor.x - width / 2, bounds.minX, bounds.maxX - width)
        let top = bounds.minY
        return ZStack(alignment: .topLeading) {
            Path { path in
                path.move(to: anchor)
                path.addLine(to: CGPoint(x: clamped(anchor.x, left + 20, left + width - 20), y: top))
            }.stroke(.tint.opacity(0.8), lineWidth: 2).allowsHitTesting(false)
            GymPanel(bridge: bridge) { bridge.send(["action": "close_gym"]) }
                .frame(width: width, height: height)
                .position(x: left + width / 2, y: top + height / 2)
        }
    }

    private func panelBounds(size: CGSize, safe: EdgeInsets) -> CGRect {
        let top = safe.top + 64
        return CGRect(x: safe.leading + 12, y: top,
                      width: max(1, size.width - safe.leading - safe.trailing - 24),
                      height: max(80, size.height - top - safe.bottom - 16))
    }

    private func mount(resource: String, label: String) -> AnyView {
        guard resource == "verse.world" else {
            return AnyView(Text("This world surface is unavailable."))
        }
        return AnyView(VerseSurface(bridge: bridge, active: active, label: label)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .allowsHitTesting(!panelOpen))
    }

    private func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
        min(max(value, lower), max(lower, upper))
    }
}
