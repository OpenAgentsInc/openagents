// Firm reason: 6 (AppKit host for side-by-side google-deepmind/gbench Inkling variant comparison & hillclimbing workbench using embedded Coax GPU chat views; all benchmark, #authority policy, scoring, ATIF-v1.8 export, and hillclimbing logic lives in Coax coder/tui/gbench-run.coax & coax/lib/gbench/).
import AppKit
import Foundation

// MARK: - Wire Snapshots (decoded from gbench-run outputs; single writer is coax/lib/gbench/wire.coax)

private struct GbDoctorReport: Codable {
    var schema: String?
    var ok: Bool
    var venv_ok: Bool
    var gbench_version: String
    var python_version: String
    var psionic_ok: Bool
    var llama_ok: Bool
    var tokenizer_ok: Bool
    var bwrap_ok: Bool
    var docker_ok: Bool
    var gemini_key_set: Bool
    var hf_token_set: Bool
    var variants_installed: Int
}

private struct GbItemRecord: Codable {
    var schema: String?
    var item_id: String
    var pillar: String
    var category: String
    var arm: String
    var status: String
    var passed: Bool
    var score_bps: Int
    var graded: Int
    var passed_count: Int
    var harness_errors: Int
    var token_leaks: Int
    var wall_ms: Int
    var ttft_us: Int
    var tpot_us: Int
    var itl_p99_us: Int
    var tok_per_s_x10: Int
    var cv_bps: Int
    var scaffold_id: String
    var subject_id: String
    var leaderboard_comparable: Bool
    var distill_allowed: Bool
    var atif_verified: Bool
    var atif_digest: String
    var atif_path: String
    var detail: String
}

private struct GbArmSummary: Codable {
    var id: String
    var label: String
    var variant: String
    var backend: String
    var model_id: String
    var effort: String
    var thinking: Bool
    var policy_sha: String
    var scaffold_id: String
    var subject_id: String
    var boot_ms: Int
    var peak_rss_mb: Int
    var total: Int
    var graded: Int
    var passed: Int
    var failed: Int
    var infra_errors: Int
    var score_bps: Int
    var wilson_low_bps: Int
    var wilson_high_bps: Int
    var golden_passed: Int
    var golden_applicable: Int
    var token_leaks: Int
    var ttft_p50_us: Int
    var tpot_p50_us: Int
    var itl_p99_us: Int
    var tok_per_s_x10: Int
    var cv_bps: Int
    var total_wall_ms: Int
    var atif_all_verified: Bool
    var leaderboard_comparable: Bool
}

private struct GbCompareSnapshot: Codable {
    var schema: String?
    var run_id: String
    var timestamp: String
    var profile: String
    var pillar: String
    var scaffold_locked: Bool
    var arm_a: GbArmSummary
    var arm_b: GbArmSummary
    var rows_a: [GbItemRecord]
    var rows_b: [GbItemRecord]
}

private struct GbHillclimbEntry: Codable {
    var schema: String?
    var round: Int
    var timestamp: String
    var profile: String
    var champion_id: String
    var champion_sha: String
    var challenger_id: String
    var challenger_sha: String
    var mutation_note: String
    var scaffold_id: String
    var graded_total: Int
    var champ_score_bps: Int
    var cand_score_bps: Int
    var champ_ttft_us: Int
    var cand_ttft_us: Int
    var champ_tpot_us: Int
    var cand_tpot_us: Int
    var cand_harness_errors: Int
    var verdict: String
    var reason: String
    var distilled_atif: Int
    var run_dir: String
}

private struct GbProfileItem: Codable {
    var id: String
    var distill_allowed: Bool
}

private struct GbProfileCatalogEntry: Codable {
    var slug: String
    var title: String
    var pillar: String
    var count: Int
    var requires_linux_bwrap: Bool
    var default_limit: Int
    var default_max_output_tokens: Int
    var items: [GbProfileItem]
}

private struct GbProfilesResponse: Codable {
    var profiles: [GbProfileCatalogEntry]
}

private struct GbVariantCatalogEntry: Codable {
    var slug: String
    var label: String
    var backend: String
    var model_id: String
    var quant: String
    var has_lora: Bool
    var installed: Bool
    var min_ram_gb: Int
}

private struct GbVariantsResponse: Codable {
    var variants: [GbVariantCatalogEntry]
}

private struct GbLiveEvent: Codable {
    var kind: String
    var side: String
    var arm_id: String
    var item_id: String
    var idx: Int
    var total: Int
    var status: String
    var score_bps: Int
    var wall_ms: Int
    var note: String
}

private enum GbSide {
    case armA
    case armB
}

private enum GbRowStatus {
    case pending
    case running(note: String)
    case done(status: String, scoreBps: Int, wallMs: Int, detail: String)
}

private final class GbFlippedView: NSView {
    override var isFlipped: Bool { true }
}

// MARK: - gBench Scorecard

private final class GbScoreCardView: NSView {
    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private let scoreLabel = NSTextField(labelWithString: "—")
    private let wallLabel = NSTextField(labelWithString: "0.0 s")
    private let line1Label = NSTextField(labelWithString: "Wilson 95% CI —  •  Golden —  •  Leaks 0  •  Scaffold —")
    private let line2Label = NSTextField(labelWithString: "TTFT —  •  TPOT —  •  P99 ITL —  •  tok/s —  •  RSS —  •  ATIF —")

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

        scoreLabel.translatesAutoresizingMaskIntoConstraints = false
        scoreLabel.font = NSFont.monospacedSystemFont(ofSize: 13.5, weight: .bold)
        scoreLabel.textColor = NSColor(white: 0.96, alpha: 1)
        addSubview(scoreLabel)

        wallLabel.translatesAutoresizingMaskIntoConstraints = false
        wallLabel.font = NSFont.monospacedSystemFont(ofSize: 15.5, weight: .bold)
        wallLabel.textColor = NSColor(white: 0.98, alpha: 1)
        wallLabel.alignment = .right
        addSubview(wallLabel)

