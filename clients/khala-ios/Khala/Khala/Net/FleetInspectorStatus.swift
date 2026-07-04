import Foundation

struct FleetInspectorStatus: Equatable {
    enum Availability: String, Equatable {
        case available
        case stale
        case blocked
        case unknown

        var label: String {
            switch self {
            case .available: return "Available"
            case .stale: return "Stale"
            case .blocked: return "Blocked"
            case .unknown: return "Unknown"
            }
        }
    }

    struct ProviderAccount: Equatable, Identifiable {
        var id: String { provider + ":" + ref }
        let provider: String
        let ref: String
        let readiness: Availability
        let detail: String?
    }

    struct AppleFM: Equatable {
        let readiness: Availability
        let summary: String
        let blockerRefs: [String]
    }

    struct RecentRef: Equatable, Identifiable {
        var id: String { kind + ":" + value }
        let kind: String
        let value: String
    }

    let fetchedAt: Date
    let connectedIdentity: String?
    let localAgentIdentity: String?
    let pylonRef: String?
    let pylonReadiness: Availability
    let heartbeatObservedAt: String?
    let heartbeatFresh: Bool?
    let providerAccounts: [ProviderAccount]
    let appleFM: AppleFM
    let capacityRefs: [String]
    let loadRefs: [String]
    let blockerRefs: [String]
    let recentRefs: [RecentRef]
    let proofRefs: [String]

