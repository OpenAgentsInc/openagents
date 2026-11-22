// Firm reason: 6 (standalone Swift verification check for LocalModels.swift CryptoKit SHA-256, resumable state, RAM/disk guardrails, and tamper refusal; #13826).
import CryptoKit
import Foundation

func check(_ cond: @autoclosure () -> Bool, _ msg: String) {
    if !cond() {
        fputs("FAIL: \(msg)\n", stderr)
        exit(1)
    }
}

@main
struct LocalModelsCheck {
    static func main() {
        let fm = FileManager.default
        let tmp = fm.temporaryDirectory.appendingPathComponent("oa-local-models-check-\(UUID().uuidString)", isDirectory: true)
        try! fm.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: tmp) }

        // 1. Verify pinned production catalog matches Inkling-2-Mini-IQ4_NL.gguf HF LFS metadata.
        let prod = LocalModelCatalogEntry.inkling2Mini
        check(prod.id == "inkling-2-mini", "prod id")
        check(prod.expectedBytes == 17_627_940_768, "prod expectedBytes")
        check(prod.expectedSHA256 == "cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4", "prod sha256")
        check(prod.minRamGB == 32, "prod minRamGB")

        // 2. Test isolated store with a real 512 KiB binary payload and real streaming CryptoKit SHA-256.
        var rawBytes = [UInt8](repeating: 0, count: 512 * 1024)
        for i in 0..<rawBytes.count { rawBytes[i] = UInt8((i &* 31 &+ 7) & 0xFF) }
        let payload = Data(rawBytes)
        let digest = SHA256.hash(data: payload).map { String(format: "%02x", $0) }.joined()
        LocalModelStore.rootDirOverride = tmp
        LocalModelStore.physicalMemoryGBOverride = 64
        LocalModelStore.availableDiskBytesOverride = 50_000_000_000
        LocalModelStore.catalogOverride = LocalModelCatalogEntry(
            id: "inkling-2-mini",
            name: "Inkling 2 Mini",
            architecture: "26.4B total / 3.4B active looped MoE (3 passes)",
            quantization: "IQ4_NL GGUF",
            fileName: "Inkling-2-Mini-IQ4_NL.gguf",
            downloadURL: "http://127.0.0.1:1/unused",
            expectedBytes: Int64(payload.count),
            expectedSHA256: digest,
            minRamGB: 32,
            diskHeadroomBytes: 1024 * 1024
        )

        // Initially not installed.
        check(LocalModelStore.state == .notInstalled, "initial state notInstalled")
        check(!LocalModelStore.isInstalled && !LocalModelStore.isSelectable, "initial not selectable")
        check(LocalModelStore.hardwareSupported && LocalModelStore.hasEnoughDiskForDownload, "64GB + 50GB disk compatible")

        // 3. Hardware RAM guardrail (< 32 GB) refuses even when valid files exist later.
        LocalModelStore.physicalMemoryGBOverride = 16
        check(LocalModelStore.state == .unsupportedHardware, "16GB unsupportedHardware")
        check(LocalModelStore.statusSummary().contains("Requires 32 GB RAM"), "16GB status summary")
        LocalModelStore.physicalMemoryGBOverride = 64

        // 4. Insufficient disk space guardrail.
        LocalModelStore.availableDiskBytesOverride = 100_000
        check(!LocalModelStore.hasEnoughDiskForDownload, "low disk rejected")
        check(LocalModelStore.dialogBody().contains("Insufficient free disk space"), "low disk dialog notice")
        LocalModelStore.availableDiskBytesOverride = 50_000_000_000

        // 5. Partial file represents paused resumable state; cancel deletes partial cleanly.
        try! fm.createDirectory(at: LocalModelStore.inklingDir, withIntermediateDirectories: true)
        try! payload.prefix(128 * 1024).write(to: LocalModelStore.partialURL)
        check(LocalModelStore.state == .paused, "partial triggers paused state")
        check(LocalModelStore.downloadPct == 25, "partial pct == 25")
        check(LocalModelStore.statusSummary().contains("Paused 25%"), "paused summary")
        LocalModelStore.cancelDownload()
        check(!fm.fileExists(atPath: LocalModelStore.partialURL.path), "cancel removes .part")
        check(LocalModelStore.state == .notInstalled, "after cancel -> notInstalled")

        // 6. Corrupt partial file fails SHA-256 verification and is discarded without becoming selectable.
        var corrupt = payload
        corrupt[12345] ^= 0xFF
        try! corrupt.write(to: LocalModelStore.partialURL)
        let badRes = LocalModelStore.finalizeDownloadedPart()
        check(!badRes.ok, "corrupt part fails SHA-256")
        check(!fm.fileExists(atPath: LocalModelStore.partialURL.path), "corrupt .part discarded")
        check(!LocalModelStore.isInstalled && !LocalModelStore.isSelectable, "corrupt never selectable")
        check(LocalModelStore.lastError?.contains("SHA-256 mismatch") == true, "corrupt error reported")

        // 7. Truncated partial file fails size check and is discarded.
        try! payload.prefix(payload.count - 1).write(to: LocalModelStore.partialURL)
        let truncRes = LocalModelStore.finalizeDownloadedPart()
        check(!truncRes.ok, "truncated part fails")
        check(LocalModelStore.lastError?.contains("Size mismatch") == true, "size mismatch error")

        // 8. Valid partial file passes streaming SHA-256, atomically promotes to weightsURL, writes verified manifest.
        try! payload.write(to: LocalModelStore.partialURL)
        var saw100 = false
        let okRes = LocalModelStore.finalizeDownloadedPart { pct in if pct == 100 { saw100 = true } }
        check(okRes.ok && okRes.sha256 == digest && okRes.bytes == Int64(payload.count), "valid part verified")
        check(saw100, "progress reached 100")
        check(!fm.fileExists(atPath: LocalModelStore.partialURL.path), "partial moved away")
        check(fm.fileExists(atPath: LocalModelStore.weightsURL.path), "weights promoted")
        check(LocalModelStore.isInstalled && LocalModelStore.isSelectable, "installed & selectable")
        check(LocalModelStore.state == .installed, "state == installed")
        check(LocalModelStore.statusSummary().contains("SHA-256 verified"), "status mentions SHA-256 verified")
        let manifest = LocalModelStore.readManifest()
        check(manifest?.verified == true && manifest?.sha256 == digest && manifest?.bytes == Int64(payload.count), "manifest verified")

        // 9. Size-only unverified file (deleting manifest) is NOT reported installed until verified.
        try! fm.removeItem(at: LocalModelStore.manifestURL)
        check(!LocalModelStore.isInstalled && !LocalModelStore.isSelectable, "missing manifest refuses installed")
        check(LocalModelStore.hasUnverifiedWeights, "hasUnverifiedWeights true")

        // Re-verify via writeVerifiedManifest, then tamper with weights on disk (change mtime) -> immediately invalidated!
        check(LocalModelStore.writeVerifiedManifest(sha256: digest), "re-write manifest")
        check(LocalModelStore.isInstalled, "installed again")
        let futureDate = Date(timeIntervalSinceNow: 120)
        try! fm.setAttributes([.modificationDate: futureDate], ofItemAtPath: LocalModelStore.weightsURL.path)
        check(!LocalModelStore.isInstalled, "mtime tamper immediately invalidates manifest")

        // 10. Deletion stops server callback and removes cache directory cleanly.
        var stopped = false
        LocalModelStore.removeInkling { stopped = true }
        check(stopped, "removeInkling invoked stopServer")
        check(!fm.fileExists(atPath: LocalModelStore.inklingDir.path), "inklingDir deleted")
        check(LocalModelStore.diskUsageBytes == 0 && LocalModelStore.state == .notInstalled, "disk usage zero after delete")

        // 11. Real-world formatting checks.
        check(LocalModelStore.formatBytes(17_627_940_768) == "17.63 GB", "formatBytes 17.63 GB")
        check(LocalModelStore.formatSpeed(85_200_000) == "85.2 MB/s", "formatSpeed 85.2 MB/s")
        check(LocalModelStore.formatETA(154) == "2m 34s", "formatETA 2m 34s")
        check(LocalModelStore.progressBar(pct: 50, width: 10) == "[█████░░░░░] 50%", "progressBar")

        print("local-models-check: ok (11/11 checks passed, SHA-256 \(digest.prefix(12))…)")
    }
}
