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
    static func loadScene(for item: PleGymRunItem, workingFolder: URL?) throws -> PleScene {
        if !item.isIfc {
            return try PleSceneLoader.load(from: item.url)
        }
        return try convertAndLoadIfc(from: item.url, workingFolder: workingFolder)
    }

    static func convertAndLoadIfc(from url: URL, workingFolder: URL?) throws -> PleScene {
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
        let cacheValid = fm.fileExists(atPath: outURL.path) && (srcDate == nil || dstDate == nil || dstDate! >= srcDate!)

        if !cacheValid {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            proc.arguments = ["python3", script.path, "--ifc", url.path, "--out", outURL.path]
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

// MARK: - GymWorkbenchView

final class GymWorkbenchView: NSView {
    private let railWidth: CGFloat = 264
    private let railView = NSView()
    private let dividerView = NSView()
    private let headerLabel = NSTextField(labelWithString: "PLE Gym Workbench")
    private let refreshButton = NSButton(title: "Refresh", target: nil, action: nil)
    private let openFileButton = NSButton(title: "Open…", target: nil, action: nil)
    private let scrollView = NSScrollView()
    private let stackView = NSStackView()
    private let statusLabel = NSTextField(labelWithString: "")
    private let emptyLabel = NSTextField(labelWithString: "Select a PLE scene or run to inspect in 3D.")

    let scenePanel = PleScenePanelView(frame: .zero)

    var workingFolderProvider: (() -> URL)?

    private var items: [PleGymRunItem] = []
    private var rowViews: [PleGymRowButton] = []
    private var selectedId: String? = nil
    private var loadGeneration: Int = 0
    private var hasLoadedInitial = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor(red: 0.06, green: 0.065, blue: 0.08, alpha: 1).cgColor
        setupUI()
    }

    required init?(coder: NSCoder) { fatalError() }

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

        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.font = NSFont.systemFont(ofSize: 10.5, weight: .regular)
        statusLabel.textColor = NSColor(white: 0.55, alpha: 1)
        statusLabel.lineBreakMode = .byTruncatingMiddle
        railView.addSubview(statusLabel)

        NSLayoutConstraint.activate([
            railView.topAnchor.constraint(equalTo: topAnchor),
            railView.bottomAnchor.constraint(equalTo: bottomAnchor),
            railView.leadingAnchor.constraint(equalTo: leadingAnchor),
            railView.widthAnchor.constraint(equalToConstant: railWidth),

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

            headerLabel.topAnchor.constraint(equalTo: railView.topAnchor, constant: 12),
            headerLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),
            headerLabel.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -12),

            refreshButton.topAnchor.constraint(equalTo: headerLabel.bottomAnchor, constant: 8),
            refreshButton.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),

            openFileButton.centerYAnchor.constraint(equalTo: refreshButton.centerYAnchor),
            openFileButton.leadingAnchor.constraint(equalTo: refreshButton.trailingAnchor, constant: 6),

            scrollView.topAnchor.constraint(equalTo: refreshButton.bottomAnchor, constant: 8),
            scrollView.leadingAnchor.constraint(equalTo: railView.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: railView.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: statusLabel.topAnchor, constant: -6),

            clipDoc.widthAnchor.constraint(equalTo: scrollView.contentView.widthAnchor),

            statusLabel.leadingAnchor.constraint(equalTo: railView.leadingAnchor, constant: 12),
            statusLabel.trailingAnchor.constraint(equalTo: railView.trailingAnchor, constant: -12),
            statusLabel.bottomAnchor.constraint(equalTo: railView.bottomAnchor, constant: -10)
        ])
    }

    func refreshRuns(autoSelectFirst: Bool = false) {
        let wf = workingFolderProvider?()
        let discovered = PleGymDiscovery.discover(workingFolder: wf)
        self.items = discovered
        rebuildRows()
        statusLabel.stringValue = "\(discovered.count) scene\(discovered.count == 1 ? "" : "s") available"

        if (autoSelectFirst || !hasLoadedInitial), let first = discovered.first {
            hasLoadedInitial = true
            selectItem(first)
        } else if let cur = selectedId, let existing = discovered.first(where: { $0.id == cur }) {
            updateSelectionHighlight(existing.id)
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

    func selectItem(_ item: PleGymRunItem) {
        updateSelectionHighlight(item.id)
        loadGeneration += 1
        let gen = loadGeneration
        let wf = workingFolderProvider?()
        emptyLabel.isHidden = true
        statusLabel.stringValue = item.isIfc ? "Converting \(item.url.lastPathComponent)…" : "Loading \(item.title)…"

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            do {
                let scene = try PleGymDiscovery.loadScene(for: item, workingFolder: wf)
                DispatchQueue.main.async {
                    guard let self, self.loadGeneration == gen else { return }
                    self.scenePanel.loadScene(scene)
                    self.statusLabel.stringValue = "\(item.title) • \(scene.elements.count) elems"
                }
            } catch {
                DispatchQueue.main.async {
                    guard let self, self.loadGeneration == gen else { return }
                    self.statusLabel.stringValue = "Error: \(error.localizedDescription)"
                }
            }
        }
    }

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
        }
        selectItem(item)
    }
}
