// Firm reason: 6 (AppKit/Metal 3D PLE Gym Workbench view over PleScenePanelView and local run discovery).
// Stage 2 of GitHub #13792: 3D PLE Run Visualizer / Gym Workbench in OpenAgents Desktop.
import AppKit
import CryptoKit
import Foundation
import UniformTypeIdentifiers

struct PleGymRunItem: Equatable {
    let id: String          // canonical file path
    let title: String       // display name in sidebar list
    let subtitle: String    // source / format hint
    let url: URL            // .ple-scene.json or .ifc
    let isIfc: Bool
    let isSample: Bool
    let modifiedAt: Date?
}

enum PleGymDiscovery {
    static let cacheDir = URL(fileURLWithPath: "/tmp/ple-scenes", isDirectory: true)

    /// Locate the repository root (if running from or near a checkout).
    static func findRepoRoot(workingFolder: URL?) -> URL? {
        let fm = FileManager.default
        var seeds: [URL] = []
        if let wf = workingFolder { seeds.append(wf) }
        seeds.append(URL(fileURLWithPath: fm.currentDirectoryPath))
        if let bundleURL = Bundle.main.bundleURL as URL? {
            seeds.append(bundleURL.deletingLastPathComponent())
        }
        let home = fm.homeDirectoryForCurrentUser
        seeds.append(home.appendingPathComponent("work/openagents"))

        for seed in seeds {
            var dir = seed.standardizedFileURL
            for _ in 0..<8 {
                let fixture = dir.appendingPathComponent("metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json")
                let tool = dir.appendingPathComponent("tools/ple-ifc-to-scene.py")
                if fm.fileExists(atPath: fixture.path) || fm.fileExists(atPath: tool.path) {
                    return dir
                }
                let parent = dir.deletingLastPathComponent()
                if parent.path == dir.path { break }
                dir = parent
            }
        }
        return nil
    }

    /// Find the bundled sample fixture URL.
    static func findBundledSample(repoRoot: URL?) -> URL? {
        let fm = FileManager.default
        var candidates: [URL] = []
        if let repo = repoRoot {
            candidates.append(repo.appendingPathComponent("metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json"))
        }
        if let res = Bundle.main.resourceURL {
            candidates.append(res.appendingPathComponent("fixtures/ple-data-hall-sample.ple-scene.json"))
            candidates.append(res.appendingPathComponent("ple-data-hall-sample.ple-scene.json"))
        }
        let home = fm.homeDirectoryForCurrentUser
        candidates.append(home.appendingPathComponent("work/openagents/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json"))
        return candidates.first { fm.fileExists(atPath: $0.path) }
    }

    /// Discover bundled sample + PLE runs in ../ple-kit/runs, ~/work/ple-kit/runs, and ~/.openagents/ple/runs.
    static func discover(workingFolder: URL?) -> [PleGymRunItem] {
        let fm = FileManager.default
        let repoRoot = findRepoRoot(workingFolder: workingFolder)
        var items: [PleGymRunItem] = []
        var seenPaths = Set<String>()

        if let sampleURL = findBundledSample(repoRoot: repoRoot) {
            let canon = sampleURL.standardizedFileURL.path
            seenPaths.insert(canon)
            items.append(PleGymRunItem(
                id: canon,
                title: "data-hall-mep-routing (sample)",
                subtitle: "Bundled fixture • PLE Scene",
                url: sampleURL,
                isIfc: false,
                isSample: true,
                modifiedAt: (try? fm.attributesOfItem(atPath: canon)[.modificationDate]) as? Date
            ))
        }

        var runRoots: [(url: URL, label: String)] = []
        if let repo = repoRoot {
            let sibling = repo.deletingLastPathComponent().appendingPathComponent("ple-kit/runs")
            runRoots.append((sibling, "ple-kit/runs"))
        }
        let home = fm.homeDirectoryForCurrentUser
        runRoots.append((home.appendingPathComponent("work/ple-kit/runs"), "ple-kit/runs"))
        runRoots.append((home.appendingPathComponent(".openagents/ple/runs"), "~/.openagents/ple/runs"))

        var scannedRoots = Set<String>()
        var discovered: [PleGymRunItem] = []

        for (rootURL, sourceLabel) in runRoots {
            let canonRoot = rootURL.standardizedFileURL.path
            guard !scannedRoots.contains(canonRoot) else { continue }
            scannedRoots.insert(canonRoot)
            var isDir: ObjCBool = false
            guard fm.fileExists(atPath: canonRoot, isDirectory: &isDir), isDir.boolValue else { continue }
            scanRunDirectory(rootURL, sourceLabel: sourceLabel, seenPaths: &seenPaths, into: &discovered)
        }

        discovered.sort { a, b in
            let da = a.modifiedAt ?? .distantPast
            let db = b.modifiedAt ?? .distantPast
            if da != db { return da > db }
            return a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending
        }

        items.append(contentsOf: discovered)
        return items
    }

