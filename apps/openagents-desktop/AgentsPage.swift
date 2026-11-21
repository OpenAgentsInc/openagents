// Firm reason: 6 (AppKit proto-first Agents page, Simulated App Tester live runner &
// Game Builder 120 Hz Metal 3D side-pane launcher over coax/lib/agent/desktop/agents-catalog.coax
// and apps/shared/tester/cli.coax; GitHub #13894).
import AppKit
import Foundation

struct DesktopAgentDefinition: Decodable {
    let schema: String
    let agent_id: String
    let name: String
    let description: String
    let version: Int
    let kind: String
    let execution_mode: String
    let runtime: String
    let input_contract: String
    let output_contract: String
    let max_depth: Int
    let max_concurrency: Int
}

struct DesktopAgentInvocation {
    let id: String
    let agentID: String
    let target: String
    let startedAt: Date
    var state: String
    var artifactPath: String
}

final class AgentsPageController: NSObject {
    static let shared = AgentsPageController()

    private var panel: NSWindow?
    private var logView: NSTextView?
    private var statusLabel: NSTextField?
    private var targetPopup: NSPopUpButton?
    private var stopButton: NSButton?
    private weak var hostWindow: CoderWindow?

    private var definitions: [DesktopAgentDefinition] = []
    private var invocations: [DesktopAgentInvocation] = []
    private var activeTesterProcess: Process?
    private var activeLeaseTarget: String?

    static func bundledTool(_ name: String) -> String? {
        if let p = Bundle.main.resourceURL?.appendingPathComponent(name).path,
           FileManager.default.isExecutableFile(atPath: p) {
            return p
        }
        return nil
    }

    /// Loads canonical openagents.agent.spec.v1 AgentDefinition records from the bundled Coax binary.
    func loadDefinitions() -> [DesktopAgentDefinition] {
        guard let exe = Self.bundledTool("agents-catalog") else { return [] }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = ["list"]
        let outP = Pipe()
        p.standardOutput = outP
        do {
            try p.run()
            p.waitUntilExit()
        } catch {
            return []
        }
        guard p.terminationStatus == 0 else { return [] }
        let data = outP.fileHandleForReading.readDataToEndOfFile()
        return (try? JSONDecoder().decode([DesktopAgentDefinition].self, from: data)) ?? []
    }

    func checkLeaseConflict(targetA: String, targetB: String) -> Bool {
        guard let exe = Self.bundledTool("agents-catalog") else { return targetA == targetB }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = ["check-lease", "--a-target", targetA, "--a-device", targetA, "--b-target", targetB, "--b-device", targetB]
        do {
            try p.run()
            p.waitUntilExit()
        } catch {
            return targetA == targetB
        }
        return p.terminationStatus != 0
    }

    func present(window: CoderWindow?) {
        hostWindow = window
        definitions = loadDefinitions()

        if let existing = panel {
            existing.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 680, height: 520),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )
        win.title = "OpenAgents — Composable Agents"
        win.isReleasedWhenClosed = false
        win.center()

        let root = NSView(frame: win.contentView?.bounds ?? NSRect(x: 0, y: 0, width: 680, height: 520))
        root.autoresizingMask = [.width, .height]

        let header = NSTextField(labelWithString: "Built-in Agents (\(definitions.count) proto-first AgentDefinition records)")
        header.frame = NSRect(x: 16, y: 484, width: 648, height: 22)
        header.font = NSFont.systemFont(ofSize: 14, weight: .semibold)
        header.autoresizingMask = [.width, .minYMargin]
        root.addSubview(header)

        // Render cards directly from loaded Coax AgentDefinition records
        var y: CGFloat = 398
        for def in definitions {
            let box = NSBox(frame: NSRect(x: 16, y: y, width: 648, height: 78))
            box.boxType = .custom
            box.cornerRadius = 8
            box.borderWidth = 1
            box.borderColor = NSColor.separatorColor
            box.fillColor = NSColor.controlBackgroundColor
            box.autoresizingMask = [.width, .minYMargin]

            let title = NSTextField(labelWithString: "\(def.name)  [\(def.kind) · \(def.execution_mode) · \(def.runtime)]")
            title.frame = NSRect(x: 12, y: 50, width: 470, height: 18)
            title.font = NSFont.systemFont(ofSize: 12, weight: .semibold)
            box.addSubview(title)

            let desc = NSTextField(wrappingLabelWithString: def.description)
            desc.frame = NSRect(x: 12, y: 8, width: 470, height: 38)
            desc.font = NSFont.systemFont(ofSize: 11)
            desc.textColor = NSColor.secondaryLabelColor
            box.addSubview(desc)

            let btn = NSButton(frame: NSRect(x: 496, y: 24, width: 138, height: 28))
            btn.bezelStyle = .rounded
            btn.target = self
            if def.agent_id == "coder" {
                btn.title = "Open Coder"
                btn.action = #selector(launchCoder(_:))
            } else if def.agent_id == "simulated-app-tester" {
                btn.title = "Run Tester"
                btn.action = #selector(launchTester(_:))
            } else if def.agent_id == "game-builder" {
                btn.title = "Open 3D Viewport"
                btn.action = #selector(launchGameBuilder(_:))
            } else {
                btn.title = "Launch"
            }
            box.addSubview(btn)
            root.addSubview(box)
            y -= 86
        }

