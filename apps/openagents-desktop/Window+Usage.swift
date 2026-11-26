// Firm reason: 6 (AppKit host bridge for Settings -> Usage, local quota cache/CLI probes,
// local chat history aggregation, price overrides persistence, and limit-recovery timer; #13877).
import AppKit

struct UsagePriceOverride: Codable, Equatable {
    var model: String
    var inUsdPerM: Double
    var outUsdPerM: Double
    var cachedUsdPerM: Double
}

struct UsageWindowSnapshot {
    var label: String
    var usedBp: Int32
    var resetS: Int64
}

struct UsageAccountSnapshot {
    var provider: Int32      // 0=Claude, 1=Codex, 2=Antigravity
    var profile: String
    var plan: String
    var status: Int32        // 0=unavailable, 1=observed, 2=stale, 3=signed_out
    var why: String
    var gotMs: Int64
    var limited: Bool
    var limitResetS: Int64
    var windows: [UsageWindowSnapshot]
}

struct UsageChatRecord {
    var path: String
    var model: String
    var inTokens: Int64
    var cachedTokens: Int64
    var outTokens: Int64
    var costMicros: Int64
}

struct UsageCloudSnapshot {
    var synced: Bool
    var creditMsat: Int64
    var todayMicros: Int64
    var monthMicros: Int64
    var totalMicros: Int64
    var decisionMicros: Int64
    var decisionCalls: Int64
    var inTokens: Int64
    var cachedTokens: Int64
    var outTokens: Int64
    var calls: Int64
}

enum UsagePrefs {
    private static let pricesKey = "usagePriceOverrides"
    private static let poolKey = "usagePoolEnabled"
    private static let recoveryKey = "usageRecoveryEnabled"

    static var poolEnabled: Bool {
        get {
            if UserDefaults.standard.object(forKey: poolKey) == nil { return true }
            return UserDefaults.standard.bool(forKey: poolKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: poolKey) }
    }

    static var recoveryEnabled: Bool {
        get {
            if UserDefaults.standard.object(forKey: recoveryKey) == nil { return true }
            return UserDefaults.standard.bool(forKey: recoveryKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: recoveryKey) }
    }

    static func loadPriceOverrides() -> [UsagePriceOverride] {
        guard let data = UserDefaults.standard.data(forKey: pricesKey),
              let list = try? JSONDecoder().decode([UsagePriceOverride].self, from: data) else {
            return []
        }
        return list
    }

    static func savePriceOverrides(_ list: [UsagePriceOverride]) {
        if let data = try? JSONEncoder().encode(list) {
            UserDefaults.standard.set(data, forKey: pricesKey)
        }
    }

    /// Formats overrides as one `model=in,out,cached` line per entry (USD per 1M tokens).
    static func formatOverrides(_ list: [UsagePriceOverride]) -> String {
        list.map { o in
            String(format: "%@=%.2f,%.2f,%.2f", o.model, o.inUsdPerM, o.outUsdPerM, o.cachedUsdPerM)
        }.joined(separator: "\n")
    }

    /// Parses lines of `model=in,out[,cached]` (USD per 1M tokens). Ignores blank/# comment lines.
    static func parseOverrides(_ text: String) -> [UsagePriceOverride] {
        var out: [UsagePriceOverride] = []
        for rawLine in text.components(separatedBy: .newlines) {
            let line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty || line.hasPrefix("#") { continue }
            let parts = line.split(separator: "=", maxSplits: 1).map { String($0).trimmingCharacters(in: .whitespaces) }
            guard parts.count == 2, !parts[0].isEmpty else { continue }
            let nums = parts[1].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            guard nums.count >= 2, nums[0] >= 0, nums[1] >= 0 else { continue }
            let cached = nums.count >= 3 && nums[2] >= 0 ? nums[2] : (nums[0] * 0.1)
            out.append(UsagePriceOverride(model: parts[0], inUsdPerM: nums[0], outUsdPerM: nums[1], cachedUsdPerM: cached))
        }
        return out
    }
}

