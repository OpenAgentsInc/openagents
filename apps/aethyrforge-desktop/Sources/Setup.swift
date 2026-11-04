// AppKit rendering and process/file effects for setup. Coax owns report parsing,
// model/agent status, selection, readiness and the persisted settings document.
import AppKit

func setupText(_ field: Int64) -> String {
    var bytes: [UInt8] = []
    for i in 0..<8192 {
        let c = fgs_text_byte(field, Int64(i)); if c < 0 { break }; bytes.append(UInt8(c))
    }
    return String(decoding: bytes, as: UTF8.self)
}

@MainActor
final class ForgeSetupPane: NSView {
    let panel = NSVisualEffectView()
    let cliStatus = NSTextField(labelWithString: "Checking OpenAgents CLI…")
    let voiceStatus = NSTextField(labelWithString: "Checking shared model files…")
    let modelPath = NSTextField(labelWithString: "~/.openagents/models")
    let editModel = NSTextField(labelWithString: "Checking edit model…")
    let agentStatus = NSTextField(labelWithString: "Looking for local agents…")
    let agentChoice = NSPopUpButton(frame: .zero, pullsDown: false)
    let verify = NSButton(title: "Verify voice model", target: nil, action: nil)
    let refresh = NSButton(title: "Refresh", target: nil, action: nil)
    let enter = NSButton(title: "Open Everglade", target: nil, action: nil)
    let progress = NSProgressIndicator()
    var enterAction: (() -> Void)?
    var refreshAction: (() -> Void)?
    var verifyAction: (() -> Void)?
    var folderAction: (() -> Void)?
    var sceneReady = false
    var checking = true
    var verifying = false
    var errorMessage: String?

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true; layer?.backgroundColor = NSColor.black.withAlphaComponent(0.25).cgColor
        panel.material = .hudWindow; panel.blendingMode = .withinWindow; panel.state = .active
        panel.appearance = NSAppearance(named: .darkAqua)
        panel.wantsLayer = true; panel.layer?.cornerRadius = 16
        panel.layer?.borderWidth = 1; panel.layer?.borderColor = NSColor(calibratedRed: 0.29, green: 0.76, blue: 0.76, alpha: 0.55).cgColor
        panel.translatesAutoresizingMaskIntoConstraints = false; addSubview(panel)
        NSLayoutConstraint.activate([
            panel.centerXAnchor.constraint(equalTo: centerXAnchor), panel.centerYAnchor.constraint(equalTo: centerYAnchor),
            panel.widthAnchor.constraint(equalToConstant: 630), panel.heightAnchor.constraint(equalToConstant: 610)
        ])
        let stack = NSStackView(); stack.orientation = .vertical; stack.alignment = .leading; stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false; panel.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 32),
            stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -32),
            stack.topAnchor.constraint(equalTo: panel.topAnchor, constant: 28)
        ])
        let mark = label("AETHYRFORGE", size: 11, weight: .medium)
        mark.textColor = .init(calibratedRed: 0.35, green: 0.85, blue: 0.83, alpha: 1)
        mark.font = .monospacedSystemFont(ofSize: 11, weight: .medium)
        stack.addArrangedSubview(mark)
        stack.addArrangedSubview(label(setupText(0), size: 28, weight: .semibold))
        stack.addArrangedSubview(label(setupText(1), size: 14, secondary: true))
        stack.addArrangedSubview(rule())

        let cli = row("OPENAGENTS CLI", value: cliStatus)
        cliStatus.toolTip = "~/.openagents/bin/openagents"
        stack.addArrangedSubview(cli)

        let voice = NSStackView(); voice.orientation = .vertical; voice.alignment = .leading; voice.spacing = 5
        voice.addArrangedSubview(label("LOCAL VOICE", size: 10, weight: .semibold, secondary: true))
        let voiceHead = NSStackView(); voiceHead.orientation = .horizontal; voiceHead.spacing = 18
        voiceHead.addArrangedSubview(label(setupText(3), size: 17, weight: .medium))
        verify.bezelStyle = .rounded; verify.controlSize = .small; verify.target = self; verify.action = #selector(verifyPressed)
        voiceHead.addArrangedSubview(verify); voice.addArrangedSubview(voiceHead)
        style(voiceStatus, size: 13, secondary: true); voice.addArrangedSubview(voiceStatus)
        style(modelPath, size: 11, secondary: true); modelPath.lineBreakMode = .byTruncatingMiddle; modelPath.maximumNumberOfLines = 1
        voice.addArrangedSubview(modelPath); modelPath.widthAnchor.constraint(equalToConstant: 560).isActive = true
        style(editModel, size: 12, secondary: true); voice.addArrangedSubview(editModel)
        stack.addArrangedSubview(voice)

        let agents = NSStackView(); agents.orientation = .vertical; agents.alignment = .leading; agents.spacing = 7
        agents.addArrangedSubview(label("LOCAL AGENTS", size: 10, weight: .semibold, secondary: true))
        style(agentStatus, size: 13, secondary: true); agents.addArrangedSubview(agentStatus)
        agentChoice.target = self; agentChoice.action = #selector(agentChanged)
        agentChoice.widthAnchor.constraint(equalToConstant: 560).isActive = true
        agents.addArrangedSubview(agentChoice)
        stack.addArrangedSubview(agents)
        stack.addArrangedSubview(rule())

        let footer = label("Model files are shared with your other OpenAgents apps.", size: 12, secondary: true)
        stack.addArrangedSubview(footer)
        let buttons = NSStackView(); buttons.orientation = .horizontal; buttons.spacing = 12
        let folder = NSButton(title: "Open folder", target: self, action: #selector(folderPressed)); folder.bezelStyle = .rounded
        buttons.addArrangedSubview(folder)
        refresh.bezelStyle = .rounded; refresh.target = self; refresh.action = #selector(refreshPressed); buttons.addArrangedSubview(refresh)
        progress.style = .spinning; progress.controlSize = .small; progress.isDisplayedWhenStopped = false
        buttons.addArrangedSubview(progress)
        let spacer = NSView(); spacer.setContentHuggingPriority(.defaultLow, for: .horizontal); buttons.addArrangedSubview(spacer)
        enter.bezelStyle = .rounded; enter.bezelColor = .init(calibratedRed: 0.14, green: 0.55, blue: 0.56, alpha: 1)
        enter.target = self; enter.action = #selector(enterPressed); buttons.addArrangedSubview(enter)
        stack.addArrangedSubview(buttons); buttons.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        stack.addArrangedSubview(label("Setup is always available in the View menu.", size: 11, secondary: true))
        update()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }
    private func style(_ field: NSTextField, size: CGFloat, secondary: Bool = false) {
        field.font = .systemFont(ofSize: size); field.textColor = secondary ? .secondaryLabelColor : .labelColor
    }
    private func label(_ value: String, size: CGFloat, weight: NSFont.Weight = .regular, secondary: Bool = false) -> NSTextField {
        let field = NSTextField(labelWithString: value); style(field, size: size, secondary: secondary)
        field.font = .systemFont(ofSize: size, weight: weight); return field
    }
    private func rule() -> NSBox {
        let line = NSBox(); line.boxType = .separator; line.widthAnchor.constraint(equalToConstant: 566).isActive = true; return line
    }
    private func row(_ heading: String, value: NSTextField) -> NSStackView {
        let row = NSStackView(); row.orientation = .vertical; row.alignment = .leading; row.spacing = 5
        row.addArrangedSubview(label(heading, size: 10, weight: .semibold, secondary: true))
        style(value, size: 14); row.addArrangedSubview(value); return row
    }
    func update() {
        cliStatus.stringValue = errorMessage ?? setupText(2)
        voiceStatus.stringValue = verifying ? "Verifying the pinned weight and tokenizer files…" : setupText(4)
        modelPath.stringValue = setupText(6).isEmpty ? "~/.openagents/models" : setupText(6)
        modelPath.toolTip = modelPath.stringValue
        editModel.stringValue = setupText(5)
        agentStatus.stringValue = setupText(7)
        agentChoice.removeAllItems(); agentChoice.addItem(withTitle: "No preferred agent"); agentChoice.lastItem?.tag = -1
        for i in Int64(0)..<4 where fgs_count(i) > 0 {
            agentChoice.addItem(withTitle: "\(setupText(10 + i)) · \(fgs_count(i)) running")
            agentChoice.lastItem?.tag = Int(i)
        }
        agentChoice.selectItem(withTag: Int(fgs_selected()))
        agentChoice.isEnabled = !checking && fgs_count(-1) > 0
        refresh.isEnabled = !checking && !verifying
        verify.isEnabled = !checking && !verifying && fgs_status(1) > 0
        enter.isEnabled = errorMessage == nil && !checking && !verifying && fgs_can_enter(sceneReady)
        if checking || verifying || !sceneReady { progress.startAnimation(nil) } else { progress.stopAnimation(nil) }
        setAccessibilityLabel("Aethyrforge setup")
    }
    @objc private func enterPressed() { if enter.isEnabled { enterAction?() } }
    @objc private func refreshPressed() { refreshAction?() }
    @objc private func verifyPressed() { verifyAction?() }
    @objc private func folderPressed() { folderAction?() }
    @objc private func agentChanged() { fgs_choose(Int64(agentChoice.selectedTag())) }
}