        for lbl in [line1Label, line2Label] {
            lbl.translatesAutoresizingMaskIntoConstraints = false
            lbl.font = NSFont.monospacedSystemFont(ofSize: 9.7, weight: .regular)
            lbl.textColor = NSColor(white: 0.72, alpha: 1)
            lbl.lineBreakMode = .byTruncatingTail
            addSubview(lbl)
        }

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 86),
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 7),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            subtitleLabel.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            subtitleLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),
            subtitleLabel.trailingAnchor.constraint(lessThanOrEqualTo: wallLabel.leadingAnchor, constant: -8),

            wallLabel.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            wallLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),

            scoreLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 2),
            scoreLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),

            line1Label.topAnchor.constraint(equalTo: scoreLabel.bottomAnchor, constant: 3),
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

    func update(report: GbArmSummary, pillar: String) {
        titleLabel.stringValue = report.label.isEmpty ? report.id : report.label
        let shaShort = String(report.policy_sha.prefix(8))
        let thinkTag = report.thinking ? "think(\(report.effort))" : "no-think"
        subtitleLabel.stringValue = "\(report.backend) · \(report.variant) · \(thinkTag) · #\(shaShort)"

        let scorePct = Double(report.score_bps) / 100.0
        if pillar == "perf" {
            let tps = Double(report.tok_per_s_x10) / 10.0
            let ttftMs = Double(report.ttft_p50_us) / 1000.0
            scoreLabel.stringValue = String(format: "%.1f tok/s  ·  TTFT P50 %.1f ms", tps, ttftMs)
        } else {
            scoreLabel.stringValue = String(format: "%.1f%% (%d / %d PASS)", scorePct, report.passed, report.graded)
        }
        wallLabel.stringValue = String(format: "%.1f s", Double(report.total_wall_ms) / 1000.0)

        let wLo = Double(report.wilson_low_bps) / 100.0
        let wHi = Double(report.wilson_high_bps) / 100.0
        let goldenStr = report.golden_applicable > 0 ? "\(report.golden_passed)/\(report.golden_applicable)" : "—"
        let scStr = report.scaffold_id.isEmpty ? "—" : report.scaffold_id
        line1Label.stringValue = String(
            format: "Wilson 95%% [%.1f..%.1f%%]  •  Golden %@  •  Leaks %d  •  Infra %d  •  %@",
            wLo, wHi, goldenStr, report.token_leaks, report.infra_errors, scStr
        )

        let ttftStr = report.ttft_p50_us > 0 ? String(format: "%.0fms", Double(report.ttft_p50_us) / 1000.0) : "—"
        let tpotStr = report.tpot_p50_us > 0 ? String(format: "%.1fms", Double(report.tpot_p50_us) / 1000.0) : "—"
        let p99Str = report.itl_p99_us > 0 ? String(format: "%.1fms", Double(report.itl_p99_us) / 1000.0) : "—"
        let tpsStr = report.tok_per_s_x10 > 0 ? String(format: "%.1f tok/s", Double(report.tok_per_s_x10) / 10.0) : "—"
        let cvStr = report.cv_bps > 0 ? String(format: "CV %.1f%%", Double(report.cv_bps) / 100.0) : "CV —"
        let rssStr = report.peak_rss_mb > 0 ? String(format: "%.1f GiB", Double(report.peak_rss_mb) / 1024.0) : "—"
        let bootStr = report.boot_ms > 0 ? String(format: "%.2fs", Double(report.boot_ms) / 1000.0) : "—"
        let atifStr = report.atif_all_verified ? "ATIF-v1.8 ✓" : "ATIF —"
        line2Label.stringValue = "TTFT \(ttftStr) • TPOT \(tpotStr) • P99 \(p99Str) • \(tpsStr) • \(cvStr) • RSS \(rssStr) • Boot \(bootStr) • \(atifStr)"
    }

    func reset(total: Int) {
        scoreLabel.stringValue = "— / \(total)"
        wallLabel.stringValue = "0.0 s"
        line1Label.stringValue = "Wilson 95% CI —  •  Golden —  •  Leaks 0  •  Infra 0  •  Scaffold —"
        line2Label.stringValue = "TTFT —  •  TPOT —  •  P99 —  •  tok/s —  •  RSS —  •  Boot —  •  ATIF —"
    }
}

// MARK: - Per-Suite / Per-Case Race Row View

private final class GbItemRowView: NSView {
    let itemId: String
    let distillAllowed: Bool
    var onSelect: (() -> Void)?

    private let itemLabel = NSTextField(labelWithString: "")
    private let licenseBadge = NSTextField(labelWithString: "")
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

    private var aState: GbRowStatus = .pending
    private var bState: GbRowStatus = .pending