        // Tester target & Stop controls
        let targetLbl = NSTextField(labelWithString: "Tester Target:")
        targetLbl.frame = NSRect(x: 16, y: 196, width: 92, height: 20)
        targetLbl.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        root.addSubview(targetLbl)

        let popup = NSPopUpButton(frame: NSRect(x: 110, y: 192, width: 150, height: 26), pullsDown: false)
        popup.addItems(withTitles: ["desktop", "ios-sim", "web"])
        root.addSubview(popup)
        targetPopup = popup

        let stop = NSButton(title: "Stop Active Run", target: self, action: #selector(stopTester(_:)))
        stop.bezelStyle = .rounded
        stop.frame = NSRect(x: 272, y: 192, width: 132, height: 26)
        stop.isEnabled = false
        root.addSubview(stop)
        stopButton = stop

        let status = NSTextField(labelWithString: "Idle — no active target leases")
        status.frame = NSRect(x: 416, y: 196, width: 248, height: 20)
        status.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        status.textColor = NSColor.secondaryLabelColor
        root.addSubview(status)
        statusLabel = status

        let scroll = NSScrollView(frame: NSRect(x: 16, y: 16, width: 648, height: 166))
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        scroll.autoresizingMask = [.width, .height]
        let tv = NSTextView(frame: NSRect(x: 0, y: 0, width: 630, height: 166))
        tv.isEditable = false
        tv.isSelectable = true
        tv.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        tv.string = "Select an agent above to invoke it with real proto-first TargetBinding leases.\n"
        scroll.documentView = tv
        root.addSubview(scroll)
        logView = tv

        win.contentView = root
        panel = win
        win.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    private func appendLog(_ text: String) {
        DispatchQueue.main.async {
            self.logView?.string += text
            self.logView?.scrollToEndOfDocument(nil)
        }
    }

    @objc private func launchCoder(_ sender: Any?) {
        hostWindow?.newChat()
        hostWindow?.show()
        appendLog("[coder] Opened new Coder workspace session.\n")
    }

    @objc private func launchGameBuilder(_ sender: Any?) {
        guard let win = hostWindow else { return }
        if let fixture = Bundle.main.resourceURL?.appendingPathComponent("fixtures/ple-data-hall-sample.ple-scene.json"),
           FileManager.default.fileExists(atPath: fixture.path) {
            win.openSceneInPane(fixture, shown: "Game Builder — Live 3D Scene")
            win.show()
            appendLog("[game-builder] Opened live 120 Hz Metal 3D viewport: \(fixture.lastPathComponent)\n")
        } else {
            win.chooseSceneFile()
            appendLog("[game-builder] Prompted for .ple-scene.json to open in live 3D viewport.\n")
        }
    }

    @objc private func launchTester(_ sender: Any?) {
        let target = targetPopup?.titleOfSelectedItem ?? "desktop"
        if let active = activeLeaseTarget, checkLeaseConflict(targetA: active, targetB: target) {
            appendLog("[simulated-app-tester] BLOCKED by #authority lease policy: target '\(target)' overlaps active lease '\(active)'.\n")
            return
        }
        guard let exe = Self.bundledTool("tester-cli") else {
            appendLog("[simulated-app-tester] tester-cli helper not bundled in app resources.\n")
            return
        }

        let invID = "inv-\(Int(Date().timeIntervalSince1970))"
        let outDir = FileManager.default.temporaryDirectory.appendingPathComponent("oa-tester-\(invID)").path
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: exe)
        proc.arguments = ["--target", target, "--out", outDir]
        let outPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = outPipe

        activeTesterProcess = proc
        activeLeaseTarget = target
        stopButton?.isEnabled = true
        statusLabel?.stringValue = "Running tester (\(target))…"
        invocations.append(DesktopAgentInvocation(id: invID, agentID: "simulated-app-tester", target: target, startedAt: Date(), state: "running", artifactPath: outDir))
        appendLog("[simulated-app-tester] Started \(invID) target=\(target) out=\(outDir)\n")

        outPipe.fileHandleForReading.readabilityHandler = { [weak self] fh in
            let d = fh.availableData
            guard !d.isEmpty, let s = String(data: d, encoding: .utf8) else { return }
            self?.appendLog(s)
        }

        proc.terminationHandler = { [weak self] p in
            outPipe.fileHandleForReading.readabilityHandler = nil
            DispatchQueue.main.async {
                self?.activeTesterProcess = nil
                self?.activeLeaseTarget = nil
                self?.stopButton?.isEnabled = false
                self?.statusLabel?.stringValue = "Finished (exit \(p.terminationStatus))"
                self?.appendLog("[simulated-app-tester] \(invID) finished exit=\(p.terminationStatus) report=\(outDir)/report.json\n")
            }
        }

        do {
            try proc.run()
        } catch {
            activeTesterProcess = nil
            activeLeaseTarget = nil
            stopButton?.isEnabled = false
            appendLog("[simulated-app-tester] Failed to launch: \(error.localizedDescription)\n")
        }
    }

    @objc private func stopTester(_ sender: Any?) {
        guard let proc = activeTesterProcess else { return }
        appendLog("[simulated-app-tester] Stopping active invocation (cascading cancel)…\n")
        proc.terminate()
    }
}
