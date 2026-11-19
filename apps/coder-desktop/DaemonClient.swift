// Local Unix-socket client for the unified OpenAgents daemon (`coder host serve`).
// Speaks the newline-framed `openagents.environment.daemon.v1alpha1` control plane
// over ~/.openagents/coder/host/service.sock so Coder Desktop and OpenAgents Mobile
// share daemon health, Foreman policy, and session metadata (pin, rename, archive).
import Darwin
import Foundation

struct DaemonDiscovery: Decodable {
    let schema: String?
    let host: String?
    let pid: Int?
    let socket: String?
    let status: String?
    let updated_at: Int?
}

struct DaemonForemanState: Decodable {
    let tier: Int?
    let prompt: String?
    let updated: Int?
}

struct DaemonSessionMeta: Decodable {
    let session_id: String
    let title: String?
    let folder: String?
    let pinned: Bool?
    let archived: Bool?
    let updated: Int?
}

struct DaemonView: Decodable {
    let schema: String?
    let ok: Bool?
    let host: String?
    let label: String?
    let method: String?
    let pid: Int?
    let daemon_status: String?
    let foreman: DaemonForemanState?
    let meta: [DaemonSessionMeta]?
    let error: String?
}

final class CoderDaemonClient {
    static let schema = "openagents.environment.daemon.v1alpha1"
    private let queue = DispatchQueue(label: "openagents.coder.daemon-client", qos: .utility)
    private let hostDir: URL

    init() {
        let fm = FileManager.default
        if let custom = ProcessInfo.processInfo.environment["CODER_HOME"], !custom.isEmpty {
            self.hostDir = URL(fileURLWithPath: custom, isDirectory: true).appendingPathComponent("host", isDirectory: true)
        } else {
            self.hostDir = fm.homeDirectoryForCurrentUser
                .appendingPathComponent(".openagents/coder/host", isDirectory: true)
        }
    }

    var discoveryURL: URL { hostDir.appendingPathComponent("daemon.json") }
    var socketPath: String {
        if let d = readDiscovery(), let s = d.socket, !s.isEmpty { return s }
        return hostDir.appendingPathComponent("service.sock").path
    }

    func readDiscovery() -> DaemonDiscovery? {
        guard let data = try? Data(contentsOf: discoveryURL) else { return nil }
        return try? JSONDecoder().decode(DaemonDiscovery.self, from: data)
    }

    var isDaemonAvailable: Bool {
        FileManager.default.fileExists(atPath: socketPath)
    }

    func describeDaemon(completion: @escaping (DaemonView?) -> Void) {
        let req: [String: Any] = [
            "schema": Self.schema,
            "method": "DaemonHealth"
        ]
        sendAsync(req, completion: completion)
    }

    func readForeman(completion: @escaping (DaemonForemanState?) -> Void) {
        let req: [String: Any] = [
            "schema": Self.schema,
            "method": "ReadForeman"
        ]
        sendAsync(req) { view in
            completion(view?.foreman)
        }
    }

    func updateForeman(tier: Int, prompt: String? = nil, completion: ((Bool) -> Void)? = nil) {
        let now = Int(Date().timeIntervalSince1970 * 1000)
        var req: [String: Any] = [
            "schema": Self.schema,
            "method": "UpdateForeman",
            "command_id": Self.newCommandID(),
            "tier": tier,
            "issued_at": now,
            "expires_at": now + 60_000
        ]
        if let prompt { req["text"] = prompt }
        sendAsync(req) { view in
            completion?(view?.ok == true)
        }
    }

    func listSessionMeta(completion: @escaping ([DaemonSessionMeta]) -> Void) {
        let req: [String: Any] = [
            "schema": Self.schema,
            "method": "ListSessionMeta"
        ]
        sendAsync(req) { view in
            completion(view?.meta ?? [])
        }
    }

    func updateSessionMeta(sessionID: String, title: String? = nil, folder: String? = nil, pinned: Bool? = nil, archived: Bool? = nil, completion: ((Bool) -> Void)? = nil) {
        guard !sessionID.isEmpty else { completion?(false); return }
        let now = Int(Date().timeIntervalSince1970 * 1000)
        var req: [String: Any] = [
            "schema": Self.schema,
            "method": "UpdateSessionMeta",
            "session_id": sessionID,
            "command_id": Self.newCommandID(),
            "issued_at": now,
            "expires_at": now + 60_000
        ]
        if let title { req["title"] = title }
        if let folder { req["folder"] = folder }
        if let pinned { req["pinned"] = pinned }
        if let archived { req["archived"] = archived }
        sendAsync(req) { view in
            completion?(view?.ok == true)
        }
    }

    private static func newCommandID() -> String {
        UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
    }

    private func sendAsync(_ payload: [String: Any], completion: @escaping (DaemonView?) -> Void) {
        queue.async { [weak self] in
            guard let self else { return }
            let result = self.sendSync(payload)
            DispatchQueue.main.async { completion(result) }
        }
    }

    private func sendSync(_ payload: [String: Any]) -> DaemonView? {
        let path = socketPath
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        guard let json = try? JSONSerialization.data(withJSONObject: payload, options: []) else { return nil }
        var line = json
        line.append(0x0A)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        defer { close(fd) }

        var tv = timeval(tv_sec: 2, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = path.utf8CString
        let maxLen = MemoryLayout.size(ofValue: addr.sun_path)
        guard pathBytes.count <= maxLen else { return nil }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: maxLen) { dst in
                for i in 0..<pathBytes.count { dst[i] = pathBytes[i] }
            }
        }

        let addrLen = socklen_t(MemoryLayout<sockaddr_un>.size)
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                connect(fd, sa, addrLen)
            }
        }
        guard rc == 0 else { return nil }

        let sent: Bool = line.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return false }
            var offset = 0
            while offset < raw.count {
                let w = Darwin.write(fd, base.advanced(by: offset), raw.count - offset)
                if w <= 0 { return false }
                offset += w
            }
            return true
        }
        guard sent else { return nil }

        var response = Data()
        var buf = [UInt8](repeating: 0, count: 4096)
        while response.count < 65_536 {
            let r = Darwin.read(fd, &buf, buf.count)
            if r <= 0 { break }
            if let nl = buf[0..<r].firstIndex(of: 0x0A) {
                response.append(contentsOf: buf[0..<nl])
                break
            }
            response.append(contentsOf: buf[0..<r])
        }
        guard !response.isEmpty else { return nil }
        return try? JSONDecoder().decode(DaemonView.self, from: response)
    }
}
