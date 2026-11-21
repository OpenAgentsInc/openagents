// Firm reason: 6 (AppKit/Metal 3D PLE Gym Workbench view over PleScenePanelView and local run discovery).
// Stage 2 of GitHub #13792: 3D PLE Run Visualizer / Gym Workbench in OpenAgents Desktop.
import AppKit
import CryptoKit
import Foundation
import UniformTypeIdentifiers

// Split from GymWorkbenchView.swift (#13848): gym run discovery and file signature.

struct PleGymRunItem: Equatable {
    let id: String          // canonical file path
    let title: String       // display name in sidebar list
    let subtitle: String    // source / format hint
    let url: URL            // .ple-scene.json or .ifc
    let isIfc: Bool
    let isSample: Bool
    let modifiedAt: Date?
}

/// Stage 4: A single timeline snapshot (discovered on disk, captured during live tailing, or manually pinned).
struct PleGymSnapshot {
    let id: String
    let label: String
    let url: URL?
    let isIfc: Bool
    var scene: PleScene?
    var score: Double?
    let isPinned: Bool
    let isLiveCapture: Bool
    let capturedAt: Date
}

/// Stage 4: A discovered diagnostics/log artifact next to a run file.
struct PleGymLogFile {
    let name: String
    let url: URL?
    let syntheticContent: String?
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
    /// Stage 4: Discover sibling step_*.ple-scene.json, iter_*.ple-scene.json, snapshots/*.ple-scene.json, or snapshots/*.ifc sorted naturally.
    static func discoverSnapshots(for item: PleGymRunItem) -> [PleGymSnapshot] {
        let fm = FileManager.default
        let dir = item.url.deletingLastPathComponent()
        var foundURLs: [URL] = []
        var seen = Set<String>()

        func addIfValid(_ url: URL) {
            let canon = url.standardizedFileURL.path
            guard !seen.contains(canon) else { return }
            seen.insert(canon)
            foundURLs.append(url)
        }

        if let siblings = try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.isRegularFileKey], options: [.skipsHiddenFiles]) {
            for u in siblings {
                let name = u.lastPathComponent.lowercased()
                if (name.hasPrefix("step_") || name.hasPrefix("iter_")) && (name.hasSuffix(".ple-scene.json") || name.hasSuffix(".ifc")) {
                    addIfValid(u)
                }
            }
        }

        let snapDir = dir.appendingPathComponent("snapshots", isDirectory: true)
        var isDir: ObjCBool = false
        if fm.fileExists(atPath: snapDir.path, isDirectory: &isDir), isDir.boolValue,
           let snapFiles = try? fm.contentsOfDirectory(at: snapDir, includingPropertiesForKeys: [.isRegularFileKey], options: [.skipsHiddenFiles]) {
            for u in snapFiles {
                let name = u.lastPathComponent.lowercased()
                if name.hasSuffix(".ple-scene.json") || name.hasSuffix(".ifc") {
                    addIfValid(u)
                }
            }
        }

        foundURLs.sort { a, b in
            a.lastPathComponent.localizedStandardCompare(b.lastPathComponent) == .orderedAscending
        }

        var snapshots: [PleGymSnapshot] = foundURLs.map { u in
            let name = u.lastPathComponent
            let label = name
                .replacingOccurrences(of: ".ple-scene.json", with: "")
                .replacingOccurrences(of: ".ifc", with: "")
            let mtime = (try? fm.attributesOfItem(atPath: u.path)[.modificationDate]) as? Date ?? Date()
            return PleGymSnapshot(
                id: u.standardizedFileURL.path,
                label: label,
                url: u,
                isIfc: name.lowercased().hasSuffix(".ifc"),
                scene: nil,
                score: nil,
                isPinned: false,
                isLiveCapture: false,
                capturedAt: mtime
            )
        }

        let itemCanon = item.url.standardizedFileURL.path
        if !seen.contains(itemCanon) {
            let mtime = item.modifiedAt ?? Date()
            snapshots.append(PleGymSnapshot(
                id: itemCanon,
                label: snapshots.isEmpty ? "Current" : "Latest",
                url: item.url,
                isIfc: item.isIfc,
                scene: nil,
                score: nil,
                isPinned: false,
                isLiveCapture: false,
                capturedAt: mtime
            ))
        }
        return snapshots
    }

    /// Stage 4: Discover sibling trial.log, harbor.log, reconciliation.json, or verifier.json next to the selected run.
    static func discoverLogs(for item: PleGymRunItem) -> [PleGymLogFile] {
        let fm = FileManager.default
        let names = ["verifier.json", "reconciliation.json", "trial.log", "harbor.log"]
        let dirs = [item.url.deletingLastPathComponent(), item.url.deletingLastPathComponent().deletingLastPathComponent()]
        var seen = Set<String>()
        var out: [PleGymLogFile] = []
        for d in dirs {
            for n in names {
                let u = d.appendingPathComponent(n)
                let canon = u.standardizedFileURL.path
                guard !seen.contains(canon), fm.fileExists(atPath: canon) else { continue }
                seen.insert(canon)
                let prefix = (d == dirs[0]) ? "" : "../"
                out.append(PleGymLogFile(name: prefix + n, url: u, syntheticContent: nil))
            }
        }
        return out
    }

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

