// Rust draws and hit-tests the computer in the world, and draws its
// Computers and terminal screens in the world HUD. Native code shows only the
// Chats page, the keyboard, and the camera scanner those screens ask for.
import SwiftUI

struct VerseScreen: View {
    @StateObject private var bridge: VerseBridge
    @ObservedObject var reader: MobileBridge
    @Environment(\.scenePhase) private var phase
    @State private var pairing = false
    /// The input request the HUD asked the keyboard or scanner for.
    @State private var inputToken: String?
    @State private var scanning = false
    /// The terminal keyboard has focus.
    @State private var typing = false
    let synthetic: Bool

    init(reader: MobileBridge, synthetic: Bool) {
        self.reader = reader
        self.synthetic = synthetic
        _bridge = StateObject(wrappedValue: VerseBridge(synthetic: synthetic))
    }

    private var active: Bool { phase == .active }
    private var computerOpen: Bool { bridge.packet?.computer_open == true }
    private var chatsOpen: Bool { computerOpen && bridge.packet?.computer_page == "chats" }
    private var hudOpen: Bool { computerOpen && bridge.packet?.computer_hud.visible == true }
    private var terminalOpen: Bool { hudOpen && bridge.packet?.computer_page == "terminal" }
    private var gymOpen: Bool { bridge.packet?.gym_open == true }
    /// Everglade's Agent Studio panel, a Rust Native view, covers the world.
    private var studioOpen: Bool { bridge.packet?.studio_open == true }
    /// A native control covers the world: the world surface takes no touches.
    private var nativeOpen: Bool { chatsOpen || gymOpen || studioOpen || currentInput != nil }
    private var motionLook: Bool { bridge.packet?.camera_mode == "motion" }
    /// The input request the keyboard or scanner is answering, while it is
    /// still the Computers surface's current one.
    private var currentInput: ComputersInput? {
        guard hudOpen, let token = inputToken, let input = reader.packet?.computers_input,
              input.token == token else { return nil }
        return input
    }

