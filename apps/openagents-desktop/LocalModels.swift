// Firm reason: 6 (macOS platform filesystem, curl process, CryptoKit SHA-256, and disk-space APIs for Local Models manager; #13826, #14028).
import CryptoKit
import Foundation

/// Metadata for a local checkpoint or LoRA adapter managed by OpenAgents Desktop (~/.openagents/models/<id>/).
struct LocalModelCatalogEntry: Equatable {
    let id: String
    let name: String
    let shortLabel: String
    let architecture: String
    let quantization: String
    let fileName: String
    let downloadURL: String
    let expectedBytes: Int64
    let expectedSHA256: String
    let minRamGB: Int
    let diskHeadroomBytes: Int64
    let baseVariantId: String?
    let requiresHFToken: Bool
    let hfRepoURL: String?
    let isCustom: Bool
    let sourceLabel: String
    let httpRangeEnd: Int64?

    init(
        id: String,
        name: String,
        shortLabel: String? = nil,
        architecture: String,
        quantization: String,
        fileName: String,
        downloadURL: String,
        expectedBytes: Int64,
        expectedSHA256: String,
        minRamGB: Int,
        diskHeadroomBytes: Int64,
        baseVariantId: String? = nil,
        requiresHFToken: Bool = false,
        hfRepoURL: String? = nil,
        isCustom: Bool = false,
        sourceLabel: String = "Hugging Face",
        httpRangeEnd: Int64? = nil
    ) {
        self.id = id
        self.name = name
        self.shortLabel = shortLabel ?? name
        self.architecture = architecture
        self.quantization = quantization
        self.fileName = fileName
        self.downloadURL = downloadURL
        self.expectedBytes = expectedBytes
        self.expectedSHA256 = expectedSHA256
        self.minRamGB = minRamGB
        self.diskHeadroomBytes = diskHeadroomBytes
        self.baseVariantId = baseVariantId
        self.requiresHFToken = requiresHFToken
        self.hfRepoURL = hfRepoURL
        self.isCustom = isCustom
        self.sourceLabel = sourceLabel
        self.httpRangeEnd = httpRangeEnd
    }

    var isLoraOverlay: Bool { baseVariantId != nil }
    var isSafetensorsOverlay: Bool { fileName.lowercased().hasSuffix(".safetensors") }

    static let inkling2Mini = LocalModelCatalogEntry(
        id: "inkling-2-mini",
        name: "Inkling 2 Mini",
        shortLabel: "Official IQ4_NL",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes)",
        quantization: "IQ4_NL GGUF",
        fileName: "Inkling-2-Mini-IQ4_NL.gguf",
        downloadURL: "https://huggingface.co/ggml-org/Inkling-2-Mini-GGUF/resolve/main/Inkling-2-Mini-IQ4_NL.gguf",
        expectedBytes: 17_627_940_768,
        expectedSHA256: "cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4",
        minRamGB: 32,
        diskHeadroomBytes: 536_870_912,
        hfRepoURL: "https://huggingface.co/ggml-org/Inkling-2-Mini-GGUF",
        sourceLabel: "ggml-org (Official)"
    )

    static let inkling2MiniUncensoredLora = LocalModelCatalogEntry(
        id: "inkling-2-mini-uncensored-lora",
        name: "Inkling 2 Mini Uncensored (LoRA)",
        shortLabel: "Uncensored LoRA",
        architecture: "43.8 MB direction-ablation LoRA over official IQ4_NL base",
        quantization: "GGUF LoRA Adapter",
        fileName: "Inkling-2-Mini-Uncensored-LoRA.gguf",
        downloadURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF/resolve/main/Inkling-2-Mini-Uncensored-LoRA.gguf",
        expectedBytes: 43_805_184,
        expectedSHA256: "42914b85c874e7fe249e93f0ac6e8abf08dc0bdf32aa1ec46c72714e64808f12",
        minRamGB: 32,
        diskHeadroomBytes: 67_108_864,
        baseVariantId: "inkling-2-mini",
        hfRepoURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF",
        sourceLabel: "AtomicChat (LoRA)"
    )

    static let inkling2MiniUncensoredMaple = LocalModelCatalogEntry(
        id: "inkling-2-mini-uncensored-maple",
        name: "Inkling 2 Mini Uncensored (Maple)",
        shortLabel: "Uncensored (Maple)",
        architecture: "2.29 GB on-policy BF16 attention overlay (120 tensors) over official IQ4_NL base · Sponsored by Maple AI",
        quantization: "Safetensors Attn Overlay",
        fileName: "Inkling-2-Mini-Uncensored-Maple-Attn.safetensors",
        downloadURL: "https://huggingface.co/benthecarman/Inkling-2-Mini-uncensored/resolve/main/model-00001-of-00001.safetensors",
        expectedBytes: 2_287_385_184,
        expectedSHA256: "de1ab65c04db287320bdda55f6fb070cecf2e6a589870f08c48a65fac41e03fe",
        minRamGB: 32,
        diskHeadroomBytes: 268_435_456,
        baseVariantId: "inkling-2-mini",
        hfRepoURL: "https://huggingface.co/benthecarman/Inkling-2-Mini-uncensored",
        sourceLabel: "Maple AI (benthecarman)",
        httpRangeEnd: 2_287_385_183
    )

    static let inkling2MiniUncensoredQ4K = LocalModelCatalogEntry(
        id: "inkling-2-mini-uncensored-q4k",
        name: "Inkling 2 Mini Uncensored (AD-Q4_K)",
        shortLabel: "Uncensored Q4_K",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes, uncensored)",
        quantization: "AD-Q4_K GGUF",
        fileName: "Inkling-2-Mini-Uncensored-AD-Q4_K.gguf",
        downloadURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF/resolve/main/Inkling-2-Mini-Uncensored-AD-Q4_K.gguf",
        expectedBytes: 16_525_559_776,
        expectedSHA256: "36e99f1f56ca33cb356f19d7e3822938f65af1cde978ad13170238928d4c955f",
        minRamGB: 24,
        diskHeadroomBytes: 536_870_912,
        hfRepoURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF",
        sourceLabel: "AtomicChat"
    )

    static let inkling2MiniUncensoredIQ3S = LocalModelCatalogEntry(
        id: "inkling-2-mini-uncensored-iq3s",
        name: "Inkling 2 Mini Uncensored (AD-IQ3_S)",
        shortLabel: "Uncensored IQ3_S",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes, uncensored)",
        quantization: "AD-IQ3_S GGUF",
        fileName: "Inkling-2-Mini-Uncensored-AD-IQ3_S.gguf",
        downloadURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF/resolve/main/Inkling-2-Mini-Uncensored-AD-IQ3_S.gguf",
        expectedBytes: 13_340_510_176,
        expectedSHA256: "23a8e6792481b6f1d0ebad51399e9ee2a2c2be8525b09ed1ae6737811159f892",
        minRamGB: 16,
        diskHeadroomBytes: 536_870_912,
        hfRepoURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF",
        sourceLabel: "AtomicChat"
    )

    static let inkling2MiniUncensoredIQ2M = LocalModelCatalogEntry(
        id: "inkling-2-mini-uncensored-iq2m",
        name: "Inkling 2 Mini Uncensored (AD-IQ2_M)",
        shortLabel: "Uncensored IQ2_M",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes, uncensored)",
        quantization: "AD-IQ2_M GGUF",
        fileName: "Inkling-2-Mini-Uncensored-AD-IQ2_M.gguf",
        downloadURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF/resolve/main/Inkling-2-Mini-Uncensored-AD-IQ2_M.gguf",
        expectedBytes: 8_665_091_680,
        expectedSHA256: "fca99d488bff5d401c8bc6b0110c5377c743f0ffb3a6d19c1ae4a3b9071fa2cd",
        minRamGB: 12,
        diskHeadroomBytes: 536_870_912,
        hfRepoURL: "https://huggingface.co/AtomicChat/Inkling-2-Mini-Uncensored-GGUF",
        sourceLabel: "AtomicChat"
    )

    static let inkling2MiniAbliteratedMix16G = LocalModelCatalogEntry(
        id: "inkling-2-mini-abliterated-mix16g",
        name: "Inkling 2 Mini Abliterated (Mix-16G)",
        shortLabel: "Abliterated Mix-16G",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes, abliterated)",
        quantization: "Mix-16G GGUF (HF gated)",
        fileName: "Inkling-2-Mini-abliterated-Mix-16G.gguf",
        downloadURL: "https://huggingface.co/orcarouter/Inkling-2-Mini-abliterated-GGUF/resolve/main/Inkling-2-Mini-abliterated-Mix-16G.gguf",
        expectedBytes: 15_721_831_424,
        expectedSHA256: "",
        minRamGB: 24,
        diskHeadroomBytes: 536_870_912,
        requiresHFToken: true,
        hfRepoURL: "https://huggingface.co/orcarouter/Inkling-2-Mini-abliterated-GGUF",
        sourceLabel: "OrcaRouter (HF Gated)"
    )

    static let inkling2MiniAbliteratedMix12G = LocalModelCatalogEntry(
        id: "inkling-2-mini-abliterated-mix12g",
        name: "Inkling 2 Mini Abliterated (Mix-12G)",
        shortLabel: "Abliterated Mix-12G",
        architecture: "26.4B total / 3.4B active looped MoE (3 passes, abliterated)",
        quantization: "Mix-12G GGUF (HF gated)",
        fileName: "Inkling-2-Mini-abliterated-Mix-12G.gguf",
        downloadURL: "https://huggingface.co/orcarouter/Inkling-2-Mini-abliterated-GGUF/resolve/main/Inkling-2-Mini-abliterated-Mix-12G.gguf",
        expectedBytes: 11_634_058_560,
        expectedSHA256: "",
        minRamGB: 16,
        diskHeadroomBytes: 536_870_912,
        requiresHFToken: true,
        hfRepoURL: "https://huggingface.co/orcarouter/Inkling-2-Mini-abliterated-GGUF",
        sourceLabel: "OrcaRouter (HF Gated)"
    )

    static let inkling2MiniCustom = LocalModelCatalogEntry(
        id: "inkling-2-mini-custom",
        name: "Inkling 2 Mini (Custom Local .gguf)",
        shortLabel: "Custom .gguf",
        architecture: "26.4B total / 3.4B active looped MoE (imported GGUF v3)",
        quantization: "Custom GGUF",
        fileName: "Custom-Inkling-2-Mini.gguf",
        downloadURL: "",
        expectedBytes: 0,
        expectedSHA256: "",
        minRamGB: 12,
        diskHeadroomBytes: 536_870_912,
        isCustom: true,
        sourceLabel: "Local File Import"
    )

    static let allVariants: [LocalModelCatalogEntry] = [
        .inkling2MiniUncensoredLora,
        .inkling2MiniUncensoredMaple,
        .inkling2MiniUncensoredQ4K,
        .inkling2MiniUncensoredIQ3S,
        .inkling2MiniUncensoredIQ2M,
        .inkling2Mini,
        .inkling2MiniAbliteratedMix16G,
        .inkling2MiniAbliteratedMix12G,
        .inkling2MiniCustom,
    ]
}

