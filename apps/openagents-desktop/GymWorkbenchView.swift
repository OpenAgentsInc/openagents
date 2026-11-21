// Firm reason: 6 (AppKit/Metal 3D PLE Gym Workbench view over PleScenePanelView and local run discovery).
// Stage 2 of GitHub #13792: 3D PLE Run Visualizer / Gym Workbench in OpenAgents Desktop.
import AppKit
import CryptoKit
import Foundation
import UniformTypeIdentifiers

// MARK: - Run List Row View

private final class PleGymRowButton: NSView {
    let item: PleGymRunItem
    var onSelect: (() -> Void)?
    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private var hovered = false
    private var selected = false

    init(item: PleGymRunItem) {
        self.item = item
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 7
        layer?.masksToBounds = true

        titleLabel.stringValue = item.title
        titleLabel.font = NSFont.systemFont(ofSize: 12.5, weight: .medium)
        titleLabel.textColor = NSColor(white: 0.93, alpha: 1)
        titleLabel.lineBreakMode = .byTruncatingMiddle
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        subtitleLabel.stringValue = item.subtitle
        subtitleLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .regular)
        subtitleLabel.textColor = NSColor(white: 0.56, alpha: 1)
        subtitleLabel.lineBreakMode = .byTruncatingTail
        subtitleLabel.translatesAutoresizingMaskIntoConstraints = false

        addSubview(titleLabel)
        addSubview(subtitleLabel)
        NSLayoutConstraint.activate([
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 7),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            titleLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            subtitleLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 2),
            subtitleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            subtitleLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            subtitleLabel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -7)
        ])

        let area = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self, userInfo: nil)
        addTrackingArea(area)
        updateAppearance()
    }

    required init?(coder: NSCoder) { fatalError() }

    func setSelected(_ on: Bool) {
        selected = on
        updateAppearance()
    }

    private func updateAppearance() {
        if selected {
            layer?.backgroundColor = NSColor(red: 0.22, green: 0.34, blue: 0.56, alpha: 0.42).cgColor
            layer?.borderWidth = 1
            layer?.borderColor = NSColor(red: 0.40, green: 0.62, blue: 0.95, alpha: 0.55).cgColor
            titleLabel.textColor = .white
        } else if hovered {
            layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.06).cgColor
            layer?.borderWidth = 0
            titleLabel.textColor = NSColor(white: 0.95, alpha: 1)
        } else {
            layer?.backgroundColor = NSColor.clear.cgColor
            layer?.borderWidth = 0
            titleLabel.textColor = NSColor(white: 0.88, alpha: 1)
        }
    }

    override func mouseEntered(with event: NSEvent) { hovered = true; updateAppearance() }
    override func mouseExited(with event: NSEvent) { hovered = false; updateAppearance() }
    override func mouseUp(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        if bounds.contains(pt) { onSelect?() }
    }
}

// MARK: - Clash List Row View (Stage 3)

private final class PleGymClashRowButton: NSView {
    let clash: PleResolvedClash
    var onSelect: (() -> Void)?
    private let titleLabel = NSTextField(labelWithString: "")
    private let detailLabel = NSTextField(labelWithString: "")
    private var hovered = false
    private var selected = false

    init(clash: PleResolvedClash) {
        self.clash = clash
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 6

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 11.5, weight: .semibold)
        titleLabel.textColor = NSColor(calibratedRed: 1.0, green: 0.72, blue: 0.72, alpha: 1.0)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.stringValue = "\(clash.a) ↔ \(clash.b)"

        detailLabel.translatesAutoresizingMaskIntoConstraints = false
        detailLabel.font = NSFont.monospacedSystemFont(ofSize: 10.0, weight: .regular)
        detailLabel.textColor = NSColor(white: 0.68, alpha: 1.0)
        detailLabel.lineBreakMode = .byTruncatingTail
        var parts: [String] = []
        if let pen = clash.penetrationMM {
            parts.append(String(format: "%.1f mm", pen))
        } else if let sev = clash.severity, !sev.isEmpty {
            parts.append(sev)
        }
        let p = clash.point
        parts.append(String(format: "(%.1f, %.1f, %.1f)", p.x, p.y, p.z))
        detailLabel.stringValue = parts.joined(separator: " • ")

        addSubview(titleLabel)
        addSubview(detailLabel)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(greaterThanOrEqualToConstant: 38),
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 5),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            titleLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            detailLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 1),
            detailLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            detailLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            detailLabel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -5),
        ])
        updateStyle()
    }

    required init?(coder: NSCoder) { fatalError() }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas { removeTrackingArea(area) }
        addTrackingArea(NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        ))
    }

    override func mouseEntered(with event: NSEvent) { hovered = true; updateStyle() }
    override func mouseExited(with event: NSEvent) { hovered = false; updateStyle() }
    override func mouseDown(with event: NSEvent) { onSelect?() }

    func setSelected(_ flag: Bool) {
        selected = flag
        updateStyle()
    }

    private func updateStyle() {
        if selected {
            layer?.backgroundColor = NSColor(calibratedRed: 0.60, green: 0.14, blue: 0.18, alpha: 0.42).cgColor
            layer?.borderWidth = 1
            layer?.borderColor = NSColor(calibratedRed: 1.0, green: 0.34, blue: 0.38, alpha: 0.90).cgColor
        } else if hovered {
            layer?.backgroundColor = NSColor(calibratedRed: 0.95, green: 0.25, blue: 0.28, alpha: 0.14).cgColor
            layer?.borderWidth = 0
        } else {
            layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.03).cgColor
            layer?.borderWidth = 0
        }
    }
}


// MARK: - GymWorkbenchView

/// Full-content Gym / 3D PLE Workbench view hosted beside the sidebar when Gym mode is active.
/// Stage 3 adds Live Run Tailing, Clash Inspector & Camera Focus, and Run Diff Overlay.
final class GymWorkbenchView: NSView {
    var workingFolderProvider: (() -> URL?)?

    private let railView = NSView()
    private let dividerView = NSView()
    let scenePanel = PleScenePanelView(frame: .zero)

    private let headerLabel = NSTextField(labelWithString: "PLE Gym Runs")
    private let refreshButton = NSButton(title: "Refresh", target: nil, action: nil)
    private let openFileButton = NSButton(title: "Open…", target: nil, action: nil)

    // Stage 3: Live Run Tailing controls
    private let liveToggleButton = NSButton(title: "Live", target: nil, action: nil)
    private let liveIndicatorDot = NSView()
    private let liveStatusLabel = NSTextField(labelWithString: "Live off")
    var liveEnabled = false
    private var liveTimer: Timer?
    private var lastObservedSignature: PleFileSignature?
    private var liveReloadInFlight = false

    // Stage 3: Run Diff Overlay controls
    private let compareLabel = NSTextField(labelWithString: "Compare:")
    private let comparePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    var activeScene: PleScene?
    private var baselineId: String?
    private var baselineScene: PleScene?
    private var baselineLabel: String?
    private var baselineGeneration: Int = 0

    // Runs list
    private let scrollView = NSScrollView()
    private let stackView = NSStackView()
    private var rowViews: [PleGymRowButton] = []

    // Stage 3 + Stage 4: Bottom rail drawer (Clashes | Diagnostics & Logs)
    private let clashDivider = NSView()
    private let railTabControl = NSSegmentedControl(labels: ["Clashes (0)", "Diagnostics"], trackingMode: .selectOne, target: nil, action: nil)
    private let clashHeaderLabel = NSTextField(labelWithString: "Clashes (0)")
    private let clashScrollView = NSScrollView()
    private let clashStackView = NSStackView()
    private let clashEmptyLabel = NSTextField(labelWithString: "No clashes detected ✓")
    private var clashRowViews: [PleGymClashRowButton] = []
    private var selectedClashId: String?

