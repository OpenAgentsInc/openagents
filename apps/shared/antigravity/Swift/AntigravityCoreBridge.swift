// Firm reason: 6 (cross-Apple Swift wrapper over apps/shared/antigravity/client-core-bridge.h; all protocol, crypto, workspace, and Antigravity policy rules execute in Coax).
import Foundation
import Security

/// Typed diagnostic errors returned from `apps/shared/antigravity/client-core-bridge.c` (#14006).
enum MobileCoreError: Int32, Error, LocalizedError, Equatable {
    case ok = 0
    case nullWork = -1
    case textTooLong = -2
    case invalidModel = -3
    case invalidFolder = -4
    case invalidMode = -5
    case cryptoFail = -6
    case bufferSmall = -7
    case jsonSyntax = -8
    case schemaMismatch = -9
    case encodeFail = -10

    var errorDescription: String? {
        switch self {
        case .ok: return nil
        case .nullWork: return "Internal crypto/codec arena unavailable."
        case .textTooLong: return "Message exceeds the 16 KiB single-turn limit."
        case .invalidModel: return "Selected model is not permitted for Antigravity."
        case .invalidFolder: return "Selected workspace folder path is invalid."
        case .invalidMode: return "Selected review mode is not recognized."
        case .cryptoFail: return "NIP-44 encryption or signature verification failed."
        case .bufferSmall: return "Payload exceeds the wire buffer limit."
        case .jsonSyntax: return "Host returned non-JSON or truncated data."
        case .schemaMismatch: return "Host response did not match the expected wire schema."
        case .encodeFail: return "Could not encode Antigravity wire request."
        }
    }
}

/// RAII wrapper around the heap-allocated Coax work arena (`DvWork` + `EaRequest` + `EaView` + `WsRequest` + `WsView`).
final class MobileCoreWork: @unchecked Sendable {
    let raw: UnsafeMutableRawPointer

    init?() {
        guard let ptr = oa_mobile_work_open() else { return nil }
        self.raw = ptr
    }

    var lastError: MobileCoreError {
        MobileCoreError(rawValue: oa_mobile_last_error(raw)) ?? .encodeFail
    }

    deinit {
        oa_mobile_work_close(raw)
    }
}

/// Reusable pool of `MobileCoreWork` arenas eliminating 2.14 MiB/RPC heap allocation churn (#14006).
/// Every arena is securely zeroized via `oa_mobile_work_close` when drained or deallocated.
final class MobileCorePool: @unchecked Sendable {
    static let shared = MobileCorePool()

    private let lock = NSLock()
    private var available: [MobileCoreWork] = []
    private let maxCached: Int

    init(maxCached: Int = 4) {
        self.maxCached = maxCached
    }

    func checkout() -> MobileCoreWork? {
        lock.lock()
        if let existing = available.popLast() {
            lock.unlock()
            return existing
        }
        lock.unlock()
        return MobileCoreWork()
    }

    func checkin(_ work: MobileCoreWork) {
        lock.lock()
        if available.count < maxCached {
            available.append(work)
        }
        lock.unlock()
    }

    func withWork<T>(_ body: (MobileCoreWork) throws -> T) rethrows -> T? {
        guard let work = checkout() else { return nil }
        defer { checkin(work) }
        return try body(work)
    }

    func drain() {
        lock.lock()
        available.removeAll()
        lock.unlock()
    }
}

enum MobileCore {
    static let opPair: Int64 = 1
    static let opStatus: Int64 = 2
    static let opRPC: Int64 = 3
    static let directRelayLabel = "tailscale://direct"

    static func randomBytes(_ count: Int) -> Data? {
        guard count > 0 else { return Data() }
        var bytes = [UInt8](repeating: 0, count: count)
        if SecRandomCopyBytes(kSecRandomDefault, count, &bytes) == errSecSuccess {
            return Data(bytes)
        }
        // Fail closed on predictable zeros: fall back to kernel CSPRNG `arc4random_buf` only if SecRandom fails (#14006).
        arc4random_buf(&bytes, count)
        guard bytes.contains(where: { $0 != 0 }) else { return nil }
        return Data(bytes)
    }

    static func randomHexOrNil(bytes count: Int) -> String? {
        guard let data = randomBytes(count) else { return nil }
        return data.map { String(format: "%02x", $0) }.joined()
    }

    static func randomHex(bytes count: Int) -> String {
        if let hex = randomHexOrNil(bytes: count) { return hex }
        // Never emit predictable all-zeros; mix kernel arc4random_buf directly.
        var fallback = [UInt8](repeating: 0, count: max(1, count))
        arc4random_buf(&fallback, fallback.count)
        return fallback.prefix(count).map { String(format: "%02x", $0) }.joined()
    }