struct ForgeCLI {
    static func run(_ executable: URL, _ arguments: [String], timeout: TimeInterval = 15, environment: [String: String]? = nil) async throws -> Data {
        try await Task.detached(priority: .userInitiated) {
            let process = Process(); process.executableURL = executable; process.arguments = arguments
            var env = environment ?? ProcessInfo.processInfo.environment
            let shared = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin").path
            env["PATH"] = shared + ":" + (env["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin")
            process.environment = env
            let pipe = Pipe(); process.standardOutput = pipe; process.standardError = FileHandle.nullDevice
            try process.run()
            let limit = DispatchWorkItem { if process.isRunning { process.terminate() } }
            let killLimit = DispatchWorkItem { if process.isRunning { kill(process.processIdentifier, SIGKILL) } }
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: limit)
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout + 2, execute: killLimit)
            defer { limit.cancel(); killLimit.cancel() }
            var data = Data()
            while let part = try pipe.fileHandleForReading.read(upToCount: 8192), !part.isEmpty {
                data.append(part)
                if data.count > 65536 { process.terminate(); throw ShellError.failed("The local CLI report exceeded its size limit.") }
            }
            process.waitUntilExit()
            guard !data.isEmpty else { throw ShellError.failed("The local CLI did not return a status report.") }
            return data
        }.value
    }
}