    private static func scanRunDirectory(
        _ root: URL,
        sourceLabel: String,
        seenPaths: inout Set<String>,
        into out: inout [PleGymRunItem]
    ) {
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(
            at: root,
            includingPropertiesForKeys: [.isDirectoryKey, .contentModificationDateKey],
            options: [.skipsHiddenFiles]
        ) else { return }

        let rootComponents = root.standardizedFileURL.pathComponents.count

        for case let fileURL as URL in enumerator {
            let comps = fileURL.standardizedFileURL.pathComponents
            let depth = comps.count - rootComponents
            if depth > 7 {
                enumerator.skipDescendants()
                continue
            }
            let values = try? fileURL.resourceValues(forKeys: [.isDirectoryKey, .contentModificationDateKey])
            if values?.isDirectory == true {
                let dirName = fileURL.lastPathComponent
                if dirName == ".git" || dirName == "node_modules" || dirName == "__pycache__" {
                    enumerator.skipDescendants()
                }
                continue
            }

            let lower = fileURL.lastPathComponent.lowercased()
            let isScene = PleSceneLoader.isSceneFile(fileURL)
            let isOutputIfc = lower == "model.ifc" && fileURL.deletingLastPathComponent().lastPathComponent.lowercased() == "output"
            let isOtherIfc = !isOutputIfc && PleSceneLoader.isIfcFile(fileURL)
            guard isScene || isOutputIfc || isOtherIfc else { continue }

            let canon = fileURL.standardizedFileURL.path
            guard !seenPaths.contains(canon) else { continue }
            seenPaths.insert(canon)

            let relComponents = Array(comps.dropFirst(rootComponents))
            let runName = relComponents.first ?? fileURL.deletingLastPathComponent().lastPathComponent
            let title: String
            if isOutputIfc {
                title = runName
            } else if relComponents.count >= 2 {
                title = "\(runName) / \(fileURL.lastPathComponent)"
            } else {
                title = fileURL.lastPathComponent
            }
            let kind = isScene ? "PLE Scene" : (isOutputIfc ? "output/model.ifc" : "IFC")
            out.append(PleGymRunItem(
                id: canon,
                title: title,
                subtitle: "\(sourceLabel) • \(kind)",
                url: fileURL,
                isIfc: !isScene,
                isSample: false,
                modifiedAt: values?.contentModificationDate
            ))
        }
    }

    /// Load a scene or convert an .ifc file on demand via tools/ple-ifc-to-scene.py into /tmp/ple-scenes/.
    static func loadScene(for item: PleGymRunItem, workingFolder: URL?, forceReconvert: Bool = false) throws -> PleScene {
        if !item.isIfc {
            return try PleSceneLoader.load(from: item.url)
        }
        return try convertAndLoadIfc(from: item.url, workingFolder: workingFolder, forceReconvert: forceReconvert)
    }

