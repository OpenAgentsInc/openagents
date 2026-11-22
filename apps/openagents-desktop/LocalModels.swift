// Firm reason: 6 (macOS platform filesystem, curl process, CryptoKit SHA-256, and disk-space APIs for Local Models manager; #13826).
import CryptoKit
import Foundation

/// Metadata for a local checkpoint managed by OpenAgents Desktop (~/.openagents/models/<id>/).
struct LocalModelCatalogEntry {
    let id: String
    let name: String
    let architecture: String
    let quantization: String
    let fileName: String
    let downloadURL: String
    let expectedBytes: Int64
    let expectedSHA256: String
    let minRamGB: Int
    let diskHeadroomBytes: Int64

    static let inkling2Mini = LocalModelCatalogEntry(
        id: "inkling-2-mini",
        name: "Inkling 2 Mini",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes)",
        quantization: "IQ4_NL GGUF",
        fileName: "Inkling-2-Mini-IQ4_NL.gguf",
        downloadURL: "https://huggingface.co/ggml-org/Inkling-2-Mini-GGUF/resolve/main/Inkling-2-Mini-IQ4_NL.gguf",
        expectedBytes: 17_627_940_768,
        expectedSHA256: "cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4",
        minRamGB: 32,
        diskHeadroomBytes: 536_870_912
    )
}

struct LocalModelManifest: Codable, Equatable {
    let model_id: String
    let weights: String
    let bytes: Int64
    let sha256: String
    let mtime: Int64
    let verified: Bool
    let loops: Int
    let verified_at: Int64
}

enum LocalModelState: Equatable {
    case unsupportedHardware
    case notInstalled
    case downloading
    case paused
    case verifying
    case installed
}

struct LocalModelVerificationResult: Equatable {
    let ok: Bool
    let bytes: Int64
    let sha256: String
    let error: String?
}

/// Verified local GGUF checkpoint downloader and manager (~/.openagents/models) for OpenAgents Desktop (#13825, #13826).
enum LocalModelStore {
    // Test overrides (nil in production).
    static var rootDirOverride: URL?
    static var physicalMemoryGBOverride: Int?
    static var availableDiskBytesOverride: Int64?
    static var catalogOverride: LocalModelCatalogEntry?

    static var entry: LocalModelCatalogEntry { catalogOverride ?? .inkling2Mini }
    static var catalog: [LocalModelCatalogEntry] { [entry] }

    static var rootDir: URL {
        if let rootDirOverride { return rootDirOverride }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("models", isDirectory: true)
    }
    static var inklingDir: URL { rootDir.appendingPathComponent(entry.id, isDirectory: true) }
    static var weightsURL: URL { inklingDir.appendingPathComponent(entry.fileName) }
    static var partialURL: URL { inklingDir.appendingPathComponent(entry.fileName + ".part") }
    static var manifestURL: URL { inklingDir.appendingPathComponent("manifest.json") }
    static var downloadURL: String { entry.downloadURL }
    static var expectedBytes: Int64 { entry.expectedBytes }
    static var expectedSHA256: String { entry.expectedSHA256 }

