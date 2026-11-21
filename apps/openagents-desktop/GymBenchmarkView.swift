// Firm reason: 6 (AppKit host for live Psionic Metal vs llama.cpp coding benchmark workbench using embedded Coax GPU chat views).
import AppKit
import Foundation

// MARK: - Data Models

struct GymBenchTaskSpec {
    let tier: String
    let id: String
    let langBadge: String
}

struct GymBenchTaskResult: Codable {
    var task: String
    var tier: String
    var passed: Bool
    var wall_s: Double
    var turns: Int
    var toks: Int
    var decode_tok_s: Double
    var ttfts: [Double]
    var checks: [[String]] // [name, "pass"|"fail"]
    var prompt: String?
    var chat_events: [String]?
}

struct GymBenchBackendReport: Codable {
    var label: String
    var boot_s: Double
    var peak_rss_mib: Double
    var passed: Int
    var total: Int
    var total_wall_s: Double
    var median_wall_s: Double
    var total_toks: Int
    var rows: [GymBenchTaskResult]
}

struct GymBenchSavedRun: Codable {
    var timestamp: String
    var suite: String
    var psionic: GymBenchBackendReport?
    var llamacpp: GymBenchBackendReport?
}

private enum GymBenchSide: String {
    case psionic = "Psionic Metal"
    case llamacpp = "llama.cpp Metal"
}

private enum GymBenchRowState {
    case pending
    case running(turn: Int, elapsed: Double, tokRate: Double, activeTool: String)
    case done(GymBenchTaskResult)
}

private struct GymBenchStreamTurn {
    var ttft: Double
    var dur: Double
    var promptTokens: Int
    var completionTokens: Int
    var content: String
    var reasoning: String
    var toolCalls: [[String: Any]]
}

private final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

// MARK: - Embedded Real OpenAgents GPU Chat Component

private final class GymChatSurfaceView: NSView {
    private let headerBar = NSView()
    private let titleLabel = NSTextField(labelWithString: "")
    private let badgeLabel = NSTextField(labelWithString: "Ready")
    private let headerDivider = NSView()
    private let hostContainer = NSView()

    private var page: UnsafeMutableRawPointer?
    private var gpuView: NSView?
    private var frameTimer: Timer?
    private var owedFrames = 6

    init(title: String, accent: NSColor) {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 8
        layer?.backgroundColor = NSColor(red: 0.045, green: 0.050, blue: 0.062, alpha: 1).cgColor
        layer?.borderWidth = 1
        layer?.borderColor = accent.withAlphaComponent(0.32).cgColor
        layer?.masksToBounds = true

        headerBar.translatesAutoresizingMaskIntoConstraints = false
        headerBar.wantsLayer = true
        headerBar.layer?.backgroundColor = NSColor(red: 0.072, green: 0.078, blue: 0.095, alpha: 1).cgColor
        addSubview(headerBar)

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.stringValue = title
        titleLabel.font = NSFont.systemFont(ofSize: 11.5, weight: .semibold)
        titleLabel.textColor = accent
        headerBar.addSubview(titleLabel)

        badgeLabel.translatesAutoresizingMaskIntoConstraints = false
        badgeLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        badgeLabel.textColor = NSColor(white: 0.68, alpha: 1)
        badgeLabel.alignment = .right
        badgeLabel.lineBreakMode = .byTruncatingTail
        headerBar.addSubview(badgeLabel)

        headerDivider.translatesAutoresizingMaskIntoConstraints = false
        headerDivider.wantsLayer = true
        headerDivider.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.08).cgColor
        addSubview(headerDivider)

        hostContainer.translatesAutoresizingMaskIntoConstraints = false
        addSubview(hostContainer)

        NSLayoutConstraint.activate([
            headerBar.topAnchor.constraint(equalTo: topAnchor),
            headerBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerBar.heightAnchor.constraint(equalToConstant: 28),

            titleLabel.centerYAnchor.constraint(equalTo: headerBar.centerYAnchor),
            titleLabel.leadingAnchor.constraint(equalTo: headerBar.leadingAnchor, constant: 10),

            badgeLabel.centerYAnchor.constraint(equalTo: headerBar.centerYAnchor),
            badgeLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),
            badgeLabel.trailingAnchor.constraint(equalTo: headerBar.trailingAnchor, constant: -10),

            headerDivider.topAnchor.constraint(equalTo: headerBar.bottomAnchor),
            headerDivider.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerDivider.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerDivider.heightAnchor.constraint(equalToConstant: 1),

            hostContainer.topAnchor.constraint(equalTo: headerDivider.bottomAnchor),
            hostContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostContainer.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])

        openCoaxChat()
    }

    required init?(coder: NSCoder) { fatalError() }

    deinit {
        frameTimer?.invalidate()
        if let p = page {
            cd_close(p)
            page = nil
        }
    }

    private func openCoaxChat() {
        guard let res = Bundle.main.resourceURL?.appendingPathComponent("Components").path else { return }
        guard let p = cd_open(res) else { return }
        page = p
        cd_toggle_sidebar(p)
        _ = cd_pick_model(p, "inkling")
        cd_env(p, 0, 0)
        if let rawView = cd_view(p) {
            let v = Unmanaged<NSView>.fromOpaque(rawView).takeUnretainedValue()
            v.translatesAutoresizingMaskIntoConstraints = false
            hostContainer.addSubview(v)
            NSLayoutConstraint.activate([
                v.topAnchor.constraint(equalTo: hostContainer.topAnchor),
                v.leadingAnchor.constraint(equalTo: hostContainer.leadingAnchor),
                v.trailingAnchor.constraint(equalTo: hostContainer.trailingAnchor),
                v.bottomAnchor.constraint(equalTo: hostContainer.bottomAnchor),
            ])
            gpuView = v
        }
        owedFrames = 6
        startFrameTimer()
    }

    private func startFrameTimer() {
        frameTimer?.invalidate()
        let t = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            self?.tickFrame()
        }
        RunLoop.main.add(t, forMode: .common)
        frameTimer = t
    }

    private func tickFrame() {
        guard let p = page, window != nil, !isHiddenOrHasHiddenAncestor else { return }
        if owedFrames > 0 || cd_moving(p) != 0 {
            cd_frame(p)
            if owedFrames > 0 { owedFrames -= 1 }
        }
    }

    func wake() {
        owedFrames = max(owedFrames, 4)
    }

    func setBadge(_ text: String) {
        badgeLabel.stringValue = text
    }

    func startTask(prompt: String, taskName: String) {
        guard let p = page else { return }
        cd_line(p, "{\"kind\":\"ended\",\"ending\":\"stopped\",\"text\":\"\"}")
        _ = cd_new(p)
        cd_folder(p, taskName)
        _ = cd_set_text(p, prompt)
        _ = cd_submit(p)
        while cd_action(p) != 0 {}
        wake()
    }

    func feedLine(_ jsonLine: String) {
        guard let p = page else { return }
        cd_line(p, jsonLine)
        wake()
    }

    func replay(prompt: String, taskName: String, events: [String], badge: String) {
        setBadge(badge)
        startTask(prompt: prompt, taskName: taskName)
        for ev in events {
            feedLine(ev)
        }
        wake()
    }

    static func encodeJSON(_ obj: [String: Any]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: obj),
              let s = String(data: data, encoding: .utf8) else { return "{}" }
        return s
    }
}

// MARK: - Per-Task Race Row View

private final class GymBenchTaskRowView: NSView {
    let spec: GymBenchTaskSpec
    var onSelect: (() -> Void)?

    private let taskLabel = NSTextField(labelWithString: "")
    private let langBadge = NSTextField(labelWithString: "")

    private let psiBarTrack = NSView()
    private let psiBarFill = NSView()
    private var psiBarWidth: NSLayoutConstraint!
    private let psiValueLabel = NSTextField(labelWithString: "—")

    private let llamaBarTrack = NSView()
    private let llamaBarFill = NSView()
    private var llamaBarWidth: NSLayoutConstraint!
    private let llamaValueLabel = NSTextField(labelWithString: "—")

    private let deltaLabel = NSTextField(labelWithString: "")

    private var psiState: GymBenchRowState = .pending
    private var llamaState: GymBenchRowState = .pending
    private var selected = false