enum UsageProber {
    private static let staleMs: Int64 = 900_000

    private static var homeURL: URL { FileManager.default.homeDirectoryForCurrentUser }

    /// Reads cached usage files written by `coder/tui/usage.coax` (`~/.openagents/coder/usage-*.txt`)
    /// without ever reading, logging, or exposing OAuth/API tokens.
    static func probeAll() -> [UsageAccountSnapshot] {
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000.0)
        let coderDir = homeURL.appendingPathComponent(".openagents/coder", isDirectory: true)
        var results: [UsageAccountSnapshot] = []

        // 1. Default accounts for Claude (0), Codex (1), Antigravity (2)
        results.append(readProviderCache(provider: 0, profile: "Default", cacheURL: coderDir.appendingPathComponent("usage-claude.txt"), nowMs: nowMs))
        results.append(readProviderCache(provider: 1, profile: "Default", cacheURL: coderDir.appendingPathComponent("usage-codex.txt"), nowMs: nowMs))
        results.append(readProviderCache(provider: 2, profile: "Default", cacheURL: coderDir.appendingPathComponent("usage-antigravity.txt"), nowMs: nowMs))

        // 2. Additional multi-account profile caches: usage-claude-<profile>.txt, usage-codex-<profile>.txt, usage-antigravity-<profile>.txt
        if let files = try? FileManager.default.contentsOfDirectory(atPath: coderDir.path) {
            for name in files.sorted() where name.hasPrefix("usage-") && name.hasSuffix(".txt") {
                if name == "usage-claude.txt" || name == "usage-codex.txt" || name == "usage-antigravity.txt" { continue }
                let stem = String(name.dropFirst("usage-".count).dropLast(".txt".count))
                if let dash = stem.firstIndex(of: "-") {
                    let provName = String(stem[..<dash])
                    let prof = String(stem[stem.index(after: dash)...])
                    guard !prof.isEmpty else { continue }
                    let prov: Int32
                    switch provName {
                    case "claude": prov = 0
                    case "codex": prov = 1
                    case "antigravity", "agy": prov = 2
                    default: continue
                    }
                    results.append(readProviderCache(provider: prov, profile: prof, cacheURL: coderDir.appendingPathComponent(name), nowMs: nowMs))
                }
            }
        }
        return results
    }

    private static func readProviderCache(provider: Int32, profile: String, cacheURL: URL, nowMs: Int64) -> UsageAccountSnapshot {
        if let text = try? String(contentsOf: cacheURL, encoding: .utf8), !text.isEmpty {
            let lines = text.components(separatedBy: .newlines)
            let gotMs = Int64(lines.first?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "") ?? 0
            let plan = lines.count > 1 ? lines[1].trimmingCharacters(in: .whitespacesAndNewlines) : ""
            var wins: [UsageWindowSnapshot] = []
            var limited = false
            var limitReset: Int64 = 0
            if lines.count > 2 {
                for raw in lines[2...] {
                    let parts = raw.split(separator: " ")
                    guard parts.count >= 3, let bp = Int32(parts[1]), let rs = Int64(parts[2]) else { continue }
                    let clampedBp = max(0, min(10000, bp))
                    wins.append(UsageWindowSnapshot(label: String(parts[0]), usedBp: clampedBp, resetS: rs))
                    if clampedBp >= 10000 {
                        limited = true
                        if rs > 0 && (limitReset == 0 || rs < limitReset) { limitReset = rs }
                    }
                }
            }
            if !wins.isEmpty {
                let isStale = gotMs > 0 && nowMs > gotMs && (nowMs - gotMs) > staleMs
                return UsageAccountSnapshot(
                    provider: provider,
                    profile: profile,
                    plan: plan,
                    status: isStale ? 2 : 1,
                    why: "",
                    gotMs: gotMs,
                    limited: limited,
                    limitResetS: limitReset,
                    windows: wins
                )
            }
        }

        // No cached windows: report explicit signed_out vs unavailable without reading any token bytes.
        let fm = FileManager.default
        switch provider {
        case 0: // Claude
            let claudeCreds = homeURL.appendingPathComponent(".claude/.credentials.json").path
            let hasFile = fm.fileExists(atPath: claudeCreds)
            let hasClaudeBin = CoderWorker.hasClaude
            if !hasFile && !hasClaudeBin {
                return UsageAccountSnapshot(provider: 0, profile: profile, plan: "", status: 3, why: "Claude Code isn't signed in here", gotMs: 0, limited: false, limitResetS: 0, windows: [])
            }
            return UsageAccountSnapshot(provider: 0, profile: profile, plan: "", status: 0, why: "Run /usage in coder CLI or click Refresh to probe Claude quota", gotMs: 0, limited: false, limitResetS: 0, windows: [])
        case 1: // Codex
            let codexAuth = homeURL.appendingPathComponent(".codex/auth.json").path
            if !fm.fileExists(atPath: codexAuth) {
                return UsageAccountSnapshot(provider: 1, profile: profile, plan: "", status: 3, why: "Codex isn't signed in here", gotMs: 0, limited: false, limitResetS: 0, windows: [])
            }
            return UsageAccountSnapshot(provider: 1, profile: profile, plan: "", status: 0, why: "No cached Codex quota window yet", gotMs: 0, limited: false, limitResetS: 0, windows: [])
        default: // Antigravity
            let agyTok = homeURL.appendingPathComponent(".gemini/antigravity-acp/antigravity-oauth-token").path
            let agyLegacy = homeURL.appendingPathComponent(".gemini/antigravity/antigravity-oauth-token").path
            if !fm.fileExists(atPath: agyTok) && !fm.fileExists(atPath: agyLegacy) {
                return UsageAccountSnapshot(provider: 2, profile: profile, plan: "", status: 3, why: "Antigravity isn't signed in here", gotMs: 0, limited: false, limitResetS: 0, windows: [])
            }
            return UsageAccountSnapshot(provider: 2, profile: profile, plan: "Antigravity", status: 0, why: "No cached Antigravity quota window yet", gotMs: 0, limited: false, limitResetS: 0, windows: [])
        }
    }

    /// Fetches authoritative OpenAgents Cloud spend & credit from `GET /api/v1/inference/usage` (#14072).
    static func fetchCloudUsage() -> UsageCloudSnapshot {
        let empty = UsageCloudSnapshot(synced: false, creditMsat: 0, todayMicros: 0, monthMicros: 0, totalMicros: 0, decisionMicros: 0, decisionCalls: 0, inTokens: 0, cachedTokens: 0, outTokens: 0, calls: 0)
        let origin = MacAccount.origin
        var authHeader: String? = nil
        if let tok = MacAccount.shared.store.load()?.access, !tok.isEmpty {
            authHeader = "Bearer \(tok)"
        } else if let signed = try? MacKey.signer(MacAccount.shared.keyFile)(origin + "/api/v1/inference/usage", "", Date()), !signed.isEmpty {
            authHeader = signed
        }
        guard let authHeader, let url = URL(string: origin + "/api/v1/inference/usage") else { return empty }
        var req = URLRequest(url: url, timeoutInterval: 6.0)
        req.httpMethod = "GET"
        req.setValue(authHeader, forHTTPHeaderField: "Authorization")
        let sem = DispatchSemaphore(value: 0)
        var result = empty
        URLSession.shared.dataTask(with: req) { data, resp, _ in
            defer { sem.signal() }
            guard let http = resp as? HTTPURLResponse, http.statusCode == 200,
                  let data,
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
            let creditMsat = (obj["credit_msat"] as? NSNumber)?.int64Value ?? 0
            let today = obj["today"] as? [String: Any] ?? [:]
            let total = obj["total"] as? [String: Any] ?? [:]
            let decisions = obj["decisions"] as? [String: Any] ?? [:]
            let todayMicros = (today["usd_micros"] as? NSNumber)?.int64Value ?? 0
            let totalMicros = (total["usd_micros"] as? NSNumber)?.int64Value ?? 0
            let decisionMicros = (decisions["usd_micros"] as? NSNumber)?.int64Value ?? 0
            let decisionCalls = (decisions["calls"] as? NSNumber)?.int64Value ?? 0
            let inTok = (total["in_tokens"] as? NSNumber)?.int64Value ?? 0
            let cachedTok = (total["cached_in_tokens"] as? NSNumber)?.int64Value ?? 0
            let outTok = (total["out_tokens"] as? NSNumber)?.int64Value ?? 0
            let calls = (total["calls"] as? NSNumber)?.int64Value ?? 0
            result = UsageCloudSnapshot(
                synced: true,
                creditMsat: creditMsat,
                todayMicros: todayMicros,
                monthMicros: totalMicros,
                totalMicros: totalMicros,
                decisionMicros: decisionMicros,
                decisionCalls: decisionCalls,
                inTokens: inTok,
                cachedTokens: cachedTok,
                outTokens: outTok,
                calls: calls
            )
        }.resume()
        _ = sem.wait(timeout: .now() + 7.0)
        return result
    }

    /// Scans local stored chats from `CoderStore`, skipping any Antigravity index DBs (`cd_usage_is_index_db`).
    static func scanLocalChats(store: CoderStore) -> [UsageChatRecord] {
        var out: [UsageChatRecord] = []
        let chats = store.list
        for chat in chats {
            let filename = chat.id + ".json"
            if cd_usage_is_index_db(filename) != 0 { continue }
            guard let text = store.text(chat.id),
                  let data = text.data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
            let model = (obj["model"] as? String) ?? "gemini-4-neon"
            let costMicros = (obj["cost_usd_micros"] as? NSNumber)?.int64Value ?? 0
            let inTok = (obj["ctx_in"] as? NSNumber)?.int64Value ?? 0
            let cachedTok = (obj["ctx_cached"] as? NSNumber)?.int64Value ?? 0
            let outTok = (obj["ctx_out"] as? NSNumber)?.int64Value ?? 0
            if costMicros > 0 || inTok > 0 || outTok > 0 {
                out.append(UsageChatRecord(path: filename, model: model, inTokens: inTok, cachedTokens: cachedTok, outTokens: outTok, costMicros: costMicros))
            }
        }
        return out
    }
}

