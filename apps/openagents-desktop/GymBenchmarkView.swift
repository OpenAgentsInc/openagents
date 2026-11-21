// Firm reason: 6 (AppKit live Psionic Metal vs llama.cpp coding benchmark workbench view inside Gym).
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
    var toolCalls: [[String: Any]]
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
        layer?.cornerRadius = 6
        layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.025).cgColor
        setup()
    }

    required init?(coder: NSCoder) { fatalError() }

    private func setup() {
        taskLabel.translatesAutoresizingMaskIntoConstraints = false
        taskLabel.stringValue = spec.id
        taskLabel.font = NSFont.monospacedSystemFont(ofSize: 11.5, weight: .medium)
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
            heightAnchor.constraint(equalToConstant: 36),

            taskLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            taskLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            taskLabel.widthAnchor.constraint(equalToConstant: 176),

            langBadge.leadingAnchor.constraint(equalTo: taskLabel.trailingAnchor, constant: 4),
            langBadge.centerYAnchor.constraint(equalTo: centerYAnchor),
            langBadge.widthAnchor.constraint(equalToConstant: 56),

            psiBarTrack.leadingAnchor.constraint(equalTo: langBadge.trailingAnchor, constant: 12),
            psiBarTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            psiBarTrack.heightAnchor.constraint(equalToConstant: 10),
            psiBarTrack.widthAnchor.constraint(equalToConstant: 110),

            psiBarFill.leadingAnchor.constraint(equalTo: psiBarTrack.leadingAnchor),
            psiBarFill.topAnchor.constraint(equalTo: psiBarTrack.topAnchor),
            psiBarFill.bottomAnchor.constraint(equalTo: psiBarTrack.bottomAnchor),
            psiBarWidth,

            psiValueLabel.leadingAnchor.constraint(equalTo: psiBarTrack.trailingAnchor, constant: 8),
            psiValueLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            psiValueLabel.widthAnchor.constraint(equalToConstant: 150),

            llamaBarTrack.leadingAnchor.constraint(equalTo: psiValueLabel.trailingAnchor, constant: 8),
            llamaBarTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            llamaBarTrack.heightAnchor.constraint(equalToConstant: 10),
            llamaBarTrack.widthAnchor.constraint(equalToConstant: 110),

            llamaBarFill.leadingAnchor.constraint(equalTo: llamaBarTrack.leadingAnchor),
            llamaBarFill.topAnchor.constraint(equalTo: llamaBarTrack.topAnchor),
            llamaBarFill.bottomAnchor.constraint(equalTo: llamaBarTrack.bottomAnchor),
            llamaBarWidth,

            llamaValueLabel.leadingAnchor.constraint(equalTo: llamaBarTrack.trailingAnchor, constant: 8),
            llamaValueLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            llamaValueLabel.widthAnchor.constraint(equalToConstant: 150),

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
        let trackW: CGFloat = 110
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
        titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        titleLabel.textColor = accent
        addSubview(titleLabel)

        subtitleLabel.translatesAutoresizingMaskIntoConstraints = false
        subtitleLabel.stringValue = subtitle
        subtitleLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .regular)
        subtitleLabel.textColor = NSColor(white: 0.58, alpha: 1)
        addSubview(subtitleLabel)

        passLabel.translatesAutoresizingMaskIntoConstraints = false
        passLabel.font = NSFont.monospacedSystemFont(ofSize: 15, weight: .bold)
        passLabel.textColor = NSColor(white: 0.95, alpha: 1)
        addSubview(passLabel)

        wallLabel.translatesAutoresizingMaskIntoConstraints = false
        wallLabel.font = NSFont.monospacedSystemFont(ofSize: 18, weight: .bold)
        wallLabel.textColor = NSColor(white: 0.98, alpha: 1)
        wallLabel.alignment = .right
        addSubview(wallLabel)

        metricsLabel.translatesAutoresizingMaskIntoConstraints = false
        metricsLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
        metricsLabel.textColor = NSColor(white: 0.72, alpha: 1)
        metricsLabel.lineBreakMode = .byTruncatingTail
        addSubview(metricsLabel)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 82),

            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            subtitleLabel.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            subtitleLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),

            wallLabel.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            wallLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),

            passLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 4),
            passLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            metricsLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            metricsLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            metricsLabel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -9),
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
    private let deltaBanner = NSTextField(labelWithString: "Click Run Benchmark to execute live side-by-side coding evals on this Mac.")

    private let colHeaderLabel = NSTextField(labelWithString: "TASK                        TYPE     PSIONIC METAL (Coax → Metal 4)                  LLAMA.CPP METAL (--jinja)                       DELTA")
    private let rowsScroll = NSScrollView()
    private let rowsStack = NSStackView()
    private var rowViews: [String: GymBenchTaskRowView] = [:]
    private var activeSpecs: [GymBenchTaskSpec] = allTenTasks

    private let streamHeader = NSTextField(labelWithString: "Live Agent Stream & Deterministic Checks")
    private let streamScroll = NSScrollView()
    private let streamTextView = NSTextView()

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
            "10-Task Real Coding Suite (Tier 2 + Tier 4)",
            "Tier 2 Quick Suite (3 Python Tasks)",
            "Tier 4 Multi-Language Suite (7 Tasks: Py / C / JS / Bash)",
        ])
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
        phaseLabel.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .medium)
        phaseLabel.textColor = NSColor(white: 0.68, alpha: 1)
        phaseLabel.lineBreakMode = .byTruncatingTail
        addSubview(phaseLabel)

        psiCard.translatesAutoresizingMaskIntoConstraints = false
        llamaCard.translatesAutoresizingMaskIntoConstraints = false
        addSubview(psiCard)
        addSubview(llamaCard)

        deltaBanner.translatesAutoresizingMaskIntoConstraints = false
        deltaBanner.font = NSFont.monospacedSystemFont(ofSize: 11.5, weight: .semibold)
        deltaBanner.textColor = NSColor(calibratedRed: 0.72, green: 0.90, blue: 0.84, alpha: 1)
        deltaBanner.alignment = .center
        deltaBanner.wantsLayer = true
        deltaBanner.layer?.cornerRadius = 6
        deltaBanner.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.04).cgColor
        addSubview(deltaBanner)

        colHeaderLabel.translatesAutoresizingMaskIntoConstraints = false
        colHeaderLabel.font = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold)
        colHeaderLabel.textColor = NSColor(white: 0.48, alpha: 1)
        addSubview(colHeaderLabel)

        rowsStack.orientation = .vertical
        rowsStack.alignment = .leading
        rowsStack.spacing = 4
        rowsStack.edgeInsets = NSEdgeInsets(top: 4, left: 0, bottom: 4, right: 0)
        rowsStack.translatesAutoresizingMaskIntoConstraints = false

        let rowsDoc = NSView()
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

        streamHeader.translatesAutoresizingMaskIntoConstraints = false
        streamHeader.font = NSFont.systemFont(ofSize: 11.5, weight: .semibold)
        streamHeader.textColor = NSColor(white: 0.82, alpha: 1)
        addSubview(streamHeader)

        streamTextView.isEditable = false
        streamTextView.isSelectable = true
        streamTextView.drawsBackground = true
        streamTextView.backgroundColor = NSColor(red: 0.04, green: 0.045, blue: 0.058, alpha: 1)
        streamTextView.textColor = NSColor(white: 0.82, alpha: 1)
        streamTextView.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
        streamTextView.textContainerInset = NSSize(width: 8, height: 6)

        streamScroll.translatesAutoresizingMaskIntoConstraints = false
        streamScroll.hasVerticalScroller = true
        streamScroll.autohidesScrollers = true
        streamScroll.borderType = .noBorder
        streamScroll.wantsLayer = true
        streamScroll.layer?.cornerRadius = 6
        streamScroll.layer?.borderWidth = 1
        streamScroll.layer?.borderColor = NSColor(white: 1.0, alpha: 0.08).cgColor
        streamScroll.documentView = streamTextView
        addSubview(streamScroll)

        NSLayoutConstraint.activate([
            suitePopup.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            suitePopup.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            suitePopup.widthAnchor.constraint(equalToConstant: 320),

            runButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            runButton.leadingAnchor.constraint(equalTo: suitePopup.trailingAnchor, constant: 10),

            stopButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            stopButton.leadingAnchor.constraint(equalTo: runButton.trailingAnchor, constant: 6),

            loadSavedButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            loadSavedButton.leadingAnchor.constraint(equalTo: stopButton.trailingAnchor, constant: 6),

            phaseLabel.topAnchor.constraint(equalTo: suitePopup.bottomAnchor, constant: 8),
            phaseLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            phaseLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),

            psiCard.topAnchor.constraint(equalTo: phaseLabel.bottomAnchor, constant: 10),
            psiCard.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            psiCard.trailingAnchor.constraint(equalTo: centerXAnchor, constant: -6),

            llamaCard.topAnchor.constraint(equalTo: phaseLabel.bottomAnchor, constant: 10),
            llamaCard.leadingAnchor.constraint(equalTo: centerXAnchor, constant: 6),
            llamaCard.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),

            deltaBanner.topAnchor.constraint(equalTo: psiCard.bottomAnchor, constant: 8),
            deltaBanner.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            deltaBanner.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
            deltaBanner.heightAnchor.constraint(equalToConstant: 26),

            colHeaderLabel.topAnchor.constraint(equalTo: deltaBanner.bottomAnchor, constant: 12),
            colHeaderLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 26),
            colHeaderLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),

            rowsScroll.topAnchor.constraint(equalTo: colHeaderLabel.bottomAnchor, constant: 4),
            rowsScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            rowsScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
            rowsScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 220),
            rowsDoc.widthAnchor.constraint(equalTo: rowsScroll.widthAnchor),

            streamHeader.topAnchor.constraint(equalTo: rowsScroll.bottomAnchor, constant: 10),
            streamHeader.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),

            streamScroll.topAnchor.constraint(equalTo: streamHeader.bottomAnchor, constant: 6),
            streamScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            streamScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
            streamScroll.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -14),
            streamScroll.heightAnchor.constraint(equalToConstant: 175),
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
        if !isRunning {
            var lines: [String] = ["=== Task Details: \(id) ==="]
            if let p = psiResults[id] {
                let ttftStr = p.ttfts.map { String(format: "%.2fs", $0) }.joined(separator: ", ")
                let chkStr = p.checks.map { "\($0[1] == "pass" ? "✓" : "✗") \($0[0])" }.joined(separator: "  ")
                lines.append(String(format: "Psionic Metal   : %@  %.2fs  %d turns  %d tok  %.1f tok/s  TTFTs=[%@]", p.passed ? "PASS" : "FAIL", p.wall_s, p.turns, p.toks, p.decode_tok_s, ttftStr))
                lines.append("  Checks        : \(chkStr)")
            }
            if let l = llamaResults[id] {
                let ttftStr = l.ttfts.map { String(format: "%.2fs", $0) }.joined(separator: ", ")
                let chkStr = l.checks.map { "\($0[1] == "pass" ? "✓" : "✗") \($0[0])" }.joined(separator: "  ")
                lines.append(String(format: "llama.cpp Metal : %@  %.2fs  %d turns  %d tok  %.1f tok/s  TTFTs=[%@]", l.passed ? "PASS" : "FAIL", l.wall_s, l.turns, l.toks, l.decode_tok_s, ttftStr))
                lines.append("  Checks        : \(chkStr)")
            }
            streamTextView.string = lines.joined(separator: "\n") + "\n"
        }
    }

    @objc private func suiteChanged() {
        guard !isRunning else { return }
        switch suitePopup.indexOfSelectedItem {
        case 1: activeSpecs = Self.allTenTasks.filter { $0.tier == "tier2" }
        case 2: activeSpecs = Self.allTenTasks.filter { $0.tier == "tier4" }
        default: activeSpecs = Self.allTenTasks
        }
        psiResults.removeAll()
        llamaResults.removeAll()
        rebuildTaskRows()
        psiCard.reset(total: activeSpecs.count)
        llamaCard.reset(total: activeSpecs.count)
        deltaBanner.stringValue = "Suite selected: \(activeSpecs.count) real coding eval tasks. Click Run Benchmark to begin."
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
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            let cur = self.streamTextView.string
            let next = cur.count > 48000 ? String(cur.suffix(32000)) + line + "\n" : cur + line + "\n"
            self.streamTextView.string = next
            self.streamTextView.scrollToEndOfDocument(nil)
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

    private func loadSavedIfPresent(silent: Bool) {
        let url = Self.savedRunURL
        guard let data = try? Data(contentsOf: url),
              let saved = try? JSONDecoder().decode(GymBenchSavedRun.self, from: data) else {
            if !silent {
                appendLog("[info] No saved benchmark found at \(url.path).")
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
        phaseLabel.stringValue = "Loaded saved run (\(saved.timestamp)) — click any row for details or Run Benchmark for a fresh live run."
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

    // MARK: - Live Benchmark Execution Engine (Native Swift)

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

    private func startBenchmarkRun() {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        let modelPath = "\(home)/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf"
        let llamaBin = "\(home)/.openagents/bin/llama-server"

        guard fm.fileExists(atPath: modelPath) else {
            appendLog("[error] Missing model checkpoint: \(modelPath)")
            return
        }
        guard let psi = resolvePsionicBinaries() else {
            appendLog("[error] Could not locate psionic-inkling-serve + inkling-kernels.metal.")
            return
        }
        guard fm.isExecutableFile(atPath: llamaBin) else {
            appendLog("[error] Missing llama-server at \(llamaBin)")
            return
        }
        guard let evalRoot = resolveEvalRoot() else {
            appendLog("[error] Could not locate evals/openagents-desktop/ in workspace.")
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
        streamTextView.string = ""
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
                    self.phaseLabel.stringValue = "Benchmark complete! Saved to ~/.openagents/gym/last-benchmark.json."
                    self.appendLog("[done] Head-to-head benchmark finished and saved.")
                }
                self.setRunningUI(false)
            }
        }
    }

    private func runSide(side: GymBenchSide, executable: String, args: [String], port: Int, evalRoot: URL, specs: [GymBenchTaskSpec]) {
        DispatchQueue.main.async {
            self.phaseLabel.stringValue = "Booting \(side.rawValue)…"
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
        _ = streamTurn(port: port, messages: [["role": "user", "content": "hi"]], maxTokens: 8, onDelta: nil)

        for (idx, spec) in specs.enumerated() {
            if cancelFlag { break }
            DispatchQueue.main.async {
                self.phaseLabel.stringValue = "Running \(side.rawValue) [\(idx + 1)/\(specs.count)] \(spec.id)…"
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
        let tierDir = evalRoot.appendingPathComponent(spec.tier)
        let taskFile = tierDir.appendingPathComponent("tasks/\(spec.id).json")
        guard let taskData = try? Data(contentsOf: taskFile),
              let taskObj = try? JSONSerialization.jsonObject(with: taskData) as? [String: Any],
              let fixtureName = taskObj["fixture"] as? String,
              let steps = taskObj["steps"] as? [[String: Any]],
              let prompt = steps.first?["send"] as? String else {
            appendLog("[error] Failed to load task spec \(taskFile.path)")
            return GymBenchTaskResult(task: spec.id, tier: spec.tier, passed: false, wall_s: 0, turns: 0, toks: 0, decode_tok_s: 0, ttfts: [], checks: [["load_spec", "fail"]])
        }

        let fixDir = tierDir.appendingPathComponent("fixtures/\(fixtureName)")
        let tmpDir = FileManager.default.temporaryDirectory.appendingPathComponent("oa-gym-\(spec.id)-\(UUID().uuidString.prefix(8))")
        defer { try? FileManager.default.removeItem(at: tmpDir) }
        try? FileManager.default.copyItem(at: fixDir, to: tmpDir)

        let files = listRelativeFiles(in: tmpDir)
        let sysPrompt = "You are OpenAgents, a coding agent working in a local repository. Read the relevant files first, make the required edits without modifying test files, run the test command with Bash to verify everything passes, then give a concise summary. Keep thinking brief.\n\nWorkspace files: " + files.joined(separator: ", ")

        var messages: [[String: Any]] = [
            ["role": "system", "content": sysPrompt],
            ["role": "user", "content": prompt],
        ]

        appendLog("[\(side.rawValue)] ▶ \(spec.id) (\(spec.langBadge))")
        let t0 = CFAbsoluteTimeGetCurrent()
        var ttfts: [Double] = []
        var totalToks = 0
        var totalDecodeS = 0.0
        var turns = 0

        for turnIdx in 1...8 {
            if cancelFlag { break }
            turns = turnIdx
            DispatchQueue.main.async {
                self.rowViews[spec.id]?.update(side: side, state: .running(turn: turnIdx, elapsed: CFAbsoluteTimeGetCurrent() - t0, tokRate: 0, activeTool: ""), scaleMaxSeconds: 25.0)
            }
            var streamedChars = 0
            let turnStart = CFAbsoluteTimeGetCurrent()
            let tr = streamTurn(port: port, messages: messages, maxTokens: 1024) { [weak self] chunkChars, elapsedTurn in
                streamedChars += chunkChars
                let approxTok = Double(streamedChars) / 4.0
                let rate = elapsedTurn > 0.2 ? approxTok / elapsedTurn : 0
                DispatchQueue.main.async {
                    self?.rowViews[spec.id]?.update(side: side, state: .running(turn: turnIdx, elapsed: CFAbsoluteTimeGetCurrent() - t0, tokRate: rate, activeTool: ""), scaleMaxSeconds: 25.0)
                }
            }
            let rssNow = pollRssMiB(pid: pid)
            DispatchQueue.main.async {
                if side == .psionic { self.psiPeakRss = max(self.psiPeakRss, rssNow) }
                else { self.llamaPeakRss = max(self.llamaPeakRss, rssNow) }
                self.refreshScoreboards()
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
                    self.rowViews[spec.id]?.update(side: side, state: .running(turn: turnIdx, elapsed: CFAbsoluteTimeGetCurrent() - t0, tokRate: turnTokRate, activeTool: fnName), scaleMaxSeconds: 25.0)
                }
                let toolOut = execTool(workdir: tmpDir, name: fnName, args: argObj)
                let preview = (argObj["path"] as? String) ?? (argObj["command"] as? String) ?? ""
                appendLog("    ↳ \(fnName)(\(preview.prefix(64)))")
                messages.append(["role": "tool", "tool_call_id": tcId, "content": toolOut])
            }
            _ = turnStart
        }

        let wallS = CFAbsoluteTimeGetCurrent() - t0
        let (passed, checks) = gradeTask(taskObj: taskObj, fixDir: fixDir, workdir: tmpDir)
        let decodeTokS = totalDecodeS > 0 ? Double(totalToks) / totalDecodeS : 0
        let chkSummary = checks.map { "\($0[1] == "pass" ? "✓" : "✗") \($0[0])" }.joined(separator: "  ")
        appendLog(String(format: "  ⇒ %@ in %.2fs (%d turns, %d tok, %.1f tok/s)  %@", passed ? "PASS" : "FAIL", wallS, turns, totalToks, decodeTokS, chkSummary))

        return GymBenchTaskResult(
            task: spec.id,
            tier: spec.tier,
            passed: passed,
            wall_s: (wallS * 100).rounded() / 100,
            turns: turns,
            toks: totalToks,
            decode_tok_s: (decodeTokS * 10).rounded() / 10,
            ttfts: ttfts.map { ($0 * 1000).rounded() / 1000 },
            checks: checks
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

    private func streamTurn(port: Int, messages: [[String: Any]], maxTokens: Int, onDelta: ((Int, Double) -> Void)?) -> GymBenchStreamTurn {
        let url = URL(string: "http://127.0.0.1:\(port)/v1/chat/completions")!
        let payload: [String: Any] = [
            "model": "inkling-2-mini",
            "messages": messages,
            "tools": Self.toolsSchema,
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
        var result = GymBenchStreamTurn(ttft: 0, dur: 0, promptTokens: 0, completionTokens: 0, content: "", toolCalls: [])

        final class SSEDelegate: NSObject, URLSessionDataDelegate {
            let t0: Double
            let onDelta: ((Int, Double) -> Void)?
            let done: (GymBenchStreamTurn) -> Void
            var buf = Data()
            var ttft: Double?
            var content = ""
            var promptTokens = 0
            var completionTokens = 0
            var tcMap: [Int: (id: String, name: String, args: String)] = [:]
            var lastUI = 0.0

            init(t0: Double, onDelta: ((Int, Double) -> Void)?, done: @escaping (GymBenchStreamTurn) -> Void) {
                self.t0 = t0; self.onDelta = onDelta; self.done = done
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
                    if !ct.isEmpty { content += ct }
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
                    if now - lastUI > 0.12 {
                        lastUI = now
                        onDelta?(ct.count + rc.count, now - (t0 + (ttft ?? 0)))
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
                    toolCalls: tcs
                ))
            }
        }

        let delegate = SSEDelegate(t0: t0, onDelta: onDelta) { tr in
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

    private func execTool(workdir: URL, name: String, args: [String: Any]) -> String {
        let fm = FileManager.default
        if name == "Read" {
            let rel = ((args["path"] as? String) ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let full = workdir.appendingPathComponent(rel)
            return (try? String(contentsOf: full, encoding: .utf8)) ?? "(file not found: \(rel))"
        } else if name == "Write" {
            let rel = ((args["path"] as? String) ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let content = (args["content"] as? String) ?? ""
            let full = workdir.appendingPathComponent(rel)
            try? fm.createDirectory(at: full.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? Data(content.utf8).write(to: full)
            return "Wrote \(rel) (\(content.utf8.count) bytes)."
        } else if name == "Bash" {
            let cmd = (args["command"] as? String) ?? ""
            let (code, out) = runShell(cmd, cwd: workdir)
            return "exit=\(code)\n\(out.trimmingCharacters(in: .whitespacesAndNewlines))"
        }
        return "(unknown tool \(name))"
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

    private func gradeTask(taskObj: [String: Any], fixDir: URL, workdir: URL) -> (Bool, [[String]]) {
        let fm = FileManager.default
        let graders = (taskObj["graders"] as? [[String: Any]]) ?? []
        var checks: [[String]] = []
        var allOk = true

        for g in graders {
            let gt = (g["type"] as? String) ?? ""
            if gt == "files_unchanged" {
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