    init(spec: GymBenchTaskSpec) {
        self.spec = spec
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 5
        layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.025).cgColor
        setup()
    }

    required init?(coder: NSCoder) { fatalError() }

    private func setup() {
        taskLabel.translatesAutoresizingMaskIntoConstraints = false
        taskLabel.stringValue = spec.id
        taskLabel.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .medium)
        taskLabel.textColor = NSColor(white: 0.92, alpha: 1)
        taskLabel.lineBreakMode = .byTruncatingTail
        addSubview(taskLabel)

        langBadge.translatesAutoresizingMaskIntoConstraints = false
        langBadge.stringValue = spec.langBadge
        langBadge.alignment = .center
        langBadge.font = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold)
        langBadge.textColor = NSColor(calibratedRed: 0.62, green: 0.80, blue: 0.98, alpha: 1)
        langBadge.wantsLayer = true
        langBadge.layer?.cornerRadius = 3
        langBadge.layer?.backgroundColor = NSColor(calibratedRed: 0.16, green: 0.30, blue: 0.48, alpha: 0.38).cgColor
        addSubview(langBadge)

        for (track, fill, color) in [
            (psiBarTrack, psiBarFill, NSColor(calibratedRed: 0.18, green: 0.82, blue: 0.56, alpha: 0.92)),
            (llamaBarTrack, llamaBarFill, NSColor(calibratedRed: 0.92, green: 0.62, blue: 0.24, alpha: 0.86)),
        ] {
            track.translatesAutoresizingMaskIntoConstraints = false
            track.wantsLayer = true
            track.layer?.cornerRadius = 4
            track.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.06).cgColor
            addSubview(track)

            fill.translatesAutoresizingMaskIntoConstraints = false
            fill.wantsLayer = true
            fill.layer?.cornerRadius = 4
            fill.layer?.backgroundColor = color.cgColor
            track.addSubview(fill)
        }

        for lbl in [psiValueLabel, llamaValueLabel, deltaLabel] {
            lbl.translatesAutoresizingMaskIntoConstraints = false
            lbl.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
            lbl.textColor = NSColor(white: 0.72, alpha: 1)
            lbl.lineBreakMode = .byTruncatingTail
            addSubview(lbl)
        }
        deltaLabel.alignment = .right
        deltaLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .semibold)

        psiBarWidth = psiBarFill.widthAnchor.constraint(equalToConstant: 0)
        llamaBarWidth = llamaBarFill.widthAnchor.constraint(equalToConstant: 0)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 30),

            taskLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            taskLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            taskLabel.widthAnchor.constraint(equalToConstant: 170),

            langBadge.leadingAnchor.constraint(equalTo: taskLabel.trailingAnchor, constant: 4),
            langBadge.centerYAnchor.constraint(equalTo: centerYAnchor),
            langBadge.widthAnchor.constraint(equalToConstant: 56),

            psiBarTrack.leadingAnchor.constraint(equalTo: langBadge.trailingAnchor, constant: 12),
            psiBarTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            psiBarTrack.heightAnchor.constraint(equalToConstant: 9),
            psiBarTrack.widthAnchor.constraint(equalToConstant: 100),

            psiBarFill.leadingAnchor.constraint(equalTo: psiBarTrack.leadingAnchor),
            psiBarFill.topAnchor.constraint(equalTo: psiBarTrack.topAnchor),
            psiBarFill.bottomAnchor.constraint(equalTo: psiBarTrack.bottomAnchor),
            psiBarWidth,

            psiValueLabel.leadingAnchor.constraint(equalTo: psiBarTrack.trailingAnchor, constant: 8),
            psiValueLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            psiValueLabel.widthAnchor.constraint(equalToConstant: 145),

            llamaBarTrack.leadingAnchor.constraint(equalTo: psiValueLabel.trailingAnchor, constant: 8),
            llamaBarTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            llamaBarTrack.heightAnchor.constraint(equalToConstant: 9),
            llamaBarTrack.widthAnchor.constraint(equalToConstant: 100),

            llamaBarFill.leadingAnchor.constraint(equalTo: llamaBarTrack.leadingAnchor),
            llamaBarFill.topAnchor.constraint(equalTo: llamaBarTrack.topAnchor),
            llamaBarFill.bottomAnchor.constraint(equalTo: llamaBarTrack.bottomAnchor),
            llamaBarWidth,

            llamaValueLabel.leadingAnchor.constraint(equalTo: llamaBarTrack.trailingAnchor, constant: 8),
            llamaValueLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            llamaValueLabel.widthAnchor.constraint(equalToConstant: 145),

            deltaLabel.leadingAnchor.constraint(equalTo: llamaValueLabel.trailingAnchor, constant: 6),
            deltaLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            deltaLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    func setSelected(_ sel: Bool) {
        selected = sel
        layer?.backgroundColor = sel
            ? NSColor(calibratedRed: 0.16, green: 0.34, blue: 0.56, alpha: 0.32).cgColor
            : NSColor(white: 1.0, alpha: 0.025).cgColor
        layer?.borderWidth = sel ? 1 : 0
        layer?.borderColor = NSColor(calibratedRed: 0.30, green: 0.64, blue: 0.96, alpha: 0.65).cgColor
    }

    func update(side: GymBenchSide, state: GymBenchRowState, scaleMaxSeconds: Double) {
        switch side {
        case .psionic: psiState = state
        case .llamacpp: llamaState = state
        }
        render(scaleMaxSeconds: max(scaleMaxSeconds, 25.0))
    }

    func render(scaleMaxSeconds: Double) {
        applyState(psiState, fill: psiBarFill, widthConstraint: psiBarWidth, label: psiValueLabel, isPsionic: true, maxS: scaleMaxSeconds)
        applyState(llamaState, fill: llamaBarFill, widthConstraint: llamaBarWidth, label: llamaValueLabel, isPsionic: false, maxS: scaleMaxSeconds)

        if case .done(let p) = psiState, case .done(let l) = llamaState {
            let diff = p.wall_s - l.wall_s
            if diff <= -0.05 {
                deltaLabel.stringValue = String(format: "%.2fs faster", -diff)
                deltaLabel.textColor = NSColor(calibratedRed: 0.28, green: 0.92, blue: 0.62, alpha: 1)
            } else if diff >= 0.05 {
                deltaLabel.stringValue = String(format: "+%.2fs", diff)
                deltaLabel.textColor = NSColor(calibratedRed: 0.95, green: 0.68, blue: 0.30, alpha: 1)
            } else {
                deltaLabel.stringValue = "tied"
                deltaLabel.textColor = NSColor(white: 0.72, alpha: 1)
            }
        } else {
            deltaLabel.stringValue = ""
        }
    }

    private func applyState(_ state: GymBenchRowState, fill: NSView, widthConstraint: NSLayoutConstraint, label: NSTextField, isPsionic: Bool, maxS: Double) {
        let trackW: CGFloat = 100
        switch state {
        case .pending:
            widthConstraint.constant = 0
            label.stringValue = "waiting"
            label.textColor = NSColor(white: 0.42, alpha: 1)
        case .running(let turn, let elapsed, let tokRate, let activeTool):
            let frac = CGFloat(min(1.0, max(0.08, elapsed / maxS)))
            widthConstraint.constant = trackW * frac
            fill.layer?.backgroundColor = (isPsionic
                ? NSColor(calibratedRed: 0.22, green: 0.88, blue: 0.64, alpha: 0.95)
                : NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 0.92)).cgColor
            let toolPart = activeTool.isEmpty ? "" : " \(activeTool)"
            if tokRate > 0.1 {
                label.stringValue = String(format: "T%d %.1fs %.0ft/s%@", turn, elapsed, tokRate, toolPart)
            } else {
                label.stringValue = String(format: "T%d %.1fs prefill%@", turn, elapsed, toolPart)
            }
            label.textColor = isPsionic
                ? NSColor(calibratedRed: 0.56, green: 0.98, blue: 0.78, alpha: 1)
                : NSColor(calibratedRed: 0.98, green: 0.82, blue: 0.50, alpha: 1)
        case .done(let res):
            let frac = CGFloat(min(1.0, max(0.06, res.wall_s / maxS)))
            widthConstraint.constant = trackW * frac
            if res.passed {
                fill.layer?.backgroundColor = (isPsionic
                    ? NSColor(calibratedRed: 0.18, green: 0.82, blue: 0.56, alpha: 0.92)
                    : NSColor(calibratedRed: 0.92, green: 0.62, blue: 0.24, alpha: 0.86)).cgColor
                label.stringValue = String(format: "PASS %.2fs (%dt)", res.wall_s, res.turns)
                label.textColor = NSColor(white: 0.90, alpha: 1)
            } else {
                fill.layer?.backgroundColor = NSColor(calibratedRed: 0.90, green: 0.28, blue: 0.28, alpha: 0.90).cgColor
                label.stringValue = String(format: "FAIL %.2fs (%dt)", res.wall_s, res.turns)
                label.textColor = NSColor(calibratedRed: 0.98, green: 0.45, blue: 0.45, alpha: 1)
            }
        }
    }

    override func mouseDown(with event: NSEvent) {
        onSelect?()
    }
}

// MARK: - Scoreboard Card View

private final class GymBenchScoreCard: NSView {
    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private let passLabel = NSTextField(labelWithString: "— / —")
    private let wallLabel = NSTextField(labelWithString: "0.00 s")
    private let metricsLabel = NSTextField(labelWithString: "RSS —  •  Boot —  •  Median —  •  Tokens —")

    init(title: String, subtitle: String, accent: NSColor) {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 8
        layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.035).cgColor
        layer?.borderWidth = 1
        layer?.borderColor = accent.withAlphaComponent(0.35).cgColor

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.stringValue = title
        titleLabel.font = NSFont.systemFont(ofSize: 12.5, weight: .semibold)
        titleLabel.textColor = accent
        addSubview(titleLabel)

        subtitleLabel.translatesAutoresizingMaskIntoConstraints = false
        subtitleLabel.stringValue = subtitle
        subtitleLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .regular)
        subtitleLabel.textColor = NSColor(white: 0.58, alpha: 1)
        addSubview(subtitleLabel)

        passLabel.translatesAutoresizingMaskIntoConstraints = false
        passLabel.font = NSFont.monospacedSystemFont(ofSize: 14, weight: .bold)
        passLabel.textColor = NSColor(white: 0.95, alpha: 1)
        addSubview(passLabel)

        wallLabel.translatesAutoresizingMaskIntoConstraints = false
        wallLabel.font = NSFont.monospacedSystemFont(ofSize: 17, weight: .bold)
        wallLabel.textColor = NSColor(white: 0.98, alpha: 1)
        wallLabel.alignment = .right
        addSubview(wallLabel)

        metricsLabel.translatesAutoresizingMaskIntoConstraints = false
        metricsLabel.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
        metricsLabel.textColor = NSColor(white: 0.72, alpha: 1)
        metricsLabel.lineBreakMode = .byTruncatingTail
        addSubview(metricsLabel)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 72),

            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            subtitleLabel.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            subtitleLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),

            wallLabel.topAnchor.constraint(equalTo: topAnchor, constant: 7),
            wallLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),

            passLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 2),
            passLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            metricsLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            metricsLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            metricsLabel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -7),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    func update(passed: Int, completed: Int, total: Int, wallS: Double, rssMiB: Double, bootS: Double, medianS: Double, toks: Int) {
        passLabel.stringValue = completed == 0 ? "0 / \(total) PASS" : "\(passed) / \(completed) PASS (of \(total))"
        wallLabel.stringValue = String(format: "%.2f s", wallS)
        let rssStr = rssMiB > 0 ? String(format: "%.2f GiB", rssMiB / 1024.0) : "—"
        let bootStr = bootS > 0 ? String(format: "%.3fs", bootS) : "—"
        let medStr = medianS > 0 ? String(format: "%.2fs", medianS) : "—"
        metricsLabel.stringValue = "RSS \(rssStr)  •  Boot \(bootStr)  •  Median \(medStr)  •  \(toks) tok"
    }

    func reset(total: Int) {
        passLabel.stringValue = "— / \(total)"
        wallLabel.stringValue = "0.00 s"
        metricsLabel.stringValue = "RSS —  •  Boot —  •  Median —  •  0 tok"
    }
}

