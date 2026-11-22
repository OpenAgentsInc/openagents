// Firm reason: 6 (AppKit host for side-by-side Terminal-Bench Harbor Docker & Microcoder Executor-Network Hillclimbing workbench using embedded Coax GPU chat views; all benchmark, policy, scoring, and hillclimbing logic lives in Coax coder/tui/tbench-run.coax & coax/lib/tbench/).
import AppKit
import Foundation

// MARK: - Wire Snapshots (decoded from tbench-run outputs; single writer is coax/lib/tbench/wire.coax)

private struct TbTrialRow: Codable {
    var task: String
    var dataset: String
    var partition: String
    var arm: String
    var status: String
    var passed: Bool
    var reward_milli: Int
    var wall_ms: Int
    var turns: Int
    var prompt_tokens: Int
    var completion_tokens: Int
    var cost_micros: Int
    var exec_checks: Int
    var exec_intercepts: Int
    var exec_replays: Int
    var atif_verified: Bool
    var atif_digest: String
    var atif_path: String
    var events_path: String
    var harbor_dir: String
    var error_note: String
}

private struct TbArmReport: Codable {
    var id: String
    var label: String
    var backend: String
    var model: String
    var policy_sha: String
    var executor_guard: Bool
    var boot_ms: Int
    var peak_rss_mb: Int
    var total: Int
    var graded: Int
    var passed: Int
    var failed: Int
    var infra_errors: Int
    var timeouts: Int
    var train_total: Int
    var train_passed: Int
    var heldout_total: Int
    var heldout_passed: Int
    var wilson_low_bps: Int
    var wilson_high_bps: Int
    var total_wall_ms: Int
    var median_wall_ms: Int
    var total_tokens: Int
    var cost_micros: Int
    var exec_checks: Int
    var exec_intercepts: Int
    var exec_replays: Int
    var atif_all_verified: Bool
}

private struct TbCompareSnapshot: Codable {
    var run_id: String
    var timestamp: String
    var suite: String
    var dataset: String
    var arm_a: TbArmReport
    var arm_b: TbArmReport
    var rows_a: [TbTrialRow]
    var rows_b: [TbTrialRow]
}

private struct TbHillclimbEntry: Codable {
    var round: Int
    var timestamp: String
    var suite: String
    var dataset: String
    var champion_id: String
    var champion_sha: String
    var challenger_id: String
    var challenger_sha: String
    var mutation_note: String
    var train_total: Int
    var heldout_total: Int
    var champ_train_pass: Int
    var cand_train_pass: Int
    var champ_heldout_pass: Int
    var cand_heldout_pass: Int
    var cand_infra_errors: Int
    var champ_wall_ms: Int
    var cand_wall_ms: Int
    var champ_tokens: Int
    var cand_tokens: Int
    var verdict: String
    var reason: String
    var distilled_train_atif: Int
    var run_dir: String
}

private struct TbDoctorReport: Codable {
    var ok: Bool
    var docker_ok: Bool
    var docker_host: String
    var docker_version: String
    var harbor_ok: Bool
    var harbor_version: String
    var adapter_ok: Bool
    var psionic_ok: Bool
    var llama_ok: Bool
    var agy_ok: Bool?
    var model_ok: Bool
    var exec_modules: Int
    var openrouter_key_set: Bool
}

private struct TbSuiteTaskItem: Codable {
    var id: String
    var partition: String
}

private struct TbSuiteCatalogEntry: Codable {
    var slug: String
    var title: String
    var dataset: String
    var count: Int
    var train_count: Int
    var heldout_count: Int
    var tasks: [TbSuiteTaskItem]
}

private struct TbSuitesResponse: Codable {
    var suites: [TbSuiteCatalogEntry]
}

private struct TbLiveEvent: Codable {
    var kind: String
    var side: String
    var arm_id: String
    var task: String
    var idx: Int
    var total: Int
    var status: String
    var wall_ms: Int
    var turns: Int
    var exec_checks: Int
    var exec_intercepts: Int
    var events_path: String
    var note: String
}

private enum TbSide {
    case armA
    case armB
}

private enum TbRowStatus {
    case pending
    case running(note: String)
    case done(status: String, wallMs: Int, turns: Int, checks: Int, intercepts: Int)
}

private final class TbFlippedView: NSView {
    override var isFlipped: Bool { true }
}

// MARK: - Terminal-Bench Scorecard

private final class TbScoreCardView: NSView {
    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private let passLabel = NSTextField(labelWithString: "— / —")
    private let wallLabel = NSTextField(labelWithString: "0.0 s")
    private let line1Label = NSTextField(labelWithString: "Wilson 95% CI —  •  Train —  •  Held-out —  •  Infra 0")
    private let line2Label = NSTextField(labelWithString: "RSS —  •  Boot —  •  Tokens 0  •  Class-E Guards —  •  ATIF —")

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
        subtitleLabel.lineBreakMode = .byTruncatingTail
        addSubview(subtitleLabel)

        passLabel.translatesAutoresizingMaskIntoConstraints = false
        passLabel.font = NSFont.monospacedSystemFont(ofSize: 13.5, weight: .bold)
        passLabel.textColor = NSColor(white: 0.96, alpha: 1)
        addSubview(passLabel)

        wallLabel.translatesAutoresizingMaskIntoConstraints = false
        wallLabel.font = NSFont.monospacedSystemFont(ofSize: 16, weight: .bold)
        wallLabel.textColor = NSColor(white: 0.98, alpha: 1)
        wallLabel.alignment = .right
        addSubview(wallLabel)