/// Controls whether Cloud & BYOK models appear in the composer model picker alongside Local Inkling.
/// Defaults to true in v1.0.0 so OpenAgents Cloud (Gemini 4 Neon) and BYOK (Gemini/Claude/OpenRouter)
/// work out of the box alongside local Inkling 2 Mini.
enum ComposerModelVisibility {
    private static let key = "composer.showOtherModels"
    static var showOtherModels: Bool {
        get {
            if UserDefaults.standard.object(forKey: key) == nil { return true }
            return UserDefaults.standard.bool(forKey: key)
        }
        set { UserDefaults.standard.set(newValue, forKey: key) }
    }
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

/// Verified local GGUF checkpoint downloader and manager (~/.openagents/models) for OpenAgents Desktop (#13825, #13826, #14028).
enum LocalModelStore {
    private static let activeVariantDefaultsKey = "localModel.activeVariantId"
    private static let customDisplayNameDefaultsKey = "localModel.customDisplayName"

    // Test overrides (nil in production).
    static var rootDirOverride: URL?
    static var physicalMemoryGBOverride: Int?
    static var availableDiskBytesOverride: Int64?
    static var catalogOverride: LocalModelCatalogEntry?
    static var activeVariantIdOverride: String?
    static var hfTokenURLOverride: URL?

    static var catalog: [LocalModelCatalogEntry] {
        if let catalogOverride { return [catalogOverride] }
        return LocalModelCatalogEntry.allVariants
    }

    static func variant(id: String) -> LocalModelCatalogEntry? {
        catalog.first { $0.id == id }
    }

    /// Best default variant for this Mac's unified memory when the user has not explicitly picked one yet.
    /// Defaults to our gBench-verified uncensored champion on every RAM tier (LoRA over IQ4_NL at >= 32 GB,
    /// AD-Q4_K at 24 GB, AD-IQ3_S at 16 GB, AD-IQ2_M at 12 GB).
    static func defaultVariantId() -> String {
        if let catalogOverride { return catalogOverride.id }
        if physicalMemoryGB >= 32 {
            return LocalModelCatalogEntry.inkling2MiniUncensoredLora.id
        }
        if physicalMemoryGB >= 24 { return LocalModelCatalogEntry.inkling2MiniUncensoredQ4K.id }
        if physicalMemoryGB >= 16 { return LocalModelCatalogEntry.inkling2MiniUncensoredIQ3S.id }
        return LocalModelCatalogEntry.inkling2MiniUncensoredIQ2M.id
    }

    static var activeVariantId: String {
        if let catalogOverride { return catalogOverride.id }
        if let activeVariantIdOverride, variant(id: activeVariantIdOverride) != nil {
            return activeVariantIdOverride
        }
        if let saved = UserDefaults.standard.string(forKey: activeVariantDefaultsKey),
           variant(id: saved) != nil {
            return saved
        }
        return defaultVariantId()
    }

    @discardableResult
    static func setActiveVariant(_ id: String) -> Bool {
        guard !isDownloading, !isVerifying else { return false }
        guard variant(id: id) != nil else { return false }
        if activeVariantIdOverride != nil {
            activeVariantIdOverride = id
        } else {
            UserDefaults.standard.set(id, forKey: activeVariantDefaultsKey)
        }
        lastError = nil
        autoVerifyAttemptedPath = nil
        return true
    }

    static var customDisplayName: String? {
        get { UserDefaults.standard.string(forKey: customDisplayNameDefaultsKey) }
        set {
            if let v = newValue, !v.isEmpty {
                UserDefaults.standard.set(v, forKey: customDisplayNameDefaultsKey)
            } else {
                UserDefaults.standard.removeObject(forKey: customDisplayNameDefaultsKey)
            }
        }
    }

    static var entry: LocalModelCatalogEntry {
        if let catalogOverride { return catalogOverride }
        return variant(id: activeVariantId) ?? .inkling2Mini
    }