// MARK: - Main GymBenchmarkView

final class GymBenchmarkView: NSView {
    var workingFolderProvider: (() -> URL?)?

    static let allTenTasks: [GymBenchTaskSpec] = [
        .init(tier: "tier2", id: "a1_fix_mean_off_by_one", langBadge: "Python"),
        .init(tier: "tier2", id: "a2_fix_slugify", langBadge: "Python"),
        .init(tier: "tier2", id: "a3_add_restock", langBadge: "Python"),
        .init(tier: "tier4", id: "k01_csv_to_json", langBadge: "Py CLI"),
        .init(tier: "tier4", id: "k04_lru_cache", langBadge: "Python"),
        .init(tier: "tier4", id: "k05_rename_function", langBadge: "6-turn"),
        .init(tier: "tier4", id: "k08_roman_numerals", langBadge: "Python"),
        .init(tier: "tier4", id: "k11_bash_quoting", langBadge: "Bash"),
        .init(tier: "tier4", id: "k14_c_reverse", langBadge: "C/Make"),
        .init(tier: "tier4", id: "k17_js_group_by", langBadge: "JS/Node"),
    ]

    static let leanCoreSmokeTasks: [GymBenchTaskSpec] = [
        .init(tier: "lean/lean-core-smoke", id: "l01_add_zero_right", langBadge: "Lean 4"),
        .init(tier: "lean/lean-core-smoke", id: "l02_add_comm_nat", langBadge: "Lean 4"),
        .init(tier: "lean/lean-core-smoke", id: "l03_mul_one_right", langBadge: "Lean 4"),
        .init(tier: "lean/lean-core-smoke", id: "l04_and_comm", langBadge: "Lean 4"),
        .init(tier: "lean/lean-core-smoke", id: "l05_list_append_nil", langBadge: "Lean 4"),
    ]

    static let leanAlgebraLogicTasks: [GymBenchTaskSpec] = [
        .init(tier: "lean/lean-algebra-logic", id: "la01_add_assoc", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la02_double_eq_add", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la03_or_comm", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la04_list_length_append", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la05_le_add_right", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la06_mul_zero_left", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la07_succ_add", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la08_imp_trans", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la09_list_reverse_nil", langBadge: "Lean 4"),
        .init(tier: "lean/lean-algebra-logic", id: "la10_sub_self_nat", langBadge: "Lean 4"),
    ]

    private var externalLeanSuites: [(title: String, root: URL, specs: [GymBenchTaskSpec])] = []

    private let suitePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let runButton = NSButton(title: "Run Benchmark", target: nil, action: nil)
    private let stopButton = NSButton(title: "Stop", target: nil, action: nil)
    private let loadSavedButton = NSButton(title: "Load Last Saved", target: nil, action: nil)
    private let phaseLabel = NSTextField(labelWithString: "Ready — Inkling 2 Mini IQ4_NL (16.4 GB) on Apple Silicon Metal")