extension CoderWindow {
    /// Applies persisted Usage preferences (price overrides, pooling, recovery) and kicks off an async refresh.
    func setupUsageDashboard() {
        cd_usage_pool_set(page, UsagePrefs.poolEnabled ? 1 : 0)
        cd_usage_recovery_set(page, UsagePrefs.recoveryEnabled ? 1 : 0)
        applyUsagePriceOverrides()
        refreshUsageDashboard()
        usageTimer?.invalidate()
        usageTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { [weak self] _ in
            self?.tickUsageRecovery()
        }
    }

    func applyUsagePriceOverrides() {
        cd_usage_price_clear(page)
        for o in UsagePrefs.loadPriceOverrides() {
            let inM = Int64((o.inUsdPerM * 1_000_000.0).rounded())
            let outM = Int64((o.outUsdPerM * 1_000_000.0).rounded())
            let cachedM = Int64((o.cachedUsdPerM * 1_000_000.0).rounded())
            _ = cd_usage_price_set(page, o.model, inM, outM, cachedM)
        }
    }

    /// Syncs authoritative OpenAgents Cloud usage/credit, aggregates local chats, and (only when
    /// opted in via AgentSettings.allowLocalHarnesses) probes external local harness quota caches (#14072).
    func refreshUsageDashboard() {
        cd_usage_refreshing_set(page, 1)
        wake()
        let storeRef = self.store
        let probeExternalHarnesses = AgentSettings.allowLocalHarnesses
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let cloud = UsageProber.fetchCloudUsage()
            let accounts = probeExternalHarnesses ? UsageProber.probeAll() : []
            let records = UsageProber.scanLocalChats(store: storeRef)
            let nowS = Int64(Date().timeIntervalSince1970)
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.applyUsagePriceOverrides()
                cd_usage_cloud_set(
                    self.page,
                    cloud.creditMsat,
                    cloud.todayMicros,
                    cloud.monthMicros,
                    cloud.totalMicros,
                    cloud.decisionMicros,
                    cloud.decisionCalls,
                    cloud.inTokens,
                    cloud.cachedTokens,
                    cloud.outTokens,
                    cloud.calls,
                    cloud.synced ? 1 : 0
                )
                cd_usage_clear(self.page)
                for a in accounts {
                    let idx = cd_usage_set_account(self.page, a.provider, a.profile, a.plan, a.status, a.why, a.gotMs, a.limited ? 1 : 0, a.limitResetS)
                    if idx >= 0 {
                        for w in a.windows {
                            _ = cd_usage_add_window(self.page, idx, w.label, w.usedBp, w.resetS)
                        }
                    }
                }
                cd_usage_spend_reset(self.page)
                for r in records {
                    _ = cd_usage_spend_add(self.page, r.path, -1, r.model, r.inTokens, r.cachedTokens, r.outTokens, r.costMicros)
                }
                _ = cd_usage_tick(self.page, nowS)
                cd_usage_refreshing_set(self.page, 0)
                self.wake()
            }
        }
    }

    func tickUsageRecovery() {
        guard !retired else { return }
        let nowS = Int64(Date().timeIntervalSince1970)
        if cd_usage_tick(page, nowS) != 0 {
            handleUsageRecoveryEvent()
        }
    }

    func handleUsagePoolChanged() {
        UsagePrefs.poolEnabled = cd_usage_pool_enabled(page) != 0
        wake()
    }

    func handleUsageRecoveryAction() {
        UsagePrefs.recoveryEnabled = cd_usage_recovery_enabled(page) != 0
        handleUsageRecoveryEvent()
        wake()
    }

    private func handleUsageRecoveryEvent() {
        var buf = [CChar](repeating: 0, count: 256)
        let n = cd_usage_recovery_msg(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let msg = String(cString: buf)
        cd_toast(page, msg, 0)
        wake()
    }

    /// Settings -> Usage -> Edit prices… dialog (`CP_ASK_USAGE_PRICES`).
    func editUsagePriceOverrides() {
        let current = UsagePrefs.formatOverrides(UsagePrefs.loadPriceOverrides())
        var d = CoderDialog(
            size: .md,
            title: "Model Price Overrides",
            body: "Enter one override per line as `model=input,output,cached` in USD per 1M tokens (for example: `gemini-4-neon=2.00,8.00,0.50`). Leave empty to use recorded local costs."
        )
        d.fields = [.init(label: "Overrides (USD / 1M tokens)", value: current, hint: "gemini-4-neon=2.00,8.00,0.50", lines: 6)]
        d.buttons = [.init(title: "Clear All", kind: .danger, aside: true), .init(title: "Cancel"), .init(title: "Save", kind: .primary)]
        d.cancel = 1
        d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 0 {
                UsagePrefs.savePriceOverrides([])
                self.refreshUsageDashboard()
            } else if answer == 2 {
                let parsed = UsagePrefs.parseOverrides(values.first ?? "")
                UsagePrefs.savePriceOverrides(parsed)
                self.refreshUsageDashboard()
            }
            self.refocus()
        }
    }
}