    // Stage 4: Diagnostics & Log Drawer
    private let diagContainer = NSView()
    private let diagMetricsLabel = NSTextField(labelWithString: "No run metadata")
    private let diagGatesScroll = NSScrollView()
    private let diagGatesStack = NSStackView()
    private let diagFilePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let diagCopyButton = NSButton(title: "Copy", target: nil, action: nil)
    private let diagLogScroll = NSScrollView()
    private let diagLogTextView = NSTextView()
    private var discoveredLogs: [PleGymLogFile] = []
    private var showingDiagnostics = false

    // Stage 4: Timeline Scrubber & Snapshot Playback bar
    private let timelineBar = NSView()
    private let timelineTopDivider = NSView()
    private let playPauseButton = NSButton(title: "▶", target: nil, action: nil)
    private let prevSnapButton = NSButton(title: "◀", target: nil, action: nil)
    private let nextSnapButton = NSButton(title: "▶", target: nil, action: nil)
    private let timelineSlider = NSSlider(value: 0, minValue: 0, maxValue: 0, target: nil, action: nil)
    private let timelineLabel = NSTextField(labelWithString: "1/1 • Current")
    private let pinSnapshotButton = NSButton(title: "Pin", target: nil, action: nil)
    var snapshots: [PleGymSnapshot] = []
    var currentSnapshotIndex: Int = 0
    private var isPlayingTimeline = false
    private var playbackTimer: Timer?
    private var snapshotLoadGeneration: Int = 0
    private var pinCounter: Int = 0

    let statusLabel = NSTextField(labelWithString: "")
    private let emptyLabel = NSTextField(labelWithString: "Select a PLE run or click Open… to inspect a .ple-scene.json or .ifc model")

    var items: [PleGymRunItem] = []
    private var selectedId: String?
    private var loadGeneration: Int = 0

    // A program's request being carried out (applyPlan): what is left of it once a scene is in, and
    // who hears how it went.
    var pendingPlan: PleGymPlan?
    var pendingPlanDone: ((String?) -> Void)?