    // Host compatibility.
    static var isAppleSilicon: Bool {
        #if arch(arm64)
        return true
        #else
        return false
        #endif
    }
    static var logsDir: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("logs", isDirectory: true)
    }
    static var physicalMemoryGB: Int {
        if let physicalMemoryGBOverride { return physicalMemoryGBOverride }
        return Int(ProcessInfo.processInfo.physicalMemory / (1024 * 1024 * 1024))
    }
    static var hardwareSupported: Bool {
        if physicalMemoryGBOverride != nil { return physicalMemoryGB >= entry.minRamGB }
        return isAppleSilicon && physicalMemoryGB >= entry.minRamGB
    }

    struct UIState {
        let osOk: Int32
        let hwOk: Int32
        let ramGB: Int32
        let statusCode: Int32   // 0=not downloaded, 1=downloading, 2=verifying, 3=ready, 4=error
        let pct: Int32
        let dlMB: Int32
        let totalMB: Int32
        let err: String
    }

    static func uiState() -> UIState {
        let totalMB = Int32(expectedBytes / (1024 * 1024))
        let dlMB = Int32((isInstalled ? expectedBytes : partialBytes) / (1024 * 1024))
        let hwOk: Int32 = hardwareSupported ? 1 : 0
        let ram = Int32(physicalMemoryGB)
        if isVerifying {
            return UIState(osOk: 1, hwOk: hwOk, ramGB: ram, statusCode: 2, pct: Int32(verifyPct), dlMB: totalMB, totalMB: totalMB, err: "")
        }
        if isDownloading {
            return UIState(osOk: 1, hwOk: hwOk, ramGB: ram, statusCode: 1, pct: Int32(downloadPct), dlMB: dlMB, totalMB: totalMB, err: "")
        }
        if isInstalled {
            return UIState(osOk: 1, hwOk: hwOk, ramGB: ram, statusCode: 3, pct: 100, dlMB: totalMB, totalMB: totalMB, err: "")
        }
        if let err = lastError, !err.isEmpty {
            return UIState(osOk: 1, hwOk: hwOk, ramGB: ram, statusCode: 4, pct: Int32(downloadPct), dlMB: dlMB, totalMB: totalMB, err: err)
        }
        return UIState(osOk: 1, hwOk: hwOk, ramGB: ram, statusCode: 0, pct: Int32(downloadPct), dlMB: dlMB, totalMB: totalMB, err: "")
    }

    static func availableDiskBytes() -> Int64 {
        if let availableDiskBytesOverride { return availableDiskBytesOverride }
        let fm = FileManager.default
        var probe = inklingDir
        while !fm.fileExists(atPath: probe.path) {
            let parent = probe.deletingLastPathComponent()
            if parent.path == probe.path { break }
            probe = parent
        }
        if let vals = try? probe.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey, .volumeAvailableCapacityKey]) {
            if let important = vals.volumeAvailableCapacityForImportantUsage, important > 0 { return important }
            if let cap = vals.volumeAvailableCapacity, cap > 0 { return Int64(cap) }
        }
        if let attrs = try? fm.attributesOfFileSystem(forPath: probe.path),
           let free = attrs[.systemFreeSize] as? NSNumber {
            return free.int64Value
        }
        return 0
    }

    static var weightsBytes: Int64 {
        (try? FileManager.default.attributesOfItem(atPath: weightsURL.path)[.size] as? NSNumber)?.int64Value ?? 0
    }
    static var weightsMtime: Int64 {
        guard let date = (try? FileManager.default.attributesOfItem(atPath: weightsURL.path)[.modificationDate] as? Date) else { return 0 }
        return Int64(date.timeIntervalSince1970)
    }
    static var partialBytes: Int64 {
        (try? FileManager.default.attributesOfItem(atPath: partialURL.path)[.size] as? NSNumber)?.int64Value ?? 0
    }
    static var diskUsageBytes: Int64 { weightsBytes + partialBytes }
    static var remainingDownloadBytes: Int64 { max(0, expectedBytes - partialBytes) }
    static var requiredFreeDiskBytes: Int64 { remainingDownloadBytes + entry.diskHeadroomBytes }
    static var hasEnoughDiskForDownload: Bool { availableDiskBytes() >= requiredFreeDiskBytes }

    // Verified manifest gate: never reports installed from file size alone.
    static func readManifest() -> LocalModelManifest? {
        guard let data = try? Data(contentsOf: manifestURL) else { return nil }
        return try? JSONDecoder().decode(LocalModelManifest.self, from: data)
    }

    static func manifestIsValid() -> Bool {
        guard weightsBytes == expectedBytes, weightsMtime > 0 else { return false }
        guard let m = readManifest() else { return false }
        return m.verified
            && m.model_id == entry.id
            && m.weights == entry.fileName
            && m.bytes == expectedBytes
            && m.sha256.lowercased() == expectedSHA256.lowercased()
            && m.mtime == weightsMtime
    }

    static var isInstalled: Bool { manifestIsValid() }
    static var hasUnverifiedWeights: Bool { weightsBytes == expectedBytes && !manifestIsValid() }
    static var isSelectable: Bool { hardwareSupported && isInstalled }

    // Live download & verification state.
    private static var downloadProcess: Process?
    private static var progressTimer: Timer?
    private static var pauseRequested = false
    private static var cancelRequested = false
    private static var lastSampleBytes: Int64 = 0
    private static var lastSampleTime: TimeInterval = 0
    private static var autoVerifyAttemptedPath: String?
    private static let verifyQueue = DispatchQueue(label: "openagents.local-models.verify", qos: .utility)

    static var isDownloading: Bool { downloadProcess?.isRunning == true }
    static private(set) var isVerifying: Bool = false
    static private(set) var verifyPct: Int = 0
    static private(set) var bytesPerSec: Double = 0
    static private(set) var etaSeconds: Double = -1
    static var lastError: String?

    static var hasPartial: Bool { partialBytes > 0 }
    static var downloadPct: Int {
        guard expectedBytes > 0 else { return 0 }
        let b = isInstalled ? expectedBytes : partialBytes
        let pct = Int((Double(b) / Double(expectedBytes)) * 100.0)
        return isInstalled ? 100 : min(99, max(0, pct))
    }

    static var state: LocalModelState {
        if !hardwareSupported { return .unsupportedHardware }
        if isVerifying { return .verifying }
        if isDownloading { return .downloading }
        if isInstalled { return .installed }
        if hasPartial { return .paused }
        return .notInstalled
    }

    // Streaming CryptoKit SHA-256 verification (4 MiB chunks; safe on 17.63 GB files).
    static func verifyFileSHA256(
        at url: URL,
        expectedSHA256: String,
        expectedBytes: Int64,
        onProgress: ((Int) -> Void)? = nil
    ) -> LocalModelVerificationResult {
        guard let handle = try? FileHandle(forReadingFrom: url) else {
            return LocalModelVerificationResult(ok: false, bytes: 0, sha256: "", error: "Could not open \(url.lastPathComponent) for SHA-256 verification.")
        }
        defer { try? handle.close() }
        var hasher = SHA256()
        var total: Int64 = 0
        var lastPct = -1
        let chunkSize = 4 * 1024 * 1024
        while true {
            var done = false
            autoreleasepool {
                let chunk = handle.readData(ofLength: chunkSize)
                if chunk.isEmpty {
                    done = true
                    return
                }
                hasher.update(data: chunk)
                total += Int64(chunk.count)
                if expectedBytes > 0, let onProgress {
                    let pct = min(100, max(0, Int((Double(total) / Double(expectedBytes)) * 100.0)))
                    if pct != lastPct {
                        lastPct = pct
                        onProgress(pct)
                    }
                }
            }
            if done { break }
        }
        let digest = hasher.finalize().map { String(format: "%02x", $0) }.joined()
        if total != expectedBytes {
            return LocalModelVerificationResult(
                ok: false, bytes: total, sha256: digest,
                error: "Size mismatch: got \(total) bytes, expected \(expectedBytes) bytes."
            )
        }
        if digest.lowercased() != expectedSHA256.lowercased() {
            return LocalModelVerificationResult(
                ok: false, bytes: total, sha256: digest,
                error: "SHA-256 mismatch: got \(digest.prefix(12))…, expected \(expectedSHA256.prefix(12))…."
            )
        }
        return LocalModelVerificationResult(ok: true, bytes: total, sha256: digest, error: nil)
    }

    @discardableResult
    static func writeVerifiedManifest(sha256: String) -> Bool {
        do {
            try FileManager.default.createDirectory(at: inklingDir, withIntermediateDirectories: true)
            let m = LocalModelManifest(
                model_id: entry.id,
                weights: entry.fileName,
                bytes: weightsBytes,
                sha256: sha256.lowercased(),
                mtime: weightsMtime,
                verified: true,
                loops: 3,
                verified_at: Int64(Date().timeIntervalSince1970)
            )
            let enc = JSONEncoder()
            enc.outputFormatting = [.sortedKeys]
            var data = try enc.encode(m)
            data.append(0x0A)
            try data.write(to: manifestURL, options: .atomic)
            return true
        } catch {
            lastError = "Could not write verified manifest: \(error.localizedDescription)"
            return false
        }
    }

    /// Atomically verifies a completed .part file and promotes it to weightsURL only if bytes & SHA-256 match.
    /// Removes corrupt .part files cleanly so corrupt checkpoints never become selectable.
    @discardableResult
    static func finalizeDownloadedPart(onProgress: ((Int) -> Void)? = nil) -> LocalModelVerificationResult {
        let res = verifyFileSHA256(at: partialURL, expectedSHA256: expectedSHA256, expectedBytes: expectedBytes, onProgress: onProgress)
        if !res.ok {
            try? FileManager.default.removeItem(at: partialURL)
            lastError = (res.error ?? "SHA-256 verification failed.") + " Corrupt download discarded."
            return res
        }
        do {
            if FileManager.default.fileExists(atPath: weightsURL.path) {
                try FileManager.default.removeItem(at: weightsURL)
            }
            try FileManager.default.moveItem(at: partialURL, to: weightsURL)
            _ = writeVerifiedManifest(sha256: res.sha256)
            lastError = nil
            return res
        } catch {
            let msg = "Could not finalize verified weights: \(error.localizedDescription)"
            lastError = msg
            return LocalModelVerificationResult(ok: false, bytes: res.bytes, sha256: res.sha256, error: msg)
        }
    }

    /// Automatically runs one-time background SHA-256 verification when an unmanifested GGUF already exists on disk.
    static func verifyExistingIfNeeded(onUpdate: @escaping () -> Void) {
        guard hardwareSupported, !isDownloading, !isVerifying, hasUnverifiedWeights else { return }
        let key = "\(weightsURL.path)#\(weightsMtime)#\(weightsBytes)"
        if autoVerifyAttemptedPath == key { return }
        autoVerifyAttemptedPath = key
        verifyInstalledNow(onUpdate: onUpdate)
    }

    /// User-triggered or startup background streaming SHA-256 check on weightsURL.
    static func verifyInstalledNow(onUpdate: @escaping () -> Void) {
        guard !isDownloading, !isVerifying, weightsBytes > 0 else { return }
        isVerifying = true
        verifyPct = 0
        lastError = nil
        onUpdate()
        let url = weightsURL
        let expHash = expectedSHA256
        let expBytes = expectedBytes
        verifyQueue.async {
            let res = verifyFileSHA256(at: url, expectedSHA256: expHash, expectedBytes: expBytes) { pct in
                DispatchQueue.main.async {
                    verifyPct = pct
                    onUpdate()
                }
            }
            DispatchQueue.main.async {
                isVerifying = false
                verifyPct = res.ok ? 100 : 0
                if res.ok {
                    _ = writeVerifiedManifest(sha256: res.sha256)
                    lastError = nil
                } else {
                    try? FileManager.default.removeItem(at: manifestURL)
                    lastError = (res.error ?? "SHA-256 verification failed.") + " Delete and re-download to restore."
                }
                onUpdate()
            }
        }
    }

    // Formatting helpers.
    static func formatBytes(_ bytes: Int64) -> String {
        let d = Double(max(0, bytes))
        if d >= 1_000_000_000 { return String(format: "%.2f GB", d / 1_000_000_000.0) }
        if d >= 1_000_000 { return String(format: "%.1f MB", d / 1_000_000.0) }
        if d >= 1_000 { return String(format: "%.0f KB", d / 1_000.0) }
        return "\(max(0, bytes)) B"
    }

    static func formatSpeed(_ bps: Double) -> String {
        guard bps > 1 else { return "—" }
        if bps >= 1_000_000 { return String(format: "%.1f MB/s", bps / 1_000_000.0) }
        return String(format: "%.0f KB/s", bps / 1_000.0)
    }

    static func formatETA(_ seconds: Double) -> String {
        guard seconds.isFinite, seconds >= 0 else { return "—" }
        let s = Int(seconds.rounded())
        if s < 60 { return "\(s)s" }
        let m = s / 60
        let remS = s % 60
        if m < 60 { return String(format: "%dm %02ds", m, remS) }
        return String(format: "%dh %02dm", m / 60, m % 60)
    }

    static func progressBar(pct: Int, width: Int = 20) -> String {
        let clamped = min(100, max(0, pct))
        let filled = Int((Double(clamped) / 100.0 * Double(width)).rounded())
        let empty = max(0, width - filled)
        return "[" + String(repeating: "█", count: filled) + String(repeating: "░", count: empty) + "] \(clamped)%"
    }

    /// Concise live status shown in Settings > Models and in the composer model picker footer.
    static func statusSummary() -> String {
        switch state {
        case .unsupportedHardware:
            return "Requires \(entry.minRamGB) GB RAM (\(physicalMemoryGB) GB detected)"
        case .verifying:
            return "Verifying SHA-256… (\(verifyPct)%)"
        case .downloading:
            let done = formatBytes(partialBytes)
            let tot = formatBytes(expectedBytes)
            if bytesPerSec > 1024 {
                return "Downloading \(downloadPct)% · \(done) / \(tot) · \(formatSpeed(bytesPerSec)) · ETA \(formatETA(etaSeconds))"
            }
            return "Downloading \(downloadPct)% · \(done) / \(tot)"
        case .paused:
            return "Paused \(downloadPct)% · \(formatBytes(partialBytes)) / \(formatBytes(expectedBytes)) · Resume…"
        case .installed:
            return "Installed (\(formatBytes(weightsBytes)) · SHA-256 verified)"
        case .notInstalled:
            if hasUnverifiedWeights {
                return "Unverified weights on disk · Verify SHA-256"
            }
            return "Not downloaded (\(formatBytes(expectedBytes))) · Download…"
        }
    }

    /// Detailed catalog, hardware compatibility, live download progress, and storage inspection body for the Local Models UI.
    static func dialogBody() -> String {
        let ramGB = physicalMemoryGB
        let freeBytes = availableDiskBytes()
        let ramMark = hardwareSupported ? "✓" : "✗"
        let diskOK = isInstalled || hasEnoughDiskForDownload
        let diskMark = diskOK ? "✓" : "✗"
        let shaShort = "\(expectedSHA256.prefix(12))…\(expectedSHA256.suffix(8))"

        var lines: [String] = []
        lines.append("\(entry.name) — \(entry.architecture)")
        lines.append("Format: \(entry.quantization) · Size: \(formatBytes(expectedBytes)) · SHA-256: \(shaShort)")
        lines.append("Host: \(ramGB) GB unified memory (\(ramMark) >= \(entry.minRamGB) GB) · Free disk: \(formatBytes(freeBytes)) (\(diskMark))")
        lines.append("Cache: \(inklingDir.path) · Disk usage: \(formatBytes(diskUsageBytes))")

        switch state {
        case .unsupportedHardware:
            lines.append("")
            lines.append("Unavailable on this Mac: \(entry.name) requires at least \(entry.minRamGB) GB of Apple Silicon unified memory.")
        case .installed:
            lines.append("")
            lines.append("Status: Installed & SHA-256 verified · Ready in composer model picker (Psionic Metal 4, 32K context).")
        case .verifying:
            lines.append("")
            lines.append("Status: Verifying SHA-256 checksum on disk…")
            lines.append(progressBar(pct: verifyPct))
        case .downloading:
            lines.append("")
            lines.append("Status: Downloading from Hugging Face (resumable)")
            lines.append("\(progressBar(pct: downloadPct))  \(formatBytes(partialBytes)) / \(formatBytes(expectedBytes))")
            lines.append("Speed: \(formatSpeed(bytesPerSec)) · ETA: \(formatETA(etaSeconds))")
        case .paused:
            lines.append("")
            lines.append("Status: Download paused (partial file kept for resume)")
            lines.append("\(progressBar(pct: downloadPct))  \(formatBytes(partialBytes)) / \(formatBytes(expectedBytes))")
        case .notInstalled:
            lines.append("")
            if hasUnverifiedWeights {
                lines.append("Status: Checkpoint found on disk without a verified SHA-256 manifest. Click Verify SHA-256 to authenticate it.")
            } else if !hasEnoughDiskForDownload {
                lines.append("Status: Insufficient free disk space — need \(formatBytes(requiredFreeDiskBytes)) free (\(formatBytes(freeBytes)) available).")
            } else {
                lines.append("Status: Ready to download (\(formatBytes(expectedBytes)), resumable, SHA-256 verified on completion).")
            }
        }
        if let err = lastError, !err.isEmpty {
            lines.append("")
            lines.append("Last error: \(err)")
        }
        return lines.joined(separator: "\n")
    }

    private static func stopProgressTimer() {
        progressTimer?.invalidate()
        progressTimer = nil
    }

    private static func resetSpeed() {
        bytesPerSec = 0
        etaSeconds = -1
        lastSampleBytes = 0
        lastSampleTime = 0
    }

    private static func sampleProgress() {
        let now = Date().timeIntervalSinceReferenceDate
        let cur = partialBytes
        if lastSampleTime > 0 {
            let dt = now - lastSampleTime
            if dt >= 0.25 {
                let delta = Double(max(0, cur - lastSampleBytes))
                let inst = delta / dt
                bytesPerSec = bytesPerSec > 0 ? (bytesPerSec * 0.65 + inst * 0.35) : inst
                if bytesPerSec > 1024 {
                    etaSeconds = Double(max(0, expectedBytes - cur)) / bytesPerSec
                } else {
                    etaSeconds = -1
                }
                lastSampleBytes = cur
                lastSampleTime = now
            }
        } else {
            lastSampleBytes = cur
            lastSampleTime = now
        }
    }

    /// Starts or resumes a resumable download (`curl -C -`), tracks speed/ETA, and verifies SHA-256 on completion.
    static func startDownload(onUpdate: @escaping () -> Void) {
        guard !isDownloading, !isVerifying else { return }
        guard hardwareSupported else {
            lastError = "\(entry.name) requires at least \(entry.minRamGB) GB unified memory (\(physicalMemoryGB) GB detected)."
            onUpdate()
            return
        }
        guard hasEnoughDiskForDownload else {
            lastError = "Insufficient disk space: need \(formatBytes(requiredFreeDiskBytes)) free, only \(formatBytes(availableDiskBytes())) available."
            onUpdate()
            return
        }
        lastError = nil
        pauseRequested = false
        cancelRequested = false
        resetSpeed()
        do {
            try FileManager.default.createDirectory(at: inklingDir, withIntermediateDirectories: true)
        } catch {
            lastError = "Could not create \(inklingDir.path): \(error.localizedDescription)"
            onUpdate()
            return
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
        proc.arguments = ["-fSL", "-C", "-", "--retry", "3", "--retry-delay", "2", "-o", partialURL.path, downloadURL]
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        proc.terminationHandler = { p in
            DispatchQueue.main.async {
                downloadProcess = nil
                stopProgressTimer()
                resetSpeed()
                if cancelRequested {
                    cancelRequested = false
                    try? FileManager.default.removeItem(at: partialURL)
                    onUpdate()
                    return
                }
                if pauseRequested {
                    pauseRequested = false
                    onUpdate()
                    return
                }
                if p.terminationStatus == 0 {
                    isVerifying = true
                    verifyPct = 0
                    onUpdate()
                    verifyQueue.async {
                        let res = finalizeDownloadedPart { pct in
                            DispatchQueue.main.async {
                                verifyPct = pct
                                onUpdate()
                            }
                        }
                        DispatchQueue.main.async {
                            isVerifying = false
                            verifyPct = res.ok ? 100 : 0
                            onUpdate()
                        }
                    }
                } else {
                    lastError = "Download interrupted (curl exit \(p.terminationStatus)). Click Resume to continue from \(formatBytes(partialBytes))."
                    onUpdate()
                }
            }
        }
        do {
            try proc.run()
            downloadProcess = proc
            lastSampleBytes = partialBytes
            lastSampleTime = Date().timeIntervalSinceReferenceDate
            stopProgressTimer()
            let t = Timer(timeInterval: 0.5, repeats: true) { _ in
                sampleProgress()
                onUpdate()
            }
            RunLoop.main.add(t, forMode: .common)
            progressTimer = t
            onUpdate()
        } catch {
            lastError = "Could not launch curl: \(error.localizedDescription)"
            onUpdate()
        }
    }

    /// Pauses an active download while preserving the `.part` file for later `curl -C -` resume.
    static func pauseDownload(onUpdate: @escaping () -> Void) {
        guard isDownloading else { return }
        pauseRequested = true
        stopProgressTimer()
        resetSpeed()
        downloadProcess?.terminate()
        onUpdate()
    }

    /// Cancels an active or paused download and deletes any `.part` file cleanly.
    static func cancelDownload(onUpdate: (() -> Void)? = nil) {
        cancelRequested = true
        pauseRequested = false
        stopProgressTimer()
        resetSpeed()
        if let proc = downloadProcess, proc.isRunning {
            proc.terminate()
        } else {
            downloadProcess = nil
            cancelRequested = false
            try? FileManager.default.removeItem(at: partialURL)
        }
        lastError = nil
        onUpdate?()
    }

    /// Removes installed/partial Inkling 2 Mini files and invokes stopServer before deletion.
    static func removeInkling(stopServer: () -> Void = {}) {
        cancelDownload()
        stopServer()
        autoVerifyAttemptedPath = nil
        try? FileManager.default.removeItem(at: inklingDir)
        lastError = nil
    }
}