    init(itemId: String, distillAllowed: Bool) {
        self.itemId = itemId
        self.distillAllowed = distillAllowed
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 5

        itemLabel.translatesAutoresizingMaskIntoConstraints = false
        itemLabel.stringValue = itemId
        itemLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        itemLabel.textColor = NSColor(white: 0.90, alpha: 1)
        itemLabel.lineBreakMode = .byTruncatingTail
        addSubview(itemLabel)

        licenseBadge.translatesAutoresizingMaskIntoConstraints = false
        licenseBadge.stringValue = distillAllowed ? "DISTILL" : "HOLD"
        licenseBadge.font = NSFont.monospacedSystemFont(ofSize: 8.5, weight: .semibold)
        licenseBadge.textColor = distillAllowed
            ? NSColor(calibratedRed: 0.45, green: 0.85, blue: 0.98, alpha: 1)
            : NSColor(calibratedRed: 0.96, green: 0.72, blue: 0.34, alpha: 1)
        licenseBadge.alignment = .center
        licenseBadge.wantsLayer = true
        licenseBadge.layer?.cornerRadius = 3
        licenseBadge.layer?.backgroundColor = (distillAllowed
            ? NSColor(calibratedRed: 0.45, green: 0.85, blue: 0.98, alpha: 0.14)
            : NSColor(calibratedRed: 0.96, green: 0.72, blue: 0.34, alpha: 0.14)).cgColor
        addSubview(licenseBadge)

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

            itemLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            itemLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            itemLabel.widthAnchor.constraint(equalToConstant: 192),

            licenseBadge.centerYAnchor.constraint(equalTo: centerYAnchor),
            licenseBadge.leadingAnchor.constraint(equalTo: itemLabel.trailingAnchor, constant: 4),
            licenseBadge.widthAnchor.constraint(equalToConstant: 52),
            licenseBadge.heightAnchor.constraint(equalToConstant: 14),

            aTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            aTrack.leadingAnchor.constraint(equalTo: licenseBadge.trailingAnchor, constant: 10),
            aTrack.widthAnchor.constraint(equalToConstant: 72),
            aTrack.heightAnchor.constraint(equalToConstant: 8),

            aFill.leadingAnchor.constraint(equalTo: aTrack.leadingAnchor),
            aFill.topAnchor.constraint(equalTo: aTrack.topAnchor),
            aFill.bottomAnchor.constraint(equalTo: aTrack.bottomAnchor),
            aFillW,

            aStat.centerYAnchor.constraint(equalTo: centerYAnchor),
            aStat.leadingAnchor.constraint(equalTo: aTrack.trailingAnchor, constant: 6),
            aStat.widthAnchor.constraint(equalToConstant: 152),

            bTrack.centerYAnchor.constraint(equalTo: centerYAnchor),
            bTrack.leadingAnchor.constraint(equalTo: aStat.trailingAnchor, constant: 8),
            bTrack.widthAnchor.constraint(equalToConstant: 72),
            bTrack.heightAnchor.constraint(equalToConstant: 8),

            bFill.leadingAnchor.constraint(equalTo: bTrack.leadingAnchor),
            bFill.topAnchor.constraint(equalTo: bTrack.topAnchor),
            bFill.bottomAnchor.constraint(equalTo: bTrack.bottomAnchor),
            bFillW,

            bStat.centerYAnchor.constraint(equalTo: centerYAnchor),
            bStat.leadingAnchor.constraint(equalTo: bTrack.trailingAnchor, constant: 6),
            bStat.widthAnchor.constraint(equalToConstant: 152),

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

    func update(side: GbSide, state: GbRowStatus) {
        if side == .armA { aState = state } else { bState = state }
        apply(state: aState, fill: aFill, widthC: aFillW, label: aStat, isA: true)
        apply(state: bState, fill: bFill, widthC: bFillW, label: bStat, isA: false)

        if case .done(let sa, let sca, let wa, _) = aState, case .done(let sb, let scb, let wb, _) = bState {
            let diffBps = scb - sca
            if abs(diffBps) >= 10 {
                let pp = Double(diffBps) / 100.0
                deltaLabel.stringValue = String(format: "B %+.1fpp", pp)
                deltaLabel.textColor = diffBps > 0
                    ? NSColor(calibratedRed: 0.98, green: 0.72, blue: 0.32, alpha: 1)
                    : NSColor(calibratedRed: 0.30, green: 0.90, blue: 0.62, alpha: 1)
            } else if sa == "PASS" && sb != "PASS" {
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

    private func apply(state: GbRowStatus, fill: NSView, widthC: NSLayoutConstraint, label: NSTextField, isA: Bool) {
        let trackW: CGFloat = 72
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
            label.stringValue = note.isEmpty ? "running gbench…" : note
            label.textColor = isA
                ? NSColor(calibratedRed: 0.56, green: 0.98, blue: 0.78, alpha: 1)
                : NSColor(calibratedRed: 0.98, green: 0.82, blue: 0.50, alpha: 1)
        case .done(let status, let scoreBps, let wallMs, let detail):
            let frac = CGFloat(min(1.0, max(0.08, Double(scoreBps) / 10000.0)))
            widthC.constant = trackW * frac
            let sec = Double(wallMs) / 1000.0
            let shortDetail = detail.isEmpty ? String(format: "%.1f%%", Double(scoreBps) / 100.0) : detail
            label.stringValue = String(format: "%@ %@ (%.1fs)", status, shortDetail, sec)
            if status == "PASS" {
                fill.layer?.backgroundColor = (isA
                    ? NSColor(calibratedRed: 0.18, green: 0.82, blue: 0.56, alpha: 0.92)
                    : NSColor(calibratedRed: 0.92, green: 0.62, blue: 0.24, alpha: 0.86)).cgColor
                label.textColor = NSColor(white: 0.92, alpha: 1)
            } else if status == "NOT_APPLICABLE" || status == "WITHHELD" {
                fill.layer?.backgroundColor = NSColor(white: 0.45, alpha: 0.6).cgColor
                label.textColor = NSColor(white: 0.60, alpha: 1)
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

// MARK: - Main GymGBenchView

final class GymGBenchView: NSView {
    var workingFolderProvider: (() -> URL?)?

    private static var homeURL: URL { FileManager.default.homeDirectoryForCurrentUser }
    private static var savedCompareURL: URL { homeURL.appendingPathComponent(".openagents/gym/last-gbench.json") }
    private static var liveStatusURL: URL { homeURL.appendingPathComponent(".openagents/gbench/live-status.jsonl") }
    private static var ledgerURL: URL { homeURL.appendingPathComponent(".openagents/gbench/hillclimb/ledger.jsonl") }
    private static var championURL: URL { homeURL.appendingPathComponent(".openagents/gbench/hillclimb/champion.json") }

    // Top controls
    private let headerBar = NSView()
    private let titleLabel = NSTextField(labelWithString: "gBench · Inkling Variants & Serving Eval")
    private let scaffoldBadge = NSTextField(labelWithString: "SCAFFOLD —")
    private let doctorPill = NSTextField(labelWithString: "Checking gBench / Psionic / Tokenizer…")

    private let profilePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let armAPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let vsLabel = NSTextField(labelWithString: "vs")
    private let armBPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let thinkingCheck = NSButton(checkboxWithTitle: "Think", target: nil, action: nil)
    private let limitField = NSTextField(string: "10")
    private let runButton = NSButton(title: "Run Compare", target: nil, action: nil)
    private let hillclimbButton = NSButton(title: "Hillclimb", target: nil, action: nil)
    private let stopButton = NSButton(title: "Stop", target: nil, action: nil)
    private let loadLastButton = NSButton(title: "Load Last", target: nil, action: nil)

    // Scorecards & Banners
    private let cardA = GbScoreCardView(
        title: "Arm A · Inkling 2 Mini (Psionic IQ4_NL)",
        subtitle: "psionic-metal · inkling-2-mini",
        accent: NSColor(calibratedRed: 0.24, green: 0.88, blue: 0.64, alpha: 1)
    )
    private let cardB = GbScoreCardView(
        title: "Arm B · Inkling 2 Mini Uncensored LoRA",
        subtitle: "psionic-metal · inkling-2-mini-uncensored-lora",
        accent: NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 1)
    )
    private let deltaBanner = NSTextField(labelWithString: "Select a gBench profile (Golden Smoke, Serving Perf, or Local Evals) and Inkling variants to compare or hillclimb.")
    private let hillclimbBanner = NSTextField(labelWithString: "Hillclimb Champion: inkling-2-mini  •  0 rounds recorded  •  #authority gate + license-checked ATIF distillation")

    // Scrollable Item Race Table
    private let raceBox = NSView()
    private let raceScroll = NSScrollView()
    private let raceDocView = GbFlippedView()
    private let raceStack = NSStackView()
    private var rowViews: [String: GbItemRowView] = [:]

    // Dual Real Embedded Coax GPU Chat Views (ATIF-v1.8 replay)
    private let splitChatContainer = NSView()
    private let chatA = GymChatSurfaceView(
        title: "Arm A · ATIF-v1.8 Trace",
        accent: NSColor(calibratedRed: 0.24, green: 0.88, blue: 0.64, alpha: 1)
    )
    private let chatB = GymChatSurfaceView(
        title: "Arm B · ATIF-v1.8 Trace",
        accent: NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 1)
    )

    // State
    private var profiles: [GbProfileCatalogEntry] = []
    private var variants: [GbVariantCatalogEntry] = []
    private var lastDoctor: GbDoctorReport?
    private var lastSnapshot: GbCompareSnapshot?
    private var activeProcess: Process?
    private var pollTimer: Timer?
    private var liveCursor = 0
    private var liveRowsA: [String: GbItemRecord] = [:]
    private var liveRowsB: [String: GbItemRecord] = [:]
    private var hydratedOnce = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        setupUI()
        seedFallbacks()
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
            refreshCatalogsFromTool()
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

        scaffoldBadge.translatesAutoresizingMaskIntoConstraints = false
        scaffoldBadge.font = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold)
        scaffoldBadge.textColor = NSColor(white: 0.70, alpha: 1)
        scaffoldBadge.alignment = .center
        scaffoldBadge.wantsLayer = true
        scaffoldBadge.layer?.cornerRadius = 4
        scaffoldBadge.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.07).cgColor
        headerBar.addSubview(scaffoldBadge)

        doctorPill.translatesAutoresizingMaskIntoConstraints = false
        doctorPill.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .medium)
        doctorPill.textColor = NSColor(white: 0.72, alpha: 1)
        doctorPill.alignment = .right
        doctorPill.lineBreakMode = .byTruncatingTail
        headerBar.addSubview(doctorPill)

        profilePopup.translatesAutoresizingMaskIntoConstraints = false
        profilePopup.controlSize = .small
        profilePopup.font = NSFont.systemFont(ofSize: 11)
        profilePopup.target = self
        profilePopup.action = #selector(profileChanged)
        headerBar.addSubview(profilePopup)

        for popup in [armAPopup, armBPopup] {
            popup.translatesAutoresizingMaskIntoConstraints = false
            popup.controlSize = .small
            popup.font = NSFont.systemFont(ofSize: 11)
            headerBar.addSubview(popup)
        }

        vsLabel.translatesAutoresizingMaskIntoConstraints = false
        vsLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .semibold)
        vsLabel.textColor = NSColor(white: 0.52, alpha: 1)
        headerBar.addSubview(vsLabel)

        thinkingCheck.translatesAutoresizingMaskIntoConstraints = false
        thinkingCheck.controlSize = .small
        thinkingCheck.font = NSFont.systemFont(ofSize: 10.5, weight: .medium)
        thinkingCheck.state = .off
        headerBar.addSubview(thinkingCheck)

        limitField.translatesAutoresizingMaskIntoConstraints = false
        limitField.controlSize = .small
        limitField.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
        limitField.placeholderString = "Limit"
        limitField.alignment = .center
        headerBar.addSubview(limitField)

        for (btn, sel) in [
            (runButton, #selector(onRunCompare)),
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
            headerBar.heightAnchor.constraint(equalToConstant: 68),

            titleLabel.topAnchor.constraint(equalTo: headerBar.topAnchor, constant: 8),
            titleLabel.leadingAnchor.constraint(equalTo: headerBar.leadingAnchor, constant: 14),

            scaffoldBadge.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            scaffoldBadge.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 10),
            scaffoldBadge.widthAnchor.constraint(greaterThanOrEqualToConstant: 112),
            scaffoldBadge.heightAnchor.constraint(equalToConstant: 17),

            doctorPill.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            doctorPill.leadingAnchor.constraint(greaterThanOrEqualTo: scaffoldBadge.trailingAnchor, constant: 12),
            doctorPill.trailingAnchor.constraint(equalTo: headerBar.trailingAnchor, constant: -14),

            profilePopup.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 8),
            profilePopup.leadingAnchor.constraint(equalTo: headerBar.leadingAnchor, constant: 14),
            profilePopup.widthAnchor.constraint(lessThanOrEqualToConstant: 196),

            armAPopup.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            armAPopup.leadingAnchor.constraint(equalTo: profilePopup.trailingAnchor, constant: 6),
            armAPopup.widthAnchor.constraint(lessThanOrEqualToConstant: 176),

            vsLabel.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            vsLabel.leadingAnchor.constraint(equalTo: armAPopup.trailingAnchor, constant: 5),

            armBPopup.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            armBPopup.leadingAnchor.constraint(equalTo: vsLabel.trailingAnchor, constant: 5),
            armBPopup.widthAnchor.constraint(lessThanOrEqualToConstant: 176),

            thinkingCheck.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            thinkingCheck.leadingAnchor.constraint(equalTo: armBPopup.trailingAnchor, constant: 8),

            limitField.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            limitField.leadingAnchor.constraint(equalTo: thinkingCheck.trailingAnchor, constant: 6),
            limitField.widthAnchor.constraint(equalToConstant: 44),

            loadLastButton.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            loadLastButton.trailingAnchor.constraint(equalTo: headerBar.trailingAnchor, constant: -14),

            stopButton.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            stopButton.trailingAnchor.constraint(equalTo: loadLastButton.leadingAnchor, constant: -6),

            hillclimbButton.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
            hillclimbButton.trailingAnchor.constraint(equalTo: stopButton.leadingAnchor, constant: -6),

            runButton.centerYAnchor.constraint(equalTo: profilePopup.centerYAnchor),
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

    private func gbenchRunBinary() -> String? {
        let fm = FileManager.default
        if let bundled = Bundle.main.resourceURL?.appendingPathComponent("gbench-run").path,
           fm.isExecutableFile(atPath: bundled) {
            return bundled
        }
        let shared = Self.homeURL.appendingPathComponent(".openagents/bin/gbench-run").path
        if fm.isExecutableFile(atPath: shared) { return shared }
        return nil
    }

    private func augmentedEnv() -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let home = Self.homeURL.path
        let extra = "\(home)/.openagents/bin:\(home)/.local/bin:/opt/homebrew/bin:/usr/local/bin"
        let cur = env["PATH"] ?? "/usr/bin:/bin"
        env["PATH"] = "\(extra):\(cur)"
        return env
    }