    static func decode(from data: Data, fetchedAt: Date = Date()) throws -> FleetInspectorStatus {
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw KhalaClient.KhalaError.decoding
        }
        return fromPayload(object, fetchedAt: fetchedAt)
    }

    static func fromPayload(_ payload: [String: Any], fetchedAt: Date = Date()) -> FleetInspectorStatus {
        let allRefs = publicRefs(in: payload)
        let pylon = firstRecord(payload, keys: ["pylon", "node", "registration", "pylonRegistration"])
        let identity = firstRecord(payload, keys: ["identity", "owner", "openagentsIdentity", "openAgentsIdentity"])
        let localAgent = firstRecord(payload, keys: ["localAgent", "agent", "pylonAgent"])
        let apple = firstRecord(payload, keys: ["appleFM", "appleFm", "apple_fm", "appleFoundationModels"])

        let pylonRef = firstString(in: pylon, keys: ["pylonRef", "ref", "registrationRef", "agentRef"])
            ?? firstString(in: payload, keys: ["pylonRef", "targetPylonRef"])
            ?? allRefs.first(where: { $0.hasPrefix("pylon.") })
        let heartbeat = firstString(in: pylon, keys: ["lastHeartbeatAt", "latestHeartbeatAt", "heartbeatAt"])
            ?? firstString(in: payload, keys: ["lastHeartbeatAt", "latestHeartbeatAt", "heartbeatAt"])
        let heartbeatFresh = firstBool(in: pylon, keys: ["heartbeatFresh", "fresh", "isFresh"])
            ?? firstBool(in: payload, keys: ["heartbeatFresh", "pylonHeartbeatFresh"])

        let capacityRefs = unique(
            explicitStringArray(in: payload, keys: ["capacityRefs", "advertisedCapacityRefs"])
            + explicitStringArray(in: pylon, keys: ["capacityRefs", "advertisedCapacityRefs"])
            + allRefs.filter { $0.hasPrefix("capacity.") }
        )
        let loadRefs = unique(
            explicitStringArray(in: payload, keys: ["loadRefs"])
            + explicitStringArray(in: pylon, keys: ["loadRefs"])
            + allRefs.filter { $0.hasPrefix("load.") }
        )
        let blockerRefs = unique(
            explicitStringArray(in: payload, keys: ["blockerRefs", "blockers"])
            + explicitStringArray(in: pylon, keys: ["blockerRefs", "blockers"])
            + explicitStringArray(in: apple, keys: ["blockerRefs", "blockers"])
            + allRefs.filter { $0.hasPrefix("blocker.") }
        )
        let proofRefs = unique(
            explicitStringArray(in: payload, keys: ["proofRefs"])
            + allRefs.filter { $0.hasPrefix("proof.") || $0.hasPrefix("artifact.") }
        )
        let recentRefs = recentWorkRefs(from: payload, allRefs: allRefs)
        let pylonReadinessRefs = unique(
            [pylonRef].compactMap { $0 }
                + capacityRefs
                + loadRefs
        )

        return FleetInspectorStatus(
            fetchedAt: fetchedAt,
            connectedIdentity: firstString(in: identity, keys: ["userRef", "ownerRef", "actorUserId", "accountRef", "ref"])
                ?? firstString(in: payload, keys: ["userRef", "ownerRef", "actorUserId"]),
            localAgentIdentity: firstString(in: localAgent, keys: ["agentRef", "pylonRef", "ref"])
                ?? pylonRef,
            pylonRef: pylonRef,
            pylonReadiness: readiness(from: pylon, fallbackRefs: pylonReadinessRefs, availableHints: ["active", "ready", "online"]),
            heartbeatObservedAt: heartbeat,
            heartbeatFresh: heartbeatFresh,
            providerAccounts: providerAccounts(from: payload, refs: allRefs),
            appleFM: appleFM(from: apple, refs: allRefs),
            capacityRefs: capacityRefs,
            loadRefs: loadRefs,
            blockerRefs: blockerRefs,
            recentRefs: recentRefs,
            proofRefs: proofRefs
        )
    }

    static func redactedForDisplay(_ value: String, key: String? = nil) -> String {
        sanitize(value, key: key) ?? "[redacted]"
    }

    private static func providerAccounts(from payload: [String: Any], refs: [String]) -> [ProviderAccount] {
        let candidates = firstArray(payload, keys: ["providerAccounts", "accounts", "codexAccounts", "connectedAccounts"])
        var accounts: [ProviderAccount] = candidates.compactMap { entry in
            guard let record = entry as? [String: Any] else { return nil }
            let provider = firstString(in: record, keys: ["provider", "kind", "type"]) ?? "codex"
            guard let ref = firstString(in: record, keys: ["accountRef", "ref", "id"]) else { return nil }
            return ProviderAccount(
                provider: provider,
                ref: ref,
                readiness: readiness(from: record, fallbackRefs: []),
                detail: firstString(in: record, keys: ["readiness", "state", "status"])
            )
        }
        if accounts.isEmpty {
            accounts = refs
                .filter { $0.hasPrefix("account.") || $0.hasPrefix("capacity.coding.codex.account.") }
                .prefix(6)
                .map { ProviderAccount(provider: "codex", ref: $0, readiness: .unknown, detail: nil) }
        }
        return uniqueAccounts(accounts)
    }

    private static func appleFM(from record: [String: Any]?, refs: [String]) -> AppleFM {
        let appleRefs = refs.filter { $0.contains("apple_fm") || $0.contains("apple-fm") || $0.contains("apple.foundation") }
        let blockerRefs = unique(
            explicitStringArray(in: record, keys: ["blockerRefs", "blockers"])
            + appleRefs.filter { $0.hasPrefix("blocker.") }
        )
        let status = readiness(from: record, fallbackRefs: appleRefs)
        let summary = firstString(in: record, keys: ["summary", "status", "state", "readiness"])
            ?? (appleRefs.isEmpty ? "No Apple FM status published" : "Apple FM refs published")
        return AppleFM(readiness: blockerRefs.isEmpty ? status : .blocked, summary: summary, blockerRefs: blockerRefs)
    }

    private static func recentWorkRefs(from payload: [String: Any], allRefs: [String]) -> [RecentRef] {
        let explicit = explicitStringArray(in: payload, keys: ["recentRefs", "recentCloseoutRefs", "closeoutRefs", "assignmentRefs"])
        return unique(explicit + allRefs)
            .filter {
                $0.hasPrefix("assignment.")
                    || $0.hasPrefix("closeout.")
                    || $0.hasPrefix("accepted_work.")
                    || $0.hasPrefix("worker_closeout.")
            }
            .prefix(10)
            .map { ref in
                let kind = ref.hasPrefix("assignment.") ? "assignment"
                    : ref.hasPrefix("closeout.") || ref.hasPrefix("worker_closeout.") ? "closeout"
                    : "work"
                return RecentRef(kind: kind, value: ref)
            }
    }

    private static func publicRefs(in value: Any, key: String? = nil) -> [String] {
        if let text = value as? String {
            guard let safe = sanitize(text, key: key), looksLikePublicRef(safe) else { return [] }
            return [safe]
        }
        if let array = value as? [Any] {
            return array.flatMap { publicRefs(in: $0, key: key) }
        }
        if let record = value as? [String: Any] {
            return record.flatMap { entry in publicRefs(in: entry.value, key: entry.key) }
        }
        return []
    }

    private static func firstRecord(_ record: [String: Any], keys: [String]) -> [String: Any]? {
        for key in keys {
            if let value = record[key] as? [String: Any] { return value }
        }
        return nil
    }

    private static func firstString(in record: [String: Any]?, keys: [String]) -> String? {
        guard let record else { return nil }
        for key in keys {
            if let value = record[key] as? String,
               let safe = sanitize(value, key: key),
               !safe.isEmpty {
                return safe
            }
            if let value = record[key], !(value is [String: Any]), !(value is [Any]) {
                let text = String(describing: value)
                if let safe = sanitize(text, key: key), !safe.isEmpty {
                    return safe
                }
            }
        }
        return nil
    }

    private static func firstBool(in record: [String: Any]?, keys: [String]) -> Bool? {
        guard let record else { return nil }
        for key in keys {
            if let value = record[key] as? Bool { return value }
        }
        return nil
    }

    private static func firstArray(_ record: [String: Any], keys: [String]) -> [Any] {
        for key in keys {
            if let value = record[key] as? [Any] { return value }
        }
        return []
    }

    private static func explicitStringArray(in record: [String: Any]?, keys: [String]) -> [String] {
        guard let record else { return [] }
        return keys.flatMap { key -> [String] in
            if let values = record[key] as? [String] {
                return values.compactMap { sanitize($0, key: key) }
            }
            if let values = record[key] as? [Any] {
                return values.compactMap { value in sanitize(String(describing: value), key: key) }
            }
            if let value = record[key] as? String, let safe = sanitize(value, key: key) {
                return [safe]
            }
            return []
        }
    }

    private static func readiness(
        from record: [String: Any]?,
        fallbackRefs: [String],
        availableHints: [String] = ["ready", "available", "healthy", "online", "fresh"]
    ) -> Availability {
        let status = firstString(in: record, keys: ["readiness", "state", "status", "availability"])?.lowercased()
        let text = ([status].compactMap { $0 } + fallbackRefs.map { $0.lowercased() }).joined(separator: " ")
        if text.contains("blocked") || text.contains("unavailable") || text.contains("missing") || text.contains("failed") {
            return .blocked
        }
        if text.contains("stale") || text.contains("expired") {
            return .stale
        }
        if availableHints.contains(where: { text.contains($0) }) {
            return .available
        }
        return .unknown
    }

    private static func sanitize(_ value: String, key: String?) -> String? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let lowerKey = key?.lowercased() ?? ""
        if lowerKey.contains("email") {
            return "[redacted-email]"
        }
        let sensitiveKeyParts = ["auth", "credential", "mnemonic", "password", "private", "raw", "secret", "wallet"]
        if sensitiveKeyParts.contains(where: { lowerKey.contains($0) }) {
            return "[redacted]"
        }
        let sensitivePatterns = [
            #"oa_agent_[A-Za-z0-9._-]+"#,
            #"sk-[A-Za-z0-9._-]+"#,
            #"Bearer\s+[A-Za-z0-9._-]+"#,
            #"/Users/[^\s,\]\}"]+"#,
            #"/home/[^\s,\]\}"]+"#,
            #"~/.codex[^\s,\]\}"]*"#,
            #"auth\.json"#,
            #"spark1[ac-hj-np-z02-9]+"#,
            #"lnbc[ac-hj-np-z02-9]+"#,
            #"lntb[ac-hj-np-z02-9]+"#
        ]
        if sensitivePatterns.contains(where: { trimmed.range(of: $0, options: .regularExpression) != nil }) {
            return "[redacted]"
        }
        return trimmed
    }

    private static func looksLikePublicRef(_ value: String) -> Bool {
        value.range(of: #"^[a-z][a-z0-9_.:-]+(\.[a-z0-9_.:-]+)*=?[A-Za-z0-9_.:-]*$"#, options: .regularExpression) != nil
    }

    private static func unique(_ values: [String]) -> [String] {
        var seen = Set<String>()
        var output: [String] = []
        for value in values where !seen.contains(value) {
            seen.insert(value)
            output.append(value)
        }
        return output
    }

    private static func uniqueAccounts(_ values: [ProviderAccount]) -> [ProviderAccount] {
        var seen = Set<String>()
        var output: [ProviderAccount] = []
        for value in values where !seen.contains(value.id) {
            seen.insert(value.id)
            output.append(value)
        }
        return output
    }
}