    private let psiCard = GymBenchScoreCard(
        title: "Psionic Metal",
        subtitle: "OpenAgents Coax → Metal 4",
        accent: NSColor(calibratedRed: 0.24, green: 0.90, blue: 0.62, alpha: 1)
    )
    private let llamaCard = GymBenchScoreCard(
        title: "llama.cpp Metal",
        subtitle: "llama-server --jinja",
        accent: NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.28, alpha: 1)
    )
    private let deltaBanner = NSTextField(labelWithString: "Click Run Benchmark to watch Psionic Metal vs llama.cpp race live in our GPU chat components.")

    private let rowsScroll = NSScrollView()
    private let rowsStack = NSStackView()
    private var rowViews: [String: GymBenchTaskRowView] = [:]
    private var activeSpecs: [GymBenchTaskSpec] = allTenTasks

    // Two real side-by-side OpenAgents Coax GPU chat components!
    private let psiChatView = GymChatSurfaceView(
        title: "Psionic Metal · Inkling 2 Mini",
        accent: NSColor(calibratedRed: 0.24, green: 0.90, blue: 0.62, alpha: 1)
    )
    private let llamaChatView = GymChatSurfaceView(
        title: "llama.cpp Metal · Inkling 2 Mini",
        accent: NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.28, alpha: 1)
    )

    // Runtime state
    private var isRunning = false
    private var cancelFlag = false
    private var activeServerProcess: Process?
    private var psiResults: [String: GymBenchTaskResult] = [:]
    private var llamaResults: [String: GymBenchTaskResult] = [:]
    private var psiBootS: Double = 0
    private var llamaBootS: Double = 0
    private var psiPeakRss: Double = 0
    private var llamaPeakRss: Double = 0
    private var selectedTaskId: String?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor(red: 0.055, green: 0.060, blue: 0.075, alpha: 1).cgColor
        setupUI()
        let args = ProcessInfo.processInfo.arguments
        if args.contains("--gym-suite=tier2") {
            suitePopup.selectItem(at: 1)
            activeSpecs = Self.allTenTasks.filter { $0.tier == "tier2" }
        } else if args.contains("--gym-suite=tier4") {
            suitePopup.selectItem(at: 2)
            activeSpecs = Self.allTenTasks.filter { $0.tier == "tier4" }
        } else if args.contains("--gym-suite=lean-smoke") {
            suitePopup.selectItem(at: 3)
            activeSpecs = Self.leanCoreSmokeTasks
        } else if args.contains("--gym-suite=lean-algebra") {
            suitePopup.selectItem(at: 4)
            activeSpecs = Self.leanAlgebraLogicTasks
        }
        rebuildTaskRows()
        loadSavedIfPresent(silent: true)
        if args.contains("--gym-run") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in
                self?.startBenchmarkRun()
            }
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    deinit {
        stopActiveServer()
    }

    private func setupUI() {
        suitePopup.translatesAutoresizingMaskIntoConstraints = false
        suitePopup.controlSize = .small
        suitePopup.font = NSFont.systemFont(ofSize: 11.5, weight: .medium)
        suitePopup.addItems(withTitles: [
            "Coding — All 10 Real Tasks (Tier 2 + Tier 4)",
            "Coding — Tier 2 Quick Suite (3 Python Tasks)",
            "Coding — Tier 4 Multi-Language (7 Tasks: Py / C / JS / Bash)",
            "Lean Proofs — Core Smoke (5 real Lean 4 proofs)",
            "Lean Proofs — Algebra & Logic (10 multi-step proofs)",
        ])
        discoverExternalLeanSuites()
        for ext in externalLeanSuites {
            suitePopup.addItem(withTitle: ext.title)
        }
        suitePopup.target = self
        suitePopup.action = #selector(suiteChanged)
        addSubview(suitePopup)

        runButton.translatesAutoresizingMaskIntoConstraints = false
        runButton.bezelStyle = .rounded
        runButton.controlSize = .small
        runButton.font = NSFont.systemFont(ofSize: 11.5, weight: .semibold)
        runButton.contentTintColor = NSColor(calibratedRed: 0.20, green: 0.86, blue: 0.58, alpha: 1)
        runButton.target = self
        runButton.action = #selector(runClicked)
        addSubview(runButton)

        stopButton.translatesAutoresizingMaskIntoConstraints = false
        stopButton.bezelStyle = .rounded
        stopButton.controlSize = .small
        stopButton.font = NSFont.systemFont(ofSize: 11.5, weight: .medium)
        stopButton.isEnabled = false
        stopButton.target = self
        stopButton.action = #selector(stopClicked)
        addSubview(stopButton)

        loadSavedButton.translatesAutoresizingMaskIntoConstraints = false
        loadSavedButton.bezelStyle = .rounded
        loadSavedButton.controlSize = .small
        loadSavedButton.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        loadSavedButton.target = self
        loadSavedButton.action = #selector(loadSavedClicked)
        addSubview(loadSavedButton)

        phaseLabel.translatesAutoresizingMaskIntoConstraints = false
        phaseLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        phaseLabel.textColor = NSColor(white: 0.68, alpha: 1)
        phaseLabel.lineBreakMode = .byTruncatingTail
        addSubview(phaseLabel)

        psiCard.translatesAutoresizingMaskIntoConstraints = false
        llamaCard.translatesAutoresizingMaskIntoConstraints = false
        addSubview(psiCard)
        addSubview(llamaCard)

        deltaBanner.translatesAutoresizingMaskIntoConstraints = false
        deltaBanner.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .semibold)
        deltaBanner.textColor = NSColor(calibratedRed: 0.72, green: 0.90, blue: 0.84, alpha: 1)
        deltaBanner.alignment = .center
        deltaBanner.wantsLayer = true
        deltaBanner.layer?.cornerRadius = 5
        deltaBanner.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.04).cgColor
        addSubview(deltaBanner)

        rowsStack.orientation = .vertical
        rowsStack.alignment = .leading
        rowsStack.spacing = 3
        rowsStack.edgeInsets = NSEdgeInsets(top: 2, left: 0, bottom: 2, right: 0)
        rowsStack.translatesAutoresizingMaskIntoConstraints = false

        let rowsDoc = FlippedView()
        rowsDoc.translatesAutoresizingMaskIntoConstraints = false
        rowsDoc.addSubview(rowsStack)
        NSLayoutConstraint.activate([
            rowsStack.topAnchor.constraint(equalTo: rowsDoc.topAnchor),
            rowsStack.leadingAnchor.constraint(equalTo: rowsDoc.leadingAnchor),
            rowsStack.trailingAnchor.constraint(equalTo: rowsDoc.trailingAnchor),
            rowsStack.bottomAnchor.constraint(equalTo: rowsDoc.bottomAnchor),
        ])

        rowsScroll.translatesAutoresizingMaskIntoConstraints = false
        rowsScroll.drawsBackground = false
        rowsScroll.hasVerticalScroller = true
        rowsScroll.autohidesScrollers = true
        rowsScroll.documentView = rowsDoc
        addSubview(rowsScroll)

        psiChatView.translatesAutoresizingMaskIntoConstraints = false
        llamaChatView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(psiChatView)
        addSubview(llamaChatView)

        NSLayoutConstraint.activate([
            suitePopup.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            suitePopup.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            suitePopup.widthAnchor.constraint(equalToConstant: 310),

            runButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            runButton.leadingAnchor.constraint(equalTo: suitePopup.trailingAnchor, constant: 8),

            stopButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            stopButton.leadingAnchor.constraint(equalTo: runButton.trailingAnchor, constant: 6),

            loadSavedButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            loadSavedButton.leadingAnchor.constraint(equalTo: stopButton.trailingAnchor, constant: 6),

            phaseLabel.topAnchor.constraint(equalTo: suitePopup.bottomAnchor, constant: 6),
            phaseLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            phaseLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),

            psiCard.topAnchor.constraint(equalTo: phaseLabel.bottomAnchor, constant: 8),
            psiCard.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            psiCard.trailingAnchor.constraint(equalTo: centerXAnchor, constant: -5),

            llamaCard.topAnchor.constraint(equalTo: phaseLabel.bottomAnchor, constant: 8),
            llamaCard.leadingAnchor.constraint(equalTo: centerXAnchor, constant: 5),
            llamaCard.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),

            deltaBanner.topAnchor.constraint(equalTo: psiCard.bottomAnchor, constant: 6),
            deltaBanner.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            deltaBanner.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            deltaBanner.heightAnchor.constraint(equalToConstant: 24),

            rowsScroll.topAnchor.constraint(equalTo: deltaBanner.bottomAnchor, constant: 8),
            rowsScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            rowsScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            rowsScroll.heightAnchor.constraint(equalToConstant: 120),
            rowsDoc.widthAnchor.constraint(equalTo: rowsScroll.widthAnchor),

            psiChatView.topAnchor.constraint(equalTo: rowsScroll.bottomAnchor, constant: 8),
            psiChatView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            psiChatView.trailingAnchor.constraint(equalTo: centerXAnchor, constant: -5),
            psiChatView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),

            llamaChatView.topAnchor.constraint(equalTo: rowsScroll.bottomAnchor, constant: 8),
            llamaChatView.leadingAnchor.constraint(equalTo: centerXAnchor, constant: 5),
            llamaChatView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            llamaChatView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
        ])
    }

    private func rebuildTaskRows() {
        for v in rowsStack.arrangedSubviews {
            rowsStack.removeArrangedSubview(v)
            v.removeFromSuperview()
        }
        rowViews.removeAll()
        for spec in activeSpecs {
            let row = GymBenchTaskRowView(spec: spec)
            row.translatesAutoresizingMaskIntoConstraints = false
            row.onSelect = { [weak self] in
                self?.selectTask(spec.id)
            }
            rowsStack.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: rowsStack.widthAnchor).isActive = true
            rowViews[spec.id] = row
        }
    }

    private func selectTask(_ id: String) {
        selectedTaskId = id
        for (k, v) in rowViews {
            v.setSelected(k == id)
        }
        guard !isRunning else { return }
        if let p = psiResults[id] {
            let badge = String(format: "%@ · %.2fs · %.1f tok/s", p.passed ? "PASS" : "FAIL", p.wall_s, p.decode_tok_s)
            if let prompt = p.prompt, let events = p.chat_events, !events.isEmpty {
                psiChatView.replay(prompt: prompt, taskName: id, events: events, badge: badge)
            } else {
                let summary = Self.formatSummaryMarkdown(p)
                psiChatView.replay(prompt: "Task: \(id)", taskName: id, events: [
                    GymChatSurfaceView.encodeJSON(["kind": "ended", "ending": "finished", "text": summary]),
                ], badge: badge)
            }
        }
        if let l = llamaResults[id] {
            let badge = String(format: "%@ · %.2fs · %.1f tok/s", l.passed ? "PASS" : "FAIL", l.wall_s, l.decode_tok_s)
            if let prompt = l.prompt, let events = l.chat_events, !events.isEmpty {
                llamaChatView.replay(prompt: prompt, taskName: id, events: events, badge: badge)
            } else {
                let summary = Self.formatSummaryMarkdown(l)
                llamaChatView.replay(prompt: "Task: \(id)", taskName: id, events: [
                    GymChatSurfaceView.encodeJSON(["kind": "ended", "ending": "finished", "text": summary]),
                ], badge: badge)
            }
        }
    }

    private static func formatSummaryMarkdown(_ r: GymBenchTaskResult) -> String {
        let ttftStr = r.ttfts.map { String(format: "%.2fs", $0) }.joined(separator: ", ")
        let checksMd = r.checks.map { "- \($0[1] == "pass" ? "✓" : "✗") `\($0[0])`" }.joined(separator: "\n")
        return String(format: "**%@** in **%.2fs** (%d turns · %d tok · %.1f tok/s)\n\nTTFTs: `%@`\n\n%@", r.passed ? "PASS" : "FAIL", r.wall_s, r.turns, r.toks, r.decode_tok_s, ttftStr, checksMd)
    }

    private func discoverExternalLeanSuites() {
        externalLeanSuites.removeAll()
        let fm = FileManager.default
        let extRoot = fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/benchmarks")
        guard let dirs = try? fm.contentsOfDirectory(at: extRoot, includingPropertiesForKeys: [.isDirectoryKey]) else { return }
        for d in dirs.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
            let tasksDir = d.appendingPathComponent("tasks")
            guard let files = try? fm.contentsOfDirectory(at: tasksDir, includingPropertiesForKeys: nil) else { continue }
            let jsonIds = files.filter { $0.pathExtension == "json" }.map { $0.deletingPathExtension().lastPathComponent }.sorted()
            if jsonIds.isEmpty { continue }
            let name = d.lastPathComponent
            let specs = jsonIds.map { GymBenchTaskSpec(tier: "ext-lean:\(name)", id: $0, langBadge: "Lean 4") }
            externalLeanSuites.append((title: "Lean External — \(name) (\(specs.count) tasks)", root: d, specs: specs))
        }
    }

    @objc private func suiteChanged() {
        guard !isRunning else { return }
        let idx = suitePopup.indexOfSelectedItem
        switch idx {
        case 1: activeSpecs = Self.allTenTasks.filter { $0.tier == "tier2" }
        case 2: activeSpecs = Self.allTenTasks.filter { $0.tier == "tier4" }
        case 3: activeSpecs = Self.leanCoreSmokeTasks
        case 4: activeSpecs = Self.leanAlgebraLogicTasks
        default:
            let extIdx = idx - 5
            if extIdx >= 0 && extIdx < externalLeanSuites.count {
                activeSpecs = externalLeanSuites[extIdx].specs
            } else {
                activeSpecs = Self.allTenTasks
            }
        }
        psiResults.removeAll()
        llamaResults.removeAll()
        rebuildTaskRows()
        psiCard.reset(total: activeSpecs.count)
        llamaCard.reset(total: activeSpecs.count)
        let isLean = activeSpecs.first?.tier.contains("lean") == true
        deltaBanner.stringValue = isLean
            ? "Lean 4 Proof Suite: \(activeSpecs.count) kernel-verified theorems (ATIF-v1.8 -> Psionic W3.2). Click Run Benchmark."
            : "Suite selected: \(activeSpecs.count) real coding eval tasks. Click Run Benchmark to begin."
    }

    @objc private func runClicked() {
        guard !isRunning else { return }
        startBenchmarkRun()
    }

    @objc private func stopClicked() {
        cancelFlag = true
        stopActiveServer()
        appendLog("[stop] Benchmark cancelled by user.")
        phaseLabel.stringValue = "Stopped."
        setRunningUI(false)
    }

    @objc private func loadSavedClicked() {
        loadSavedIfPresent(silent: false)
    }

    private func setRunningUI(_ running: Bool) {
        isRunning = running
        runButton.isEnabled = !running
        suitePopup.isEnabled = !running
        loadSavedButton.isEnabled = !running
        stopButton.isEnabled = running
    }

    private func appendLog(_ line: String) {
        let logURL = Self.savedRunURL.deletingLastPathComponent().appendingPathComponent("last-run.log")
        try? FileManager.default.createDirectory(at: logURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let fh = try? FileHandle(forWritingTo: logURL) {
            fh.seekToEndOfFile()
            fh.write(Data((line + "\n").utf8))
            try? fh.close()
        } else {
            try? Data((line + "\n").utf8).write(to: logURL)
        }
    }

    private func refreshScoreboards() {
        let total = activeSpecs.count
        let pRows = activeSpecs.compactMap { psiResults[$0.id] }
        let lRows = activeSpecs.compactMap { llamaResults[$0.id] }
        let pWall = pRows.reduce(0.0) { $0 + $1.wall_s }
        let lWall = lRows.reduce(0.0) { $0 + $1.wall_s }
        let pPass = pRows.filter(\.passed).count
        let lPass = lRows.filter(\.passed).count
        let pToks = pRows.reduce(0) { $0 + $1.toks }
        let lToks = lRows.reduce(0) { $0 + $1.toks }

        let pMed = Self.median(pRows.map(\.wall_s))
        let lMed = Self.median(lRows.map(\.wall_s))
        psiCard.update(passed: pPass, completed: pRows.count, total: total, wallS: pWall, rssMiB: psiPeakRss, bootS: psiBootS, medianS: pMed, toks: pToks)
        llamaCard.update(passed: lPass, completed: lRows.count, total: total, wallS: lWall, rssMiB: llamaPeakRss, bootS: llamaBootS, medianS: lMed, toks: lToks)

        let maxS = max(25.0, (pRows.map(\.wall_s) + lRows.map(\.wall_s)).max() ?? 25.0)
        for v in rowViews.values {
            v.render(scaleMaxSeconds: maxS)
        }

        if !pRows.isEmpty && !lRows.isEmpty {
            let sharedIds = Set(pRows.map(\.task)).intersection(Set(lRows.map(\.task)))
            let pShared = sharedIds.reduce(0.0) { $0 + (psiResults[$1]?.wall_s ?? 0) }
            let lShared = sharedIds.reduce(0.0) { $0 + (llamaResults[$1]?.wall_s ?? 0) }
            let diff = pShared - lShared
            let wallPhrase = diff <= 0
                ? String(format: "Psionic %.2fs faster (%.1f%%)", -diff, lShared > 0 ? (-diff / lShared * 100.0) : 0)
                : String(format: "Psionic +%.2fs (+%.1f%%)", diff, lShared > 0 ? (diff / lShared * 100.0) : 0)
            let rssPhrase = (psiPeakRss > 0 && llamaPeakRss > 0)
                ? String(format: "  •  %.1fx lower RSS (%.1f vs %.1f GiB)", llamaPeakRss / psiPeakRss, psiPeakRss / 1024.0, llamaPeakRss / 1024.0)
                : ""
            let bootPhrase = (psiBootS > 0 && llamaBootS > 0)
                ? String(format: "  •  %.1fx faster boot", llamaBootS / psiBootS)
                : ""
            deltaBanner.stringValue = "\(wallPhrase)\(rssPhrase)\(bootPhrase)"
            deltaBanner.textColor = diff <= 0
                ? NSColor(calibratedRed: 0.30, green: 0.94, blue: 0.64, alpha: 1)
                : NSColor(calibratedRed: 0.96, green: 0.80, blue: 0.42, alpha: 1)
        }
    }

    private static func median(_ xs: [Double]) -> Double {
        guard !xs.isEmpty else { return 0 }
        let s = xs.sorted()
        let m = s.count / 2
        return s.count % 2 == 1 ? s[m] : (s[m - 1] + s[m]) * 0.5
    }

    // MARK: - Paths & Persistence

    private static var savedRunURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/gym/last-benchmark.json")
    }

    private func resolveEvalRoot() -> URL? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser
        var candidates: [URL] = []
        if let res = Bundle.main.resourceURL {
            candidates.append(res.appendingPathComponent("gym-evals"))
        }
        if let wf = workingFolderProvider?() {
            candidates.append(wf.appendingPathComponent("evals/openagents-desktop"))
        }
        candidates.append(home.appendingPathComponent(".cache/openagents/coder-release/src/evals/openagents-desktop"))
        candidates.append(URL(fileURLWithPath: "/Users/christopherdavid/work/openagents/evals/openagents-desktop"))
        for c in candidates {
            if fm.fileExists(atPath: c.appendingPathComponent("tier2/tasks").path) { return c }
        }
        return nil
    }

    private func resolvePsionicBinaries() -> (bin: String, metal: String)? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        var pairs: [(String, String)] = []
        if let res = Bundle.main.resourceURL {
            pairs.append((res.appendingPathComponent("psionic-inkling-serve").path, res.appendingPathComponent("inkling-kernels.metal").path))
        }
        pairs.append(("\(home)/.openagents/bin/psionic-inkling-serve", "\(home)/.openagents/bin/inkling-kernels.metal"))
        if let wf = workingFolderProvider?() {
            pairs.append((wf.appendingPathComponent("psionic/build/psionic-inkling-serve").path, wf.appendingPathComponent("psionic/build/inkling-kernels.metal").path))
        }
        pairs.append(("/Users/christopherdavid/work/openagents/psionic/build/psionic-inkling-serve", "/Users/christopherdavid/work/openagents/psionic/build/inkling-kernels.metal"))
        for (b, m) in pairs {
            if fm.isExecutableFile(atPath: b) && fm.fileExists(atPath: m) { return (b, m) }
        }
        return nil
    }

    private func resolveLeanRunBinary() -> String? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        var candidates: [String] = []
        if let res = Bundle.main.resourceURL {
            candidates.append(res.appendingPathComponent("lean-run").path)
        }
        candidates.append("\(home)/.openagents/bin/lean-run")
        if let wf = workingFolderProvider?() {
            let td = wf.appendingPathComponent("coax/web/build/tools")
            if let entries = try? fm.contentsOfDirectory(atPath: td.path) {
                for e in entries.sorted().reversed() where e.hasPrefix("lean-run-") && !e.hasSuffix(".sources") {
                    candidates.append(td.appendingPathComponent(e).path)
                }
            }
        }
        for c in candidates {
            if fm.isExecutableFile(atPath: c) { return c }
        }
        return nil
    }

    private func loadSavedIfPresent(silent: Bool) {
        let url = Self.savedRunURL
        guard let data = try? Data(contentsOf: url),
              let saved = try? JSONDecoder().decode(GymBenchSavedRun.self, from: data) else {
            if !silent {
                phaseLabel.stringValue = "No saved benchmark found at \(url.path)."
            }
            return
        }
        psiResults.removeAll()
        llamaResults.removeAll()
        if let p = saved.psionic {
            psiBootS = p.boot_s
            psiPeakRss = p.peak_rss_mib
            for r in p.rows {
                psiResults[r.task] = r
                rowViews[r.task]?.update(side: .psionic, state: .done(r), scaleMaxSeconds: 25.0)
            }
        }
        if let l = saved.llamacpp {
            llamaBootS = l.boot_s
            llamaPeakRss = l.peak_rss_mib
            for r in l.rows {
                llamaResults[r.task] = r
                rowViews[r.task]?.update(side: .llamacpp, state: .done(r), scaleMaxSeconds: 25.0)
            }
        }
        refreshScoreboards()
        phaseLabel.stringValue = "Loaded saved run (\(saved.timestamp)) — click any task row to inspect both transcripts, or click Run Benchmark."
        if let first = activeSpecs.first(where: { psiResults[$0.id] != nil || llamaResults[$0.id] != nil }) {
            selectTask(first.id)
        }
    }

    private func saveRun() {
        let pRows = activeSpecs.compactMap { psiResults[$0.id] }
        let lRows = activeSpecs.compactMap { llamaResults[$0.id] }
        let df = ISO8601DateFormatter()
        let saved = GymBenchSavedRun(
            timestamp: df.string(from: Date()),
            suite: suitePopup.titleOfSelectedItem ?? "10-Task",
            psionic: pRows.isEmpty ? nil : GymBenchBackendReport(
                label: "Psionic Metal",
                boot_s: psiBootS,
                peak_rss_mib: psiPeakRss,
                passed: pRows.filter(\.passed).count,
                total: activeSpecs.count,
                total_wall_s: pRows.reduce(0) { $0 + $1.wall_s },
                median_wall_s: Self.median(pRows.map(\.wall_s)),
                total_toks: pRows.reduce(0) { $0 + $1.toks },
                rows: pRows
            ),
            llamacpp: lRows.isEmpty ? nil : GymBenchBackendReport(
                label: "llama.cpp Metal",
                boot_s: llamaBootS,
                peak_rss_mib: llamaPeakRss,
                passed: lRows.filter(\.passed).count,
                total: activeSpecs.count,
                total_wall_s: lRows.reduce(0) { $0 + $1.wall_s },
                median_wall_s: Self.median(lRows.map(\.wall_s)),
                total_toks: lRows.reduce(0) { $0 + $1.toks },
                rows: lRows
            )
        )
        let url = Self.savedRunURL
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(saved) {
            try? data.write(to: url, options: .atomic)
        }
    }

    // MARK: - Live Benchmark Execution Engine (Native Swift -> Coax GPU Chat Views)

    private func terminateServer(_ proc: Process) {
        guard proc.isRunning else { return }
        proc.terminate()
        let deadline = CFAbsoluteTimeGetCurrent() + 0.4
        while proc.isRunning && CFAbsoluteTimeGetCurrent() < deadline {
            Thread.sleep(forTimeInterval: 0.02)
        }
        if proc.isRunning {
            kill(proc.processIdentifier, SIGKILL)
        }
        proc.waitUntilExit()
    }

    private func stopActiveServer() {
        if let p = activeServerProcess {
            terminateServer(p)
            activeServerProcess = nil
        }
    }

    private func chatView(for side: GymBenchSide) -> GymChatSurfaceView {
        side == .psionic ? psiChatView : llamaChatView
    }

    private func startBenchmarkRun() {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        let modelPath = "\(home)/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf"
        let llamaBin = "\(home)/.openagents/bin/llama-server"

        guard fm.fileExists(atPath: modelPath) else {
            phaseLabel.stringValue = "Missing model checkpoint: \(modelPath)"
            return
        }
        guard let psi = resolvePsionicBinaries() else {
            phaseLabel.stringValue = "Could not locate psionic-inkling-serve + inkling-kernels.metal."
            return
        }
        guard fm.isExecutableFile(atPath: llamaBin) else {
            phaseLabel.stringValue = "Missing llama-server at \(llamaBin)"
            return
        }
        guard let evalRoot = resolveEvalRoot() else {
            phaseLabel.stringValue = "Could not locate evals/openagents-desktop/."
            return
        }

        cancelFlag = false
        setRunningUI(true)
        let logURL = Self.savedRunURL.deletingLastPathComponent().appendingPathComponent("last-run.log")
        try? FileManager.default.removeItem(at: logURL)
        psiResults.removeAll()
        llamaResults.removeAll()
        psiBootS = 0; llamaBootS = 0; psiPeakRss = 0; llamaPeakRss = 0
        for (_, row) in rowViews {
            row.update(side: .psionic, state: .pending, scaleMaxSeconds: 25.0)
            row.update(side: .llamacpp, state: .pending, scaleMaxSeconds: 25.0)
        }
        psiCard.reset(total: activeSpecs.count)
        llamaCard.reset(total: activeSpecs.count)
        psiChatView.setBadge("Booting…")
        llamaChatView.setBadge("Queued")
        deltaBanner.stringValue = "Running live head-to-head benchmark…"
        let specs = activeSpecs

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            self.runSide(
                side: .psionic,
                executable: psi.bin,
                args: ["--model", modelPath, "--metal", psi.metal, "--port", "18445", "--ctx", "8192"],
                port: 18445,
                evalRoot: evalRoot,
                specs: specs
            )
            if !self.cancelFlag {
                self.runSide(
                    side: .llamacpp,
                    executable: llamaBin,
                    args: ["-m", modelPath, "--port", "18446", "-c", "8192", "-ngl", "99", "--jinja", "--cache-prompt"],
                    port: 18446,
                    evalRoot: evalRoot,
                    specs: specs
                )
            }
            DispatchQueue.main.async {
                if !self.cancelFlag {
                    self.saveRun()
                    self.refreshScoreboards()
                    self.phaseLabel.stringValue = "Benchmark complete! Click any task row above to inspect both GPU chat transcripts."
                    self.appendLog("[done] Head-to-head benchmark finished and saved.")
                }
                self.setRunningUI(false)
            }
        }
    }

    private func runSide(side: GymBenchSide, executable: String, args: [String], port: Int, evalRoot: URL, specs: [GymBenchTaskSpec]) {
        DispatchQueue.main.async {
            self.phaseLabel.stringValue = "Booting \(side.rawValue)…"
            self.chatView(for: side).setBadge("Booting…")
        }
        appendLog("\n=== Starting \(side.rawValue) on port \(port) ===")

        let tBoot0 = CFAbsoluteTimeGetCurrent()
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: executable)
        proc.arguments = args
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
        } catch {
            appendLog("[error] Failed to launch \(side.rawValue): \(error.localizedDescription)")
            return
        }
        activeServerProcess = proc
        defer {
            terminateServer(proc)
            if activeServerProcess === proc { activeServerProcess = nil }
        }

        if !waitHealth(port: port) {
            appendLog("[error] \(side.rawValue) failed health check on port \(port).")
            return
        }
        let bootS = CFAbsoluteTimeGetCurrent() - tBoot0
        let bootRss = pollRssMiB(pid: proc.processIdentifier)
        DispatchQueue.main.async {
            if side == .psionic { self.psiBootS = bootS; self.psiPeakRss = max(self.psiPeakRss, bootRss) }
            else { self.llamaBootS = bootS; self.llamaPeakRss = max(self.llamaPeakRss, bootRss) }
            self.refreshScoreboards()
        }
        appendLog(String(format: "[boot] %@ ready in %.3fs (RSS %.1f MiB)", side.rawValue, bootS, bootRss))

        // Warmup
        _ = streamTurn(port: port, messages: [["role": "user", "content": "hi"]], maxTokens: 8, onStream: nil)

        for (idx, spec) in specs.enumerated() {
            if cancelFlag { break }
            DispatchQueue.main.async {
                self.phaseLabel.stringValue = "Running \(side.rawValue) [\(idx + 1)/\(specs.count)] \(spec.id)…"
                for (k, v) in self.rowViews { v.setSelected(k == spec.id) }
            }
            let result = runSingleTask(side: side, port: port, pid: proc.processIdentifier, evalRoot: evalRoot, spec: spec)
            DispatchQueue.main.async {
                if side == .psionic { self.psiResults[spec.id] = result }
                else { self.llamaResults[spec.id] = result }
                let maxS = max(25.0, (self.psiResults.values.map(\.wall_s) + self.llamaResults.values.map(\.wall_s)).max() ?? 25.0)
                self.rowViews[spec.id]?.update(side: side, state: .done(result), scaleMaxSeconds: maxS)
                self.refreshScoreboards()
            }
        }
    }

    private func runSingleTask(side: GymBenchSide, port: Int, pid: Int32, evalRoot: URL, spec: GymBenchTaskSpec) -> GymBenchTaskResult {
        let isLean = spec.tier.hasPrefix("lean/") || spec.tier.hasPrefix("ext-lean:")
        let tierDir: URL
        if spec.tier.hasPrefix("ext-lean:") {
            let extName = String(spec.tier.dropFirst("ext-lean:".count))
            tierDir = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".openagents/lean/benchmarks")
                .appendingPathComponent(extName)
        } else {
            tierDir = evalRoot.appendingPathComponent(spec.tier)
        }
        let taskFile = tierDir.appendingPathComponent("tasks/\(spec.id).json")
        guard let taskData = try? Data(contentsOf: taskFile),
              let taskObj = try? JSONSerialization.jsonObject(with: taskData) as? [String: Any],
              let fixtureName = taskObj["fixture"] as? String,
              let steps = taskObj["steps"] as? [[String: Any]],
              let prompt = steps.first?["send"] as? String else {
            appendLog("[error] Failed to load task spec \(taskFile.path)")
            return GymBenchTaskResult(task: spec.id, tier: spec.tier, passed: false, wall_s: 0, turns: 0, toks: 0, decode_tok_s: 0, ttfts: [], checks: [["load_spec", "fail"]], prompt: nil, chat_events: nil)
        }

        let fixDir = tierDir.appendingPathComponent("fixtures/\(fixtureName)")
        let tmpDir = FileManager.default.temporaryDirectory.appendingPathComponent("oa-gym-\(spec.id)-\(UUID().uuidString.prefix(8))")
        defer { try? FileManager.default.removeItem(at: tmpDir) }
        try? FileManager.default.copyItem(at: fixDir, to: tmpDir)

        let files = listRelativeFiles(in: tmpDir)
        let sysPrompt: String
        if isLean {
            sysPrompt = "You are OpenAgents, a Lean 4 theorem prover working in a local workspace. Read Problem.lean, write a complete proof without using sorry, custom axioms, or native_decide, and never alter the theorem signature. Verify with LeanCheck (or inspect tactic states with LeanGoal and LeanTry) until Lean 4 verifies ok, then give a concise summary. Keep thinking brief.\n\nWorkspace files: " + files.joined(separator: ", ")
        } else {
            sysPrompt = "You are OpenAgents, a coding agent working in a local repository. Read the relevant files first, make the required edits without modifying test files, run the test command with Bash to verify everything passes, then give a concise summary. Keep thinking brief.\n\nWorkspace files: " + files.joined(separator: ", ")
        }

        var messages: [[String: Any]] = [
            ["role": "system", "content": sysPrompt],
            ["role": "user", "content": prompt],
        ]

        var recordedEvents: [String] = []
        let emitChat: ([String: Any]) -> Void = { [weak self] obj in
            let line = GymChatSurfaceView.encodeJSON(obj)
            recordedEvents.append(line)
            DispatchQueue.main.async {
                self?.chatView(for: side).feedLine(line)
            }
        }

        DispatchQueue.main.async {
            let cv = self.chatView(for: side)
            cv.setBadge("\(spec.id) · Turn 1")
            cv.startTask(prompt: prompt, taskName: spec.id)
        }

        appendLog("[\(side.rawValue)] ▶ \(spec.id) (\(spec.langBadge))")
        let t0 = CFAbsoluteTimeGetCurrent()
        var ttfts: [Double] = []
        var totalToks = 0
        var totalDecodeS = 0.0
        var turns = 0
        var lastAssistantText = ""

        for turnIdx in 1...8 {
            if cancelFlag { break }
            turns = turnIdx
            DispatchQueue.main.async {
                self.chatView(for: side).setBadge("\(spec.id) · Turn \(turnIdx) prefill…")
                self.rowViews[spec.id]?.update(side: side, state: .running(turn: turnIdx, elapsed: CFAbsoluteTimeGetCurrent() - t0, tokRate: 0, activeTool: ""), scaleMaxSeconds: 25.0)
            }
            var streamedChars = 0
            let tr = streamTurn(port: port, messages: messages, maxTokens: 1024, useLeanTools: isLean) { [weak self] deltaContent, deltaReason, elapsedDecode in
                streamedChars += deltaContent.count + deltaReason.count
                let approxTok = Double(streamedChars) / 4.0
                let rate = elapsedDecode > 0.2 ? approxTok / elapsedDecode : 0
                DispatchQueue.main.async {
                    guard let self else { return }
                    let cv = self.chatView(for: side)
                    if !deltaContent.isEmpty {
                        cv.feedLine(GymChatSurfaceView.encodeJSON(["kind": "reply", "text": deltaContent]))
                    }
                    if rate > 0.1 {
                        cv.setBadge(String(format: "%@ · T%d · %.0f tok/s", spec.id, turnIdx, rate))
                    }
                    self.rowViews[spec.id]?.update(side: side, state: .running(turn: turnIdx, elapsed: CFAbsoluteTimeGetCurrent() - t0, tokRate: rate, activeTool: ""), scaleMaxSeconds: 25.0)
                }
            }
            let rssNow = pollRssMiB(pid: pid)
            DispatchQueue.main.async {
                if side == .psionic { self.psiPeakRss = max(self.psiPeakRss, rssNow) }
                else { self.llamaPeakRss = max(self.llamaPeakRss, rssNow) }
                self.refreshScoreboards()
            }

            if !tr.reasoning.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                emitChat(["kind": "thought", "text": tr.reasoning.trimmingCharacters(in: .whitespacesAndNewlines)])
            }
            if !tr.content.isEmpty {
                lastAssistantText = tr.content
                emitChat(["kind": "reply", "text": tr.content])
            }

            ttfts.append(tr.ttft)
            totalToks += tr.completionTokens
            totalDecodeS += max(0.001, tr.dur - tr.ttft)
            let turnTokRate = tr.completionTokens > 0 ? Double(tr.completionTokens) / max(0.001, tr.dur - tr.ttft) : 0
            appendLog(String(format: "  T%d: TTFT %.2fs · %d tok · %.1f tok/s · %d tool(s)", turnIdx, tr.ttft, tr.completionTokens, turnTokRate, tr.toolCalls.count))

            if tr.toolCalls.isEmpty {
                messages.append(["role": "assistant", "content": tr.content])
                break
            }
            messages.append(["role": "assistant", "content": tr.content, "tool_calls": tr.toolCalls])
            for tc in tr.toolCalls {
                let tcId = (tc["id"] as? String) ?? "call_0"
                let fn = (tc["function"] as? [String: Any]) ?? [:]
                let fnName = (fn["name"] as? String) ?? ""
                let argStr = (fn["arguments"] as? String) ?? "{}"
                let argObj = (try? JSONSerialization.jsonObject(with: Data(argStr.utf8)) as? [String: Any]) ?? [:]
                DispatchQueue.main.async {
                    self.chatView(for: side).setBadge("\(spec.id) · T\(turnIdx) · \(fnName)")
                    self.rowViews[spec.id]?.update(side: side, state: .running(turn: turnIdx, elapsed: CFAbsoluteTimeGetCurrent() - t0, tokRate: turnTokRate, activeTool: fnName), scaleMaxSeconds: 25.0)
                }
                let (toolOut, exitCode) = execTool(workdir: tmpDir, name: fnName, args: argObj)
                let preview = (argObj["path"] as? String) ?? (argObj["file"] as? String) ?? (argObj["command"] as? String) ?? ""
                appendLog("    ↳ \(fnName)(\(preview.prefix(64)))")
                if fnName == "Read" {
                    emitChat(["kind": "view", "path": preview, "found": !toolOut.hasPrefix("(file not found")])
                } else if fnName == "Write" {
                    emitChat(["kind": "command", "command": "Write \(preview)", "exit": 0, "timed_out": false, "output": toolOut])
                } else {
                    emitChat(["kind": "command", "command": preview, "exit": Int(exitCode), "timed_out": false, "output": String(toolOut.prefix(1200))])
                }
                messages.append(["role": "tool", "tool_call_id": tcId, "content": toolOut])
            }
        }

        let wallS = CFAbsoluteTimeGetCurrent() - t0
        let (passed, checks) = gradeTask(taskObj: taskObj, fixDir: fixDir, workdir: tmpDir, spec: spec, side: side, prompt: prompt)
        let decodeTokS = totalDecodeS > 0 ? Double(totalToks) / totalDecodeS : 0
        let chkSummary = checks.map { "\($0[1] == "pass" ? "✓" : "✗") \($0[0])" }.joined(separator: "  ")
        appendLog(String(format: "  ⇒ %@ in %.2fs (%d turns, %d tok, %.1f tok/s)  %@", passed ? "PASS" : "FAIL", wallS, turns, totalToks, decodeTokS, chkSummary))

        let verdictFooter = String(format: "\n\n---\n**%@** · `%.2fs` · `%d turns` · `%d tok` · `%.1f tok/s`  \n%@", passed ? "✓ PASS" : "✗ FAIL", wallS, turns, totalToks, decodeTokS, chkSummary)
        let finalReply = (lastAssistantText.isEmpty ? "Completed." : lastAssistantText) + verdictFooter
        emitChat(["kind": "ended", "ending": "finished", "text": finalReply])
        DispatchQueue.main.async {
            self.chatView(for: side).setBadge(String(format: "%@ · %@ %.2fs (%.1f tok/s)", spec.id, passed ? "PASS" : "FAIL", wallS, decodeTokS))
        }

        return GymBenchTaskResult(
            task: spec.id,
            tier: spec.tier,
            passed: passed,
            wall_s: (wallS * 100).rounded() / 100,
            turns: turns,
            toks: totalToks,
            decode_tok_s: (decodeTokS * 10).rounded() / 10,
            ttfts: ttfts.map { ($0 * 1000).rounded() / 1000 },
            checks: checks,
            prompt: prompt,
            chat_events: recordedEvents
        )
    }

    // MARK: - HTTP SSE Streaming & Tool Execution

    private static let toolsSchema: [[String: Any]] = [
        [
            "type": "function",
            "function": [
                "name": "Read",
                "description": "Read a file in the workspace.",
                "parameters": ["type": "object", "properties": ["path": ["type": "string"]], "required": ["path"]],
            ],
        ],
        [
            "type": "function",
            "function": [
                "name": "Write",
                "description": "Write the full contents of a file in the workspace.",
                "parameters": ["type": "object", "properties": ["path": ["type": "string"], "content": ["type": "string"]], "required": ["path", "content"]],
            ],
        ],
        [
            "type": "function",
            "function": [
                "name": "Bash",
                "description": "Run a shell command in the workspace.",
                "parameters": ["type": "object", "properties": ["command": ["type": "string"]], "required": ["command"]],
            ],
        ],
    ]

    private static let leanToolsSchema: [[String: Any]] = toolsSchema + [
        [
            "type": "function",
            "function": [
                "name": "LeanCheck",
                "description": "Verify a Lean 4 theorem with the real Lean 4 kernel and #authority axiom policy.",
                "parameters": [
                    "type": "object",
                    "properties": [
                        "file": ["type": "string", "description": "Relative path to the .lean file (e.g. Problem.lean)"],
                        "target": ["type": "string", "description": "Theorem name to verify"],
                    ],
                    "required": ["file", "target"],
                ],
            ],
        ],
        [
            "type": "function",
            "function": [
                "name": "LeanGoal",
                "description": "Query the live Lean 4 tactic proof state at a 1-based line and column.",
                "parameters": [
                    "type": "object",
                    "properties": [
                        "file": ["type": "string"],
                        "line": ["type": "integer"],
                        "col": ["type": "integer"],
                    ],
                    "required": ["file", "line", "col"],
                ],
            ],
        ],
        [
            "type": "function",
            "function": [
                "name": "LeanTry",
                "description": "Try a Lean 4 tactic in memory via oa-leanhost without modifying the file on disk.",
                "parameters": [
                    "type": "object",
                    "properties": [
                        "file": ["type": "string"],
                        "line": ["type": "integer"],
                        "col": ["type": "integer"],
                        "tactic": ["type": "string"],
                    ],
                    "required": ["file", "line", "col", "tactic"],
                ],
            ],
        ],
    ]

    private func waitHealth(port: Int) -> Bool {
        let url = URL(string: "http://127.0.0.1:\(port)/health")!
        for _ in 0..<300 {
            if cancelFlag { return false }
            let sem = DispatchSemaphore(value: 0)
            var ok = false
            var req = URLRequest(url: url)
            req.timeoutInterval = 0.5
            URLSession.shared.dataTask(with: req) { _, resp, _ in
                if let http = resp as? HTTPURLResponse, http.statusCode == 200 { ok = true }
                sem.signal()
            }.resume()
            _ = sem.wait(timeout: .now() + 0.6)
            if ok { return true }
            Thread.sleep(forTimeInterval: 0.05)
        }
        return false
    }

    private func pollRssMiB(pid: Int32) -> Double {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/ps")
        p.arguments = ["-o", "rss=", "-p", String(pid)]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        try? p.run()
        p.waitUntilExit()
        let out = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        guard let kb = Double(out) else { return 0 }
        return ((kb / 1024.0) * 10).rounded() / 10
    }

    private func streamTurn(port: Int, messages: [[String: Any]], maxTokens: Int, useLeanTools: Bool = false, onStream: ((String, String, Double) -> Void)?) -> GymBenchStreamTurn {
        let url = URL(string: "http://127.0.0.1:\(port)/v1/chat/completions")!
        let payload: [String: Any] = [
            "model": "inkling-2-mini",
            "messages": messages,
            "tools": useLeanTools ? Self.leanToolsSchema : Self.toolsSchema,
            "max_tokens": maxTokens,
            "temperature": 0.0,
            "stream": true,
            "stream_options": ["include_usage": true],
            "cache_prompt": true,
        ]
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try? JSONSerialization.data(withJSONObject: payload)
        req.timeoutInterval = 180

        let t0 = CFAbsoluteTimeGetCurrent()
        let sem = DispatchSemaphore(value: 0)
        var result = GymBenchStreamTurn(ttft: 0, dur: 0, promptTokens: 0, completionTokens: 0, content: "", reasoning: "", toolCalls: [])

        final class SSEDelegate: NSObject, URLSessionDataDelegate {
            let t0: Double
            let onStream: ((String, String, Double) -> Void)?
            let done: (GymBenchStreamTurn) -> Void
            var buf = Data()
            var ttft: Double?
            var content = ""
            var reasoning = ""
            var pendingContent = ""
            var pendingReason = ""
            var promptTokens = 0
            var completionTokens = 0
            var tcMap: [Int: (id: String, name: String, args: String)] = [:]
            var lastUI = 0.0

            init(t0: Double, onStream: ((String, String, Double) -> Void)?, done: @escaping (GymBenchStreamTurn) -> Void) {
                self.t0 = t0; self.onStream = onStream; self.done = done
            }

            func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
                buf.append(data)
                while let nl = buf.firstIndex(of: 0x0A) {
                    let lineData = buf.subdata(in: buf.startIndex..<nl)
                    buf.removeSubrange(buf.startIndex...nl)
                    let line = String(decoding: lineData, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                    guard line.hasPrefix("data: ") else { continue }
                    let body = String(line.dropFirst(6))
                    if body == "[DONE]" { continue }
                    guard let jData = body.data(using: .utf8),
                          let obj = try? JSONSerialization.jsonObject(with: jData) as? [String: Any] else { continue }
                    if let u = obj["usage"] as? [String: Any] {
                        if let pt = u["prompt_tokens"] as? Int { promptTokens = pt }
                        if let ct = u["completion_tokens"] as? Int { completionTokens = ct }
                    }
                    guard let choices = obj["choices"] as? [[String: Any]], let first = choices.first,
                          let delta = first["delta"] as? [String: Any] else { continue }
                    let ct = (delta["content"] as? String) ?? ""
                    let rc = (delta["reasoning_content"] as? String) ?? ""
                    let tcs = (delta["tool_calls"] as? [[String: Any]]) ?? []
                    if !ct.isEmpty || !rc.isEmpty || !tcs.isEmpty {
                        if ttft == nil { ttft = CFAbsoluteTimeGetCurrent() - t0 }
                    }
                    if !ct.isEmpty { content += ct; pendingContent += ct }
                    if !rc.isEmpty { reasoning += rc; pendingReason += rc }
                    for tc in tcs {
                        let idx = (tc["index"] as? Int) ?? 0
                        let fn = (tc["function"] as? [String: Any]) ?? [:]
                        var entry = tcMap[idx] ?? (id: (tc["id"] as? String) ?? "call_\(idx)", name: "", args: "")
                        if let tid = tc["id"] as? String, !tid.isEmpty { entry.id = tid }
                        if let n = fn["name"] as? String { entry.name += n }
                        if let a = fn["arguments"] as? String { entry.args += a }
                        tcMap[idx] = entry
                    }
                    let now = CFAbsoluteTimeGetCurrent()
                    if now - lastUI > 0.08 && (!pendingContent.isEmpty || !pendingReason.isEmpty) {
                        lastUI = now
                        let cOut = pendingContent; let rOut = pendingReason
                        pendingContent = ""; pendingReason = ""
                        onStream?(cOut, rOut, now - (t0 + (ttft ?? 0)))
                    }
                }
            }

            func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
                let dur = CFAbsoluteTimeGetCurrent() - t0
                let sortedKeys = tcMap.keys.sorted()
                let tcs: [[String: Any]] = sortedKeys.compactMap { k in
                    guard let e = tcMap[k] else { return nil }
                    return [
                        "id": e.id,
                        "type": "function",
                        "function": ["name": e.name, "arguments": e.args.isEmpty ? "{}" : e.args],
                    ]
                }
                done(GymBenchStreamTurn(
                    ttft: ttft ?? dur,
                    dur: dur,
                    promptTokens: promptTokens,
                    completionTokens: completionTokens,
                    content: content,
                    reasoning: reasoning,
                    toolCalls: tcs
                ))
            }
        }

        let delegate = SSEDelegate(t0: t0, onStream: onStream) { tr in
            result = tr
            sem.signal()
        }
        let cfg = URLSessionConfiguration.ephemeral
        cfg.timeoutIntervalForRequest = 180
        cfg.timeoutIntervalForResource = 180
        let session = URLSession(configuration: cfg, delegate: delegate, delegateQueue: nil)
        session.dataTask(with: req).resume()
        _ = sem.wait(timeout: .now() + 185)
        session.invalidateAndCancel()
        return result
    }

    private func execTool(workdir: URL, name: String, args: [String: Any]) -> (String, Int32) {
        let fm = FileManager.default
        if name == "Read" {
            let rel = ((args["path"] as? String) ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let full = workdir.appendingPathComponent(rel)
            if let txt = try? String(contentsOf: full, encoding: .utf8) { return (txt, 0) }
            return ("(file not found: \(rel))", 1)
        } else if name == "Write" {
            let rel = ((args["path"] as? String) ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let content = (args["content"] as? String) ?? ""
            let full = workdir.appendingPathComponent(rel)
            try? fm.createDirectory(at: full.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? Data(content.utf8).write(to: full)
            return ("Wrote \(rel) (\(content.utf8.count) bytes).", 0)
        } else if name == "Bash" {
            let cmd = (args["command"] as? String) ?? ""
            let (code, out) = runShell(cmd, cwd: workdir)
            return ("exit=\(code)\n\(out.trimmingCharacters(in: .whitespacesAndNewlines))", code)
        } else if name == "LeanCheck" || name == "LeanGoal" || name == "LeanTry" {
            guard let leanRun = resolveLeanRunBinary() else {
                return ("(lean-run binary not found)", 1)
            }
            let rel = ((args["file"] as? String) ?? "Problem.lean").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let full = workdir.appendingPathComponent(rel).path
            var argv: [String] = []
            if name == "LeanCheck" {
                let target = (args["target"] as? String) ?? ""
                argv = ["verify", full, target, "--json"]
            } else if name == "LeanGoal" {
                let line = (args["line"] as? Int) ?? 1
                let col = (args["col"] as? Int) ?? 1
                argv = ["goal", full, String(line), String(col), "--json"]
            } else {
                let line = (args["line"] as? Int) ?? 1
                let col = (args["col"] as? Int) ?? 1
                let tactic = (args["tactic"] as? String) ?? "rfl"
                argv = ["try", full, String(line), String(col), tactic, "--json"]
            }
            let (code, out) = runExec(leanRun, args: argv, cwd: workdir)
            return (out.trimmingCharacters(in: .whitespacesAndNewlines), code)
        }
        return ("(unknown tool \(name))", 1)
    }

    private func runExec(_ exe: String, args: [String], cwd: URL) -> (Int32, String) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = args
        p.currentDirectoryURL = cwd
        var env = ProcessInfo.processInfo.environment
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let extraPath = "\(home)/.elan/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
        env["PATH"] = (env["PATH"]?.isEmpty == false) ? "\(env["PATH"]!):\(extraPath)" : extraPath
        p.environment = env
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = pipe
        do {
            try p.run()
            p.waitUntilExit()
        } catch {
            return (1, error.localizedDescription)
        }
        let out = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        return (p.terminationStatus, out)
    }

    private func runShell(_ cmd: String, cwd: URL) -> (Int32, String) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/sh")
        p.arguments = ["-c", cmd]
        p.currentDirectoryURL = cwd
        var env = ProcessInfo.processInfo.environment
        let extraPath = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        if let cur = env["PATH"], !cur.isEmpty {
            env["PATH"] = cur + ":" + extraPath
        } else {
            env["PATH"] = extraPath
        }
        p.environment = env
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = pipe
        do {
            try p.run()
            p.waitUntilExit()
        } catch {
            return (1, error.localizedDescription)
        }
        let out = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        return (p.terminationStatus, out)
    }

    private func listRelativeFiles(in root: URL) -> [String] {
        var out: [String] = []
        guard let en = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey]) else { return out }
        let base = root.standardizedFileURL.path + "/"
        for case let u as URL in en {
            if (try? u.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true {
                let p = u.standardizedFileURL.path
                if p.hasPrefix(base) { out.append(String(p.dropFirst(base.count))) }
            }
        }
        return out.sorted()
    }

    private func gradeTask(taskObj: [String: Any], fixDir: URL, workdir: URL, spec: GymBenchTaskSpec, side: GymBenchSide, prompt: String) -> (Bool, [[String]]) {
        let fm = FileManager.default
        let graders = (taskObj["graders"] as? [[String: Any]]) ?? []
        var checks: [[String]] = []
        var allOk = true

        for g in graders {
            let gt = (g["type"] as? String) ?? ""
            if gt == "lean_verify" {
                let rel = (g["file"] as? String) ?? "Problem.lean"
                let target = (g["target"] as? String) ?? spec.id
                let statement = (g["statement"] as? String) ?? ""
                let full = workdir.appendingPathComponent(rel).path
                guard let leanRun = resolveLeanRunBinary() else {
                    checks.append(["lean_verify:missing_lean_run", "fail"])
                    allOk = false
                    continue
                }
                let sideSlug = (side == .psionic) ? "psionic" : "llamacpp"
                let runId = "gym-\(sideSlug)-\(spec.id)-\(Int(Date().timeIntervalSince1970))"
                var argv = [
                    "atif-export",
                    "--file", full,
                    "--target", target,
                    "--run", runId,
                    "--task", spec.id,
                    "--model", side.rawValue,
                    "--prompt", prompt,
                    "--json",
                ]
                if !statement.isEmpty {
                    argv.append(contentsOf: ["--statement", statement])
                }
                let (code, out) = runExec(leanRun, args: argv, cwd: workdir)
                let trimmed = out.trimmingCharacters(in: .whitespacesAndNewlines)
                var verified = false
                var verdict = "error"
                var axiomsStr = ""
                var ms = 0
                if let line = trimmed.split(separator: "\n").last(where: { $0.hasPrefix("{") }),
                   let data = String(line).data(using: .utf8),
                   let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                    verified = (obj["verified"] as? Bool) ?? false
                    verdict = (obj["verdict"] as? String) ?? "error"
                    ms = (obj["elapsed_ms"] as? Int) ?? 0
                    if let axs = obj["axioms"] as? [String], !axs.isEmpty {
                        axiomsStr = axs.joined(separator: ",")
                    }
                }
                let ok = (code == 0) && verified && (verdict == "ok")
                let label: String
                if ok {
                    label = "lean_verify:ok (\(ms)ms\(axiomsStr.isEmpty ? "" : ", \(axiomsStr)"))"
                } else {
                    label = "lean_verify:REJECTED[\(verdict)]\(axiomsStr.isEmpty ? "" : " (\(axiomsStr))")"
                }
                checks.append([label, ok ? "pass" : "fail"])
                if !ok { allOk = false }
            } else if gt == "files_unchanged" {
                let patterns = (g["paths"] as? [String]) ?? []
                var ok = true
                for rel in listRelativeFiles(in: fixDir) {
                    let fname = URL(fileURLWithPath: rel).lastPathComponent
                    let matches = patterns.contains { pat in
                        fnmatch(pat, rel, 0) == 0 || fnmatch(pat, fname, 0) == 0
                    }
                    if matches {
                        let orig = try? Data(contentsOf: fixDir.appendingPathComponent(rel))
                        let cur = try? Data(contentsOf: workdir.appendingPathComponent(rel))
                        if orig != cur { ok = false }
                    }
                }
                checks.append(["files_unchanged", ok ? "pass" : "fail"])
                if !ok { allOk = false }
            } else if gt == "command" {
                let runCmd = (g["run"] as? String) ?? ""
                let name = (g["name"] as? String) ?? runCmd
                let tmpG = fm.temporaryDirectory.appendingPathComponent("oa-grade-\(UUID().uuidString.prefix(8))")
                defer { try? fm.removeItem(at: tmpG) }
                try? fm.copyItem(at: workdir, to: tmpG)
                if let restores = g["restore"] as? [String] {
                    for rf in restores {
                        let src = fixDir.appendingPathComponent(rf)
                        let dst = tmpG.appendingPathComponent(rf)
                        if fm.fileExists(atPath: src.path) {
                            try? fm.removeItem(at: dst)
                            try? fm.copyItem(at: src, to: dst)
                        }
                    }
                }
                if let adds = g["add"] as? [String: String] {
                    for (af, ac) in adds {
                        let dst = tmpG.appendingPathComponent(af)
                        try? fm.createDirectory(at: dst.deletingLastPathComponent(), withIntermediateDirectories: true)
                        try? Data(ac.utf8).write(to: dst)
                    }
                }
                let (code, _) = runShell(runCmd, cwd: tmpG)
                let ok = (code == 0)
                checks.append([name, ok ? "pass" : "fail"])
                if !ok { allOk = false }
            } else if gt == "file_exists" {
                let rel = (g["path"] as? String) ?? ""
                let ok = fm.fileExists(atPath: workdir.appendingPathComponent(rel).path)
                checks.append(["exists:\(rel)", ok ? "pass" : "fail"])
                if !ok { allOk = false }
            }
        }
        return (allOk, checks)
    }
}