    var body: some View {
        GeometryReader { safeGeometry in
            // Native panels avoid the keyboard and physical display edges.
            // The Metal view reads its own window insets for the world HUD.
            // World projection coordinates still use the full canvas size.
            let safe = safeGeometry.safeAreaInsets
            GeometryReader { geometry in
                canvas(size: geometry.size, safe: safe)
            }
            .ignoresSafeArea()
        }
        .overlay(alignment: .bottom) {
            if let input = currentInput {
                ComputerInputBar(input: input, scanning: scanning, busy: reader.busy,
                                 submit: { value in
                                     reader.submitComputers(token: input.token, value: value)
                                     inputToken = nil; scanning = false
                                 },
                                 cancel: {
                                     reader.cancelComputers(token: input.token)
                                     inputToken = nil; scanning = false
                                 },
                                 stopScanning: { scanning = false })
                    .id("\(input.token)-\(scanning)")
                    .padding(.horizontal, 12).padding(.bottom, 8)
            }
        }
        .overlay(alignment: .bottomLeading) {
            if terminalOpen {
                // The keyboard target for the terminal: Rust encodes every key.
                TerminalKeyInput(focused: $typing,
                                 text: { reader.terminal(["op": "terminal_text", "text": $0]) },
                                 key: { name, ctrl, alt, shift in
                                     reader.terminal(["op": "terminal_key", "key": name,
                                                      "ctrl": ctrl, "alt": alt, "shift": shift])
                                 },
                                 paste: { reader.terminal(["op": "terminal_paste", "text": $0]) })
                    .frame(width: 1, height: 1).opacity(0.02)
            }
        }
        .onChange(of: terminalOpen) { _, open in
            if !open { typing = false }
            TerminalOrientation.allow(open)
        }
        .onChange(of: reader.terminalPaste) { _, asked in
            if asked { reader.terminal(["op": "terminal_paste", "text": UIPasteboard.general.string ?? ""]) }
        }
        // The terminal streams: poll it quickly while its page shows.
        .task(id: terminalOpen && active) {
            guard terminalOpen && active else { return }
            while !Task.isCancelled {
                reader.pollTerminal()
                try? await Task.sleep(for: .milliseconds(120))
            }
        }
        .onChange(of: active) { _, enabled in reader.setLifecycle(enabled) }
        .onAppear { bridge.computerCommands = { commands in commands.forEach(run) } }
        .onChange(of: reader.hudFeedRevision) { _, _ in forwardFeed() }
        .onChange(of: reader.busy) { _, _ in forwardFeed() }
        .onChange(of: computerOpen) { _, open in
            if open { forwardFeed() } else { inputToken = nil; scanning = false; pairing = false }
        }
        .onChange(of: reader.packet?.computers_input?.token) { _, token in
            if token != inputToken { inputToken = nil; scanning = false }
        }
        // Host status moves on its own; poll while the HUD shows Computers.
        .task(id: hudOpen && active && currentInput == nil) {
            guard hudOpen && active && currentInput == nil else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled { reader.pollComputers() }
            }
        }
    }

    /// Hand the reader's latest Computers and terminal views to the HUD.
    private func forwardFeed() {
        guard computerOpen else { return }
        var feed = reader.hudFeed
        feed["busy"] = reader.busy
        if let error = reader.nativeError { feed["error"] = error }
        bridge.send(["action": "computer_feed", "feed": feed])
    }

    private func run(_ command: VerseComputerCommand) {
        switch command.kind {
        case "activate":
            guard let surface = command.surface, let instance = command.instance,
                  let revision = command.revision, let node = command.node else { return }
            reader.activateSurface(surface, instance: instance, revision: revision, node: node)
        case "scan":
            inputToken = command.token; scanning = true
        case "type":
            inputToken = command.token; scanning = false
        case "cancel_input":
            if let token = command.token { reader.cancelComputers(token: token) }
            inputToken = nil; scanning = false
        case "refresh":
            reader.refreshComputers()
        case "terminal_resize":
            guard let rows = command.rows, let cols = command.cols else { return }
            reader.terminal(["op": "terminal_resize", "rows": rows, "cols": cols])
        case "copy":
            if let text = command.text { UIPasteboard.general.string = text }
        case "terminal_keyboard":
            typing = true
        default: break
        }
    }

    private func canvas(size: CGSize, safe: EdgeInsets) -> some View {
        ZStack(alignment: .topLeading) {
            if let packet = bridge.packet, let view = packet.view {
                NativeRenderer(node: view.root, revision: view.revision,
                               followTarget: nil, followChanged: nil, surface: { resource, label in
                                   mount(resource: resource, label: label)
                               }) { _ in }
                    .frame(width: size.width, height: size.height)
            }
            if hudOpen, let hud = bridge.packet?.computer_hud {
                ComputerHudAccessibility(items: hud.items, activate: { key in
                    bridge.send(["action": "computer_hud_tap", "key": key])
                }, scroll: { delta in
                    bridge.send(["action": "computer_hud_scroll", "delta": delta])
                })
                .frame(width: size.width, height: size.height)
                .allowsHitTesting(false)
            }
            if bridge.synthetic {
                // XCTest's full-screen pinch starts at the camera controls.
                // This observation-only region targets exposed world pixels;
                // all touch events still reach the real Metal surface.
                Color.clear
                    .frame(width: size.width * 0.8, height: size.height * 0.36)
                    .position(x: size.width * 0.5, y: size.height * 0.54)
                    .accessibilityElement()
                    .accessibilityLabel("World gesture area")
                    .accessibilityIdentifier("verse-pinch-region")
                    .allowsHitTesting(false)
            }
            VStack(alignment: .leading, spacing: 8) {
                if let error = bridge.nativeError ?? bridge.packet?.error {
                    Text(error).font(.paper(.callout)).textSelection(.enabled).accessibilityIdentifier("verse-error")
                    Button("Retry world renderer") { bridge.retry() }
                }
                if let error = bridge.doorStorageError ?? bridge.packet?.doors.error {
                    Text(error).font(.paper(.callout)).textSelection(.enabled).accessibilityIdentifier("door-storage-error")
                    if bridge.canRetryDoorSave {
                        Button("Retry saving choices") { bridge.retryDoorPreferences() }
                            .accessibilityIdentifier("door-save-retry")
                    }
                }
                Spacer()
                if !computerOpen && !gymOpen && !studioOpen {
                    HStack(spacing: 16) {
                        Spacer()
                        Button {
                            bridge.toggleCameraMode()
                        } label: {
                            Image(systemName: motionLook ? "gyroscope" : "hand.draw")
                                .frame(width: 44, height: 44)
                        }
                        .disabled(!active || (!bridge.motionAvailable && !motionLook))
                        .accessibilityLabel(motionLook ? "Motion look" : "Touch look")
                        .accessibilityIdentifier("verse-camera-mode")
                        .accessibilityHint(bridge.motionAvailable
                            ? "Switches between dragging and phone orientation for camera control."
                            : "Motion look is unavailable on this device. Drag the world to look around.")
                        .highPriorityGesture(
                            LongPressGesture(minimumDuration: 0.6).onEnded { _ in bridge.previewMotion() },
                            including: bridge.motionSynthetic ? .all : .none)
                        Button { bridge.recenterMotion() } label: {
                            Image(systemName: "scope").frame(width: 44, height: 44)
                        }
                        .accessibilityLabel("Recenter camera")
                        .disabled(!active).accessibilityIdentifier("verse-motion-recenter")
                    }
                    if let error = bridge.motionError {
                        Text(error).font(.paper(.caption)).accessibilityIdentifier("verse-motion-error")
                    }
                }
            }
            .padding(.top, safe.top + 12)
            .padding(.bottom, safe.bottom + 12)
            .padding(.leading, safe.leading + 16)
            .padding(.trailing, safe.trailing + 16)
            .allowsHitTesting(!computerOpen && !gymOpen && !studioOpen)
            if let computer = bridge.packet?.computer, !gymOpen, chatsOpen {
                let anchor = CGPoint(x: clamped(computer.screen_x, 0, 1) * size.width,
                                     y: clamped(computer.screen_y, 0, 1) * size.height)
                anchoredPanel(anchor: anchor, size: size, safe: safe)
            }
            if let gym = bridge.packet?.gym, !computerOpen, gymOpen {
                let anchor = CGPoint(x: clamped(gym.screen_x, 0, 1) * size.width,
                                     y: clamped(gym.screen_y, 0, 1) * size.height)
                gymPanel(anchor: anchor, size: size, safe: safe)
            }
            if studioOpen, !computerOpen, !gymOpen {
                studioPanel(size: size, safe: safe)
            }
        }
        .frame(width: size.width, height: size.height)
        .background(Color(red: 0.025, green: 0.02, blue: 0))
    }

    /// The Chats page: the read-only reader and its pairing, as before.
    private func anchoredPanel(anchor: CGPoint, size: CGSize, safe: EdgeInsets) -> some View {
        let bounds = panelBounds(size: size, safe: safe)
        let width = min(bounds.width, 540)
        let minimum = min(340, bounds.height)
        let maximum = max(minimum, min(560, bounds.height * 0.75))
        let reading = reader.packet?.paired == true && !pairing
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
            }, computers: {
                pairing = false
                bridge.send(["action": "computer_page", "page": "computers"])
            }, worldAction: { bridge.send($0) }, worldConnection: bridge.packet?.connection,
           worldStorageError: bridge.worldStorageError ?? bridge.nativeError ?? bridge.packet?.error,
           worldCredits: bridge.verseCredits, pairing: $pairing)
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

    /// The Agent Studio panel: Rust's view mounted as is. Its close control
    /// is part of the view, so this adds only the frame.
    private func studioPanel(size: CGSize, safe: EdgeInsets) -> some View {
        let bounds = panelBounds(size: size, safe: safe)
        let width = min(bounds.width, 540)
        return Group {
            if let view = bridge.studioView {
                NativeRenderer(node: view.root, revision: view.revision,
                               followTarget: nil, followChanged: nil) { key in
                    bridge.activateStudio(key)
                }
            } else {
                ProgressView("Loading studio…").accessibilityIdentifier("studio-loading")
            }
        }
        .frame(width: width, height: bounds.height, alignment: .top)
        .background(Color(red: 0.025, green: 0.02, blue: 0).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.tint.opacity(0.7), lineWidth: 1))
        .accessibilityIdentifier("studio-panel")
        .position(x: bounds.midX, y: bounds.midY)
    }

    private func panelBounds(size: CGSize, safe: EdgeInsets) -> CGRect {
        let top = safe.top + 12
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
            .allowsHitTesting(!nativeOpen))
    }

    private func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
        min(max(value, lower), max(lower, upper))
    }
}