    private func runToolSync(_ args: [String], done: @escaping (String?) -> Void) {
        guard let bin = gbenchRunBinary() else {
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

    private func seedFallbacks() {
        profiles = [
            GbProfileCatalogEntry(
                slug: "oa-local-fast",
                title: "Local Fast Evals (6 zero-infra suites)",
                pillar: "eval",
                count: 6,
                requires_linux_bwrap: false,
                default_limit: 10,
                default_max_output_tokens: 2048,
                items: [
                    .init(id: "gsm8k", distill_allowed: true),
                    .init(id: "amc_aime", distill_allowed: true),
                    .init(id: "ifeval", distill_allowed: true),
                    .init(id: "arc_agi", distill_allowed: true),
                    .init(id: "mmlu_redux", distill_allowed: true),
                    .init(id: "seal_tools", distill_allowed: true),
                ]
            ),
            GbProfileCatalogEntry(
                slug: "oa-template-smoke",
                title: "Golden Set + Chat-Template Integrity",
                pillar: "golden",
                count: 16,
                requires_linux_bwrap: false,
                default_limit: 0,
                default_max_output_tokens: 1024,
                items: [.init(id: "chat_template_leak_check", distill_allowed: false)]
            ),
            GbProfileCatalogEntry(
                slug: "oa-perf-quick",
                title: "Serving Perf Quick (b=1, 128/64)",
                pillar: "perf",
                count: 1,
                requires_linux_bwrap: false,
                default_limit: 0,
                default_max_output_tokens: 64,
                items: [.init(id: "serving_quick", distill_allowed: false)]
            ),
        ]
        variants = [
            GbVariantCatalogEntry(slug: "inkling-2-mini", label: "Inkling 2 Mini · Psionic IQ4_NL", backend: "psionic-metal", model_id: "inkling-2-mini", quant: "IQ4_NL", has_lora: false, installed: true, min_ram_gb: 24),
            GbVariantCatalogEntry(slug: "inkling-2-mini-uncensored-lora", label: "Inkling 2 Mini · Uncensored LoRA", backend: "psionic-metal", model_id: "inkling-2-mini-uncensored-lora", quant: "IQ4_NL+LoRA", has_lora: true, installed: true, min_ram_gb: 24),
            GbVariantCatalogEntry(slug: "inkling-2-mini-uncensored-q4k", label: "Inkling 2 Mini · Uncensored AD-Q4_K", backend: "psionic-metal", model_id: "inkling-2-mini-uncensored-q4k", quant: "AD-Q4_K", has_lora: false, installed: true, min_ram_gb: 24),
            GbVariantCatalogEntry(slug: "llamacpp-inkling-2-mini", label: "Inkling 2 Mini · llama.cpp IQ4_NL", backend: "llamacpp-metal", model_id: "inkling-2-mini", quant: "IQ4_NL", has_lora: false, installed: true, min_ram_gb: 24),
        ]
        rebuildProfileMenu(selectSlug: "oa-local-fast")
        rebuildVariantMenus(selectA: "inkling-2-mini", selectB: "inkling-2-mini-uncensored-lora")
        rebuildRaceRows(items: profiles[0].items)
    }

    private func refreshDoctor() {
        runToolSync(["doctor", "--json"]) { [weak self] out in
            guard let self else { return }
            guard let out, let data = out.data(using: .utf8),
                  let doc = try? JSONDecoder().decode(GbDoctorReport.self, from: data) else {
                self.doctorPill.stringValue = "gbench-run not found — rebuild app"
                self.doctorPill.textColor = NSColor(calibratedRed: 0.98, green: 0.45, blue: 0.45, alpha: 1)
                return
            }
            self.lastDoctor = doc
            let vStr = doc.venv_ok ? "gBench \(doc.gbench_version)" : "venv OFF"
            let pStr = doc.psionic_ok ? "Psionic ✓" : "Psionic —"
            let lStr = doc.llama_ok ? "llama.cpp ✓" : "llama.cpp —"
            let tStr = doc.tokenizer_ok ? "Tok ✓" : "Tok —"
            let mStr = "\(doc.variants_installed) Inkling"
            self.doctorPill.stringValue = "\(vStr)  •  \(pStr)  •  \(lStr)  •  \(tStr)  •  \(mStr)"
            self.doctorPill.textColor = doc.ok
                ? NSColor(calibratedRed: 0.45, green: 0.92, blue: 0.72, alpha: 1)
                : NSColor(calibratedRed: 0.98, green: 0.65, blue: 0.35, alpha: 1)
        }
    }

    private func refreshCatalogsFromTool() {
        runToolSync(["suites", "--json"]) { [weak self] out in
            guard let self, let out, let data = out.data(using: .utf8),
                  let resp = try? JSONDecoder().decode(GbProfilesResponse.self, from: data),
                  !resp.profiles.isEmpty else { return }
            let curSlug = self.selectedProfile()?.slug ?? "oa-local-fast"
            self.profiles = resp.profiles
            self.rebuildProfileMenu(selectSlug: curSlug)
            if self.lastSnapshot == nil, let p = self.selectedProfile() {
                self.rebuildRaceRows(items: p.items)
            }
        }
        runToolSync(["variants", "--json"]) { [weak self] out in
            guard let self, let out, let data = out.data(using: .utf8),
                  let resp = try? JSONDecoder().decode(GbVariantsResponse.self, from: data),
                  !resp.variants.isEmpty else { return }
            let curA = self.selectedVariant(self.armAPopup)?.slug ?? "inkling-2-mini"
            let curB = self.selectedVariant(self.armBPopup)?.slug ?? "inkling-2-mini-uncensored-lora"
            self.variants = resp.variants
            self.rebuildVariantMenus(selectA: curA, selectB: curB)
        }
    }

    private func rebuildProfileMenu(selectSlug: String? = nil) {
        profilePopup.removeAllItems()
        var targetIdx = 0
        for (i, p) in profiles.enumerated() {
            let tag = p.requires_linux_bwrap ? " [Linux]" : ""
            profilePopup.addItem(withTitle: "\(p.slug) (\(p.pillar) · \(p.count))\(tag)")
            if p.slug == selectSlug { targetIdx = i }
        }
        if !profiles.isEmpty { profilePopup.selectItem(at: targetIdx) }
    }

    private func rebuildVariantMenus(selectA: String, selectB: String) {
        armAPopup.removeAllItems()
        armBPopup.removeAllItems()
        var idxA = 0
        var idxB = min(1, max(0, variants.count - 1))
        for (i, v) in variants.enumerated() {
            let mark = v.installed ? "✓ " : "○ "
            let title = "\(mark)\(v.label)"
            armAPopup.addItem(withTitle: title)
            armBPopup.addItem(withTitle: title)
            if v.slug == selectA { idxA = i }
            if v.slug == selectB { idxB = i }
        }
        if !variants.isEmpty {
            armAPopup.selectItem(at: idxA)
            armBPopup.selectItem(at: idxB)
        }
    }

    private func selectedProfile() -> GbProfileCatalogEntry? {
        let idx = profilePopup.indexOfSelectedItem
        guard idx >= 0 && idx < profiles.count else { return profiles.first }
        return profiles[idx]
    }

    private func selectedVariant(_ popup: NSPopUpButton) -> GbVariantCatalogEntry? {
        let idx = popup.indexOfSelectedItem
        guard idx >= 0 && idx < variants.count else { return variants.first }
        return variants[idx]
    }

    @objc private func profileChanged() {
        guard let p = selectedProfile() else { return }
        if p.default_limit > 0 { limitField.stringValue = "\(p.default_limit)" }
        rebuildRaceRows(items: p.items)
        cardA.reset(total: p.count)
        cardB.reset(total: p.count)
    }

    private func rebuildRaceRows(items: [GbProfileItem]) {
        for v in raceStack.arrangedSubviews {
            raceStack.removeArrangedSubview(v)
            v.removeFromSuperview()
        }
        rowViews.removeAll()
        for it in items {
            let rv = GbItemRowView(itemId: it.id, distillAllowed: it.distill_allowed)
            rv.translatesAutoresizingMaskIntoConstraints = false
            rv.onSelect = { [weak self] in
                self?.selectItem(it.id)
            }
            raceStack.addArrangedSubview(rv)
            rv.widthAnchor.constraint(equalTo: raceStack.widthAnchor).isActive = true
            rowViews[it.id] = rv
        }
    }

    // MARK: - Load Saved Snapshot & Hillclimb Ledger

    @objc private func onLoadLast() {
        loadSavedSnapshot()
        refreshHillclimbLedger()
    }

    private func loadSavedSnapshot() {
        guard let data = try? Data(contentsOf: Self.savedCompareURL),
              let snap = try? JSONDecoder().decode(GbCompareSnapshot.self, from: data) else {
            return
        }
        applySnapshot(snap)
    }

    private func applySnapshot(_ snap: GbCompareSnapshot) {
        lastSnapshot = snap
        liveRowsA.removeAll()
        liveRowsB.removeAll()
        for r in snap.rows_a { liveRowsA[r.item_id] = r }
        for r in snap.rows_b { liveRowsB[r.item_id] = r }

        // Build unified ordered item list from rows_a + any extra rows_b
        var seen = Set<String>()
        var items: [GbProfileItem] = []
        for r in snap.rows_a {
            if seen.insert(r.item_id).inserted {
                items.append(.init(id: r.item_id, distill_allowed: r.distill_allowed))
            }
        }
        for r in snap.rows_b {
            if seen.insert(r.item_id).inserted {
                items.append(.init(id: r.item_id, distill_allowed: r.distill_allowed))
            }
        }
        if !items.isEmpty { rebuildRaceRows(items: items) }

        for r in snap.rows_a {
            rowViews[r.item_id]?.update(side: .armA, state: .done(status: r.status, scoreBps: r.score_bps, wallMs: r.wall_ms, detail: r.detail))
        }
        for r in snap.rows_b {
            rowViews[r.item_id]?.update(side: .armB, state: .done(status: r.status, scoreBps: r.score_bps, wallMs: r.wall_ms, detail: r.detail))
        }

        cardA.update(report: snap.arm_a, pillar: snap.pillar)
        cardB.update(report: snap.arm_b, pillar: snap.pillar)
        chatA.setTitle("Arm A · \(snap.arm_a.label)")
        chatB.setTitle("Arm B · \(snap.arm_b.label)")

        let scA = snap.arm_a.scaffold_id
        if snap.scaffold_locked && !scA.isEmpty {
            scaffoldBadge.stringValue = "LOCKED \(scA)"
            scaffoldBadge.textColor = NSColor(calibratedRed: 0.45, green: 0.92, blue: 0.72, alpha: 1)
            scaffoldBadge.layer?.backgroundColor = NSColor(calibratedRed: 0.24, green: 0.88, blue: 0.64, alpha: 0.14).cgColor
        } else {
            scaffoldBadge.stringValue = scA.isEmpty ? "SCAFFOLD —" : "UNLOCKED"
            scaffoldBadge.textColor = NSColor(calibratedRed: 0.98, green: 0.65, blue: 0.35, alpha: 1)
            scaffoldBadge.layer?.backgroundColor = NSColor(calibratedRed: 0.96, green: 0.68, blue: 0.26, alpha: 0.14).cgColor
        }

        let deltaPp = Double(snap.arm_b.score_bps - snap.arm_a.score_bps) / 100.0
        let scoreWord = snap.pillar == "perf"
            ? String(format: "A %.1f vs B %.1f tok/s", Double(snap.arm_a.tok_per_s_x10) / 10.0, Double(snap.arm_b.tok_per_s_x10) / 10.0)
            : String(format: "B vs A: %+.1f pp (%d vs %d PASS)", deltaPp, snap.arm_b.passed, snap.arm_a.passed)
        let wallRatio = snap.arm_a.total_wall_ms > 0 && snap.arm_b.total_wall_ms > 0
            ? String(format: "%.2fx wall", Double(snap.arm_b.total_wall_ms) / Double(snap.arm_a.total_wall_ms))
            : "—"
        deltaBanner.stringValue = "Run \(snap.run_id) (\(snap.profile) · \(snap.pillar))  —  \(scoreWord)  •  \(wallRatio)  •  Click any row to inspect ATIF-v1.8 traces."

        if let firstId = items.first?.id {
            selectItem(firstId)
        }
    }

    private func refreshHillclimbLedger() {
        var champLabel = "inkling-2-mini"
        if let cData = try? Data(contentsOf: Self.championURL),
           let obj = try? JSONSerialization.jsonObject(with: cData) as? [String: Any],
           let cid = obj["id"] as? String {
            let sha = (obj["manifest_sha"] as? String).map { String($0.prefix(8)) } ?? ""
            champLabel = sha.isEmpty ? cid : "\(cid) (#\(sha))"
        }
        guard let text = try? String(contentsOf: Self.ledgerURL, encoding: .utf8) else {
            hillclimbBanner.stringValue = "Hillclimb Champion: \(champLabel)  •  0 rounds in ~/.openagents/gbench/hillclimb/ledger.jsonl"
            return
        }
        let lines = text.split(separator: "\n").map(String.init).filter { !$0.isEmpty }
        var latest: GbHillclimbEntry?
        var promoted = 0
        var totalDistilled = 0
        for line in lines {
            if let d = line.data(using: .utf8), let e = try? JSONDecoder().decode(GbHillclimbEntry.self, from: d) {
                latest = e
                if e.verdict == "PROMOTED" { promoted += 1 }
                totalDistilled += e.distilled_atif
            }
        }
        if let e = latest {
            let cPct = Double(e.champ_score_bps) / 100.0
            let nPct = Double(e.cand_score_bps) / 100.0
            hillclimbBanner.stringValue = String(
                format: "Champion: %@  •  Round #%d [%@: %@]  %.1f%% → %.1f%% (%d graded)  •  %d/%d promoted  •  Distilled: %d ATIF",
                champLabel, e.round, e.verdict, e.reason, cPct, nPct, e.graded_total, promoted, lines.count, totalDistilled
            )
        } else {
            hillclimbBanner.stringValue = "Hillclimb Champion: \(champLabel)  •  \(lines.count) ledger row(s)"
        }
    }

    // MARK: - Verified ATIF-v1.8 Trace Replay into Coax GPU Chat Views

    private func selectItem(_ itemId: String) {
        for (k, v) in rowViews { v.setSelected(k == itemId) }
        replayAtif(row: liveRowsA[itemId], side: .armA)
        replayAtif(row: liveRowsB[itemId], side: .armB)
    }

    private func replayAtif(row: GbItemRecord?, side: GbSide) {
        let cv = side == .armA ? chatA : chatB
        guard let row else {
            cv.setBadge("No run data")
            return
        }
        let atifMark = row.atif_verified ? "ATIF ✓" : "ATIF —"
        let badge = String(
            format: "%@ · %@ (%.1f%% · %.1fs · %@)",
            row.item_id, row.status, Double(row.score_bps) / 100.0, Double(row.wall_ms) / 1000.0, atifMark
        )
        guard !row.atif_path.isEmpty,
              let text = try? String(contentsOfFile: row.atif_path, encoding: .utf8) else {
            cv.replay(
                prompt: "gBench item: \(row.item_id) (\(row.pillar))",
                taskName: row.item_id,
                events: [
                    GymChatSurfaceView.encodeJSON(["kind": "reply", "text": "\(row.status): \(row.detail)"]),
                    GymChatSurfaceView.encodeJSON(["kind": "ended", "ending": "done", "text": ""]),
                ],
                badge: badge
            )
            return
        }

        var firstPrompt = "gBench item: \(row.item_id)"
        var havePrompt = false
        var sampleIdx = 0
        var events: [String] = []

        for rawLine in text.split(separator: "\n") {
            guard let d = String(rawLine).data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
                  (obj["record"] as? String) == "step",
                  let step = obj["step"] as? [String: Any] else { continue }
            let src = (step["source"] as? String) ?? ""
            let msg = (step["message"] as? String) ?? ""
            let reason = (step["reasoning_content"] as? String) ?? ""

            if src == "user" {
                sampleIdx += 1
                if !havePrompt {
                    firstPrompt = msg.isEmpty ? firstPrompt : msg
                    havePrompt = true
                } else if !msg.isEmpty {
                    events.append(GymChatSurfaceView.encodeJSON([
                        "kind": "reply",
                        "text": "\n---\n**Sample #\(sampleIdx) Prompt:**\n\(msg)\n"
                    ]))
                }
            } else if src == "agent" {
                if !reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    events.append(GymChatSurfaceView.encodeJSON(["kind": "thought", "text": reason]))
                }
                var outText = msg
                if let extra = step["extra"] as? [String: Any] {
                    if let ok = extra["is_correct"] as? Bool {
                        outText = (ok ? "[PASS] " : "[FAIL] ") + outText
                    } else if let ok = extra["passed"] as? Bool, sampleIdx > 1 {
                        outText = (ok ? "[PASS] " : "[FAIL] ") + outText
                    }
                }
                if !outText.isEmpty {
                    events.append(GymChatSurfaceView.encodeJSON(["kind": "reply", "text": outText]))
                }
            }
        }
        events.append(GymChatSurfaceView.encodeJSON(["kind": "ended", "ending": "done", "text": ""]))
        cv.replay(prompt: firstPrompt, taskName: row.item_id, events: events, badge: badge)
    }

    // MARK: - Live Subprocess Execution (`gbench-run compare` / `gbench-run hillclimb`)

    @objc private func onRunCompare() {
        guard let prof = selectedProfile(),
              let varA = selectedVariant(armAPopup),
              let varB = selectedVariant(armBPopup) else { return }
        var args = [
            "compare",
            "--profile", prof.slug,
            "--arm-a", varA.slug,
            "--arm-b", varB.slug,
        ]
        if thinkingCheck.state == .on { args.append("--thinking") }
        let lim = Int(limitField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
        if prof.pillar == "eval" && lim > 0 {
            args.append(contentsOf: ["--limit", "\(lim)"])
        }
        if prof.pillar == "golden" {
            args.append("--allow-local-golden-exec")
        }
        cardA.setHeader(title: "Arm A · \(varA.label)", subtitle: varA.slug)
        cardB.setHeader(title: "Arm B · \(varB.label)", subtitle: varB.slug)
        chatA.setTitle("Arm A · \(varA.label)")
        chatB.setTitle("Arm B · \(varB.label)")
        launchGBenchProcess(args: args, profile: prof, bannerPrefix: "Running gBench compare")
    }

    @objc private func onRunHillclimb() {
        guard let prof = selectedProfile() else { return }
        let hcProfile = prof.pillar == "eval" ? prof.slug : "oa-local-fast"
        let lim = max(1, Int(limitField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 10)
        let args = [
            "hillclimb",
            "--profile", hcProfile,
            "--limit", "\(lim)",
            "--propose",
        ]
        cardA.setHeader(title: "Champion · Active Inkling Policy", subtitle: "hillclimb arm A")
        cardB.setHeader(title: "Challenger · Proposed Mutation", subtitle: "hillclimb arm B (#authority gated)")
        chatA.setTitle("Champion (Arm A)")
        chatB.setTitle("Challenger (Arm B)")
        launchGBenchProcess(args: args, profile: prof, bannerPrefix: "Running #authority gBench Hillclimb round")
    }

    @objc private func onStop() {
        stopActiveProcess()
        deltaBanner.stringValue = "Stopped active gBench run."
        setRunningUI(false)
    }

    private func setRunningUI(_ running: Bool) {
        runButton.isEnabled = !running
        hillclimbButton.isEnabled = !running
        stopButton.isEnabled = running
        profilePopup.isEnabled = !running
        armAPopup.isEnabled = !running
        armBPopup.isEnabled = !running
        thinkingCheck.isEnabled = !running
        limitField.isEnabled = !running
    }

    private func stopActiveProcess() {
        pollTimer?.invalidate()
        pollTimer = nil
        if let p = activeProcess, p.isRunning {
            p.terminate()
        }
        activeProcess = nil
        runToolSync(["stop"]) { _ in }
    }

    private func launchGBenchProcess(args: [String], profile: GbProfileCatalogEntry, bannerPrefix: String) {
        guard let bin = gbenchRunBinary() else {
            deltaBanner.stringValue = "Missing bundled gbench-run binary — rebuild OpenAgents Desktop."
            return
        }
        stopActiveProcess()

        rebuildRaceRows(items: profile.items)
        cardA.reset(total: profile.count)
        cardB.reset(total: profile.count)
        chatA.setBadge("Booting…")
        chatB.setBadge("Queued")
        deltaBanner.stringValue = "\(bannerPrefix) (\(profile.slug))…"

        liveCursor = 0
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
            let t = Timer(timeInterval: 0.3, repeats: true) { [weak self] _ in
                self?.pollLiveStatus()
            }
            RunLoop.main.add(t, forMode: .common)
            pollTimer = t
        } catch {
            deltaBanner.stringValue = "Failed to launch gbench-run: \(error.localizedDescription)"
            setRunningUI(false)
        }
    }

    private func pollLiveStatus() {
        guard let text = try? String(contentsOf: Self.liveStatusURL, encoding: .utf8) else { return }
        let lines = text.split(separator: "\n").map(String.init)
        guard lines.count > liveCursor else { return }
        for i in liveCursor..<lines.count {
            if let d = lines[i].data(using: .utf8),
               let ev = try? JSONDecoder().decode(GbLiveEvent.self, from: d) {
                handleLiveEvent(ev)
            }
        }
        liveCursor = lines.count
    }

    private func handleLiveEvent(_ ev: GbLiveEvent) {
        let side: GbSide = ev.side == "b" ? .armB : .armA
        let chat = side == .armA ? chatA : chatB
        switch ev.kind {
        case "arm_start":
            chat.setBadge("Booting \(ev.arm_id)…")
            deltaBanner.stringValue = "Arm \(ev.side.uppercased()) (\(ev.arm_id)): \(ev.note)…"
        case "arm_ready", "arm_running":
            chat.setBadge("Running \(ev.item_id)…")
            deltaBanner.stringValue = "Arm \(ev.side.uppercased()) (\(ev.arm_id)): running gBench \(ev.item_id)…"
            for rv in rowViews.values {
                rv.update(side: side, state: .running(note: "gbench running…"))
            }
        case "item_done":
            if rowViews[ev.item_id] == nil {
                let rv = GbItemRowView(itemId: ev.item_id, distillAllowed: false)
                rv.translatesAutoresizingMaskIntoConstraints = false
                rv.onSelect = { [weak self] in self?.selectItem(ev.item_id) }
                raceStack.addArrangedSubview(rv)
                rv.widthAnchor.constraint(equalTo: raceStack.widthAnchor).isActive = true
                rowViews[ev.item_id] = rv
            }
            rowViews[ev.item_id]?.update(side: side, state: .done(status: ev.status, scoreBps: ev.score_bps, wallMs: ev.wall_ms, detail: ev.note))
            chat.setBadge("\(ev.item_id) · \(ev.status) · \(ev.note)")
        case "arm_error":
            chat.setBadge("ERROR: \(ev.note)")
            deltaBanner.stringValue = "Arm \(ev.side.uppercased()) error: \(ev.note)"
        default:
            break
        }
    }

    // MARK: - Programmatic Steering (GET/POST /v1/gym/gbench & ~/.openagents/gbench/gym-request.json)

    func gbenchState() -> [String: Any] {
        var out: [String: Any] = [
            "ok": true,
            "mode": "gbench",
            "running": activeProcess?.isRunning == true,
            "profile": selectedProfile()?.slug ?? "oa-local-fast",
            "arm_a": selectedVariant(armAPopup)?.slug ?? "inkling-2-mini",
            "arm_b": selectedVariant(armBPopup)?.slug ?? "inkling-2-mini-uncensored-lora",
            "thinking": thinkingCheck.state == .on,
            "limit": Int(limitField.stringValue) ?? 10,
            "variants_installed": lastDoctor?.variants_installed ?? 0,
            "doctor_ok": lastDoctor?.ok ?? false,
        ]
        if let snap = lastSnapshot {
            out["last_run_id"] = snap.run_id
            out["last_profile"] = snap.profile
            out["scaffold_locked"] = snap.scaffold_locked
            out["arm_a_score_bps"] = snap.arm_a.score_bps
            out["arm_b_score_bps"] = snap.arm_b.score_bps
            out["arm_a_ttft_p50_us"] = snap.arm_a.ttft_p50_us
            out["arm_b_ttft_p50_us"] = snap.arm_b.ttft_p50_us
            out["arm_a_tpot_p50_us"] = snap.arm_a.tpot_p50_us
            out["arm_b_tpot_p50_us"] = snap.arm_b.tpot_p50_us
            out["arm_a_tok_per_s_x10"] = snap.arm_a.tok_per_s_x10
            out["arm_b_tok_per_s_x10"] = snap.arm_b.tok_per_s_x10
        }
        return out
    }

    func applyRequest(_ req: [String: Any]) -> [String: Any] {
        if let pSlug = req["profile"] as? String,
           let idx = profiles.firstIndex(where: { $0.slug == pSlug }) {
            profilePopup.selectItem(at: idx)
            profileChanged()
        }
        if let aSlug = req["arm_a"] as? String,
           let idx = variants.firstIndex(where: { $0.slug == aSlug }) {
            armAPopup.selectItem(at: idx)
        }
        if let bSlug = req["arm_b"] as? String,
           let idx = variants.firstIndex(where: { $0.slug == bSlug }) {
            armBPopup.selectItem(at: idx)
        }
        if let th = req["thinking"] as? Bool {
            thinkingCheck.state = th ? .on : .off
        }
        if let lim = req["limit"] as? Int, lim > 0 {
            limitField.stringValue = "\(lim)"
        }
        let action = ((req["action"] as? String) ?? "status").lowercased()
        switch action {
        case "load_last", "reload":
            loadSavedSnapshot()
            refreshHillclimbLedger()
        case "compare", "run":
            onRunCompare()
        case "hillclimb":
            onRunHillclimb()
        case "stop":
            onStop()
        default:
            break
        }
        return gbenchState()
    }
}