    static func convertAndLoadIfc(from url: URL, workingFolder: URL?, forceReconvert: Bool = false) throws -> PleScene {
        let fm = FileManager.default
        let repoRoot = findRepoRoot(workingFolder: workingFolder)
        var candidates: [URL] = []
        if let repo = repoRoot {
            candidates.append(repo.appendingPathComponent("tools/ple-ifc-to-scene.py"))
        }
        if let wf = workingFolder {
            candidates.append(wf.appendingPathComponent("tools/ple-ifc-to-scene.py"))
        }
        var dir = url.deletingLastPathComponent()
        for _ in 0..<8 {
            candidates.append(dir.appendingPathComponent("tools/ple-ifc-to-scene.py"))
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        candidates.append(URL(fileURLWithPath: fm.currentDirectoryPath).appendingPathComponent("tools/ple-ifc-to-scene.py"))
        candidates.append(fm.homeDirectoryForCurrentUser.appendingPathComponent("work/openagents/tools/ple-ifc-to-scene.py"))

        guard let script = candidates.first(where: { fm.fileExists(atPath: $0.path) }) else {
            throw NSError(domain: "PleGymDiscovery", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "Could not find tools/ple-ifc-to-scene.py to convert \(url.lastPathComponent)."
            ])
        }

        try fm.createDirectory(at: cacheDir, withIntermediateDirectories: true)
        let digest = SHA256.hash(data: Data(url.standardizedFileURL.path.utf8))
        let hashPrefix = digest.prefix(8).map { String(format: "%02x", $0) }.joined()
        let stem = url.deletingPathExtension().lastPathComponent
        let outURL = cacheDir.appendingPathComponent("\(stem)-\(hashPrefix).ple-scene.json")

        // Reuse cached conversion if newer than the source .ifc file.
        let srcDate = (try? fm.attributesOfItem(atPath: url.path)[.modificationDate]) as? Date
        let dstDate = (try? fm.attributesOfItem(atPath: outURL.path)[.modificationDate]) as? Date
        let cacheValid = !forceReconvert && fm.fileExists(atPath: outURL.path) && (srcDate == nil || dstDate == nil || dstDate! >= srcDate!)

        if !cacheValid {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            proc.arguments = ["python3", script.path, url.path, "-o", outURL.path]
            let errPipe = Pipe()
            proc.standardError = errPipe
            try proc.run()
            proc.waitUntilExit()
            guard proc.terminationStatus == 0, fm.fileExists(atPath: outURL.path) else {
                let errText = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? "conversion failed"
                throw NSError(domain: "PleGymDiscovery", code: 2, userInfo: [
                    NSLocalizedDescriptionKey: "IFC conversion failed: \(errText.trimmingCharacters(in: .whitespacesAndNewlines))"
                ])
            }
        }

        return try PleScene.load(from: outURL)
    }
}

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

// MARK: - Live File Signature (Stage 3)

struct PleFileSignature: Equatable {
    let modifiedAt: Date?
    let size: UInt64?

    static func read(from url: URL) -> PleFileSignature? {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: url.path) else {
            return nil
        }
        let mtime = attrs[.modificationDate] as? Date
        let size = (attrs[.size] as? NSNumber)?.uint64Value
        return PleFileSignature(modifiedAt: mtime, size: size)
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
    private var liveEnabled = false
    private var liveTimer: Timer?
    private var lastObservedSignature: PleFileSignature?
    private var liveReloadInFlight = false

    // Stage 3: Run Diff Overlay controls
    private let compareLabel = NSTextField(labelWithString: "Compare:")
    private let comparePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private var activeScene: PleScene?
    private var baselineId: String?
    private var baselineScene: PleScene?
    private var baselineLabel: String?
    private var baselineGeneration: Int = 0

    // Runs list
    private let scrollView = NSScrollView()
    private let stackView = NSStackView()
    private var rowViews: [PleGymRowButton] = []

    // Stage 3: Clash Inspector section
    private let clashDivider = NSView()
    private let clashHeaderLabel = NSTextField(labelWithString: "Clashes (0)")
    private let clashScrollView = NSScrollView()
    private let clashStackView = NSStackView()
    private let clashEmptyLabel = NSTextField(labelWithString: "No clashes detected ✓")
    private var clashRowViews: [PleGymClashRowButton] = []
    private var selectedClashId: String?

    private let statusLabel = NSTextField(labelWithString: "")
    private let emptyLabel = NSTextField(labelWithString: "Select a PLE run or click Open… to inspect a .ple-scene.json or .ifc model")