    static var baseEntry: LocalModelCatalogEntry? {
        guard let baseId = entry.baseVariantId else { return nil }
        return variant(id: baseId) ?? .inkling2Mini
    }

    static var rootDir: URL {
        if let rootDirOverride { return rootDirOverride }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("models", isDirectory: true)
    }

    static func dir(for item: LocalModelCatalogEntry) -> URL {
        rootDir.appendingPathComponent(item.id, isDirectory: true)
    }
    static func weightsURL(for item: LocalModelCatalogEntry) -> URL {
        dir(for: item).appendingPathComponent(item.fileName)
    }
    static func partialURL(for item: LocalModelCatalogEntry) -> URL {
        dir(for: item).appendingPathComponent(item.fileName + ".part")
    }
    static func manifestURL(for item: LocalModelCatalogEntry) -> URL {
        dir(for: item).appendingPathComponent("manifest.json")
    }

    static var inklingDir: URL { dir(for: entry) }
    static var weightsURL: URL { weightsURL(for: entry) }
    static var partialURL: URL { partialURL(for: entry) }
    static var manifestURL: URL { manifestURL(for: entry) }
    static var downloadURL: String { entry.downloadURL }
    static var expectedBytes: Int64 { entry.expectedBytes }
    static var expectedSHA256: String { entry.expectedSHA256 }

    /// Model GGUF path passed to `--model` on `psionic-inkling-serve`.
    /// When the active variant is a LoRA overlay, `--model` points at the base IQ4_NL checkpoint.
    static var activeModelURL: URL {
        if let base = baseEntry { return weightsURL(for: base) }
        return weightsURL(for: entry)
    }

    /// Optional LoRA GGUF path passed to `--lora` on `psionic-inkling-serve`.
    static var activeLoraURL: URL? {
        entry.isLoraOverlay ? weightsURL(for: entry) : nil
    }

    /// Expected `/health` `"model"` basename for the currently active variant.
    static var activeHealthModelName: String { activeModelURL.lastPathComponent }

    /// Expected `/health` `"lora_file"` basename (`""` when no LoRA is active).
    static var activeHealthLoraName: String { activeLoraURL?.lastPathComponent ?? "" }

    // Host compatibility & adaptive context window (mirrors coax/lib/agent/desktop/local-model.coax).
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
    static func hardwareSupported(for item: LocalModelCatalogEntry) -> Bool {
        if physicalMemoryGBOverride != nil { return physicalMemoryGB >= item.minRamGB }
        return isAppleSilicon && physicalMemoryGB >= item.minRamGB
    }
    static var hardwareSupported: Bool { hardwareSupported(for: entry) }
    static var anyVariantSupported: Bool { catalog.contains { hardwareSupported(for: $0) } }
    static var minSupportedRamGB: Int { catalog.map(\.minRamGB).min() ?? 12 }

    static var recommendedContextTokens: Int {
        if physicalMemoryGB >= 32 { return 32768 }
        if physicalMemoryGB >= 24 { return 16384 }
        return 8192
    }

    // ---- Hugging Face token management (for gated repos like OrcaRouter; passed only via curl -K - stdin) ----

    static var hfTokenURL: URL {
        if let hfTokenURLOverride { return hfTokenURLOverride }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("hf-token")
    }

    private static var hfCliTokenURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/huggingface/token")
    }

    static func cleanHFToken(_ raw: String?) -> String? {
        guard let raw else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        // Refuse control characters or quotes so stdin curl config cannot be injected.
        for u in trimmed.unicodeScalars {
            if u.value < 0x21 || u.value > 0x7E || u == "\"" || u == "\\" { return nil }
        }
        return trimmed
    }

    static var savedHFToken: String? {
        guard let raw = try? String(contentsOf: hfTokenURL, encoding: .utf8) else { return nil }
        return cleanHFToken(raw)
    }

    static var effectiveHFToken: String? {
        if let env = cleanHFToken(ProcessInfo.processInfo.environment["HF_TOKEN"]) { return env }
        if let saved = savedHFToken { return saved }
        if let cli = try? String(contentsOf: hfCliTokenURL, encoding: .utf8),
           let clean = cleanHFToken(cli) {
            return clean
        }
        return nil
    }

    static var hfTokenSource: String? {
        if cleanHFToken(ProcessInfo.processInfo.environment["HF_TOKEN"]) != nil { return "HF_TOKEN env" }
        if savedHFToken != nil { return "saved token" }
        if let cli = try? String(contentsOf: hfCliTokenURL, encoding: .utf8), cleanHFToken(cli) != nil {
            return "~/.cache/huggingface/token"
        }
        return nil
    }