        for lbl in [line1Label, line2Label] {
            lbl.translatesAutoresizingMaskIntoConstraints = false
            lbl.font = NSFont.monospacedSystemFont(ofSize: 9.8, weight: .regular)
            lbl.textColor = NSColor(white: 0.72, alpha: 1)
            lbl.lineBreakMode = .byTruncatingTail
            addSubview(lbl)
        }

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 84),
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 7),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            subtitleLabel.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            subtitleLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),
            subtitleLabel.trailingAnchor.constraint(lessThanOrEqualTo: wallLabel.leadingAnchor, constant: -8),

            wallLabel.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            wallLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),

            passLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 2),
            passLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            line1Label.topAnchor.constraint(equalTo: passLabel.bottomAnchor, constant: 3),
            line1Label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            line1Label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),

            line2Label.topAnchor.constraint(equalTo: line1Label.bottomAnchor, constant: 2),
            line2Label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            line2Label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    func setHeader(title: String, subtitle: String) {
        titleLabel.stringValue = title
        subtitleLabel.stringValue = subtitle
    }

    func update(report: TbArmReport) {
        titleLabel.stringValue = report.label.isEmpty ? report.id : report.label
        let shaShort = String(report.policy_sha.prefix(8))
        subtitleLabel.stringValue = "\(report.backend) · \(report.model) · #\(shaShort)"
        let pct = report.graded > 0 ? (Double(report.passed) * 100.0 / Double(report.graded)) : 0.0
        passLabel.stringValue = String(format: "%d / %d PASS (%.1f%%)", report.passed, report.graded, pct)
        wallLabel.stringValue = String(format: "%.1f s", Double(report.total_wall_ms) / 1000.0)
        let wLo = Double(report.wilson_low_bps) / 100.0
        let wHi = Double(report.wilson_high_bps) / 100.0
        line1Label.stringValue = String(
            format: "Wilson 95%% [%.1f..%.1f%%]  •  Train %d/%d  •  Held-out %d/%d  •  Infra %d",
            wLo, wHi, report.train_passed, report.train_total, report.heldout_passed, report.heldout_total, report.infra_errors
        )
        let rssStr = report.peak_rss_mb > 0 ? String(format: "%.1f GiB", Double(report.peak_rss_mb) / 1024.0) : "—"
        let bootStr = report.boot_ms > 0 ? String(format: "%.2fs", Double(report.boot_ms) / 1000.0) : "—"
        let guardStr = report.executor_guard
            ? "W2.2 \(report.exec_checks) chk (\(report.exec_intercepts) caught, \(report.exec_replays) replay)"
            : "off"
        let atifStr = report.atif_all_verified ? "ATIF-v1.8 ✓" : "ATIF —"
        line2Label.stringValue = "RSS \(rssStr)  •  Boot \(bootStr)  •  \(report.total_tokens) tok  •  \(guardStr)  •  \(atifStr)"
    }

    func reset(total: Int) {
        passLabel.stringValue = "— / \(total)"
        wallLabel.stringValue = "0.0 s"
        line1Label.stringValue = "Wilson 95% CI —  •  Train —  •  Held-out —  •  Infra 0"
        line2Label.stringValue = "RSS —  •  Boot —  •  Tokens 0  •  Class-E Guards —  •  ATIF —"
    }
}

// MARK: - Per-Task Race Row View

private final class TbTaskRowView: NSView {
    let taskId: String
    let partition: String
    var onSelect: (() -> Void)?

    private let taskLabel = NSTextField(labelWithString: "")
    private let partBadge = NSTextField(labelWithString: "")
    private let aTrack = NSView()
    private let aFill = NSView()
    private var aFillW: NSLayoutConstraint!
    private let aStat = NSTextField(labelWithString: "waiting")
    private let bTrack = NSView()
    private let bFill = NSView()
    private var bFillW: NSLayoutConstraint!
    private let bStat = NSTextField(labelWithString: "waiting")
    private let deltaLabel = NSTextField(labelWithString: "")
    private var selected = false

    private var aState: TbRowStatus = .pending
    private var bState: TbRowStatus = .pending

    init(taskId: String, partition: String) {
        self.taskId = taskId
        self.partition = partition
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 5

        taskLabel.translatesAutoresizingMaskIntoConstraints = false
        taskLabel.stringValue = taskId
        taskLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        taskLabel.textColor = NSColor(white: 0.90, alpha: 1)
        taskLabel.lineBreakMode = .byTruncatingTail
        addSubview(taskLabel)

        let isHo = partition == "heldout"
        partBadge.translatesAutoresizingMaskIntoConstraints = false
        partBadge.stringValue = isHo ? "HELDOUT" : "TRAIN"
        partBadge.font = NSFont.monospacedSystemFont(ofSize: 8.5, weight: .semibold)
        partBadge.textColor = isHo
            ? NSColor(calibratedRed: 0.96, green: 0.72, blue: 0.34, alpha: 1)
            : NSColor(calibratedRed: 0.45, green: 0.80, blue: 0.98, alpha: 1)
        partBadge.alignment = .center
        partBadge.wantsLayer = true
        partBadge.layer?.cornerRadius = 3
        partBadge.layer?.backgroundColor = (isHo
            ? NSColor(calibratedRed: 0.96, green: 0.72, blue: 0.34, alpha: 0.14)
            : NSColor(calibratedRed: 0.45, green: 0.80, blue: 0.98, alpha: 0.14)).cgColor
        addSubview(partBadge)

        for (track, fill) in [(aTrack, aFill), (bTrack, bFill)] {
            track.translatesAutoresizingMaskIntoConstraints = false
            track.wantsLayer = true
            track.layer?.cornerRadius = 3
            track.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.06).cgColor
            addSubview(track)

            fill.translatesAutoresizingMaskIntoConstraints = false
            fill.wantsLayer = true
            fill.layer?.cornerRadius = 3
            track.addSubview(fill)
        }

        for lbl in [aStat, bStat, deltaLabel] {
            lbl.translatesAutoresizingMaskIntoConstraints = false
            lbl.font = NSFont.monospacedSystemFont(ofSize: 9.8, weight: .regular)
            lbl.textColor = NSColor(white: 0.52, alpha: 1)
            lbl.lineBreakMode = .byTruncatingTail
            addSubview(lbl)
        }
        deltaLabel.alignment = .right
        deltaLabel.font = NSFont.monospacedSystemFont(ofSize: 9.8, weight: .semibold)