    /// Derives the 64-char lowercase hex x-only secp256k1 public key via Coax `dv_public`.
    static func publicKeyHex(secret: Data) -> String? {
        guard secret.count == 32 else { return nil }
        var out = [UInt8](repeating: 0, count: 64)
        let n = secret.withUnsafeBytes { raw -> Int64 in
            guard let ptr = raw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return oa_mobile_public(ptr, &out)
        }
        guard n == 64 else { return nil }
        return String(decoding: out, as: UTF8.self)
    }

    static func isValidHostKey(_ key: String) -> Bool {
        oa_mobile_host_key_ok(key) != 0
    }

    /// Coax `#authority` Antigravity-only model predicate (`phone-agent-policy.coax`).
    static func isAntigravityModel(_ id: String) -> Bool {
        oa_mobile_is_agy_model(id) != 0
    }

    /// Coax `#authority` normalization strictly within Antigravity (defaults to `gemini-4-neon`).
    static func normalizeAntigravityModel(_ want: String) -> String {
        var out = [UInt8](repeating: 0, count: 128)
        let n = oa_mobile_normalize_agy_model(want, &out, Int64(out.count))
        guard n > 0 else { return "gemini-4-neon" }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    /// Coax `#authority` effort normalization for Antigravity (`none`..`max`, or `""` for non-thinking models like Orchestrator).
    static func normalizeAntigravityEffort(model: String, want: String) -> String {
        var out = [UInt8](repeating: 0, count: 32)
        let n = oa_mobile_normalize_agy_effort(model, want, &out, Int64(out.count))
        guard n >= 0 else { return "medium" }
        if n == 0 { return "" }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    static func isValidFolder(_ folder: String) -> Bool {
        oa_mobile_folder_ok(folder) != 0
    }

    static func isValidReviewMode(_ mode: String) -> Bool {
        oa_mobile_mode_ok(mode) != 0
    }

    static func isV1WireCompatible(_ raw: Data) -> Bool {
        raw.withUnsafeBytes { buf in
            guard let ptr = buf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            return oa_mobile_v1_ea_compat(ptr, Int64(raw.count)) != 0
        }
    }

    /// Encodes a bounded, policy-verified `EaRequest` JSON payload via Coax `mc_env_request`.
    /// Non-Antigravity models and invalid review modes are refused at the wire boundary by Coax.
    static func encodeEnvironmentRequest(
        work: MobileCoreWork,
        method: String,
        session: String = "",
        command: String = "",
        generation: Int64 = 0,
        revision: Int64 = 0,
        text: String = "",
        interaction: String = "",
        allow: Bool = false,
        before: Int64 = 0,
        issuedMs: Int64 = 0,
        expiresMs: Int64 = 0,
        model: String = "",
        effort: String = "",
        folder: String = "",
        item: String = "",
        mode: String = "",
        optionID: String = "",
        attachmentsJSON: String = ""
    ) -> Data? {
        var out = [UInt8](repeating: 0, count: 65_536)
        let n = oa_mobile_env_request(
            work.raw,
            method,
            session,
            command,
            generation,
            revision,
            text,
            interaction,
            allow ? 1 : 0,
            before,
            issuedMs,
            expiresMs,
            model,
            effort,
            folder,
            item,
            mode,
            optionID,
            attachmentsJSON,
            &out,
            Int64(out.count)
        )
        guard n > 0 else { return nil }
        return Data(out[0..<Int(n)])
    }

    static func validateView(work: MobileCoreWork, host: String, method: String, raw: Data) -> Bool {
        raw.withUnsafeBytes { buf in
            guard let ptr = buf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            return oa_mobile_view_valid(work.raw, host, method, ptr, Int64(raw.count)) != 0
        }
    }

    /// Encodes a typed `WsRequest` (Git diff, branches, worktrees, push, PRs) via Coax `mc_ws_request`.
    static func encodeWorkspaceRequest(
        work: MobileCoreWork,
        method: String,
        workspace: String = "default",
        branch: String = "",
        base: String = "",
        title: String = "",
        body: String = "",
        number: Int64 = 0,
        page: Int64 = 0,
        cursor: Int64 = 0,
        expectedRevision: String = "",
        command: String = "",
        issuedMs: Int64 = 0,
        expiresMs: Int64 = 0
    ) -> Data? {
        var out = [UInt8](repeating: 0, count: 24_576)
        let n = oa_mobile_ws_request(
            work.raw,
            method,
            workspace,
            branch,
            base,
            title,
            body,
            number,
            page,
            cursor,
            expectedRevision,
            command,
            issuedMs,
            expiresMs,
            &out,
            Int64(out.count)
        )
        guard n > 0 else { return nil }
        return Data(out[0..<Int(n)])
    }

    static func validateWorkspaceView(work: MobileCoreWork, host: String, method: String, raw: Data) -> Bool {
        raw.withUnsafeBytes { buf in
            guard let ptr = buf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            return oa_mobile_ws_view_valid(work.raw, host, method, ptr, Int64(raw.count)) != 0
        }
    }

    /// Encodes a typed `DmRequest` (`ListSessionMeta`, `UpdateSessionMeta`, `DaemonHealth`) via Coax `mc_dm_request` (#14013).
    static func encodeDaemonRequest(
        work: MobileCoreWork,
        method: String,
        session: String = "",
        command: String = "",
        title: String = "",
        folder: String = "",
        pinned: Bool? = nil,
        archived: Bool? = nil,
        issuedMs: Int64 = 0,
        expiresMs: Int64 = 0
    ) -> Data? {
        var out = [UInt8](repeating: 0, count: 16_384)
        let pinFlag: Int64 = pinned.map { $0 ? 1 : 0 } ?? -1
        let archFlag: Int64 = archived.map { $0 ? 1 : 0 } ?? -1
        let n = oa_mobile_dm_request(
            work.raw,
            method,
            session,
            command,
            title,
            folder,
            pinFlag,
            archFlag,
            issuedMs,
            expiresMs,
            &out,
            Int64(out.count)
        )
        guard n > 0 else { return nil }
        return Data(out[0..<Int(n)])
    }

    static func validateDaemonView(work: MobileCoreWork, host: String, method: String, raw: Data) -> Bool {
        raw.withUnsafeBytes { buf in
            guard let ptr = buf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            return oa_mobile_dm_view_valid(work.raw, host, method, ptr, Int64(raw.count)) != 0
        }
    }

    /// Seals one NIP-44 `app.rpc` event via Coax `dv_seal` (works for both Environment and Workspace RPCs).
    static func sealRPC(
        work: MobileCoreWork,
        secret: Data,
        host: String,
        relay: String,
        grant: String,
        epoch: Int64,
        params: Data,
        nowSeconds: Int64 = Int64(Date().timeIntervalSince1970)
    ) -> (event: Data, eventID: String)? {
        guard secret.count == 32, let random = randomBytes(96) else { return nil }
        var out = [UInt8](repeating: 0, count: 131_072)
        let relayLabel = relay.isEmpty ? directRelayLabel : relay
        let n = secret.withUnsafeBytes { skRaw -> Int64 in
            guard let skPtr = skRaw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return random.withUnsafeBytes { rndRaw -> Int64 in
                guard let rndPtr = rndRaw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return params.withUnsafeBytes { pRaw -> Int64 in
                    let pPtr = pRaw.baseAddress?.assumingMemoryBound(to: UInt8.self)
                    return oa_mobile_seal(
                        work.raw,
                        opRPC,
                        skPtr,
                        host,
                        relayLabel,
                        grant,
                        epoch,
                        pPtr,
                        Int64(params.count),
                        nowSeconds,
                        rndPtr,
                        &out,
                        Int64(out.count)
                    )
                }
            }
        }
        guard n > 0 else { return nil }
        var idBuf = [UInt8](repeating: 0, count: 64)
        guard oa_mobile_event_id(work.raw, &idBuf) == 64 else { return nil }
        return (Data(out[0..<Int(n)]), String(decoding: idBuf, as: UTF8.self))
    }

    /// Opens and cryptographically matches a sealed reply event against the active request in `work`.
    static func openReply(
        work: MobileCoreWork,
        secret: Data,
        host: String,
        event: Data,
        nowSeconds: Int64 = Int64(Date().timeIntervalSince1970)
    ) -> Data? {
        guard secret.count == 32 else { return nil }
        var out = [UInt8](repeating: 0, count: 262_144)
        let n = secret.withUnsafeBytes { skRaw -> Int64 in
            guard let skPtr = skRaw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return event.withUnsafeBytes { evRaw -> Int64 in
                guard let evPtr = evRaw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return oa_mobile_open(
                    work.raw,
                    skPtr,
                    host,
                    evPtr,
                    Int64(event.count),
                    nowSeconds,
                    &out,
                    Int64(out.count)
                )
            }
        }
        guard n > 0 else { return nil }
        return Data(out[0..<Int(n)])
    }
}