    @discardableResult
    static func saveHFToken(_ raw: String) -> Bool {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            try? FileManager.default.removeItem(at: hfTokenURL)
            return true
        }
        guard let clean = cleanHFToken(trimmed) else { return false }
        do {
            let parent = hfTokenURL.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true)
            try (clean + "\n").write(to: hfTokenURL, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: hfTokenURL.path)
            return true
        } catch {
            return false
        }
    }

    static func hfTokenTail(_ token: String) -> String {
        guard token.count > 6 else { return "••••" }
        return "…\(token.suffix(4))"
    }

    // ---- UI State Bridge ----

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
        let tgt = activeDownloadEntry
        let expB = effectiveExpectedBytes(for: tgt)
        let partB = partialBytes(for: tgt)
        let totalMB = Int32(expB / (1024 * 1024))
        let dlMB = Int32((isInstalled ? expB : partB) / (1024 * 1024))
        let hwOk: Int32 = anyVariantSupported ? 1 : 0
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

    static func weightsBytes(for item: LocalModelCatalogEntry) -> Int64 {
        (try? FileManager.default.attributesOfItem(atPath: weightsURL(for: item).path)[.size] as? NSNumber)?.int64Value ?? 0
    }
    static func weightsMtime(for item: LocalModelCatalogEntry) -> Int64 {
        guard let date = (try? FileManager.default.attributesOfItem(atPath: weightsURL(for: item).path)[.modificationDate] as? Date) else { return 0 }
        return Int64(date.timeIntervalSince1970)
    }
    static func partialBytes(for item: LocalModelCatalogEntry) -> Int64 {
        (try? FileManager.default.attributesOfItem(atPath: partialURL(for: item).path)[.size] as? NSNumber)?.int64Value ?? 0
    }
    static func effectiveExpectedBytes(for item: LocalModelCatalogEntry) -> Int64 {
        if item.expectedBytes > 0 { return item.expectedBytes }
        return weightsBytes(for: item)
    }

    static var weightsBytes: Int64 { weightsBytes(for: entry) }
    static var weightsMtime: Int64 { weightsMtime(for: entry) }
    static var partialBytes: Int64 { partialBytes(for: entry) }
    static var diskUsageBytes: Int64 { weightsBytes + partialBytes }
    static var totalCacheDiskUsageBytes: Int64 {
        catalog.reduce(Int64(0)) { $0 + weightsBytes(for: $1) + partialBytes(for: $1) }
    }

    /// Next file to download to make `entry` ready: for a LoRA overlay whose base IQ4_NL weights are
    /// not yet verified on disk, download the base first and automatically chain the LoRA adapter after.
    static var nextDownloadTarget: LocalModelCatalogEntry {
        if entry.isLoraOverlay, let base = baseEntry, !isFileInstalled(for: base) {
            return base
        }
        return entry
    }

    /// Entry whose download/partial progress is actively tracked for UI display.
    static var activeDownloadEntry: LocalModelCatalogEntry {
        downloadTargetEntry ?? nextDownloadTarget
    }

    static var remainingDownloadBytes: Int64 {
        var rem: Int64 = 0
        if entry.isLoraOverlay, let base = baseEntry, !isFileInstalled(for: base) {
            rem += max(0, base.expectedBytes - partialBytes(for: base))
        }
        if !isFileInstalled(for: entry) && entry.expectedBytes > 0 {
            rem += max(0, entry.expectedBytes - partialBytes(for: entry))
        }
        return rem
    }

    static var requiredFreeDiskBytes: Int64 {
        let rem = remainingDownloadBytes
        guard rem > 0 else { return 0 }
        let headroom = (entry.isLoraOverlay && needsBaseModel)
            ? (baseEntry?.diskHeadroomBytes ?? entry.diskHeadroomBytes)
            : entry.diskHeadroomBytes
        return rem + headroom
    }
    static var hasEnoughDiskForDownload: Bool { availableDiskBytes() >= requiredFreeDiskBytes }

    // ---- Tamper-evident manifest gate ----

    static func isValidSHA256Hex(_ s: String) -> Bool {
        guard s.count == 64 else { return false }
        for b in s.utf8 {
            let ok = (b >= 48 && b <= 57) || (b >= 65 && b <= 70) || (b >= 97 && b <= 102)
            if !ok { return false }
        }
        return true
    }

    static func readManifest(for item: LocalModelCatalogEntry) -> LocalModelManifest? {
        guard let data = try? Data(contentsOf: manifestURL(for: item)) else { return nil }
        return try? JSONDecoder().decode(LocalModelManifest.self, from: data)
    }
    static func readManifest() -> LocalModelManifest? { readManifest(for: entry) }

    static func manifestIsValid(for item: LocalModelCatalogEntry) -> Bool {
        let wBytes = weightsBytes(for: item)
        let wMtime = weightsMtime(for: item)
        guard wBytes > 0, wMtime > 0 else { return false }
        if item.expectedBytes > 0, wBytes != item.expectedBytes { return false }
        guard let m = readManifest(for: item) else { return false }
        guard m.verified
            && m.model_id == item.id
            && m.weights == item.fileName
            && m.bytes == wBytes
            && m.mtime == wMtime
        else { return false }
        if !item.expectedSHA256.isEmpty {
            return m.sha256.lowercased() == item.expectedSHA256.lowercased()
        }
        return isValidSHA256Hex(m.sha256)
    }
    static func manifestIsValid() -> Bool { manifestIsValid(for: entry) }

    static func isFileInstalled(for item: LocalModelCatalogEntry) -> Bool {
        manifestIsValid(for: item)
    }

    static func isBaseInstalled(for item: LocalModelCatalogEntry) -> Bool {
        guard let baseId = item.baseVariantId else { return true }
        let base = variant(id: baseId) ?? .inkling2Mini
        return isFileInstalled(for: base)
    }

    static func isInstalled(for item: LocalModelCatalogEntry) -> Bool {
        isFileInstalled(for: item) && isBaseInstalled(for: item)
    }

    static func isSelectable(for item: LocalModelCatalogEntry) -> Bool {
        hardwareSupported(for: item) && isInstalled(for: item)
    }

    static func hasUnverifiedWeights(for item: LocalModelCatalogEntry) -> Bool {
        let wb = weightsBytes(for: item)
        if item.expectedBytes > 0 {
            return wb == item.expectedBytes && !manifestIsValid(for: item)
        }
        return wb > 0 && !manifestIsValid(for: item)
    }

    static var isInstalled: Bool { isInstalled(for: entry) }
    static var isFileInstalled: Bool { isFileInstalled(for: entry) }
    static var needsBaseModel: Bool { entry.isLoraOverlay && !isBaseInstalled(for: entry) }
    static var hasUnverifiedWeights: Bool {
        if entry.isLoraOverlay, let base = baseEntry, hasUnverifiedWeights(for: base) {
            return true
        }
        return hasUnverifiedWeights(for: entry)
    }
    static var isSelectable: Bool { isSelectable(for: entry) }

    // ---- GGUF v3 structural validation ----

    /// Verifies GGUF v3 magic/version and Inkling 2 Mini architecture (or LoRA adapter metadata) in the header.
    static func validateGGUFHeader(at url: URL, expectLora: Bool) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else {
            return "Could not open \(url.lastPathComponent) to validate GGUF header."
        }
        defer { try? handle.close() }
        let head = handle.readData(ofLength: 131_072)
        guard head.count >= 24 else {
            return "\(url.lastPathComponent) is too small to be a valid GGUF file."
        }
        if expectLora && url.pathExtension.lowercased() == "safetensors" {
            var hlen: UInt64 = 0
            for i in 0..<8 { hlen |= UInt64(head[i]) << UInt64(i * 8) }
            if hlen < 2 || hlen > 67_108_864 || head[8] != 0x7B {
                return "\(url.lastPathComponent) is not a valid safetensors attention overlay."
            }
            let attnMarker = Data("model.llm.layers.0.attn.wq_du.weight".utf8)
            if head.range(of: attnMarker) == nil {
                return "\(url.lastPathComponent) is missing Inkling 2 Mini attention overlay tensors."
            }
            return nil
        }
        // Magic "GGUF"
        if head[0] != 0x47 || head[1] != 0x47 || head[2] != 0x55 || head[3] != 0x46 {
            return "\(url.lastPathComponent) is not a valid GGUF file (bad magic header)."
        }
        // Version 3 little-endian
        let ver = UInt32(head[4])
            | (UInt32(head[5]) << 8)
            | (UInt32(head[6]) << 16)
            | (UInt32(head[7]) << 24)
        if ver != 3 {
            return "\(url.lastPathComponent) has unsupported GGUF version v\(ver) (expected v3)."
        }
        let archMarker = Data("inkling2_loop".utf8)
        if expectLora {
            let loraMarker = Data("lora".utf8)
            let adapterMarker = Data("adapter".utf8)
            if head.range(of: loraMarker) == nil && head.range(of: adapterMarker) == nil {
                return "\(url.lastPathComponent) does not appear to be a GGUF LoRA adapter."
            }
            return nil
        }
        if head.range(of: archMarker) == nil {
            return "\(url.lastPathComponent) is not an Inkling 2 Mini checkpoint (expected GGUF v3 architecture 'inkling2_loop')."
        }
        return nil
    }

    // ---- Live download & verification state ----

    private static var downloadProcess: Process?
    private static var downloadTargetEntry: LocalModelCatalogEntry?
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

    static var hasPartial: Bool { partialBytes(for: activeDownloadEntry) > 0 }
    static var downloadPct: Int {
        let tgt = activeDownloadEntry
        let expB = effectiveExpectedBytes(for: tgt)
        let installedTgt = isFileInstalled(for: tgt)
        guard expB > 0 else { return installedTgt ? 100 : 0 }
        let b = installedTgt ? expB : partialBytes(for: tgt)
        let pct = Int((Double(b) / Double(expB)) * 100.0)
        return installedTgt ? 100 : min(99, max(0, pct))
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
        let progressDenom: Int64 = expectedBytes > 0
            ? expectedBytes
            : ((try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value ?? 0)
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
                if progressDenom > 0, let onProgress {
                    let pct = min(100, max(0, Int((Double(total) / Double(progressDenom)) * 100.0)))
                    if pct != lastPct {
                        lastPct = pct
                        onProgress(pct)
                    }
                }
            }
            if done { break }
        }
        let digest = hasher.finalize().map { String(format: "%02x", $0) }.joined()
        if expectedBytes > 0, total != expectedBytes {
            return LocalModelVerificationResult(
                ok: false, bytes: total, sha256: digest,
                error: "Size mismatch: got \(total) bytes, expected \(expectedBytes) bytes."
            )
        }
        if total <= 0 {
            return LocalModelVerificationResult(
                ok: false, bytes: 0, sha256: digest,
                error: "File is empty."
            )
        }
        if !expectedSHA256.isEmpty, digest.lowercased() != expectedSHA256.lowercased() {
            return LocalModelVerificationResult(
                ok: false, bytes: total, sha256: digest,
                error: "SHA-256 mismatch: got \(digest.prefix(12))…, expected \(expectedSHA256.prefix(12))…."
            )
        }
        return LocalModelVerificationResult(ok: true, bytes: total, sha256: digest, error: nil)
    }

    @discardableResult
    static func writeVerifiedManifest(for item: LocalModelCatalogEntry, sha256: String) -> Bool {
        do {
            try FileManager.default.createDirectory(at: dir(for: item), withIntermediateDirectories: true)
            let m = LocalModelManifest(
                model_id: item.id,
                weights: item.fileName,
                bytes: weightsBytes(for: item),
                sha256: sha256.lowercased(),
                mtime: weightsMtime(for: item),
                verified: true,
                loops: 3,
                verified_at: Int64(Date().timeIntervalSince1970)
            )
            let enc = JSONEncoder()
            enc.outputFormatting = [.sortedKeys]
            var data = try enc.encode(m)
            data.append(0x0A)
            try data.write(to: manifestURL(for: item), options: .atomic)
            return true
        } catch {
            lastError = "Could not write verified manifest: \(error.localizedDescription)"
            return false
        }
    }

    @discardableResult
    static func writeVerifiedManifest(sha256: String) -> Bool {
        writeVerifiedManifest(for: entry, sha256: sha256)
    }

    /// Atomically verifies a completed .part file and promotes it to weightsURL only if bytes & SHA-256 (and GGUF header for unpinned) match.
    /// Removes corrupt .part files cleanly so corrupt checkpoints never become selectable.
    @discardableResult
    static func finalizeDownloadedPart(
        for item: LocalModelCatalogEntry? = nil,
        onProgress: ((Int) -> Void)? = nil
    ) -> LocalModelVerificationResult {
        let target = item ?? entry
        let part = partialURL(for: target)
        let dest = weightsURL(for: target)
        // For unpinned (gated/BYO) artifacts, validate GGUF v3 header + inkling2_loop before hashing.
        if target.expectedSHA256.isEmpty {
            if let hdrErr = validateGGUFHeader(at: part, expectLora: target.isLoraOverlay) {
                try? FileManager.default.removeItem(at: part)
                lastError = hdrErr + " Invalid download discarded."
                return LocalModelVerificationResult(ok: false, bytes: 0, sha256: "", error: hdrErr)
            }
        }
        let res = verifyFileSHA256(at: part, expectedSHA256: target.expectedSHA256, expectedBytes: target.expectedBytes, onProgress: onProgress)
        if !res.ok {
            try? FileManager.default.removeItem(at: part)
            lastError = (res.error ?? "SHA-256 verification failed.") + " Corrupt download discarded."
            return res
        }
        do {
            if FileManager.default.fileExists(atPath: dest.path) {
                try FileManager.default.removeItem(at: dest)
            }
            try FileManager.default.moveItem(at: part, to: dest)
            _ = writeVerifiedManifest(for: target, sha256: res.sha256)
            lastError = nil
            return res
        } catch {
            let msg = "Could not finalize verified weights: \(error.localizedDescription)"
            lastError = msg
            return LocalModelVerificationResult(ok: false, bytes: res.bytes, sha256: res.sha256, error: msg)
        }
    }

    /// Imports a user-supplied local `.gguf` into the target variant slot (APFS clone copy -> GGUF v3 check -> SHA-256 manifest).
    @discardableResult
    static func importLocalGGUFSync(
        from sourceURL: URL,
        for target: LocalModelCatalogEntry? = nil,
        onProgress: ((Int) -> Void)? = nil
    ) -> LocalModelVerificationResult {
        let item = target ?? entry
        if let hdrErr = validateGGUFHeader(at: sourceURL, expectLora: item.isLoraOverlay) {
            return LocalModelVerificationResult(ok: false, bytes: 0, sha256: "", error: hdrErr)
        }
        let srcSize = (try? FileManager.default.attributesOfItem(atPath: sourceURL.path)[.size] as? NSNumber)?.int64Value ?? 0
        if item.expectedBytes > 0 && srcSize != item.expectedBytes {
            return LocalModelVerificationResult(
                ok: false, bytes: srcSize, sha256: "",
                error: "Size mismatch for \(item.name): got \(formatBytes(srcSize)) (\(srcSize) B), expected \(formatBytes(item.expectedBytes)) (\(item.expectedBytes) B). Switch to 'Custom Local .gguf' to import arbitrary quants."
            )
        }
        let fm = FileManager.default
        let itemDir = dir(for: item)
        let dest = weightsURL(for: item)
        let staging = itemDir.appendingPathComponent(item.fileName + ".import")
        do {
            try fm.createDirectory(at: itemDir, withIntermediateDirectories: true)
            if fm.fileExists(atPath: staging.path) { try fm.removeItem(at: staging) }
            if sourceURL.standardizedFileURL != dest.standardizedFileURL {
                try fm.copyItem(at: sourceURL, to: staging)
            } else {
                // Already in place; verify directly without deleting.
                let res = verifyFileSHA256(at: dest, expectedSHA256: item.expectedSHA256, expectedBytes: item.expectedBytes, onProgress: onProgress)
                if res.ok {
                    _ = writeVerifiedManifest(for: item, sha256: res.sha256)
                }
                return res
            }
            let res = verifyFileSHA256(at: staging, expectedSHA256: item.expectedSHA256, expectedBytes: item.expectedBytes, onProgress: onProgress)
            if !res.ok {
                try? fm.removeItem(at: staging)
                return res
            }
            if fm.fileExists(atPath: dest.path) { try fm.removeItem(at: dest) }
            try fm.moveItem(at: staging, to: dest)
            if item.isCustom { customDisplayName = sourceURL.lastPathComponent }
            _ = writeVerifiedManifest(for: item, sha256: res.sha256)
            return res
        } catch {
            try? fm.removeItem(at: staging)
            return LocalModelVerificationResult(ok: false, bytes: 0, sha256: "", error: "Import failed: \(error.localizedDescription)")
        }
    }

    static func importLocalGGUF(
        from sourceURL: URL,
        for target: LocalModelCatalogEntry? = nil,
        onUpdate: @escaping () -> Void
    ) {
        let item = target ?? entry
        guard !isDownloading, !isVerifying else { return }
        isVerifying = true
        verifyPct = 0
        lastError = nil
        onUpdate()
        verifyQueue.async {
            let res = importLocalGGUFSync(from: sourceURL, for: item) { pct in
                DispatchQueue.main.async {
                    verifyPct = pct
                    onUpdate()
                }
            }
            DispatchQueue.main.async {
                isVerifying = false
                verifyPct = res.ok ? 100 : 0
                lastError = res.ok ? nil : (res.error ?? "Could not import \(sourceURL.lastPathComponent).")
                onUpdate()
            }
        }
    }

    /// Automatically runs one-time background SHA-256 verification when an unmanifested GGUF already exists on disk.
    static func verifyExistingIfNeeded(onUpdate: @escaping () -> Void) {
        guard hardwareSupported, !isDownloading, !isVerifying, hasUnverifiedWeights else { return }
        let target: LocalModelCatalogEntry
        if entry.isLoraOverlay, let base = baseEntry, hasUnverifiedWeights(for: base) {
            target = base
        } else {
            target = entry
        }
        let url = weightsURL(for: target)
        let key = "\(url.path)#\(weightsMtime(for: target))#\(weightsBytes(for: target))"
        if autoVerifyAttemptedPath == key { return }
        autoVerifyAttemptedPath = key
        verifyInstalledNow(for: target, onUpdate: onUpdate)
    }

    /// User-triggered or startup background streaming SHA-256 check on weightsURL.
    static func verifyInstalledNow(for explicitTarget: LocalModelCatalogEntry? = nil, onUpdate: @escaping () -> Void) {
        let target: LocalModelCatalogEntry
        if let explicitTarget {
            target = explicitTarget
        } else if entry.isLoraOverlay, let base = baseEntry, hasUnverifiedWeights(for: base) {
            target = base
        } else {
            target = entry
        }
        guard !isDownloading, !isVerifying, weightsBytes(for: target) > 0 else { return }
        isVerifying = true
        verifyPct = 0
        lastError = nil
        onUpdate()
        let url = weightsURL(for: target)
        let expHash = target.expectedSHA256
        let expBytes = target.expectedBytes
        verifyQueue.async {
            if expHash.isEmpty, let hdrErr = validateGGUFHeader(at: url, expectLora: target.isLoraOverlay) {
                DispatchQueue.main.async {
                    isVerifying = false
                    verifyPct = 0
                    try? FileManager.default.removeItem(at: manifestURL(for: target))
                    lastError = hdrErr
                    onUpdate()
                }
                return
            }
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
                    _ = writeVerifiedManifest(for: target, sha256: res.sha256)
                    lastError = nil
                } else {
                    try? FileManager.default.removeItem(at: manifestURL(for: target))
                    lastError = (res.error ?? "SHA-256 verification failed.") + " Delete and re-download to restore."
                }
                onUpdate()
                if res.ok && target.id != entry.id && hasUnverifiedWeights(for: entry) {
                    verifyInstalledNow(for: entry, onUpdate: onUpdate)
                }
            }
        }
    }

    // ---- Formatting helpers ----

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

    /// Canonical model id pushed into the composer model picker for the active local Inkling variant.
    static var pickerModelId: String { entry.id }

    /// Display name shown in the composer model chip and dropdown for the active local Inkling variant.
    static var pickerModelLabel: String {
        if entry.isCustom, let orig = customDisplayName, !orig.isEmpty, isFileInstalled {
            return "Inkling 2 Mini (\(orig))"
        }
        return entry.name
    }

    /// Concise subtitle shown under the active Inkling model row in the composer dropdown.
    static func pickerModelDesc(serverStarting: Bool = false, serverError: String? = nil) -> String {
        if isSelectable {
            if let err = serverError, !err.isEmpty {
                return "Server error · Click to retry"
            }
            if serverStarting {
                return "Loading model… · Psionic Metal"
            }
            if entry.isSafetensorsOverlay {
                return "Psionic Metal · IQ4_NL + Maple Attn · Local"
            }
            if entry.isLoraOverlay {
                return "Psionic Metal · IQ4_NL + LoRA · Local"
            }
            return "Psionic Metal · \(entry.quantization) · Local"
        }
        switch state {
        case .unsupportedHardware:
            return "Requires \(entry.minRamGB) GB RAM (\(physicalMemoryGB) GB detected)"
        case .verifying:
            return "Verifying SHA-256… (\(verifyPct)%)"
        case .downloading:
            let tgt = activeDownloadEntry
            let stepTag = (entry.isLoraOverlay && tgt.id != entry.id) ? "base " : ""
            let done = formatBytes(partialBytes(for: tgt))
            let tot = formatBytes(effectiveExpectedBytes(for: tgt))
            if bytesPerSec > 1024 {
                return "Downloading \(stepTag)\(downloadPct)% · \(done) / \(tot) · \(formatSpeed(bytesPerSec))"
            }
            return "Downloading \(stepTag)\(downloadPct)% · \(done) / \(tot)"
        case .paused:
            return "Paused \(downloadPct)% · Click to resume"
        case .installed:
            if entry.isSafetensorsOverlay {
                return "Psionic Metal · IQ4_NL + Maple Attn · Local"
            }
            return entry.isLoraOverlay
                ? "Psionic Metal · IQ4_NL + LoRA · Local"
                : "Psionic Metal · \(entry.quantization) · Local"
        case .notInstalled:
            if entry.isCustom {
                return "Not imported · Click to import .gguf"
            }
            if entry.requiresHFToken && effectiveHFToken == nil {
                return "HF token required · Click to configure"
            }
            if hasUnverifiedWeights {
                return "Unverified weights · Click to verify"
            }
            let need = remainingDownloadBytes > 0 ? remainingDownloadBytes : expectedBytes
            if need > 0 {
                return "Not downloaded (\(formatBytes(need))) · Click to download"
            }
            return "Not downloaded · Click to download"
        }
    }

    /// Concise live status shown in Settings > Models and in the composer model picker footer.
    static func statusSummary() -> String {
        let prefix = entry.id == LocalModelCatalogEntry.inkling2Mini.id ? "" : "\(entry.shortLabel) · "
        switch state {
        case .unsupportedHardware:
            return "\(prefix)Requires \(entry.minRamGB) GB RAM (\(physicalMemoryGB) GB detected)"
        case .verifying:
            return "\(prefix)Verifying SHA-256… (\(verifyPct)%)"
        case .downloading:
            let tgt = activeDownloadEntry
            let stepTag = (entry.isLoraOverlay && tgt.id != entry.id) ? "base " : ""
            let done = formatBytes(partialBytes(for: tgt))
            let tot = formatBytes(effectiveExpectedBytes(for: tgt))
            if bytesPerSec > 1024 {
                return "\(prefix)Downloading \(stepTag)\(downloadPct)% · \(done) / \(tot) · \(formatSpeed(bytesPerSec)) · ETA \(formatETA(etaSeconds))"
            }
            return "\(prefix)Downloading \(stepTag)\(downloadPct)% · \(done) / \(tot)"
        case .paused:
            let tgt = activeDownloadEntry
            return "\(prefix)Paused \(downloadPct)% · \(formatBytes(partialBytes(for: tgt))) / \(formatBytes(effectiveExpectedBytes(for: tgt))) · Resume…"
        case .installed:
            let installedSize = entry.isLoraOverlay
                ? (weightsBytes(for: baseEntry ?? .inkling2Mini) + weightsBytes)
                : weightsBytes
            return "\(prefix)Installed (\(formatBytes(installedSize)) · SHA-256 verified)"
        case .notInstalled:
            if entry.isCustom {
                return "\(prefix)Import a local Inkling 2 Mini .gguf…"
            }
            if isFileInstalled && needsBaseModel {
                return "\(prefix)LoRA installed · Needs Official IQ4_NL base (\(formatBytes(remainingDownloadBytes)))"
            }
            if hasUnverifiedWeights {
                return "\(prefix)Unverified weights on disk · Verify SHA-256"
            }
            let need = remainingDownloadBytes > 0 ? remainingDownloadBytes : expectedBytes
            return "\(prefix)Not downloaded (\(formatBytes(need))) · Download…"
        }
    }

    /// Compact one-line label for the Variant Switcher menu.
    static func variantMenuLabel(for item: LocalModelCatalogEntry) -> String {
        let sizeStr: String
        if item.isCustom {
            let wb = weightsBytes(for: item)
            sizeStr = wb > 0 ? formatBytes(wb) : "BYO .gguf"
        } else {
            sizeStr = formatBytes(item.expectedBytes)
        }
        var tags: [String] = [sizeStr, "\(item.minRamGB) GB+"]
        if item.id == defaultVariantId() { tags.append("Default") }
        if item.requiresHFToken { tags.append("HF Gated") }
        if isInstalled(for: item) {
            tags.append("Ready")
        } else if item.isLoraOverlay && isFileInstalled(for: item) {
            tags.append("Needs Base")
        } else if partialBytes(for: item) > 0 {
            tags.append("Paused")
        } else if !hardwareSupported(for: item) {
            tags.append("Needs \(item.minRamGB) GB")
        }
        return "\(item.name)  (\(tags.joined(separator: " · ")))"
    }

    /// Detailed catalog, hardware compatibility, live download progress, and storage inspection body for the Local Models UI.
    static func dialogBody() -> String {
        let ramGB = physicalMemoryGB
        let freeBytes = availableDiskBytes()
        let ramMark = hardwareSupported ? "✓" : "✗"
        let diskOK = isInstalled || entry.isCustom || hasEnoughDiskForDownload
        let diskMark = diskOK ? "✓" : "✗"
        let ctxK = recommendedContextTokens / 1024

        let shaStr: String
        if !expectedSHA256.isEmpty {
            shaStr = "\(expectedSHA256.prefix(12))…\(expectedSHA256.suffix(8))"
        } else if let m = readManifest(), isValidSHA256Hex(m.sha256) {
            shaStr = "\(m.sha256.prefix(12))…\(m.sha256.suffix(8)) (recorded)"
        } else {
            shaStr = "verified on download/import"
        }

        let sizeStr = expectedBytes > 0
            ? formatBytes(expectedBytes)
            : (weightsBytes > 0 ? formatBytes(weightsBytes) : "local .gguf")

        var lines: [String] = []
        lines.append("Active Variant: \(entry.name)  [\(entry.sourceLabel)]")
        lines.append("Model: \(entry.architecture)")
        lines.append("Format: \(entry.quantization) · Size: \(sizeStr) · SHA-256: \(shaStr)")
        if entry.isCustom, let orig = customDisplayName, !orig.isEmpty, isFileInstalled {
            lines.append("Imported file: \(orig)")
        }
        lines.append("Host: \(ramGB) GB unified memory (\(ramMark) >= \(entry.minRamGB) GB) · Context: \(ctxK)K · Free disk: \(formatBytes(freeBytes)) (\(diskMark))")
        lines.append("Cache: \(inklingDir.path) · Variant: \(formatBytes(diskUsageBytes)) · All local models: \(formatBytes(totalCacheDiskUsageBytes))")

        if entry.requiresHFToken {
            if let tok = effectiveHFToken, let src = hfTokenSource {
                lines.append("Hugging Face auth: Token configured (\(hfTokenTail(tok)) via \(src))")
            } else {
                lines.append("Hugging Face auth: Not set (gated repo — click 'HF Token…' after accepting repo terms, or 'Import Local .gguf…')")
            }
        }

        switch state {
        case .unsupportedHardware:
            lines.append("")
            lines.append("Unavailable on this Mac: \(entry.name) requires at least \(entry.minRamGB) GB of Apple Silicon unified memory. Click 'Switch Variant…' to pick a smaller quant (down to 12 GB).")
        case .installed:
            lines.append("")
            let modeNote: String
            if entry.isSafetensorsOverlay {
                modeNote = "Base IQ4_NL + \(formatBytes(entry.expectedBytes)) attention overlay active"
            } else if entry.isLoraOverlay {
                modeNote = "Base IQ4_NL + \(formatBytes(entry.expectedBytes)) LoRA overlay active"
            } else {
                modeNote = "Psionic Metal 4, \(ctxK)K context"
            }
            lines.append("Status: Installed & SHA-256 verified · Ready in composer model picker (\(modeNote)).")
        case .verifying:
            lines.append("")
            lines.append("Status: Verifying GGUF & SHA-256 checksum on disk…")
            lines.append(progressBar(pct: verifyPct))
        case .downloading:
            let tgt = activeDownloadEntry
            let stepNote = (entry.isLoraOverlay && tgt.id != entry.id)
                ? "Downloading Official IQ4_NL base (LoRA downloads next)"
                : "Downloading from Hugging Face (resumable)"
            lines.append("")
            lines.append("Status: \(stepNote)")
            lines.append("\(progressBar(pct: downloadPct))  \(formatBytes(partialBytes(for: tgt))) / \(formatBytes(effectiveExpectedBytes(for: tgt)))")
            lines.append("Speed: \(formatSpeed(bytesPerSec)) · ETA: \(formatETA(etaSeconds))")
        case .paused:
            let tgt = activeDownloadEntry
            lines.append("")
            lines.append("Status: Download paused (partial file kept for resume)")
            lines.append("\(progressBar(pct: downloadPct))  \(formatBytes(partialBytes(for: tgt))) / \(formatBytes(effectiveExpectedBytes(for: tgt)))")
        case .notInstalled:
            lines.append("")
            if entry.isCustom {
                lines.append("Status: No custom .gguf imported yet. Click 'Import Local .gguf…' to load any Inkling 2 Mini GGUF checkpoint.")
            } else if entry.isLoraOverlay && isFileInstalled && needsBaseModel {
                lines.append("Status: \(entry.shortLabel) overlay is verified on disk, but the Official IQ4_NL base (17.63 GB) is not installed yet. Click Download Base to install it while keeping \(entry.shortLabel) active.")
            } else if entry.isLoraOverlay && needsBaseModel {
                lines.append("Status: Ready to download Official IQ4_NL base (17.63 GB) + \(formatBytes(entry.expectedBytes)) \(entry.shortLabel) overlay (resumable, SHA-256 verified).")
            } else if hasUnverifiedWeights {
                lines.append("Status: Checkpoint found on disk without a verified SHA-256 manifest. Click Verify SHA-256 to authenticate it.")
            } else if !hasEnoughDiskForDownload {
                lines.append("Status: Insufficient free disk space — need \(formatBytes(requiredFreeDiskBytes)) free (\(formatBytes(freeBytes)) available).")
            } else if entry.requiresHFToken && effectiveHFToken == nil {
                lines.append("Status: Gated Hugging Face checkpoint (\(formatBytes(expectedBytes))). Set a Hugging Face token (after accepting the repo terms) or click 'Import Local .gguf…'.")
            } else {
                lines.append("Status: Ready to download (\(formatBytes(expectedBytes)), resumable, SHA-256 verified on completion).")
            }
        }

        // Compact overview of installed variants so users immediately see what's ready on disk.
        if catalogOverride == nil {
            let installedNames = catalog.filter { isInstalled(for: $0) }.map(\.shortLabel)
            if !installedNames.isEmpty {
                lines.append("Installed on disk: \(installedNames.joined(separator: ", "))")
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
        let target = downloadTargetEntry ?? entry
        let now = Date().timeIntervalSinceReferenceDate
        let cur = partialBytes(for: target)
        if lastSampleTime > 0 {
            let dt = now - lastSampleTime
            if dt >= 0.25 {
                let delta = Double(max(0, cur - lastSampleBytes))
                let inst = delta / dt
                bytesPerSec = bytesPerSec > 0 ? (bytesPerSec * 0.65 + inst * 0.35) : inst
                if bytesPerSec > 1024 && target.expectedBytes > 0 {
                    etaSeconds = Double(max(0, target.expectedBytes - cur)) / bytesPerSec
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
    /// For a LoRA overlay whose base IQ4_NL checkpoint is not yet installed, downloads and verifies the base first
    /// while keeping the LoRA variant active, then automatically chains downloading the 43.8 MB LoRA adapter.
    /// For gated Hugging Face repos, passes Authorization header exclusively via `curl -K -` stdin (never argv).
    static func startDownload(onUpdate: @escaping () -> Void) {
        guard !isDownloading, !isVerifying else { return }
        let active = entry
        let target = nextDownloadTarget
        guard !target.isCustom, !target.downloadURL.isEmpty else {
            lastError = "Use 'Import Local .gguf…' to load a custom local checkpoint."
            onUpdate()
            return
        }
        guard hardwareSupported(for: active) && hardwareSupported(for: target) else {
            lastError = "\(active.name) requires at least \(active.minRamGB) GB unified memory (\(physicalMemoryGB) GB detected)."
            onUpdate()
            return
        }
        guard hasEnoughDiskForDownload else {
            lastError = "Insufficient disk space: need \(formatBytes(requiredFreeDiskBytes)) free, only \(formatBytes(availableDiskBytes())) available."
            onUpdate()
            return
        }
        let hfToken: String? = target.requiresHFToken ? effectiveHFToken : nil
        if target.requiresHFToken && hfToken == nil {
            lastError = "This checkpoint is gated on Hugging Face. Click 'HF Token…' to set a read token (after accepting the repo terms), or click 'Import Local .gguf…'."
            onUpdate()
            return
        }
        lastError = nil
        pauseRequested = false
        cancelRequested = false
        resetSpeed()
        let targetDir = dir(for: target)
        let targetPart = partialURL(for: target)
        do {
            try FileManager.default.createDirectory(at: targetDir, withIntermediateDirectories: true)
        } catch {
            lastError = "Could not create \(targetDir.path): \(error.localizedDescription)"
            onUpdate()
            return
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
        var args: [String]
        var rangeOutHandle: FileHandle?
        if let rangeEnd = target.httpRangeEnd {
            var cur = partialBytes(for: target)
            if cur > target.expectedBytes {
                try? FileManager.default.removeItem(at: targetPart)
                cur = 0
            }
            if cur == target.expectedBytes && target.expectedBytes > 0 {
                isVerifying = true
                verifyPct = 0
                onUpdate()
                verifyQueue.async {
                    let res = finalizeDownloadedPart(for: target) { pct in
                        DispatchQueue.main.async {
                            verifyPct = pct
                            onUpdate()
                        }
                    }
                    DispatchQueue.main.async {
                        isVerifying = false
                        verifyPct = res.ok ? 100 : 0
                        onUpdate()
                        if res.ok && target.id != entry.id && !isFileInstalled(for: entry) {
                            startDownload(onUpdate: onUpdate)
                        }
                    }
                }
                return
            }
            if !FileManager.default.fileExists(atPath: targetPart.path) {
                FileManager.default.createFile(atPath: targetPart.path, contents: nil)
            }
            do {
                let fh = try FileHandle(forWritingTo: targetPart)
                fh.seekToEndOfFile()
                rangeOutHandle = fh
            } catch {
                lastError = "Could not open \(targetPart.lastPathComponent) for append: \(error.localizedDescription)"
                onUpdate()
                return
            }
            args = ["-fsSL", "--retry", "3", "--retry-delay", "2", "-r", "\(cur)-\(rangeEnd)"]
        } else {
            args = ["-fsSL", "-C", "-", "--retry", "3", "--retry-delay", "2"]
        }
        var stdinPipe: Pipe?
        if hfToken != nil {
            args.append(contentsOf: ["-K", "-"])
            stdinPipe = Pipe()
            proc.standardInput = stdinPipe
        }
        if let outHandle = rangeOutHandle {
            args.append(target.downloadURL)
            proc.standardOutput = outHandle
        } else {
            args.append(contentsOf: ["-o", targetPart.path, target.downloadURL])
            proc.standardOutput = FileHandle.nullDevice
        }
        proc.arguments = args
        let errPipe = Pipe()
        proc.standardError = errPipe
        proc.terminationHandler = { p in
            let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
            let errText = String(decoding: errData, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            DispatchQueue.main.async {
                downloadProcess = nil
                downloadTargetEntry = nil
                stopProgressTimer()
                resetSpeed()
                if cancelRequested {
                    cancelRequested = false
                    try? FileManager.default.removeItem(at: targetPart)
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
                        let res = finalizeDownloadedPart(for: target) { pct in
                            DispatchQueue.main.async {
                                verifyPct = pct
                                onUpdate()
                            }
                        }
                        DispatchQueue.main.async {
                            isVerifying = false
                            verifyPct = res.ok ? 100 : 0
                            onUpdate()
                            // If we just installed the base IQ4_NL for an active LoRA overlay, chain the LoRA adapter automatically.
                            if res.ok && target.id != entry.id && !isFileInstalled(for: entry) {
                                startDownload(onUpdate: onUpdate)
                            }
                        }
                    }
                } else {
                    if target.requiresHFToken && (errText.contains("401") || errText.contains("403")) {
                        let repo = target.hfRepoURL ?? "Hugging Face"
                        lastError = "Hugging Face returned 401/403 (gated repo). Accept access terms at \(repo) and verify your HF Token, or use 'Import Local .gguf…'."
                    } else {
                        let detail = errText.isEmpty ? "" : " (\(errText.prefix(120)))"
                        lastError = "Download interrupted (curl exit \(p.terminationStatus))\(detail). Click Resume to continue from \(formatBytes(partialBytes(for: target)))."
                    }
                    onUpdate()
                }
            }
        }
        do {
            try proc.run()
            if let outHandle = rangeOutHandle { try? outHandle.close() }
            if let token = hfToken, let pipe = stdinPipe {
                let cfg = "header = \"Authorization: Bearer \(token)\"\n"
                if let data = cfg.data(using: .utf8) {
                    pipe.fileHandleForWriting.write(data)
                }
                try? pipe.fileHandleForWriting.close()
            }
            downloadProcess = proc
            downloadTargetEntry = target
            lastSampleBytes = partialBytes(for: target)
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
            if let outHandle = rangeOutHandle { try? outHandle.close() }
            if let pipe = stdinPipe { try? pipe.fileHandleForWriting.close() }
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
        let target = activeDownloadEntry
        cancelRequested = true
        pauseRequested = false
        stopProgressTimer()
        resetSpeed()
        if let proc = downloadProcess, proc.isRunning {
            proc.terminate()
        } else {
            downloadProcess = nil
            downloadTargetEntry = nil
            cancelRequested = false
            try? FileManager.default.removeItem(at: partialURL(for: target))
        }
        lastError = nil
        onUpdate?()
    }

    /// Removes installed/partial files for the active variant and invokes stopServer before deletion.
    static func removeInkling(stopServer: () -> Void = {}) {
        cancelDownload()
        stopServer()
        autoVerifyAttemptedPath = nil
        if entry.isCustom { customDisplayName = nil }
        try? FileManager.default.removeItem(at: inklingDir)
        lastError = nil
    }
}