        aFillW = aFill.widthAnchor.constraint(equalToConstant: 0)
        bFillW = bFill.widthAnchor.constraint(equalToConstant: 0)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 24),

            taskLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            taskLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            taskLabel.widthAnchor.constraint(equalToConstant: 196),

            partBadge.centerYAnchor.constraint(equalTo: centerYAnchor),
            partBadge.leadingAnchor.constraint(equalTo: taskLabel.trailingAnchor, constant: 4),
            partBadge.widthAnchor.constraint(equalToConstant: 54),
            partBadge.heightAnchor.constraint(equalToConstant: 14),

            aTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            aTrack.leadingAnchor.constraint(equalTo: partBadge.trailingAnchor, constant: 10),
            aTrack.widthAnchor.constraint(equalToConstant: 76),
            aTrack.heightAnchor.constraint(equalToConstant: 8),

            aFill.leadingAnchor.constraint(equalTo: aTrack.leadingAnchor),
            aFill.topAnchor.constraint(equalTo: aTrack.topAnchor),
            aFill.bottomAnchor.constraint(equalTo: aTrack.bottomAnchor),
            aFillW,

            aStat.centerYAnchor.constraint(equalTo: centerYAnchor),
            aStat.leadingAnchor.constraint(equalTo: aTrack.trailingAnchor, constant: 6),
            aStat.widthAnchor.constraint(equalToConstant: 142),

            bTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            bTrack.leadingAnchor.constraint(equalTo: aStat.trailingAnchor, constant: 8),
            bTrack.widthAnchor.constraint(equalToConstant: 76),
            bTrack.heightAnchor.constraint(equalToConstant: 8),

            bFill.leadingAnchor.constraint(equalTo: bTrack.leadingAnchor),
            bFill.topAnchor.constraint(equalTo: bTrack.topAnchor),
            bFill.bottomAnchor.constraint(equalTo: bTrack.bottomAnchor),
            bFillW,

            bStat.centerYAnchor.constraint(equalTo: centerYAnchor),
            bStat.leadingAnchor.constraint(equalTo: bTrack.trailingAnchor, constant: 6),
            bStat.widthAnchor.constraint(equalToConstant: 142),

            deltaLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            deltaLabel.leadingAnchor.constraint(greaterThanOrEqualTo: bStat.trailingAnchor, constant: 6),
            deltaLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    func setSelected(_ on: Bool) {
        selected = on
        layer?.backgroundColor = on ? NSColor(calibratedRed: 0.22, green: 0.42, blue: 0.68, alpha: 0.28).cgColor : NSColor.clear.cgColor
    }

    func update(side: TbSide, state: TbRowStatus, maxWallMs: Int) {
        if side == .armA { aState = state } else { bState = state }
        let cap = Double(max(10_000, maxWallMs))
        apply(state: aState, fill: aFill, widthC: aFillW, label: aStat, isA: true, capMs: cap)
        apply(state: bState, fill: bFill, widthC: bFillW, label: bStat, isA: false, capMs: cap)

        if case .done(let sa, let wa, _, _, _) = aState, case .done(let sb, let wb, _, _, _) = bState {
            if sa == "PASS" && sb != "PASS" {
                deltaLabel.stringValue = "A ✓ only"
                deltaLabel.textColor = NSColor(calibratedRed: 0.30, green: 0.90, blue: 0.62, alpha: 1)
            } else if sb == "PASS" && sa != "PASS" {
                deltaLabel.stringValue = "B ✓ only"
                deltaLabel.textColor = NSColor(calibratedRed: 0.98, green: 0.72, blue: 0.32, alpha: 1)
            } else if wa > 0 && wb > 0 {
                let ratio = Double(wb) / Double(wa)
                deltaLabel.stringValue = String(format: "%.2fx", ratio)
                deltaLabel.textColor = ratio >= 1.0
                    ? NSColor(calibratedRed: 0.30, green: 0.90, blue: 0.62, alpha: 1)
                    : NSColor(calibratedRed: 0.98, green: 0.72, blue: 0.32, alpha: 1)
            } else {
                deltaLabel.stringValue = "tie"
                deltaLabel.textColor = NSColor(white: 0.56, alpha: 1)
            }
        } else {
            deltaLabel.stringValue = ""
        }
    }

    private func apply(state: TbRowStatus, fill: NSView, widthC: NSLayoutConstraint, label: NSTextField, isA: Bool, capMs: Double) {
        let trackW: CGFloat = 76
        switch state {
        case .pending:
            widthC.constant = 0
            label.stringValue = "waiting"
            label.textColor = NSColor(white: 0.42, alpha: 1)
        case .running(let note):
            widthC.constant = trackW * 0.35
            fill.layer?.backgroundColor = (isA
                ? NSColor(calibratedRed: 0.22, green: 0.88, blue: 0.64, alpha: 0.95)
                : NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 0.92)).cgColor
            label.stringValue = note.isEmpty ? "docker running…" : note
            label.textColor = isA
                ? NSColor(calibratedRed: 0.56, green: 0.98, blue: 0.78, alpha: 1)
                : NSColor(calibratedRed: 0.98, green: 0.82, blue: 0.50, alpha: 1)
        case .done(let status, let wallMs, let turns, let checks, let intercepts):
            let frac = CGFloat(min(1.0, max(0.08, Double(wallMs) / capMs)))
            widthC.constant = trackW * frac
            let sec = Double(wallMs) / 1000.0
            let gStr = checks > 0 ? " g:\(checks)/\(intercepts)" : ""
            label.stringValue = String(format: "%@ %.1fs (%dt)%@", status, sec, turns, gStr)
            if status == "PASS" {
                fill.layer?.backgroundColor = (isA
                    ? NSColor(calibratedRed: 0.18, green: 0.82, blue: 0.56, alpha: 0.92)
                    : NSColor(calibratedRed: 0.92, green: 0.62, blue: 0.24, alpha: 0.86)).cgColor
                label.textColor = NSColor(white: 0.92, alpha: 1)
            } else if status == "INFRA_ERROR" {
                fill.layer?.backgroundColor = NSColor(calibratedRed: 0.75, green: 0.48, blue: 0.92, alpha: 0.90).cgColor
                label.textColor = NSColor(calibratedRed: 0.86, green: 0.68, blue: 0.98, alpha: 1)
            } else {
                fill.layer?.backgroundColor = NSColor(calibratedRed: 0.90, green: 0.28, blue: 0.28, alpha: 0.90).cgColor
                label.textColor = NSColor(calibratedRed: 0.98, green: 0.45, blue: 0.45, alpha: 1)
            }
        }
    }

    override func mouseDown(with event: NSEvent) {
        onSelect?()
    }
}

// MARK: - Main GymTerminalBenchView

final class GymTerminalBenchView: NSView {
    var workingFolderProvider: (() -> URL?)?

    private static let armPresets: [(id: String, label: String)] = [
        ("microcoder-v2-executor", "Microcoder v2 · Psionic + Tassadar W2.2"),
        ("executor-hybrid-v1", "Psionic Metal + W2.2 Executor (v1)"),
        ("baseline-microcoder", "Psionic Metal (Base)"),
        ("llamacpp-microcoder", "llama.cpp Metal"),
        ("antigravity", "Antigravity CLI (agy)"),
        ("openrouter-claude", "OpenRouter · Claude Sonnet"),
    ]

    private static var homeURL: URL { FileManager.default.homeDirectoryForCurrentUser }
    private static var savedCompareURL: URL { homeURL.appendingPathComponent(".openagents/gym/last-tbench.json") }
    private static var liveStatusURL: URL { homeURL.appendingPathComponent(".openagents/tbench/live-status.jsonl") }
    private static var ledgerURL: URL { homeURL.appendingPathComponent(".openagents/tbench/hillclimb/ledger.jsonl") }
    private static var championURL: URL { homeURL.appendingPathComponent(".openagents/tbench/hillclimb/champion.json") }

