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
        check(LocalModelStore.formatBytes(43_805_184) == "43.8 MB", "formatBytes 43.8 MB")
        check(LocalModelStore.formatSpeed(85_200_000) == "85.2 MB/s", "formatSpeed 85.2 MB/s")
        check(LocalModelStore.formatETA(154) == "2m 34s", "formatETA 2m 34s")
        check(LocalModelStore.progressBar(pct: 50, width: 10) == "[█████░░░░░] 50%", "progressBar")

        // 12. Multi-variant catalog (Official + AtomicChat Uncensored LoRA & 3 quants + OrcaRouter Abliterated Mix-16G/12G + Custom).
        LocalModelStore.catalogOverride = nil
        LocalModelStore.activeVariantIdOverride = LocalModelCatalogEntry.inkling2Mini.id
        check(LocalModelStore.catalog.count == 8, "8 catalog variants")
        check(LocalModelCatalogEntry.inkling2MiniUncensoredLora.expectedBytes == 43_805_184, "lora bytes")
        check(LocalModelCatalogEntry.inkling2MiniUncensoredLora.expectedSHA256 == "42914b85c874e7fe249e93f0ac6e8abf08dc0bdf32aa1ec46c72714e64808f12", "lora sha256")
        check(LocalModelCatalogEntry.inkling2MiniUncensoredLora.baseVariantId == "inkling-2-mini", "lora base id")
        check(LocalModelCatalogEntry.inkling2MiniUncensoredQ4K.minRamGB == 24 && LocalModelCatalogEntry.inkling2MiniUncensoredQ4K.expectedBytes == 16_525_559_776, "AD-Q4_K meta")
        check(LocalModelCatalogEntry.inkling2MiniUncensoredIQ3S.minRamGB == 16 && LocalModelCatalogEntry.inkling2MiniUncensoredIQ3S.expectedBytes == 13_340_510_176, "AD-IQ3_S meta")
        check(LocalModelCatalogEntry.inkling2MiniUncensoredIQ2M.minRamGB == 12 && LocalModelCatalogEntry.inkling2MiniUncensoredIQ2M.expectedBytes == 8_665_091_680, "AD-IQ2_M meta")
        check(LocalModelCatalogEntry.inkling2MiniAbliteratedMix16G.requiresHFToken && LocalModelCatalogEntry.inkling2MiniAbliteratedMix16G.minRamGB == 24 && LocalModelCatalogEntry.inkling2MiniAbliteratedMix16G.expectedBytes == 15_721_831_424, "Mix-16G meta")
        check(LocalModelCatalogEntry.inkling2MiniAbliteratedMix12G.requiresHFToken && LocalModelCatalogEntry.inkling2MiniAbliteratedMix12G.minRamGB == 16 && LocalModelCatalogEntry.inkling2MiniAbliteratedMix12G.expectedBytes == 11_634_058_560, "Mix-12G meta")
        check(LocalModelCatalogEntry.inkling2MiniCustom.isCustom && LocalModelCatalogEntry.inkling2MiniCustom.minRamGB == 12, "Custom meta")

        // 13. Per-variant RAM admission (8/12/16/24/32 GB) & adaptive context sizing (8K/16K/32K).
        LocalModelStore.physicalMemoryGBOverride = 8
        check(!LocalModelStore.anyVariantSupported, "8GB unsupported for all variants")
        check(LocalModelStore.recommendedContextTokens == 8192, "8GB ctx 8192")
        LocalModelStore.physicalMemoryGBOverride = 12
        check(LocalModelStore.anyVariantSupported, "12GB supports AD-IQ2_M/Custom")
        check(LocalModelStore.hardwareSupported(for: .inkling2MiniUncensoredIQ2M), "12GB supports AD-IQ2_M")
        check(!LocalModelStore.hardwareSupported(for: .inkling2MiniUncensoredIQ3S), "12GB rejects AD-IQ3_S")
        check(LocalModelStore.defaultVariantId() == LocalModelCatalogEntry.inkling2MiniUncensoredIQ2M.id, "12GB default AD-IQ2_M")
        check(LocalModelStore.recommendedContextTokens == 8192, "12GB ctx 8192")
        LocalModelStore.physicalMemoryGBOverride = 16
        check(LocalModelStore.hardwareSupported(for: .inkling2MiniUncensoredIQ3S), "16GB supports AD-IQ3_S")
        check(LocalModelStore.hardwareSupported(for: .inkling2MiniAbliteratedMix12G), "16GB supports Mix-12G")
        check(!LocalModelStore.hardwareSupported(for: .inkling2MiniUncensoredQ4K), "16GB rejects AD-Q4_K")
        check(LocalModelStore.defaultVariantId() == LocalModelCatalogEntry.inkling2MiniUncensoredIQ3S.id, "16GB default AD-IQ3_S")
        LocalModelStore.physicalMemoryGBOverride = 24
        check(LocalModelStore.hardwareSupported(for: .inkling2MiniUncensoredQ4K), "24GB supports AD-Q4_K")
        check(LocalModelStore.hardwareSupported(for: .inkling2MiniAbliteratedMix16G), "24GB supports Mix-16G")
        check(!LocalModelStore.hardwareSupported(for: .inkling2Mini), "24GB rejects IQ4_NL")
        check(LocalModelStore.defaultVariantId() == LocalModelCatalogEntry.inkling2MiniUncensoredQ4K.id, "24GB default AD-Q4_K")
        check(LocalModelStore.recommendedContextTokens == 16384, "24GB ctx 16384")
        LocalModelStore.physicalMemoryGBOverride = 32
        check(LocalModelStore.hardwareSupported(for: .inkling2Mini), "32GB supports IQ4_NL")
        check(LocalModelStore.defaultVariantId() == LocalModelCatalogEntry.inkling2Mini.id, "32GB default IQ4_NL")
        check(LocalModelStore.recommendedContextTokens == 32768, "32GB ctx 32768")

        // 14. LoRA overlay activeModelURL/activeLoraURL & health basenames.
        check(LocalModelStore.setActiveVariant(LocalModelCatalogEntry.inkling2MiniUncensoredLora.id), "switch to LoRA variant")
        check(LocalModelStore.activeHealthModelName == "Inkling-2-Mini-IQ4_NL.gguf", "LoRA health model basename")
        check(LocalModelStore.activeHealthLoraName == "Inkling-2-Mini-Uncensored-LoRA.gguf", "LoRA health lora basename")
        check(LocalModelStore.activeLoraURL?.lastPathComponent == "Inkling-2-Mini-Uncensored-LoRA.gguf", "LoRA activeLoraURL")
        check(LocalModelStore.needsBaseModel, "LoRA reports needsBaseModel when base missing")

        // 15. GGUF v3 header validation + local BYO .gguf import + tamper detection.
        func makeSyntheticGGUF(version: UInt32, marker: String) -> Data {
            var d = Data([0x47, 0x47, 0x55, 0x46]) // "GGUF"
            var v = version.littleEndian
            d.append(Data(bytes: &v, count: 4))
            d.append(Data(repeating: 0, count: 16))
            d.append(Data(marker.utf8))
            d.append(Data(repeating: 0xAB, count: 4096))
            return d
        }
        let badMagicURL = tmp.appendingPathComponent("not-gguf.bin")
        try! Data("hello world not gguf at all 1234567890".utf8).write(to: badMagicURL)
        check(LocalModelStore.validateGGUFHeader(at: badMagicURL, expectLora: false) != nil, "rejects non-GGUF")

        let v2URL = tmp.appendingPathComponent("v2.gguf")
        try! makeSyntheticGGUF(version: 2, marker: "inkling2_loop").write(to: v2URL)
        check(LocalModelStore.validateGGUFHeader(at: v2URL, expectLora: false) != nil, "rejects GGUF v2")

        let wrongArchURL = tmp.appendingPathComponent("llama.gguf")
        try! makeSyntheticGGUF(version: 3, marker: "llama").write(to: wrongArchURL)
        check(LocalModelStore.validateGGUFHeader(at: wrongArchURL, expectLora: false) != nil, "rejects non-inkling2_loop")

        let goodModelURL = tmp.appendingPathComponent("my-abliterated.gguf")
        try! makeSyntheticGGUF(version: 3, marker: "general.architecture\0inkling2_loop").write(to: goodModelURL)
        check(LocalModelStore.validateGGUFHeader(at: goodModelURL, expectLora: false) == nil, "accepts GGUF v3 inkling2_loop")

        let goodLoraURL = tmp.appendingPathComponent("my-lora.gguf")
        try! makeSyntheticGGUF(version: 3, marker: "adapter.type\0lora").write(to: goodLoraURL)
        check(LocalModelStore.validateGGUFHeader(at: goodLoraURL, expectLora: true) == nil, "accepts GGUF v3 lora adapter")

        // Importing wrong-sized file into gated Mix-16G slot fails cleanly with size mismatch.
        let wrongSizeMix = LocalModelStore.importLocalGGUFSync(from: goodModelURL, for: .inkling2MiniAbliteratedMix16G)
        check(!wrongSizeMix.ok && (wrongSizeMix.error?.contains("Size mismatch") == true), "pinned gated slot rejects wrong size")

        // Importing into Custom slot succeeds, verifies SHA-256, and makes variant selectable!
        check(LocalModelStore.setActiveVariant(LocalModelCatalogEntry.inkling2MiniCustom.id), "switch to Custom")
        let customRes = LocalModelStore.importLocalGGUFSync(from: goodModelURL, for: .inkling2MiniCustom)
        check(customRes.ok && LocalModelStore.isValidSHA256Hex(customRes.sha256), "custom import succeeded with valid sha256")
        check(LocalModelStore.isInstalled && LocalModelStore.isSelectable, "custom variant installed & selectable")
        check(LocalModelStore.activeHealthModelName == "Custom-Inkling-2-Mini.gguf" && LocalModelStore.activeHealthLoraName == "", "custom health names")

        // 16. HF token storage, 0600 permissions, and injection sanitization.
        let tokFile = tmp.appendingPathComponent("hf-token")
        LocalModelStore.hfTokenURLOverride = tokFile
        check(LocalModelStore.cleanHFToken("hf_good_token_123") == "hf_good_token_123", "valid HF token accepted")
        check(LocalModelStore.cleanHFToken("hf_bad\"quote") == nil, "quote rejected in HF token")
        check(LocalModelStore.cleanHFToken("hf_bad\nline") == nil, "newline rejected in HF token")
        check(LocalModelStore.saveHFToken("hf_test_secret_9876"), "saveHFToken succeeds")
        check(LocalModelStore.savedHFToken == "hf_test_secret_9876", "savedHFToken round-trips")
        let perms = (try? fm.attributesOfItem(atPath: tokFile.path)[.posixPermissions] as? NSNumber)?.intValue ?? 0
        check((perms & 0o777) == 0o600, "hf-token has 0600 permissions")
        check(LocalModelStore.saveHFToken(""), "empty token deletes file")
        check(!fm.fileExists(atPath: tokFile.path), "hf-token file removed")

        // 17. All 8 variants produce valid dialogBody() under APP_DLG_BODY (1536 bytes).
        for v in LocalModelCatalogEntry.allVariants {
            check(LocalModelStore.setActiveVariant(v.id), "select \(v.id)")
            let body = LocalModelStore.dialogBody()
            check(body.utf8.count > 40 && body.utf8.count < 1400, "dialogBody fits APP_DLG_BODY for \(v.id) (\(body.utf8.count) B)")
        }

        print("local-models-check: ok (17/17 checks passed, SHA-256 \(digest.prefix(12))…)")
    }
}