    // Top mode switcher: Benchmarks (0) | Terminal-Bench (3) | PLE 3D Workbench (1)
    private let modeBar = NSView()
    private let modeBarDivider = NSView()
    private let modeTitleLabel = NSTextField(labelWithString: "GYM")
    private let modeControl = NSSegmentedControl(labels: ["Benchmarks", "PLE 3D"], trackingMode: .selectOne, target: nil, action: nil)
    let benchmarkView = GymBenchmarkView(frame: .zero)
    let terminalBenchView = GymTerminalBenchView(frame: .zero)
    private var activeGymMode: Int32 = 0

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor(red: 0.06, green: 0.065, blue: 0.08, alpha: 1).cgColor
        setupUI()
        applyModeVisibility()
    }

    required init?(coder: NSCoder) { fatalError() }

    deinit {
        liveTimer?.invalidate()
        playbackTimer?.invalidate()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            stopLiveTimer()
            stopPlayback()
        } else if liveEnabled {
            startLiveTimer()
        }
    }

    private func setupUI() {
        modeBar.translatesAutoresizingMaskIntoConstraints = false
        modeBar.wantsLayer = true
        modeBar.layer?.backgroundColor = NSColor(red: 0.068, green: 0.072, blue: 0.090, alpha: 1).cgColor
        modeBar.isHidden = true
        addSubview(modeBar)

        modeBarDivider.translatesAutoresizingMaskIntoConstraints = false
        modeBarDivider.wantsLayer = true
        modeBarDivider.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.08).cgColor
        modeBarDivider.isHidden = true
        addSubview(modeBarDivider)

        modeTitleLabel.translatesAutoresizingMaskIntoConstraints = false
        modeTitleLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .bold)
        modeTitleLabel.textColor = NSColor(white: 0.56, alpha: 1)
        modeBar.addSubview(modeTitleLabel)

        modeControl.translatesAutoresizingMaskIntoConstraints = false
        modeControl.controlSize = .small
        modeControl.segmentStyle = .rounded
        modeControl.selectedSegment = 0
        modeControl.target = self
        modeControl.action = #selector(modeSegmentChanged)
        modeBar.addSubview(modeControl)

        benchmarkView.translatesAutoresizingMaskIntoConstraints = false
        benchmarkView.workingFolderProvider = { [weak self] in self?.workingFolderProvider?() }
        addSubview(benchmarkView)

        terminalBenchView.translatesAutoresizingMaskIntoConstraints = false
        terminalBenchView.workingFolderProvider = { [weak self] in self?.workingFolderProvider?() }
        addSubview(terminalBenchView)

        railView.translatesAutoresizingMaskIntoConstraints = false
        railView.wantsLayer = true
        railView.layer?.backgroundColor = NSColor(red: 0.075, green: 0.08, blue: 0.10, alpha: 1).cgColor
        addSubview(railView)

        dividerView.translatesAutoresizingMaskIntoConstraints = false
        dividerView.wantsLayer = true
        dividerView.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.08).cgColor
        addSubview(dividerView)

        scenePanel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scenePanel)

        emptyLabel.translatesAutoresizingMaskIntoConstraints = false
        emptyLabel.font = NSFont.systemFont(ofSize: 13, weight: .medium)
        emptyLabel.textColor = NSColor(white: 0.60, alpha: 1)
        emptyLabel.alignment = .center
        emptyLabel.isHidden = true
        addSubview(emptyLabel)

        // Header row 1: Title + Refresh + Open
        headerLabel.translatesAutoresizingMaskIntoConstraints = false
        headerLabel.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        headerLabel.textColor = NSColor(white: 0.94, alpha: 1)
        railView.addSubview(headerLabel)

        for (btn, sel) in [(refreshButton, #selector(refreshClicked)), (openFileButton, #selector(openFileClicked))] {
            btn.translatesAutoresizingMaskIntoConstraints = false
            btn.bezelStyle = .rounded
            btn.controlSize = .small
            btn.font = NSFont.systemFont(ofSize: 11, weight: .medium)
            btn.target = self
            btn.action = sel
            railView.addSubview(btn)
        }

        // Header row 2 (Stage 3): Live toggle + status dot + status label
        liveToggleButton.translatesAutoresizingMaskIntoConstraints = false
        liveToggleButton.isBordered = false
        liveToggleButton.wantsLayer = true
        liveToggleButton.layer?.cornerRadius = 5
        liveToggleButton.target = self
        liveToggleButton.action = #selector(liveToggled)
        railView.addSubview(liveToggleButton)

        liveIndicatorDot.translatesAutoresizingMaskIntoConstraints = false
        liveIndicatorDot.wantsLayer = true
        liveIndicatorDot.layer?.cornerRadius = 4
        railView.addSubview(liveIndicatorDot)

        liveStatusLabel.translatesAutoresizingMaskIntoConstraints = false
        liveStatusLabel.font = NSFont.monospacedSystemFont(ofSize: 10.0, weight: .regular)
        liveStatusLabel.textColor = NSColor(white: 0.60, alpha: 1)
        liveStatusLabel.lineBreakMode = .byTruncatingTail
        railView.addSubview(liveStatusLabel)
        updateLiveControlsUI()

        // Header row 3 (Stage 3): Compare baseline picker
        compareLabel.translatesAutoresizingMaskIntoConstraints = false
        compareLabel.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        compareLabel.textColor = NSColor(white: 0.72, alpha: 1)
        railView.addSubview(compareLabel)

        comparePopup.translatesAutoresizingMaskIntoConstraints = false
        comparePopup.controlSize = .small
        comparePopup.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        comparePopup.target = self
        comparePopup.action = #selector(compareChanged)
        railView.addSubview(comparePopup)

        // Runs scroll list
        stackView.orientation = .vertical
        stackView.alignment = .leading
        stackView.spacing = 4
        stackView.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 8, right: 8)
        stackView.translatesAutoresizingMaskIntoConstraints = false

        let clipDoc = NSView()
        clipDoc.translatesAutoresizingMaskIntoConstraints = false
        clipDoc.addSubview(stackView)
        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: clipDoc.topAnchor),
            stackView.leadingAnchor.constraint(equalTo: clipDoc.leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: clipDoc.trailingAnchor),
            stackView.bottomAnchor.constraint(lessThanOrEqualTo: clipDoc.bottomAnchor)
        ])

        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.documentView = clipDoc
        railView.addSubview(scrollView)

        // Stage 3: Clash Inspector section
        clashDivider.translatesAutoresizingMaskIntoConstraints = false
        clashDivider.wantsLayer = true
        clashDivider.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.10).cgColor
        railView.addSubview(clashDivider)

        railTabControl.translatesAutoresizingMaskIntoConstraints = false
        railTabControl.controlSize = .small
        railTabControl.segmentStyle = .rounded
        railTabControl.selectedSegment = 0
        railTabControl.target = self
        railTabControl.action = #selector(railTabChanged)
        railView.addSubview(railTabControl)

        clashHeaderLabel.translatesAutoresizingMaskIntoConstraints = false
        clashHeaderLabel.isHidden = true

        clashStackView.orientation = .vertical
        clashStackView.alignment = .leading
        clashStackView.spacing = 4
        clashStackView.edgeInsets = NSEdgeInsets(top: 4, left: 8, bottom: 6, right: 8)
        clashStackView.translatesAutoresizingMaskIntoConstraints = false

        let clashDoc = NSView()
        clashDoc.translatesAutoresizingMaskIntoConstraints = false
        clashDoc.addSubview(clashStackView)
        NSLayoutConstraint.activate([
            clashStackView.topAnchor.constraint(equalTo: clashDoc.topAnchor),
            clashStackView.leadingAnchor.constraint(equalTo: clashDoc.leadingAnchor),
            clashStackView.trailingAnchor.constraint(equalTo: clashDoc.trailingAnchor),
            clashStackView.bottomAnchor.constraint(lessThanOrEqualTo: clashDoc.bottomAnchor)
        ])

        clashScrollView.translatesAutoresizingMaskIntoConstraints = false
        clashScrollView.drawsBackground = false
        clashScrollView.hasVerticalScroller = true
        clashScrollView.autohidesScrollers = true
        clashScrollView.borderType = .noBorder
        clashScrollView.documentView = clashDoc
        railView.addSubview(clashScrollView)

        clashEmptyLabel.translatesAutoresizingMaskIntoConstraints = false
        clashEmptyLabel.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        clashEmptyLabel.textColor = NSColor(calibratedRed: 0.42, green: 0.82, blue: 0.54, alpha: 0.9)
        railView.addSubview(clashEmptyLabel)

        // Stage 4: Diagnostics & Log Drawer container
        diagContainer.translatesAutoresizingMaskIntoConstraints = false
        diagContainer.isHidden = true
        railView.addSubview(diagContainer)

        diagMetricsLabel.translatesAutoresizingMaskIntoConstraints = false
        diagMetricsLabel.font = NSFont.monospacedSystemFont(ofSize: 10.0, weight: .medium)
        diagMetricsLabel.textColor = NSColor(white: 0.78, alpha: 1)
        diagMetricsLabel.lineBreakMode = .byTruncatingTail
        diagContainer.addSubview(diagMetricsLabel)

        diagGatesStack.orientation = .horizontal
        diagGatesStack.alignment = .centerY
        diagGatesStack.spacing = 4
        diagGatesStack.edgeInsets = NSEdgeInsets(top: 0, left: 2, bottom: 0, right: 2)
        diagGatesStack.translatesAutoresizingMaskIntoConstraints = false

        let gatesDoc = NSView()
        gatesDoc.translatesAutoresizingMaskIntoConstraints = false
        gatesDoc.addSubview(diagGatesStack)
        NSLayoutConstraint.activate([
            diagGatesStack.topAnchor.constraint(equalTo: gatesDoc.topAnchor),
            diagGatesStack.bottomAnchor.constraint(equalTo: gatesDoc.bottomAnchor),
            diagGatesStack.leadingAnchor.constraint(equalTo: gatesDoc.leadingAnchor),
            diagGatesStack.trailingAnchor.constraint(lessThanOrEqualTo: gatesDoc.trailingAnchor)
        ])

        diagGatesScroll.translatesAutoresizingMaskIntoConstraints = false
        diagGatesScroll.drawsBackground = false
        diagGatesScroll.hasHorizontalScroller = false
        diagGatesScroll.hasVerticalScroller = false
        diagGatesScroll.borderType = .noBorder
        diagGatesScroll.documentView = gatesDoc
        diagContainer.addSubview(diagGatesScroll)

        diagFilePopup.translatesAutoresizingMaskIntoConstraints = false
        diagFilePopup.controlSize = .small
        diagFilePopup.font = NSFont.systemFont(ofSize: 10.5)
        diagFilePopup.target = self
        diagFilePopup.action = #selector(diagFileChanged)
        diagContainer.addSubview(diagFilePopup)

        diagCopyButton.translatesAutoresizingMaskIntoConstraints = false
        diagCopyButton.bezelStyle = .rounded
        diagCopyButton.controlSize = .small
        diagCopyButton.target = self
        diagCopyButton.action = #selector(copyDiagLogPressed)
        diagContainer.addSubview(diagCopyButton)

        diagLogTextView.isEditable = false
        diagLogTextView.isSelectable = true
        diagLogTextView.drawsBackground = true
        diagLogTextView.backgroundColor = NSColor(red: 0.05, green: 0.055, blue: 0.07, alpha: 1)
        diagLogTextView.textColor = NSColor(white: 0.84, alpha: 1)
        diagLogTextView.font = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .regular)
        diagLogTextView.textContainerInset = NSSize(width: 6, height: 6)
        diagLogTextView.isHorizontallyResizable = false
        diagLogTextView.isVerticallyResizable = true
        diagLogTextView.autoresizingMask = [.width]
        diagLogTextView.textContainer?.widthTracksTextView = true

        diagLogScroll.translatesAutoresizingMaskIntoConstraints = false
        diagLogScroll.drawsBackground = true
        diagLogScroll.backgroundColor = NSColor(red: 0.05, green: 0.055, blue: 0.07, alpha: 1)
        diagLogScroll.hasVerticalScroller = true
        diagLogScroll.autohidesScrollers = true
        diagLogScroll.borderType = .noBorder
        diagLogScroll.wantsLayer = true
        diagLogScroll.layer?.cornerRadius = 5
        diagLogScroll.layer?.borderWidth = 1
        diagLogScroll.layer?.borderColor = NSColor(white: 1.0, alpha: 0.08).cgColor
        diagLogScroll.documentView = diagLogTextView
        diagContainer.addSubview(diagLogScroll)

        NSLayoutConstraint.activate([
            diagMetricsLabel.topAnchor.constraint(equalTo: diagContainer.topAnchor, constant: 2),
            diagMetricsLabel.leadingAnchor.constraint(equalTo: diagContainer.leadingAnchor, constant: 10),
            diagMetricsLabel.trailingAnchor.constraint(equalTo: diagContainer.trailingAnchor, constant: -10),

            diagGatesScroll.topAnchor.constraint(equalTo: diagMetricsLabel.bottomAnchor, constant: 4),
            diagGatesScroll.leadingAnchor.constraint(equalTo: diagContainer.leadingAnchor, constant: 8),
            diagGatesScroll.trailingAnchor.constraint(equalTo: diagContainer.trailingAnchor, constant: -8),
            diagGatesScroll.heightAnchor.constraint(equalToConstant: 22),
            gatesDoc.heightAnchor.constraint(equalTo: diagGatesScroll.heightAnchor),

            diagFilePopup.topAnchor.constraint(equalTo: diagGatesScroll.bottomAnchor, constant: 5),
            diagFilePopup.leadingAnchor.constraint(equalTo: diagContainer.leadingAnchor, constant: 8),
            diagFilePopup.trailingAnchor.constraint(equalTo: diagCopyButton.leadingAnchor, constant: -6),

            diagCopyButton.centerYAnchor.constraint(equalTo: diagFilePopup.centerYAnchor),
            diagCopyButton.trailingAnchor.constraint(equalTo: diagContainer.trailingAnchor, constant: -8),

            diagLogScroll.topAnchor.constraint(equalTo: diagFilePopup.bottomAnchor, constant: 5),
            diagLogScroll.leadingAnchor.constraint(equalTo: diagContainer.leadingAnchor, constant: 8),
            diagLogScroll.trailingAnchor.constraint(equalTo: diagContainer.trailingAnchor, constant: -8),
            diagLogScroll.bottomAnchor.constraint(equalTo: diagContainer.bottomAnchor),
        ])

        // Stage 4: Timeline Scrubber bar at the bottom of the 3D viewport
        timelineBar.translatesAutoresizingMaskIntoConstraints = false
        timelineBar.wantsLayer = true
        timelineBar.layer?.backgroundColor = NSColor(red: 0.07, green: 0.078, blue: 0.098, alpha: 1).cgColor
        addSubview(timelineBar)

        timelineTopDivider.translatesAutoresizingMaskIntoConstraints = false
        timelineTopDivider.wantsLayer = true
        timelineTopDivider.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.09).cgColor
        timelineBar.addSubview(timelineTopDivider)

        func styleTimelineBtn(_ b: NSButton, title: String, action: Selector) {
            b.translatesAutoresizingMaskIntoConstraints = false
            b.bezelStyle = .rounded
            b.controlSize = .small
            b.title = title
            b.target = self
            b.action = action
            timelineBar.addSubview(b)
        }
        styleTimelineBtn(playPauseButton, title: "▶", action: #selector(playPausePressed))
        styleTimelineBtn(prevSnapButton, title: "◀", action: #selector(prevSnapshotPressed))
        styleTimelineBtn(nextSnapButton, title: "▶|", action: #selector(nextSnapshotPressed))
        styleTimelineBtn(pinSnapshotButton, title: "Pin", action: #selector(pinSnapshotPressed))

        timelineSlider.translatesAutoresizingMaskIntoConstraints = false
        timelineSlider.controlSize = .small
        timelineSlider.isContinuous = true
        timelineSlider.target = self
        timelineSlider.action = #selector(timelineSliderChanged)
        timelineBar.addSubview(timelineSlider)

        timelineLabel.translatesAutoresizingMaskIntoConstraints = false
        timelineLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .medium)
        timelineLabel.textColor = NSColor(white: 0.82, alpha: 1)
        timelineLabel.lineBreakMode = .byTruncatingMiddle
        timelineBar.addSubview(timelineLabel)

        NSLayoutConstraint.activate([
            timelineTopDivider.topAnchor.constraint(equalTo: timelineBar.topAnchor),
            timelineTopDivider.leadingAnchor.constraint(equalTo: timelineBar.leadingAnchor),
            timelineTopDivider.trailingAnchor.constraint(equalTo: timelineBar.trailingAnchor),
            timelineTopDivider.heightAnchor.constraint(equalToConstant: 1),

            playPauseButton.centerYAnchor.constraint(equalTo: timelineBar.centerYAnchor),
            playPauseButton.leadingAnchor.constraint(equalTo: timelineBar.leadingAnchor, constant: 10),
            playPauseButton.widthAnchor.constraint(equalToConstant: 30),

            prevSnapButton.centerYAnchor.constraint(equalTo: timelineBar.centerYAnchor),
            prevSnapButton.leadingAnchor.constraint(equalTo: playPauseButton.trailingAnchor, constant: 4),
            prevSnapButton.widthAnchor.constraint(equalToConstant: 28),

            nextSnapButton.centerYAnchor.constraint(equalTo: timelineBar.centerYAnchor),
            nextSnapButton.leadingAnchor.constraint(equalTo: prevSnapButton.trailingAnchor, constant: 4),
            nextSnapButton.widthAnchor.constraint(equalToConstant: 32),

            timelineSlider.centerYAnchor.constraint(equalTo: timelineBar.centerYAnchor),
            timelineSlider.leadingAnchor.constraint(equalTo: nextSnapButton.trailingAnchor, constant: 10),
            timelineSlider.widthAnchor.constraint(greaterThanOrEqualToConstant: 140),

            timelineLabel.centerYAnchor.constraint(equalTo: timelineBar.centerYAnchor),
            timelineLabel.leadingAnchor.constraint(equalTo: timelineSlider.trailingAnchor, constant: 10),
            timelineLabel.trailingAnchor.constraint(lessThanOrEqualTo: pinSnapshotButton.leadingAnchor, constant: -10),

            pinSnapshotButton.centerYAnchor.constraint(equalTo: timelineBar.centerYAnchor),
            pinSnapshotButton.trailingAnchor.constraint(equalTo: timelineBar.trailingAnchor, constant: -10),
        ])

        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .regular)
        statusLabel.textColor = NSColor(white: 0.55, alpha: 1)
        statusLabel.lineBreakMode = .byTruncatingMiddle
        railView.addSubview(statusLabel)

        NSLayoutConstraint.activate([
            // Unified 36 pt GYM mode bar is now rendered above GymWorkbenchView by Coax (page/gym-lean.coax; #13946).
            modeBar.topAnchor.constraint(equalTo: topAnchor),
            modeBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            modeBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            modeBar.heightAnchor.constraint(equalToConstant: 0),

            modeTitleLabel.centerYAnchor.constraint(equalTo: modeBar.centerYAnchor),
            modeTitleLabel.leadingAnchor.constraint(equalTo: modeBar.leadingAnchor, constant: 14),

            modeControl.centerYAnchor.constraint(equalTo: modeBar.centerYAnchor),
            modeControl.leadingAnchor.constraint(equalTo: modeTitleLabel.trailingAnchor, constant: 10),

            modeBarDivider.topAnchor.constraint(equalTo: modeBar.bottomAnchor),
            modeBarDivider.leadingAnchor.constraint(equalTo: leadingAnchor),
            modeBarDivider.trailingAnchor.constraint(equalTo: trailingAnchor),
            modeBarDivider.heightAnchor.constraint(equalToConstant: 0),

            benchmarkView.topAnchor.constraint(equalTo: modeBarDivider.bottomAnchor),
            benchmarkView.bottomAnchor.constraint(equalTo: bottomAnchor),
            benchmarkView.leadingAnchor.constraint(equalTo: leadingAnchor),
            benchmarkView.trailingAnchor.constraint(equalTo: trailingAnchor),

            terminalBenchView.topAnchor.constraint(equalTo: modeBarDivider.bottomAnchor),
            terminalBenchView.bottomAnchor.constraint(equalTo: bottomAnchor),
            terminalBenchView.leadingAnchor.constraint(equalTo: leadingAnchor),
            terminalBenchView.trailingAnchor.constraint(equalTo: trailingAnchor),

            railView.topAnchor.constraint(equalTo: modeBarDivider.bottomAnchor),
            railView.bottomAnchor.constraint(equalTo: bottomAnchor),
            railView.leadingAnchor.constraint(equalTo: leadingAnchor),
            railView.widthAnchor.constraint(equalToConstant: 268),

            dividerView.topAnchor.constraint(equalTo: modeBarDivider.bottomAnchor),
            dividerView.bottomAnchor.constraint(equalTo: bottomAnchor),
            dividerView.leadingAnchor.constraint(equalTo: railView.trailingAnchor),
            dividerView.widthAnchor.constraint(equalToConstant: 1),

            scenePanel.topAnchor.constraint(equalTo: modeBarDivider.bottomAnchor),
            scenePanel.bottomAnchor.constraint(equalTo: timelineBar.topAnchor),
            scenePanel.leadingAnchor.constraint(equalTo: dividerView.trailingAnchor),
            scenePanel.trailingAnchor.constraint(equalTo: trailingAnchor),

            timelineBar.leadingAnchor.constraint(equalTo: dividerView.trailingAnchor),
            timelineBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            timelineBar.bottomAnchor.constraint(equalTo: bottomAnchor),
            timelineBar.heightAnchor.constraint(equalToConstant: 38),

            emptyLabel.centerXAnchor.constraint(equalTo: scenePanel.centerXAnchor),
            emptyLabel.centerYAnchor.constraint(equalTo: scenePanel.centerYAnchor),
            emptyLabel.widthAnchor.constraint(lessThanOrEqualTo: scenePanel.widthAnchor, constant: -48),

            // Row 1: Header + Refresh + Open
            headerLabel.topAnchor.constraint(equalTo: railView.topAnchor, constant: 12),
            headerLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),

            openFileButton.centerYAnchor.constraint(equalTo: headerLabel.centerYAnchor),
            openFileButton.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -10),

            refreshButton.centerYAnchor.constraint(equalTo: headerLabel.centerYAnchor),
            refreshButton.trailingAnchor.constraint(equalTo: openFileButton.leadingAnchor, constant: -6),

            // Row 2: Live toggle + dot + status
            liveToggleButton.topAnchor.constraint(equalTo: headerLabel.bottomAnchor, constant: 8),
            liveToggleButton.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),
            liveToggleButton.heightAnchor.constraint(equalToConstant: 22),

            liveIndicatorDot.centerYAnchor.constraint(equalTo: liveToggleButton.centerYAnchor),
            liveIndicatorDot.leadingAnchor.constraint(equalTo: liveToggleButton.trailingAnchor, constant: 8),
            liveIndicatorDot.widthAnchor.constraint(equalToConstant: 8),
            liveIndicatorDot.heightAnchor.constraint(equalToConstant: 8),

            liveStatusLabel.centerYAnchor.constraint(equalTo: liveToggleButton.centerYAnchor),
            liveStatusLabel.leadingAnchor.constraint(equalTo: liveIndicatorDot.trailingAnchor, constant: 6),
            liveStatusLabel.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -10),

            // Row 3: Compare picker
            compareLabel.topAnchor.constraint(equalTo: liveToggleButton.bottomAnchor, constant: 8),
            compareLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),

            comparePopup.centerYAnchor.constraint(equalTo: compareLabel.centerYAnchor),
            comparePopup.leadingAnchor.constraint(equalTo: compareLabel.trailingAnchor, constant: 6),
            comparePopup.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -10),

            // Runs list
            scrollView.topAnchor.constraint(equalTo: comparePopup.bottomAnchor, constant: 8),
            scrollView.leadingAnchor.constraint(equalTo: railView.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: railView.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: clashDivider.topAnchor, constant: -4),

            clipDoc.widthAnchor.constraint(equalTo: scrollView.widthAnchor),

            // Clash Inspector section
            clashDivider.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 8),
            clashDivider.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -8),
            clashDivider.heightAnchor.constraint(equalToConstant: 1),

            railTabControl.topAnchor.constraint(equalTo: clashDivider.bottomAnchor, constant: 6),
            railTabControl.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 10),
            railTabControl.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -10),

            clashScrollView.topAnchor.constraint(equalTo: railTabControl.bottomAnchor, constant: 6),
            clashScrollView.leadingAnchor.constraint(equalTo: railView.leadingAnchor),
            clashScrollView.trailingAnchor.constraint(equalTo: railView.trailingAnchor),
            clashScrollView.heightAnchor.constraint(equalToConstant: 210),
            clashScrollView.bottomAnchor.constraint(equalTo: statusLabel.topAnchor, constant: -6),

            clashDoc.widthAnchor.constraint(equalTo: clashScrollView.widthAnchor),

            clashEmptyLabel.topAnchor.constraint(equalTo: railTabControl.bottomAnchor, constant: 12),
            clashEmptyLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),

            diagContainer.topAnchor.constraint(equalTo: railTabControl.bottomAnchor, constant: 6),
            diagContainer.leadingAnchor.constraint(equalTo: railView.leadingAnchor),
            diagContainer.trailingAnchor.constraint(equalTo: railView.trailingAnchor),
            diagContainer.bottomAnchor.constraint(equalTo: statusLabel.topAnchor, constant: -6),

            statusLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),
            statusLabel.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -12),
            statusLabel.bottomAnchor.constraint(equalTo: railView.bottomAnchor, constant: -8),
        ])

        rebuildCompareMenu()
    }

    @objc private func modeSegmentChanged() {
        setGymMode(modeControl.selectedSegment == 0 ? 0 : 1)
    }

    func setModeBenchmarks(_ benchmarks: Bool) {
        setGymMode(benchmarks ? 0 : 1)
    }

    func setGymMode(_ mode: Int32) {
        activeGymMode = mode
        if mode == 0 || mode == 1 {
            modeControl.selectedSegment = Int(mode)
        }
        applyModeVisibility()
        if mode == 3 {
            terminalBenchView.onBecomeVisible()
        }
    }

    private func applyModeVisibility() {
        let isBench = (activeGymMode == 0)
        let isTbench = (activeGymMode == 3)
        let isPle = !isBench && !isTbench
        benchmarkView.isHidden = !isBench
        terminalBenchView.isHidden = !isTbench
        railView.isHidden = !isPle
        dividerView.isHidden = !isPle
        scenePanel.isHidden = !isPle
        timelineBar.isHidden = !isPle
        emptyLabel.isHidden = isPle ? !items.isEmpty : true
    }

    // MARK: - Discovery & Run Selection

    func refreshRuns(autoSelectFirst: Bool) {
        let wf = workingFolderProvider?()
        let discovered = PleGymDiscovery.discover(workingFolder: wf)
        items = discovered
        rebuildRows()
        rebuildCompareMenu()
        statusLabel.stringValue = discovered.isEmpty
            ? "No runs found"
            : "\(discovered.count) run\(discovered.count == 1 ? "" : "s")"

        if let sel = selectedId, let existing = discovered.first(where: { $0.id == sel }) {
            updateSelectionHighlight(existing.id)
        } else if autoSelectFirst, let first = discovered.first {
            selectItem(first)
        } else if discovered.isEmpty {
            emptyLabel.isHidden = false
        }
    }

    func rebuildRows() {
        for v in rowViews { v.removeFromSuperview() }
        rowViews.removeAll()

        for item in items {
            let row = PleGymRowButton(item: item)
            row.translatesAutoresizingMaskIntoConstraints = false
            row.setSelected(item.id == selectedId)
            row.onSelect = { [weak self] in
                self?.selectItem(item)
            }
            stackView.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: stackView.widthAnchor, constant: -16).isActive = true
            rowViews.append(row)
        }
    }

    private func updateSelectionHighlight(_ id: String?) {
        selectedId = id
        for row in rowViews {
            row.setSelected(row.item.id == id)
        }
    }

    var selectedItem: PleGymRunItem? {
        guard let id = selectedId else { return nil }
        return items.first(where: { $0.id == id })
    }

    func selectItem(_ item: PleGymRunItem) {
        updateSelectionHighlight(item.id)
        // If the newly selected item was the baseline, clear baseline diff.
        if baselineId == item.id {
            clearBaseline()
        }
        rebuildCompareMenu()
        stopPlayback()
        lastObservedSignature = PleFileSignature.read(from: item.url)
        refreshTimelineSnapshots(for: item)
        refreshDiagnosticsFiles(for: item)
        loadActiveItem(item, preserveView: false, forceReconvert: false, isLiveUpdate: false)
    }

    private func loadActiveItem(_ item: PleGymRunItem, preserveView: Bool, forceReconvert: Bool, isLiveUpdate: Bool) {
        loadGeneration += 1
        let gen = loadGeneration
        let wf = workingFolderProvider?()
        emptyLabel.isHidden = true
        if !isLiveUpdate {
            statusLabel.stringValue = item.isIfc ? "Converting \(item.url.lastPathComponent)…" : "Loading \(item.title)…"
        } else {
            liveStatusLabel.stringValue = "Reloading…"
        }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                let scene = try PleGymDiscovery.loadScene(for: item, workingFolder: wf, forceReconvert: forceReconvert)
                DispatchQueue.main.async {
                    guard let self, self.loadGeneration == gen else { return }
                    self.liveReloadInFlight = false
                    self.activeScene = scene
                    self.scenePanel.loadScene(scene, preserveView: preserveView)
                    self.rebuildClashRows(for: scene)
                    self.updateDiagnostics(for: scene, item: item)
                    self.recomputeDiff()
                    if isLiveUpdate {
                        self.appendLiveSnapshot(scene: scene)
                        let df = DateFormatter()
                        df.dateFormat = "HH:mm:ss"
                        self.liveStatusLabel.stringValue = "Updated \(df.string(from: Date()))"
                    } else {
                        self.attachLoadedSceneToCurrentSnapshot(scene: scene, item: item)
                    }
                    let clashCount = scene.resolvedClashes.count
                    self.statusLabel.stringValue = "\(item.title) • \(scene.elements.count) elems • \(clashCount) clash\(clashCount == 1 ? "" : "es")"
                    if !isLiveUpdate { self.planSceneReady() }
                }
            } catch {
                DispatchQueue.main.async {
                    guard let self, self.loadGeneration == gen else { return }
                    self.liveReloadInFlight = false
                    if isLiveUpdate {
                        self.liveStatusLabel.stringValue = "Reload error"
                    } else {
                        self.statusLabel.stringValue = "Error: \(error.localizedDescription)"
                        self.planFinished("could not load \(item.url.path): \(error.localizedDescription)")
                    }
                }
            }
        }
    }

    // MARK: - Stage 3: Live Run Tailing

    func setLiveTailingEnabled(_ enabled: Bool) {
        liveEnabled = enabled
        updateLiveControlsUI()
        if enabled {
            if let item = selectedItem {
                lastObservedSignature = PleFileSignature.read(from: item.url)
            } else {
                lastObservedSignature = nil
            }
            startLiveTimer()
        } else {
            stopLiveTimer()
        }
    }

    @objc private func liveToggled() {
        setLiveTailingEnabled(!liveEnabled)
    }

    private func updateLiveControlsUI() {
        if liveEnabled {
            liveToggleButton.layer?.backgroundColor = NSColor(calibratedRed: 0.16, green: 0.62, blue: 0.34, alpha: 0.35).cgColor
            liveToggleButton.layer?.borderWidth = 1
            liveToggleButton.layer?.borderColor = NSColor(calibratedRed: 0.28, green: 0.88, blue: 0.48, alpha: 0.85).cgColor
            let attr = NSAttributedString(string: " Live ", attributes: [
                .font: NSFont.systemFont(ofSize: 11, weight: .semibold),
                .foregroundColor: NSColor(calibratedRed: 0.72, green: 1.0, blue: 0.80, alpha: 1.0),
            ])
            liveToggleButton.attributedTitle = attr
            liveIndicatorDot.layer?.backgroundColor = NSColor(calibratedRed: 0.24, green: 0.88, blue: 0.46, alpha: 1.0).cgColor
            liveStatusLabel.stringValue = "Watching (1.5s)"
            liveStatusLabel.textColor = NSColor(calibratedRed: 0.66, green: 0.92, blue: 0.74, alpha: 1.0)
        } else {
            liveToggleButton.layer?.backgroundColor = NSColor(white: 1.0, alpha: 0.06).cgColor
            liveToggleButton.layer?.borderWidth = 1
            liveToggleButton.layer?.borderColor = NSColor(white: 1.0, alpha: 0.14).cgColor
            let attr = NSAttributedString(string: " Live ", attributes: [
                .font: NSFont.systemFont(ofSize: 11, weight: .medium),
                .foregroundColor: NSColor(white: 0.78, alpha: 1.0),
            ])
            liveToggleButton.attributedTitle = attr
            liveIndicatorDot.layer?.backgroundColor = NSColor(white: 0.40, alpha: 1.0).cgColor
            liveStatusLabel.stringValue = "Live off"
            liveStatusLabel.textColor = NSColor(white: 0.55, alpha: 1.0)
        }
    }

    private func startLiveTimer() {
        stopLiveTimer()
        guard window != nil else { return }
        let timer = Timer(timeInterval: 1.5, repeats: true) { [weak self] _ in
            self?.pollActiveRunForChanges()
        }
        RunLoop.main.add(timer, forMode: .common)
        liveTimer = timer
    }

    private func stopLiveTimer() {
        liveTimer?.invalidate()
        liveTimer = nil
    }

    /// Checks the currently selected run's source file for mtime/size changes and reloads while preserving camera & layers.
    func pollActiveRunForChanges() {
        guard liveEnabled, !isHiddenOrHasHiddenAncestor, !liveReloadInFlight, let item = selectedItem else { return }
        guard let currentSig = PleFileSignature.read(from: item.url) else { return }
        if let prev = lastObservedSignature, prev == currentSig {
            return
        }
        lastObservedSignature = currentSig
        liveReloadInFlight = true
        loadActiveItem(item, preserveView: true, forceReconvert: item.isIfc, isLiveUpdate: true)
    }

    // MARK: - Stage 3: Clash Inspector & Camera Focus

    private func rebuildClashRows(for scene: PleScene) {
        for v in clashRowViews { v.removeFromSuperview() }
        clashRowViews.removeAll()
        let clashes = scene.resolvedClashes
        clashHeaderLabel.stringValue = "Clashes (\(clashes.count))"
        railTabControl.setLabel("Clashes (\(clashes.count))", forSegment: 0)
        clashEmptyLabel.isHidden = showingDiagnostics || !clashes.isEmpty
        selectedClashId = nil

        for clash in clashes {
            let row = PleGymClashRowButton(clash: clash)
            row.translatesAutoresizingMaskIntoConstraints = false
            row.onSelect = { [weak self] in
                self?.focusClash(clash)
            }
            clashStackView.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: clashStackView.widthAnchor, constant: -16).isActive = true
            clashRowViews.append(row)
        }
    }

    func focusClash(_ clash: PleResolvedClash) {
        selectedClashId = clash.id
        for row in clashRowViews {
            row.setSelected(row.clash.id == clash.id)
        }
        scenePanel.focusClash(clash)
    }

    // MARK: - Stage 3: Run Diff Overlay (Compare Two Runs)

    func rebuildCompareMenu() {
        comparePopup.removeAllItems()
        comparePopup.addItem(withTitle: "None")
        comparePopup.lastItem?.representedObject = nil
        for item in items where item.id != selectedId {
            comparePopup.addItem(withTitle: item.title)
            comparePopup.lastItem?.representedObject = item.id
        }
        if let baseId = baselineId, let idx = comparePopup.itemArray.firstIndex(where: { ($0.representedObject as? String) == baseId }) {
            comparePopup.selectItem(at: idx)
        } else {
            comparePopup.selectItem(at: 0)
            if baselineId != nil {
                clearBaseline()
            }
        }
    }

    private func clearBaseline() {
        baselineId = nil
        baselineScene = nil
        baselineLabel = nil
        scenePanel.setDiff(nil)
    }

    private func recomputeDiff() {
        guard let active = activeScene, let base = baselineScene, let label = baselineLabel else {
            scenePanel.setDiff(nil)
            return
        }
        let diff = PleSceneDiff.compute(active: active, baseline: base, baselineTitle: label)
        scenePanel.setDiff(diff)
    }

    @objc private func compareChanged() {
        let selectedBaselineId = comparePopup.selectedItem?.representedObject as? String
        setBaselineRun(id: selectedBaselineId)
    }

    func setBaselineRun(id: String?) {
        baselineId = id
        baselineGeneration += 1
        let gen = baselineGeneration
        guard let id = id, let item = items.first(where: { $0.id == id }) else {
            clearBaseline()
            return
        }
        let wf = workingFolderProvider?()
        statusLabel.stringValue = "Comparing against \(item.title)…"
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                let baseScene = try PleGymDiscovery.loadScene(for: item, workingFolder: wf)
                DispatchQueue.main.async {
                    guard let self, self.baselineGeneration == gen else { return }
                    self.baselineScene = baseScene
                    self.baselineLabel = item.title
                    if let active = self.activeScene {
                        let diff = PleSceneDiff.compute(active: active, baseline: baseScene, baselineTitle: item.title)
                        self.scenePanel.setDiff(diff)
                        self.statusLabel.stringValue = "Diff vs \(item.title): +\(diff.addedCount) ~\(diff.movedCount) -\(diff.removedCount)"
                    } else {
                        self.scenePanel.setDiff(nil)
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    guard let self, self.baselineGeneration == gen else { return }
                    self.statusLabel.stringValue = "Compare error: \(error.localizedDescription)"
                    self.clearBaseline()
                    self.rebuildCompareMenu()
                }
            }
        }
    }

    // MARK: - Actions

    @objc private func refreshClicked() {
        refreshRuns(autoSelectFirst: false)
    }

    @objc private func openFileClicked() {
        let panel = NSOpenPanel()
        panel.title = "Open PLE Scene or IFC Model"
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        var types: [UTType] = [.json]
        if let ifc = UTType(filenameExtension: "ifc") { types.append(ifc) }
        panel.allowedContentTypes = types
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let isIfc = PleSceneLoader.isIfcFile(url)
        let item = PleGymRunItem(
            id: url.standardizedFileURL.path,
            title: url.lastPathComponent,
            subtitle: "Opened file • \(isIfc ? "IFC" : "PLE Scene")",
            url: url,
            isIfc: isIfc,
            isSample: false,
            modifiedAt: Date()
        )
        if !items.contains(where: { $0.id == item.id }) {
            items.insert(item, at: min(1, items.count))
            rebuildRows()
            rebuildCompareMenu()
        }
        selectItem(item)
    }

    // MARK: - Stage 4: Timeline Scrubbing & Snapshot Playback

    private func refreshTimelineSnapshots(for item: PleGymRunItem) {
        snapshots = PleGymDiscovery.discoverSnapshots(for: item)
        currentSnapshotIndex = max(0, snapshots.count - 1)
        updateTimelineUI()
    }

    private func attachLoadedSceneToCurrentSnapshot(scene: PleScene, item: PleGymRunItem) {
        if snapshots.isEmpty {
            snapshots = [PleGymSnapshot(
                id: item.id,
                label: "Current",
                url: item.url,
                isIfc: item.isIfc,
                scene: scene,
                score: scene.run?.score,
                isPinned: false,
                isLiveCapture: false,
                capturedAt: Date()
            )]
            currentSnapshotIndex = 0
        } else {
            let idx = min(currentSnapshotIndex, snapshots.count - 1)
            snapshots[idx].scene = scene
            snapshots[idx].score = scene.run?.score
        }
        updateTimelineUI()
    }

    private func appendLiveSnapshot(scene: PleScene) {
        let df = DateFormatter()
        df.dateFormat = "HH:mm:ss"
        let label = "Live \(df.string(from: Date()))"
        let snap = PleGymSnapshot(
            id: "live-\(UUID().uuidString)",
            label: label,
            url: selectedItem?.url,
            isIfc: selectedItem?.isIfc ?? false,
            scene: scene,
            score: scene.run?.score,
            isPinned: false,
            isLiveCapture: true,
            capturedAt: Date()
        )
        snapshots.append(snap)
        currentSnapshotIndex = snapshots.count - 1
        updateTimelineUI()
    }

    @objc private func pinSnapshotPressed() {
        guard let scene = activeScene else { return }
        pinCounter += 1
        let label = "Pin #\(pinCounter)"
        let snap = PleGymSnapshot(
            id: "pin-\(UUID().uuidString)",
            label: label,
            url: nil,
            isIfc: false,
            scene: scene,
            score: scene.run?.score,
            isPinned: true,
            isLiveCapture: false,
            capturedAt: Date()
        )
        snapshots.append(snap)
        currentSnapshotIndex = snapshots.count - 1
        updateTimelineUI()
    }

    @objc private func playPausePressed() {
        if isPlayingTimeline {
            stopPlayback()
        } else {
            startPlayback()
        }
    }

    private func startPlayback() {
        guard snapshots.count > 1 else { return }
        isPlayingTimeline = true
        playPauseButton.title = "⏸"
        playbackTimer?.invalidate()
        let t = Timer(timeInterval: 0.9, repeats: true) { [weak self] _ in
            guard let self else { return }
            guard self.snapshots.count > 1 else {
                self.stopPlayback()
                return
            }
            let nextIdx = (self.currentSnapshotIndex + 1) % self.snapshots.count
            self.scrubToSnapshot(index: nextIdx)
        }
        RunLoop.main.add(t, forMode: .common)
        playbackTimer = t
    }

    func stopPlayback() {
        isPlayingTimeline = false
        playPauseButton.title = "▶"
        playbackTimer?.invalidate()
        playbackTimer = nil
    }

    @objc private func prevSnapshotPressed() {
        guard !snapshots.isEmpty else { return }
        stopPlayback()
        scrubToSnapshot(index: max(0, currentSnapshotIndex - 1))
    }

    @objc private func nextSnapshotPressed() {
        guard !snapshots.isEmpty else { return }
        stopPlayback()
        scrubToSnapshot(index: min(snapshots.count - 1, currentSnapshotIndex + 1))
    }

    @objc private func timelineSliderChanged() {
        guard !snapshots.isEmpty else { return }
        stopPlayback()
        let idx = max(0, min(snapshots.count - 1, Int(timelineSlider.doubleValue.rounded()))) 
        if idx != currentSnapshotIndex {
            scrubToSnapshot(index: idx)
        }
    }

    func scrubToSnapshot(index: Int) {
        guard index >= 0, index < snapshots.count else { return }
        currentSnapshotIndex = index
        updateTimelineUI()
        let snap = snapshots[index]
        if let cached = snap.scene {
            applyScrubbedScene(cached)
            return
        }
        guard let url = snap.url else { return }
        snapshotLoadGeneration += 1
        let gen = snapshotLoadGeneration
        let wf = workingFolderProvider?()
        let isIfc = snap.isIfc
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                let scene: PleScene
                if isIfc {
                    scene = try PleGymDiscovery.convertAndLoadIfc(from: url, workingFolder: wf, forceReconvert: false)
                } else {
                    scene = try PleSceneLoader.load(from: url)
                }
                DispatchQueue.main.async {
                    guard let self, self.snapshotLoadGeneration == gen else { return }
                    if index < self.snapshots.count {
                        self.snapshots[index].scene = scene
                        self.snapshots[index].score = scene.run?.score
                    }
                    if self.currentSnapshotIndex == index {
                        self.applyScrubbedScene(scene)
                        self.updateTimelineUI()
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    guard let self, self.snapshotLoadGeneration == gen else { return }
                    self.statusLabel.stringValue = "Snapshot load error: \(error.localizedDescription)"
                    self.planFinished("could not load step \(index + 1): \(error.localizedDescription)")
                }
            }
        }
    }

    private func applyScrubbedScene(_ scene: PleScene) {
        activeScene = scene
        scenePanel.loadScene(scene, preserveView: true)
        rebuildClashRows(for: scene)
        if let item = selectedItem {
            updateDiagnostics(for: scene, item: item)
        }
        recomputeDiff()
        planSceneReady()
    }

    private func updateTimelineUI() {
        let count = snapshots.count
        let hasMulti = count > 1
        timelineSlider.minValue = 0
        timelineSlider.maxValue = Double(max(0, count - 1))
        timelineSlider.integerValue = currentSnapshotIndex
        timelineSlider.isEnabled = hasMulti
        playPauseButton.isEnabled = hasMulti
        prevSnapButton.isEnabled = hasMulti && currentSnapshotIndex > 0
        nextSnapButton.isEnabled = hasMulti && currentSnapshotIndex < count - 1
        pinSnapshotButton.isEnabled = activeScene != nil
        if count == 0 {
            timelineLabel.stringValue = "0/0 • No snapshots"
            return
        }
        let snap = snapshots[min(currentSnapshotIndex, count - 1)]
        let prefix = snap.isPinned ? "★ " : (snap.isLiveCapture ? "● " : "")
        var text = "\(currentSnapshotIndex + 1)/\(count) • \(prefix)\(snap.label)"
        if let sc = snap.score {
            text += String(format: " • score %.2f", sc)
        }
        timelineLabel.stringValue = text
    }

    // MARK: - Stage 4: Run Diagnostics & Log Drawer

    @objc private func railTabChanged() {
        showingDiagnostics = (railTabControl.selectedSegment == 1)
        clashScrollView.isHidden = showingDiagnostics
        let hasClashes = !(activeScene?.resolvedClashes.isEmpty ?? true)
        clashEmptyLabel.isHidden = showingDiagnostics || hasClashes
        diagContainer.isHidden = !showingDiagnostics
    }

    private func refreshDiagnosticsFiles(for item: PleGymRunItem) {
        discoveredLogs = PleGymDiscovery.discoverLogs(for: item)
        rebuildDiagFileMenu(scene: activeScene)
    }

    private func updateDiagnostics(for scene: PleScene, item: PleGymRunItem) {
        // Metrics line
        var parts: [String] = []
        if let r = scene.run {
            if let sc = r.score { parts.append(String(format: "Score %.2f", sc)) }
            if let len = r.total_length_m { parts.append(String(format: "Len %.1fm", len)) }
        }
        parts.append("\(scene.elements.count) elems")
        parts.append("\(scene.resolvedClashes.count) clashes")
        diagMetricsLabel.stringValue = parts.joined(separator: " • ")

        // Per-gate badges
        for v in diagGatesStack.arrangedSubviews {
            diagGatesStack.removeArrangedSubview(v)
            v.removeFromSuperview()
        }
        if let gates = scene.run?.gates, !gates.isEmpty {
            for key in gates.keys.sorted() {
                let val = gates[key]!
                let passed: Bool
                switch val {
                case .bool(let b): passed = b
                case .number(let n): passed = n != 0
                case .string(let s):
                    let low = s.lowercased()
                    passed = (low == "pass" || low == "passed" || low == "ok" || low == "true")
                case .null, .array, .object: passed = false
                }
                diagGatesStack.addArrangedSubview(makeGateBadge(name: key, passed: passed))
            }
        } else {
            let lbl = NSTextField(labelWithString: "No gate data")
            lbl.font = NSFont.systemFont(ofSize: 10)
            lbl.textColor = NSColor(white: 0.55, alpha: 1)
            diagGatesStack.addArrangedSubview(lbl)
        }

        if discoveredLogs.isEmpty {
            discoveredLogs = PleGymDiscovery.discoverLogs(for: item)
        }
        rebuildDiagFileMenu(scene: scene)
    }

    private func makeGateBadge(name: String, passed: Bool) -> NSView {
        let pill = NSView()
        pill.translatesAutoresizingMaskIntoConstraints = false
        pill.wantsLayer = true
        pill.layer?.cornerRadius = 4
        pill.layer?.borderWidth = 1
        if passed {
            pill.layer?.backgroundColor = NSColor(calibratedRed: 0.14, green: 0.52, blue: 0.28, alpha: 0.32).cgColor
            pill.layer?.borderColor = NSColor(calibratedRed: 0.28, green: 0.85, blue: 0.48, alpha: 0.75).cgColor
        } else {
            pill.layer?.backgroundColor = NSColor(calibratedRed: 0.65, green: 0.16, blue: 0.16, alpha: 0.35).cgColor
            pill.layer?.borderColor = NSColor(calibratedRed: 0.95, green: 0.34, blue: 0.34, alpha: 0.85).cgColor
        }
        let lbl = NSTextField(labelWithString: "\(passed ? "✓" : "✕") \(name)")
        lbl.translatesAutoresizingMaskIntoConstraints = false
        lbl.font = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold)
        lbl.textColor = passed
            ? NSColor(calibratedRed: 0.72, green: 1.0, blue: 0.80, alpha: 1)
            : NSColor(calibratedRed: 1.0, green: 0.74, blue: 0.74, alpha: 1)
        pill.addSubview(lbl)
        NSLayoutConstraint.activate([
            lbl.leadingAnchor.constraint(equalTo: pill.leadingAnchor, constant: 5),
            lbl.trailingAnchor.constraint(equalTo: pill.trailingAnchor, constant: -5),
            lbl.topAnchor.constraint(equalTo: pill.topAnchor, constant: 2),
            lbl.bottomAnchor.constraint(equalTo: pill.bottomAnchor, constant: -2),
        ])
        return pill
    }

    private func rebuildDiagFileMenu(scene: PleScene?) {
        let prevTitle = diagFilePopup.selectedItem?.title
        diagFilePopup.removeAllItems()
        var entries = discoveredLogs
        let summary = formatSceneDiagnosticsSummary(scene: scene)
        entries.append(PleGymLogFile(name: "Scene Summary (parsed)", url: nil, syntheticContent: summary))
        for entry in entries {
            diagFilePopup.addItem(withTitle: entry.name)
            diagFilePopup.lastItem?.representedObject = entry
        }
        if let prev = prevTitle, let idx = diagFilePopup.itemArray.firstIndex(where: { $0.title == prev }) {
            diagFilePopup.selectItem(at: idx)
        } else {
            diagFilePopup.selectItem(at: 0)
        }
        loadSelectedDiagLog()
    }

    private func formatSceneDiagnosticsSummary(scene: PleScene?) -> String {
        guard let scene else { return "No scene loaded." }
        var lines: [String] = []
        lines.append("Scene: \(scene.task ?? "PLE Scene")")
        lines.append("Elements: \(scene.elements.count)")
        lines.append("Clashes: \(scene.resolvedClashes.count)")
        if let r = scene.run {
            if let sc = r.score { lines.append(String(format: "Score: %.4f", sc)) }
            if let len = r.total_length_m { lines.append(String(format: "Total Length: %.2f m", len)) }
            if let gates = r.gates, !gates.isEmpty {
                lines.append("")
                lines.append("Gates:")
                for k in gates.keys.sorted() {
                    lines.append("  \(k): \(gates[k]!)")
                }
            }
        }
        if !scene.resolvedClashes.isEmpty {
            lines.append("")
            lines.append("Top Clashes:")
            for c in scene.resolvedClashes.prefix(12) {
                let pen = c.penetrationMM.map { String(format: "%.1fmm", $0) } ?? "?"
                lines.append("  • \(c.pairLabel) (\(pen))")
            }
        }
        return lines.joined(separator: "\n")
    }

    @objc private func diagFileChanged() {
        loadSelectedDiagLog()
    }

    private func loadSelectedDiagLog() {
        guard let entry = diagFilePopup.selectedItem?.representedObject as? PleGymLogFile else {
            diagLogTextView.string = ""
            return
        }
        if let synthetic = entry.syntheticContent {
            diagLogTextView.string = synthetic
            return
        }
        guard let url = entry.url else {
            diagLogTextView.string = ""
            return
        }
        do {
            let data = try Data(contentsOf: url)
            let maxBytes = 128 * 1024
            let slice = data.count > maxBytes ? data.suffix(maxBytes) : data
            let prefix = data.count > maxBytes ? "[showing last 128 KB of \(data.count) bytes]\n" : ""
            let text = String(data: slice, encoding: .utf8) ?? String(decoding: slice, as: UTF8.self)
            diagLogTextView.string = prefix + text
        } catch {
            diagLogTextView.string = "Failed to read \(entry.name): \(error.localizedDescription)"
        }
    }

    @objc private func copyDiagLogPressed() {
        let text = diagLogTextView.string
        guard !text.isEmpty else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(text, forType: .string)
        statusLabel.stringValue = "Copied diagnostics to clipboard"
    }
}