    private var items: [PleGymRunItem] = []
    private var selectedId: String?
    private var loadGeneration: Int = 0

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor(red: 0.06, green: 0.065, blue: 0.08, alpha: 1).cgColor
        setupUI()
    }

    required init?(coder: NSCoder) { fatalError() }

    deinit {
        liveTimer?.invalidate()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            stopLiveTimer()
        } else if liveEnabled {
            startLiveTimer()
        }
    }

    private func setupUI() {
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

        clashHeaderLabel.translatesAutoresizingMaskIntoConstraints = false
        clashHeaderLabel.font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        clashHeaderLabel.textColor = NSColor(white: 0.90, alpha: 1)
        railView.addSubview(clashHeaderLabel)

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

        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .regular)
        statusLabel.textColor = NSColor(white: 0.55, alpha: 1)
        statusLabel.lineBreakMode = .byTruncatingMiddle
        railView.addSubview(statusLabel)

        NSLayoutConstraint.activate([
            railView.topAnchor.constraint(equalTo: topAnchor),
            railView.bottomAnchor.constraint(equalTo: bottomAnchor),
            railView.leadingAnchor.constraint(equalTo: leadingAnchor),
            railView.widthAnchor.constraint(equalToConstant: 268),

            dividerView.topAnchor.constraint(equalTo: topAnchor),
            dividerView.bottomAnchor.constraint(equalTo: bottomAnchor),
            dividerView.leadingAnchor.constraint(equalTo: railView.trailingAnchor),
            dividerView.widthAnchor.constraint(equalToConstant: 1),

            scenePanel.topAnchor.constraint(equalTo: topAnchor),
            scenePanel.bottomAnchor.constraint(equalTo: bottomAnchor),
            scenePanel.leadingAnchor.constraint(equalTo: dividerView.trailingAnchor),
            scenePanel.trailingAnchor.constraint(equalTo: trailingAnchor),

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

            clashHeaderLabel.topAnchor.constraint(equalTo: clashDivider.bottomAnchor, constant: 8),
            clashHeaderLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),
            clashHeaderLabel.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -10),

            clashScrollView.topAnchor.constraint(equalTo: clashHeaderLabel.bottomAnchor, constant: 4),
            clashScrollView.leadingAnchor.constraint(equalTo: railView.leadingAnchor),
            clashScrollView.trailingAnchor.constraint(equalTo: railView.trailingAnchor),
            clashScrollView.heightAnchor.constraint(equalToConstant: 196),
            clashScrollView.bottomAnchor.constraint(equalTo: statusLabel.topAnchor, constant: -6),

            clashDoc.widthAnchor.constraint(equalTo: clashScrollView.widthAnchor),

            clashEmptyLabel.topAnchor.constraint(equalTo: clashHeaderLabel.bottomAnchor, constant: 10),
            clashEmptyLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),

            statusLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),
            statusLabel.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -12),
            statusLabel.bottomAnchor.constraint(equalTo: railView.bottomAnchor, constant: -8),
        ])

        rebuildCompareMenu()
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

    private func rebuildRows() {
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

    private var selectedItem: PleGymRunItem? {
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
        lastObservedSignature = PleFileSignature.read(from: item.url)
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
                    self.recomputeDiff()
                    let clashCount = scene.resolvedClashes.count
                    self.statusLabel.stringValue = "\(item.title) • \(scene.elements.count) elems • \(clashCount) clash\(clashCount == 1 ? "" : "es")"
                    if isLiveUpdate {
                        let df = DateFormatter()
                        df.dateFormat = "HH:mm:ss"
                        self.liveStatusLabel.stringValue = "Updated \(df.string(from: Date()))"
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    guard let self, self.loadGeneration == gen else { return }
                    self.liveReloadInFlight = false
                    if isLiveUpdate {
                        self.liveStatusLabel.stringValue = "Reload error"
                    } else {
                        self.statusLabel.stringValue = "Error: \(error.localizedDescription)"
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
        clashEmptyLabel.isHidden = !clashes.isEmpty
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

    private func rebuildCompareMenu() {
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
}