    // Top controls
    private let headerBar = NSView()
    private let titleLabel = NSTextField(labelWithString: "Terminal-Bench · Coax + Harbor Docker")
    private let doctorPill = NSTextField(labelWithString: "Checking Docker / Harbor…")
    private let suitePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let armAPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let vsLabel = NSTextField(labelWithString: "vs")
    private let armBPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let filterField = NSTextField(string: "")
    private let runButton = NSButton(title: "Run Side-by-Side", target: nil, action: nil)
    private let hillclimbButton = NSButton(title: "Hillclimb Round", target: nil, action: nil)
    private let stopButton = NSButton(title: "Stop", target: nil, action: nil)
    private let loadLastButton = NSButton(title: "Load Last", target: nil, action: nil)

    // Scorecards + Delta / Hillclimb Ledger Banner
    private let cardA = TbScoreCardView(
        title: "Arm A · Psionic Metal + W2.2 Executor",
        subtitle: "psionic-metal · inkling-2-mini",
        accent: NSColor(calibratedRed: 0.24, green: 0.88, blue: 0.64, alpha: 1)
    )
    private let cardB = TbScoreCardView(
        title: "Arm B · llama.cpp Metal",
        subtitle: "llamacpp-metal · inkling-2-mini",
        accent: NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 1)
    )
    private let deltaBanner = NSTextField(labelWithString: "Select a suite (3, 12, 66, or 89 official Harbor Docker tasks) and run side-by-side or kick off an #authority hillclimb round.")
    private let hillclimbBanner = NSTextField(labelWithString: "Hillclimb Champion: baseline-microcoder  •  0 rounds recorded  •  Ch 247 train-only ATIF distillation ready")

    // Scrollable Task Race Table (handles 3..89 tasks cleanly)
    private let raceBox = NSView()
    private let raceScroll = NSScrollView()
    private let raceDocView = TbFlippedView()
    private let raceStack = NSStackView()
    private var rowViews: [String: TbTaskRowView] = [:]

    // Side-by-Side Real Embedded Coax GPU Chat Views (reused from GymBenchmarkView.swift!)
    private let splitChatContainer = NSView()
    private let chatA = GymChatSurfaceView(
        title: "Arm A · Microcoder + Executor",
        accent: NSColor(calibratedRed: 0.24, green: 0.88, blue: 0.64, alpha: 1)
    )
    private let chatB = GymChatSurfaceView(
        title: "Arm B · Baseline / llama.cpp",
        accent: NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 1)
    )

    // State
    private var suites: [TbSuiteCatalogEntry] = []
    private var lastSnapshot: TbCompareSnapshot?
    private var activeProcess: Process?
    private var pollTimer: Timer?
    private var liveCursor = 0
    private var activeEventPathA = ""
    private var activeEventCursorA = 0
    private var activeEventPathB = ""
    private var activeEventCursorB = 0
    private var liveRowsA: [String: TbTrialRow] = [:]
    private var liveRowsB: [String: TbTrialRow] = [:]
    private var hydratedOnce = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        setupUI()
        seedFallbackSuites()
    }

    required init?(coder: NSCoder) { fatalError() }

    deinit {
        pollTimer?.invalidate()
        stopActiveProcess()
    }

    func onBecomeVisible() {
        if !hydratedOnce {
            hydratedOnce = true
            refreshDoctor()
            refreshSuitesFromTool()
            loadSavedSnapshot()
            refreshHillclimbLedger()
        } else {
            refreshHillclimbLedger()
        }
    }

    // MARK: - UI Layout

    private func setupUI() {
        headerBar.translatesAutoresizingMaskIntoConstraints = false
        headerBar.wantsLayer = true
        headerBar.layer?.backgroundColor = NSColor(red: 0.072, green: 0.078, blue: 0.095, alpha: 1).cgColor
        addSubview(headerBar)

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 12.5, weight: .semibold)
        titleLabel.textColor = NSColor(white: 0.94, alpha: 1)
        headerBar.addSubview(titleLabel)

        doctorPill.translatesAutoresizingMaskIntoConstraints = false
        doctorPill.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .medium)
        doctorPill.textColor = NSColor(white: 0.72, alpha: 1)
        doctorPill.alignment = .right
        doctorPill.lineBreakMode = .byTruncatingTail
        headerBar.addSubview(doctorPill)

        // Second row of headerBar: suite + arm pickers + filter + buttons
        suitePopup.translatesAutoresizingMaskIntoConstraints = false
        suitePopup.controlSize = .small
        suitePopup.font = NSFont.systemFont(ofSize: 11)
        suitePopup.target = self
        suitePopup.action = #selector(suiteChanged)
        headerBar.addSubview(suitePopup)

        for (popup, defaultIdx) in [(armAPopup, 0), (armBPopup, 2)] {
            popup.translatesAutoresizingMaskIntoConstraints = false
            popup.controlSize = .small
            popup.font = NSFont.systemFont(ofSize: 11)
            for p in Self.armPresets { popup.addItem(withTitle: p.label) }
            popup.selectItem(at: defaultIdx)
            headerBar.addSubview(popup)
        }

        vsLabel.translatesAutoresizingMaskIntoConstraints = false
        vsLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .semibold)
        vsLabel.textColor = NSColor(white: 0.52, alpha: 1)
        headerBar.addSubview(vsLabel)

        filterField.translatesAutoresizingMaskIntoConstraints = false
        filterField.controlSize = .small
        filterField.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
        filterField.placeholderString = "Optional tasks (e.g. fix-git)"
        headerBar.addSubview(filterField)

        for (btn, sel) in [
            (runButton, #selector(onRunSideBySide)),
            (hillclimbButton, #selector(onRunHillclimb)),
            (stopButton, #selector(onStop)),
            (loadLastButton, #selector(onLoadLast)),
        ] {
            btn.translatesAutoresizingMaskIntoConstraints = false
            btn.bezelStyle = .rounded
            btn.controlSize = .small
            btn.target = self
            btn.action = sel
            headerBar.addSubview(btn)
        }
        runButton.keyEquivalent = ""
        stopButton.isEnabled = false

        cardA.translatesAutoresizingMaskIntoConstraints = false
        cardB.translatesAutoresizingMaskIntoConstraints = false
        addSubview(cardA)
        addSubview(cardB)

        deltaBanner.translatesAutoresizingMaskIntoConstraints = false
        deltaBanner.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .semibold)
        deltaBanner.textColor = NSColor(calibratedRed: 0.45, green: 0.92, blue: 0.72, alpha: 1)
        deltaBanner.lineBreakMode = .byTruncatingTail
        addSubview(deltaBanner)

        hillclimbBanner.translatesAutoresizingMaskIntoConstraints = false
        hillclimbBanner.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
        hillclimbBanner.textColor = NSColor(calibratedRed: 0.94, green: 0.78, blue: 0.46, alpha: 1)
        hillclimbBanner.lineBreakMode = .byTruncatingTail
        addSubview(hillclimbBanner)

        raceBox.translatesAutoresizingMaskIntoConstraints = false
        raceBox.wantsLayer = true
        raceBox.layer?.cornerRadius = 8
        raceBox.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.025).cgColor
        raceBox.layer?.borderWidth = 1
        raceBox.layer?.borderColor = NSColor(white: 1.0, alpha: 0.07).cgColor
        addSubview(raceBox)

        raceScroll.translatesAutoresizingMaskIntoConstraints = false
        raceScroll.drawsBackground = false
        raceScroll.hasVerticalScroller = true
        raceScroll.autohidesScrollers = true
        raceScroll.borderType = .noBorder
        raceBox.addSubview(raceScroll)

        raceDocView.translatesAutoresizingMaskIntoConstraints = false
        raceStack.translatesAutoresizingMaskIntoConstraints = false
        raceStack.orientation = .vertical
        raceStack.alignment = .leading
        raceStack.spacing = 2
        raceDocView.addSubview(raceStack)
        raceScroll.documentView = raceDocView

        splitChatContainer.translatesAutoresizingMaskIntoConstraints = false
        addSubview(splitChatContainer)
        chatA.translatesAutoresizingMaskIntoConstraints = false
        chatB.translatesAutoresizingMaskIntoConstraints = false
        splitChatContainer.addSubview(chatA)
        splitChatContainer.addSubview(chatB)

        NSLayoutConstraint.activate([
            headerBar.topAnchor.constraint(equalTo: topAnchor),
            headerBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerBar.heightAnchor.constraint(equalToConstant: 66),

            titleLabel.topAnchor.constraint(equalTo: headerBar.topAnchor, constant: 8),
            titleLabel.leadingAnchor.constraint(equalTo: headerBar.leadingAnchor, constant: 14),

            doctorPill.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            doctorPill.leadingAnchor.constraint(greaterThanOrEqualTo: titleLabel.trailingAnchor, constant: 12),
            doctorPill.trailingAnchor.constraint(equalTo: headerBar.trailingAnchor, constant: -14),

            suitePopup.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 8),
            suitePopup.leadingAnchor.constraint(equalTo: headerBar.leadingAnchor, constant: 14),
            suitePopup.widthAnchor.constraint(lessThanOrEqualToConstant: 215),

            armAPopup.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            armAPopup.leadingAnchor.constraint(equalTo: suitePopup.trailingAnchor, constant: 6),
            armAPopup.widthAnchor.constraint(lessThanOrEqualToConstant: 178),

            vsLabel.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            vsLabel.leadingAnchor.constraint(equalTo: armAPopup.trailingAnchor, constant: 5),

            armBPopup.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            armBPopup.leadingAnchor.constraint(equalTo: vsLabel.trailingAnchor, constant: 5),
            armBPopup.widthAnchor.constraint(lessThanOrEqualToConstant: 165),

            filterField.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            filterField.leadingAnchor.constraint(equalTo: armBPopup.trailingAnchor, constant: 8),
            filterField.widthAnchor.constraint(equalToConstant: 148),

            loadLastButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            loadLastButton.trailingAnchor.constraint(equalTo: headerBar.trailingAnchor, constant: -14),

            stopButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            stopButton.trailingAnchor.constraint(equalTo: loadLastButton.leadingAnchor, constant: -6),

            hillclimbButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            hillclimbButton.trailingAnchor.constraint(equalTo: stopButton.leadingAnchor, constant: -6),

            runButton.centerYAnchor.constraint(equalTo: suitePopup.centerYAnchor),
            runButton.trailingAnchor.constraint(equalTo: hillclimbButton.leadingAnchor, constant: -6),

            cardA.topAnchor.constraint(equalTo: headerBar.bottomAnchor, constant: 10),
            cardA.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            cardA.trailingAnchor.constraint(equalTo: centerXAnchor, constant: -5),

            cardB.topAnchor.constraint(equalTo: headerBar.bottomAnchor, constant: 10),
            cardB.leadingAnchor.constraint(equalTo: centerXAnchor, constant: 5),
            cardB.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),

            deltaBanner.topAnchor.constraint(equalTo: cardA.bottomAnchor, constant: 7),
            deltaBanner.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            deltaBanner.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),

            hillclimbBanner.topAnchor.constraint(equalTo: deltaBanner.bottomAnchor, constant: 3),
            hillclimbBanner.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            hillclimbBanner.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),

            raceBox.topAnchor.constraint(equalTo: hillclimbBanner.bottomAnchor, constant: 8),
            raceBox.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            raceBox.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            raceBox.heightAnchor.constraint(equalToConstant: 170),

            raceScroll.topAnchor.constraint(equalTo: raceBox.topAnchor, constant: 4),
            raceScroll.bottomAnchor.constraint(equalTo: raceBox.bottomAnchor, constant: -4),
            raceScroll.leadingAnchor.constraint(equalTo: raceBox.leadingAnchor, constant: 4),
            raceScroll.trailingAnchor.constraint(equalTo: raceBox.trailingAnchor, constant: -4),

            raceDocView.topAnchor.constraint(equalTo: raceScroll.contentView.topAnchor),
            raceDocView.leadingAnchor.constraint(equalTo: raceScroll.contentView.leadingAnchor),
            raceDocView.trailingAnchor.constraint(equalTo: raceScroll.contentView.trailingAnchor),

            raceStack.topAnchor.constraint(equalTo: raceDocView.topAnchor),
            raceStack.leadingAnchor.constraint(equalTo: raceDocView.leadingAnchor),
            raceStack.trailingAnchor.constraint(equalTo: raceDocView.trailingAnchor),
            raceStack.bottomAnchor.constraint(equalTo: raceDocView.bottomAnchor),

            splitChatContainer.topAnchor.constraint(equalTo: raceBox.bottomAnchor, constant: 10),
            splitChatContainer.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            splitChatContainer.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
            splitChatContainer.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),

            chatA.topAnchor.constraint(equalTo: splitChatContainer.topAnchor),
            chatA.bottomAnchor.constraint(equalTo: splitChatContainer.bottomAnchor),
            chatA.leadingAnchor.constraint(equalTo: splitChatContainer.leadingAnchor),
            chatA.trailingAnchor.constraint(equalTo: splitChatContainer.centerXAnchor, constant: -5),

            chatB.topAnchor.constraint(equalTo: splitChatContainer.topAnchor),
            chatB.bottomAnchor.constraint(equalTo: splitChatContainer.bottomAnchor),
            chatB.leadingAnchor.constraint(equalTo: splitChatContainer.centerXAnchor, constant: 5),
            chatB.trailingAnchor.constraint(equalTo: splitChatContainer.trailingAnchor),
        ])
    }

    // MARK: - Tool Discovery

    private func tbenchRunBinary() -> String? {
        let fm = FileManager.default
        if let bundled = Bundle.main.resourceURL?.appendingPathComponent("tbench-run").path,
           fm.isExecutableFile(atPath: bundled) {
            return bundled
        }
        let shared = Self.homeURL.appendingPathComponent(".openagents/bin/tbench-run").path
        if fm.isExecutableFile(atPath: shared) { return shared }
        return nil
    }

    private func augmentedEnv() -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let home = Self.homeURL.path
        let extra = "\(home)/.local/bin:/opt/homebrew/bin:/usr/local/bin"
        let cur = env["PATH"] ?? "/usr/bin:/bin"
        env["PATH"] = "\(extra):\(cur)"
        if env["DOCKER_HOST"] == nil {
            let colimaSock = "\(home)/.colima/oa-agent0/docker.sock"
            if FileManager.default.fileExists(atPath: colimaSock) {
                env["DOCKER_HOST"] = "unix://\(colimaSock)"
            }
        }
        return env
    }

    private func runToolSync(_ args: [String], done: @escaping (String?) -> Void) {
        guard let bin = tbenchRunBinary() else {
            DispatchQueue.main.async { done(nil) }
            return
        }
        DispatchQueue.global(qos: .userInitiated).async {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: bin)
            p.arguments = args
            p.environment = self.augmentedEnv()
            let out = Pipe()
            p.standardOutput = out
            p.standardError = Pipe()
            do {
                try p.run()
                let data = out.fileHandleForReading.readDataToEndOfFile()
                p.waitUntilExit()
                let text = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                DispatchQueue.main.async { done(text.isEmpty ? nil : text) }
            } catch {
                DispatchQueue.main.async { done(nil) }
            }
        }
    }

    // MARK: - Catalog & Doctor Hydration

    private func seedFallbackSuites() {
        suites = [
            TbSuiteCatalogEntry(
                slug: "tb2-smoke",
                title: "TB 2.0 Smoke (3 Tasks)",
                dataset: "terminal-bench@2.0",
                count: 3,
                train_count: 2,
                heldout_count: 1,
                tasks: [
                    .init(id: "fix-git", partition: "train"),
                    .init(id: "log-summary-date-ranges", partition: "train"),
                    .init(id: "regex-log", partition: "heldout"),
                ]
            ),
        ]
        rebuildSuiteMenu()
        rebuildRaceRows(tasks: suites[0].tasks)
    }

    private func refreshDoctor() {
        runToolSync(["doctor", "--json"]) { [weak self] out in
            guard let self else { return }
            guard let out, let data = out.data(using: .utf8),
                  let doc = try? JSONDecoder().decode(TbDoctorReport.self, from: data) else {
                self.doctorPill.stringValue = "tbench-run not found — rebuild app"
                self.doctorPill.textColor = NSColor(calibratedRed: 0.98, green: 0.45, blue: 0.45, alpha: 1)
                return
            }
            let dStr = doc.docker_ok ? "Docker \(doc.docker_version)" : "Docker OFF"
            let hStr = doc.harbor_ok ? "Harbor \(doc.harbor_version)" : "Harbor OFF"
            let pStr = doc.psionic_ok ? "Psionic ✓" : "Psionic —"
            let lStr = doc.llama_ok ? "llama.cpp ✓" : "llama.cpp —"
            let aStr = (doc.agy_ok ?? false) ? "agy ✓" : "agy —"
            let eStr = "W2.2 ×\(doc.exec_modules)"
            self.doctorPill.stringValue = "\(dStr)  •  \(hStr)  •  \(pStr)  •  \(lStr)  •  \(aStr)  •  \(eStr)"
            self.doctorPill.textColor = doc.ok
                ? NSColor(calibratedRed: 0.45, green: 0.92, blue: 0.72, alpha: 1)
                : NSColor(calibratedRed: 0.98, green: 0.65, blue: 0.35, alpha: 1)
        }
    }

    private func refreshSuitesFromTool() {
        runToolSync(["suites", "--json"]) { [weak self] out in
            guard let self, let out, let data = out.data(using: .utf8),
                  let resp = try? JSONDecoder().decode(TbSuitesResponse.self, from: data),
                  !resp.suites.isEmpty else { return }
            let curSlug = self.selectedSuite()?.slug ?? "tb2-smoke"
            self.suites = resp.suites
            self.rebuildSuiteMenu(selectSlug: curSlug)
            if self.lastSnapshot == nil, let s = self.selectedSuite() {
                self.rebuildRaceRows(tasks: s.tasks)
            }
        }
    }

    private func rebuildSuiteMenu(selectSlug: String? = nil) {
        suitePopup.removeAllItems()
        var targetIdx = 0
        for (i, s) in suites.enumerated() {
            suitePopup.addItem(withTitle: "\(s.slug) (\(s.count) · \(s.train_count)T/\(s.heldout_count)H)")
            if s.slug == selectSlug { targetIdx = i }
        }
        if !suites.isEmpty { suitePopup.selectItem(at: targetIdx) }
    }

    private func selectedSuite() -> TbSuiteCatalogEntry? {
        let idx = suitePopup.indexOfSelectedItem
        guard idx >= 0 && idx < suites.count else { return suites.first }
        return suites[idx]
    }

    @objc private func suiteChanged() {
        guard let s = selectedSuite() else { return }
        rebuildRaceRows(tasks: s.tasks)
        cardA.reset(total: s.count)
        cardB.reset(total: s.count)
    }

    private func rebuildRaceRows(tasks: [TbSuiteTaskItem]) {
        for v in raceStack.arrangedSubviews {
            raceStack.removeArrangedSubview(v)
            v.removeFromSuperview()
        }
        rowViews.removeAll()
        for t in tasks {
            let rv = TbTaskRowView(taskId: t.id, partition: t.partition)
            rv.translatesAutoresizingMaskIntoConstraints = false
            rv.onSelect = { [weak self] in
                self?.selectTask(t.id)
            }
            raceStack.addArrangedSubview(rv)
            rv.widthAnchor.constraint(equalTo: raceStack.widthAnchor).isActive = true
            rowViews[t.id] = rv
        }
    }

    // MARK: - Load Saved Snapshot & Hillclimb Ledger

    @objc private func onLoadLast() {
        loadSavedSnapshot()
        refreshHillclimbLedger()
    }

    private func loadSavedSnapshot() {
        guard let data = try? Data(contentsOf: Self.savedCompareURL),
              let snap = try? JSONDecoder().decode(TbCompareSnapshot.self, from: data) else {
            return
        }
        applySnapshot(snap)
    }

    private func applySnapshot(_ snap: TbCompareSnapshot) {
        lastSnapshot = snap
        liveRowsA.removeAll()
        liveRowsB.removeAll()
        for r in snap.rows_a { liveRowsA[r.task] = r }
        for r in snap.rows_b { liveRowsB[r.task] = r }

        // Select matching suite if known, else build rows directly from snapshot
        var taskItems: [TbSuiteTaskItem] = []
        for r in snap.rows_a { taskItems.append(.init(id: r.task, partition: r.partition)) }
        if !taskItems.isEmpty { rebuildRaceRows(tasks: taskItems) }

        let maxWall = max(
            10_000,
            (snap.rows_a.map(\.wall_ms) + snap.rows_b.map(\.wall_ms)).max() ?? 10_000
        )
        for r in snap.rows_a {
            rowViews[r.task]?.update(
                side: .armA,
                state: .done(status: r.status, wallMs: r.wall_ms, turns: r.turns, checks: r.exec_checks, intercepts: r.exec_intercepts),
                maxWallMs: maxWall
            )
        }
        for r in snap.rows_b {
            rowViews[r.task]?.update(
                side: .armB,
                state: .done(status: r.status, wallMs: r.wall_ms, turns: r.turns, checks: r.exec_checks, intercepts: r.exec_intercepts),
                maxWallMs: maxWall
            )
        }

        cardA.update(report: snap.arm_a)
        cardB.update(report: snap.arm_b)
        chatA.setTitle("Arm A · \(snap.arm_a.label)")
        chatB.setTitle("Arm B · \(snap.arm_b.label)")

        let diffPass = snap.arm_a.passed - snap.arm_b.passed
        let passWord = diffPass > 0 ? "Arm A +\(diffPass) tasks" : (diffPass < 0 ? "Arm B +\(-diffPass) tasks" : "Tied pass count")
        let wallRatio = snap.arm_a.total_wall_ms > 0 && snap.arm_b.total_wall_ms > 0
            ? String(format: "%.2fx wall", Double(snap.arm_b.total_wall_ms) / Double(snap.arm_a.total_wall_ms))
            : "—"
        deltaBanner.stringValue = "Run \(snap.run_id) (\(snap.suite) · \(snap.dataset))  —  \(passWord)  •  \(wallRatio)  •  Click any task row to replay both GPU chats."

        if let firstTask = snap.rows_a.first?.task {
            selectTask(firstTask)
        }
    }

    private func refreshHillclimbLedger() {
        var champLabel = "baseline-microcoder"
        if let cData = try? Data(contentsOf: Self.championURL),
           let obj = try? JSONSerialization.jsonObject(with: cData) as? [String: Any],
           let cid = obj["id"] as? String {
            let sha = (obj["manifest_sha"] as? String).map { String($0.prefix(8)) } ?? ""
            champLabel = sha.isEmpty ? cid : "\(cid) (#\(sha))"
        }
        guard let text = try? String(contentsOf: Self.ledgerURL, encoding: .utf8) else {
            hillclimbBanner.stringValue = "Hillclimb Champion: \(champLabel)  •  0 rounds in ~/.openagents/tbench/hillclimb/ledger.jsonl"
            return
        }
        let lines = text.split(separator: "\n").map(String.init).filter { !$0.isEmpty }
        var latest: TbHillclimbEntry?
        var promoted = 0
        var totalDistilled = 0
        for line in lines {
            if let d = line.data(using: .utf8), let e = try? JSONDecoder().decode(TbHillclimbEntry.self, from: d) {
                latest = e
                if e.verdict == "PROMOTED" { promoted += 1 }
                totalDistilled += e.distilled_train_atif
            }
        }
        if let e = latest {
            hillclimbBanner.stringValue = "Champion: \(champLabel)  •  Round #\(e.round) [\(e.verdict): \(e.reason)]  Train \(e.champ_train_pass)→\(e.cand_train_pass)/\(e.train_total) · Held-out \(e.champ_heldout_pass)→\(e.cand_heldout_pass)/\(e.heldout_total)  •  \(promoted)/\(lines.count) promoted  •  Ch 247 distilled: \(totalDistilled) train ATIF"
        } else {
            hillclimbBanner.stringValue = "Hillclimb Champion: \(champLabel)  •  \(lines.count) ledger row(s)"
        }
    }

    // MARK: - Task Replay into Embedded Coax GPU Chat Views

    private func selectTask(_ taskId: String) {
        for (k, v) in rowViews { v.setSelected(k == taskId) }
        replayFile(row: liveRowsA[taskId], side: .armA)
        replayFile(row: liveRowsB[taskId], side: .armB)
    }

    private func replayFile(row: TbTrialRow?, side: TbSide) {
        let cv = side == .armA ? chatA : chatB
        guard let row else {
            cv.setBadge("No run data")
            return
        }
        let badge = String(
            format: "%@ · %@ %.1fs (%dt · g:%d/%d)",
            row.task, row.status, Double(row.wall_ms) / 1000.0, row.turns, row.exec_checks, row.exec_intercepts
        )
        guard !row.events_path.isEmpty,
              let text = try? String(contentsOfFile: row.events_path, encoding: .utf8) else {
            cv.setBadge(badge)
            return
        }
        var promptText = "Terminal-Bench task: \(row.task)"
        var events: [String] = []
        for rawLine in text.split(separator: "\n") {
            let line = String(rawLine)
            if let d = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
               (obj["kind"] as? String) == "prompt",
               let p = obj["text"] as? String {
                promptText = p
            } else {
                events.append(line)
            }
        }
        cv.replay(prompt: promptText, taskName: row.task, events: events, badge: badge)
    }

    // MARK: - Live Subprocess Execution (`tbench-run run` / `tbench-run hillclimb`)

    @objc private func onRunSideBySide() {
        guard let suite = selectedSuite() else { return }
        let idxA = max(0, min(Self.armPresets.count - 1, armAPopup.indexOfSelectedItem))
        let idxB = max(0, min(Self.armPresets.count - 1, armBPopup.indexOfSelectedItem))
        var args = [
            "run",
            "--suite", suite.slug,
            "--arm-a", Self.armPresets[idxA].id,
            "--arm-b", Self.armPresets[idxB].id,
        ]
        let filter = filterField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        if !filter.isEmpty { args.append(contentsOf: ["--tasks", filter]) }
        cardA.setHeader(title: "Arm A · \(Self.armPresets[idxA].label)", subtitle: Self.armPresets[idxA].id)
        cardB.setHeader(title: "Arm B · \(Self.armPresets[idxB].label)", subtitle: Self.armPresets[idxB].id)
        chatA.setTitle("Arm A · \(Self.armPresets[idxA].label)")
        chatB.setTitle("Arm B · \(Self.armPresets[idxB].label)")
        launchTbenchProcess(args: args, suite: suite, filterCsv: filter, bannerPrefix: "Running Terminal-Bench side-by-side")
    }

    @objc private func onRunHillclimb() {
        guard let suite = selectedSuite() else { return }
        var args = [
            "hillclimb",
            "--suite", suite.slug,
            "--propose",
        ]
        let filter = filterField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        if !filter.isEmpty { args.append(contentsOf: ["--tasks", filter]) }
        cardA.setHeader(title: "Champion · Active Policy", subtitle: "hillclimb arm A")
        cardB.setHeader(title: "Challenger · Proposed Mutation", subtitle: "hillclimb arm B (Executor Network)")
        chatA.setTitle("Champion (Arm A)")
        chatB.setTitle("Challenger (Arm B)")
        launchTbenchProcess(args: args, suite: suite, filterCsv: filter, bannerPrefix: "Running #authority Microcoder Hillclimb round")
    }

    @objc private func onStop() {
        stopActiveProcess()
        deltaBanner.stringValue = "Stopped active Terminal-Bench run."
        setRunningUI(false)
    }

    private func setRunningUI(_ running: Bool) {
        runButton.isEnabled = !running
        hillclimbButton.isEnabled = !running
        stopButton.isEnabled = running
        suitePopup.isEnabled = !running
        armAPopup.isEnabled = !running
        armBPopup.isEnabled = !running
    }

    private func stopActiveProcess() {
        pollTimer?.invalidate()
        pollTimer = nil
        if let p = activeProcess, p.isRunning {
            p.terminate()
        }
        activeProcess = nil
    }

    private func launchTbenchProcess(args: [String], suite: TbSuiteCatalogEntry, filterCsv: String, bannerPrefix: String) {
        guard let bin = tbenchRunBinary() else {
            deltaBanner.stringValue = "Missing bundled tbench-run binary — rebuild OpenAgents Desktop."
            return
        }
        stopActiveProcess()

        let activeTasks: [TbSuiteTaskItem]
        if filterCsv.isEmpty {
            activeTasks = suite.tasks
        } else {
            let names = filterCsv.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            activeTasks = names.enumerated().map { (i, name) in
                let part = suite.tasks.first(where: { $0.id == name })?.partition ?? (i % 3 == 2 ? "heldout" : "train")
                return TbSuiteTaskItem(id: name, partition: part)
            }
        }
        rebuildRaceRows(tasks: activeTasks)
        cardA.reset(total: activeTasks.count)
        cardB.reset(total: activeTasks.count)
        chatA.setBadge("Booting…")
        chatB.setBadge("Queued")
        deltaBanner.stringValue = "\(bannerPrefix) (\(suite.slug), \(activeTasks.count) tasks)…"

        liveCursor = 0
        activeEventPathA = ""
        activeEventCursorA = 0
        activeEventPathB = ""
        activeEventCursorB = 0
        try? FileManager.default.removeItem(at: Self.liveStatusURL)

        let p = Process()
        p.executableURL = URL(fileURLWithPath: bin)
        p.arguments = args
        p.environment = augmentedEnv()
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        p.terminationHandler = { [weak self] _ in
            DispatchQueue.main.async {
                guard let self else { return }
                self.pollTimer?.invalidate()
                self.pollTimer = nil
                self.activeProcess = nil
                self.setRunningUI(false)
                self.loadSavedSnapshot()
                self.refreshHillclimbLedger()
            }
        }
        do {
            try p.run()
            activeProcess = p
            setRunningUI(true)
            let t = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
                self?.pollLiveFiles()
            }
            RunLoop.main.add(t, forMode: .common)
            pollTimer = t
        } catch {
            deltaBanner.stringValue = "Failed to launch tbench-run: \(error.localizedDescription)"
            setRunningUI(false)
        }
    }

    private func pollLiveFiles() {
        if let text = try? String(contentsOf: Self.liveStatusURL, encoding: .utf8) {
            let lines = text.split(separator: "\n").map(String.init)
            if lines.count > liveCursor {
                for i in liveCursor..<lines.count {
                    if let d = lines[i].data(using: .utf8),
                       let ev = try? JSONDecoder().decode(TbLiveEvent.self, from: d) {
                        handleLiveEvent(ev)
                    }
                }
                liveCursor = lines.count
            }
        }
        tailEventsFile(path: activeEventPathA, cursor: &activeEventCursorA, chat: chatA)
        tailEventsFile(path: activeEventPathB, cursor: &activeEventCursorB, chat: chatB)
    }

    private func handleLiveEvent(_ ev: TbLiveEvent) {
        let side: TbSide = ev.side == "b" ? .armB : .armA
        let chat = side == .armA ? chatA : chatB
        switch ev.kind {
        case "arm_start":
            chat.setBadge("Booting \(ev.arm_id)…")
            deltaBanner.stringValue = "Booting Arm \(ev.side.uppercased()) (\(ev.arm_id))…"
        case "task_start":
            rowViews[ev.task]?.update(side: side, state: .running(note: "Docker running…"), maxWallMs: 60_000)
            for (k, v) in rowViews { v.setSelected(k == ev.task) }
            chat.setBadge("[\(ev.idx)/\(ev.total)] \(ev.task) · running")
            deltaBanner.stringValue = "Arm \(ev.side.uppercased()) [\(ev.idx)/\(ev.total)] \(ev.task) (\(ev.note))…"
            if side == .armA {
                activeEventPathA = ev.events_path
                activeEventCursorA = 0
            } else {
                activeEventPathB = ev.events_path
                activeEventCursorB = 0
            }
        case "task_done":
            rowViews[ev.task]?.update(
                side: side,
                state: .done(status: ev.status, wallMs: ev.wall_ms, turns: ev.turns, checks: ev.exec_checks, intercepts: ev.exec_intercepts),
                maxWallMs: max(30_000, ev.wall_ms)
            )
            chat.setBadge(String(format: "%@ · %@ %.1fs", ev.task, ev.status, Double(ev.wall_ms) / 1000.0))
        default:
            break
        }
    }

    private func tailEventsFile(path: String, cursor: inout Int, chat: GymChatSurfaceView) {
        guard !path.isEmpty,
              let text = try? String(contentsOfFile: path, encoding: .utf8) else { return }
        let lines = text.split(separator: "\n").map(String.init)
        guard lines.count > cursor else { return }
        for i in cursor..<lines.count {
            let line = lines[i]
            if let d = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
               (obj["kind"] as? String) == "prompt" {
                let p = (obj["text"] as? String) ?? ""
                let t = (obj["task"] as? String) ?? "task"
                chat.startTask(prompt: p, taskName: t)
            } else {
                chat.feedLine(line)
            }
        }
        cursor = lines.count
    }
}
