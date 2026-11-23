// Firm reason: 6 (Swift wrapper over the compiled Coax C bridge mobile-core-bridge.h; all protocol, crypto, and Antigravity policy rules execute in Coax).
import Foundation
import Security

/// RAII wrapper around the heap-allocated Coax work arena (`DvWork` + `EaRequest` + `EaView`).
final class MobileCoreWork {
    let raw: UnsafeMutableRawPointer

    init?() {
        guard let ptr = oa_mobile_work_open() else { return nil }
        self.raw = ptr
    }

    deinit {
        oa_mobile_work_close(raw)
    }
}

enum MobileCore {
    static let opPair: Int64 = 1
    static let opStatus: Int64 = 2
    static let opRPC: Int64 = 3

    static func randomBytes(_ count: Int) -> Data? {
        var bytes = [UInt8](repeating: 0, count: count)
        guard SecRandomCopyBytes(kSecRandomDefault, count, &bytes) == errSecSuccess else { return nil }
        return Data(bytes)
    }

    static func randomHex(bytes count: Int) -> String {
        guard let data = randomBytes(count) else {
            return String(repeating: "0", count: count * 2)
        }
        return data.map { String(format: "%02x", $0) }.joined()
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
        var out = [UInt8](repeating: 0, count: 64)
        let n = oa_mobile_normalize_agy_model(want, &out, Int64(out.count))
        guard n > 0 else { return "gemini-4-neon" }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    /// Coax `#authority` effort normalization for Antigravity (`low` | `medium` | `high` | `xhigh`).
    static func normalizeAntigravityEffort(model: String, want: String) -> String {
        var out = [UInt8](repeating: 0, count: 32)
        let n = oa_mobile_normalize_agy_effort(model, want, &out, Int64(out.count))
        guard n > 0 else { return "medium" }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    static func isValidFolder(_ folder: String) -> Bool {
        oa_mobile_folder_ok(folder) != 0
    }

    /// Encodes a bounded, policy-verified `EaRequest` JSON payload via Coax `mc_env_request`.
    /// Non-Antigravity models are refused at the wire boundary by Coax.
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
        item: String = ""
    ) -> Data? {
        var out = [UInt8](repeating: 0, count: 24_576)
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
            &out,
            Int64(out.count)
        )
        guard n > 0 else { return nil }
        return Data(out[0..<Int(n)])
    }

    /// Validates an `EaView` JSON payload via Coax `mc_view_valid`.
    static func validateView(work: MobileCoreWork, host: String, method: String, raw: Data) -> Bool {
        raw.withUnsafeBytes { buf in
            guard let ptr = buf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            return oa_mobile_view_valid(work.raw, host, method, ptr, Int64(raw.count)) != 0
        }
    }

    /// Seals one NIP-44 `app.rpc` event via Coax `dv_seal`.
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
        let relayLabel = relay.isEmpty ? "tailscale://direct" : relay
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
