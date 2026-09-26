// The persistent world is the backdrop. Rust projects the computer's position
// and decides whether interaction is available; native code lays out the panel.
import SwiftUI

struct VerseScreen: View {
    @StateObject private var bridge: VerseBridge
    @ObservedObject var reader: MobileBridge
    @Environment(\.scenePhase) private var phase
    @State private var sprint = false
    @State private var pairing = false
    let synthetic: Bool

    init(reader: MobileBridge, synthetic: Bool) {
        self.reader = reader
        self.synthetic = synthetic
        _bridge = StateObject(wrappedValue: VerseBridge(synthetic: synthetic))
    }

    private var active: Bool { phase == .active }
    private var computerOpen: Bool { bridge.packet?.computer_open == true }

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let packet = bridge.packet, let view = packet.view {
                    NativeRenderer(node: view.root, revision: view.revision,
                                   followTarget: nil, followChanged: nil, surface: mount) { _ in }
                        .frame(width: geometry.size.width, height: geometry.size.height)
                }
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Coder").font(.headline)
                        Spacer()
                        Text(bridge.packet?.status ?? "Opening world").font(.caption)
                            .accessibilityIdentifier("verse-status")
                    }
                    if let error = bridge.nativeError ?? bridge.packet?.error {
                        Text(error).font(.callout).textSelection(.enabled).accessibilityIdentifier("verse-error")
                        Button("Retry world renderer") { bridge.retry() }
                    }
                    Spacer()
                    if synthetic, let packet = bridge.packet {
                        HStack {
                            Text("Frames \(packet.frames_presented)").accessibilityIdentifier("verse-frames")
                            Spacer()
                            Text(packet.position.map { String(format: "%.2f", $0) }.joined(separator: ", "))
                                .accessibilityIdentifier("verse-position")
                        }.font(.caption2.monospacedDigit())
                    }
                    if !computerOpen {
                        Text("Walk to the computer to connect your chats.").font(.caption)
                        Text("Drag left to move · drag right to look").font(.caption2)
                        HStack {
                            Button("Jump") { bridge.send(["action": "jump"]) }.accessibilityIdentifier("verse-jump")
                            Toggle("Sprint", isOn: $sprint).fixedSize()
                                .onChange(of: sprint) { _, enabled in
                                    if active { bridge.send(["action": "sprint", "enabled": enabled]) }
                                }
                            Spacer()
                            Button("Zoom in", systemImage: "plus.magnifyingglass") { bridge.send(["action": "zoom", "delta": 1]) }
                                .labelStyle(.iconOnly)
                            Button("Zoom out", systemImage: "minus.magnifyingglass") { bridge.send(["action": "zoom", "delta": -1]) }
                                .labelStyle(.iconOnly)
                        }
                    }
                }.padding(16).allowsHitTesting(!computerOpen)
                if let computer = bridge.packet?.computer, computer.visible || computerOpen {
                    let anchor = CGPoint(x: clamped(computer.screen_x, 0, 1) * geometry.size.width,
                                         y: clamped(computer.screen_y, 0, 1) * geometry.size.height)
                    if computerOpen {
                        anchoredPanel(anchor: anchor, size: geometry.size)
                    } else {
                        Button {
                            bridge.send(["action": "interact_computer"])
                        } label: {
                            Label(computer.near ? "Use computer" : "Computer", systemImage: "desktopcomputer")
                                .padding(.horizontal, 12).padding(.vertical, 9)
                                .background(.ultraThinMaterial, in: Capsule())
                        }
                        .disabled(!computer.near || !active)
                        .accessibilityIdentifier("computer-interact")
                        .position(x: clamped(anchor.x, 94, geometry.size.width - 94),
                                  y: clamped(anchor.y - 28, 70, geometry.size.height - 160))
                    }
                }
            }
            .background(Color(red: 0.025, green: 0.02, blue: 0))
        }
        .onChange(of: active) { _, enabled in if !enabled { sprint = false } }
        .onChange(of: computerOpen) { _, open in if open { sprint = false } }
    }

    private func anchoredPanel(anchor: CGPoint, size: CGSize) -> some View {
        let width = min(size.width - 24, 540)
        let minimum = min(340, max(160, size.height - 100))
        let maximum = max(minimum, min(560, size.height * 0.62))
        let reading = reader.packet?.reading == true && !pairing
        let height = reading ? max(minimum, size.height - 100) : clamped(size.height - anchor.y - 44, minimum, maximum)
        let left = clamped(anchor.x - width / 2, 12, size.width - width - 12)
        let below = anchor.y + 30
        let top = reading ? 72 : clamped(below, 72, size.height - height - 16)
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

    private func mount(resource: String, label: String) -> AnyView {
        guard resource == "verse.world" else {
            return AnyView(Text("This world surface is unavailable."))
        }
        return AnyView(VerseSurface(bridge: bridge, active: active, label: label)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .allowsHitTesting(!computerOpen))
    }

    private func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
        min(max(value, lower), max(lower, upper))
    }
}
