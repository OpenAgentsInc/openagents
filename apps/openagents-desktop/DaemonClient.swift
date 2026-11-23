// Firm reason: 6 (Swift-to-C host wrapper over coax/lib/agent/desktop/daemon.coax; GitHub #13794).
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
    enum CodingKeys: String, CodingKey { case session_id = "id", title, folder, pinned, archived, updated }
    let title: String?
    let folder: String?
    let pinned: Bool?
    let archived: Bool?
    let updated: Int?
}

struct DaemonRun: Decodable {
    let run_id: String
    let state: String
    let pid: Int?
    let started: Int?
    let ended: Int?
    let exit: Int?
    let out: Int?
    var isLive: Bool { state == "running" || state == "stopping" }
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
    let runs: [DaemonRun]?
    let status: String?
    let error: String?
    enum CodingKeys: String, CodingKey { case schema, ok, host, label, method, pid, daemon_status, foreman, meta = "sessions_meta", runs, status, error }
}

final class CoderDaemonClient {
    static let schema = "openagents.environment.daemon.v1alpha1"
    private let queue = DispatchQueue(label: "openagents.coder.daemon-client", qos: .utility)
    let hostDirPath: String
    private let hostDir: URL

    init() {
        let p = DesktopCore.callString(4096) { oa_dc_daemon_dir($0) }
        self.hostDirPath = p
        self.hostDir = URL(fileURLWithPath: p, isDirectory: true)
    }

    var discoveryURL: URL { hostDir.appendingPathComponent("daemon.json") }
    var socketPath: String {
        DesktopCore.withCoaxString(hostDirPath) { d in
            DesktopCore.callString(4096) { out in oa_dc_daemon_socket(d, out) }
        }
    }

    func readDiscovery() -> DaemonDiscovery? {
        guard let data = try? Data(contentsOf: discoveryURL) else { return nil }
        return try? JSONDecoder().decode(DaemonDiscovery.self, from: data)
    }

    var isDaemonAvailable: Bool {
        DesktopCore.withCoaxString(hostDirPath) { oa_dc_daemon_available($0) != 0 }
    }

    func describeDaemon(completion: @escaping (DaemonView?) -> Void) {
        sendAsync(["schema": Self.schema, "method": "DaemonHealth"], completion: completion)
    }

    func readForeman(completion: @escaping (DaemonForemanState?) -> Void) {
        sendAsync(["schema": Self.schema, "method": "ReadForeman"]) { view in
            completion(view?.foreman)
        }
    }

    func updateForeman(tier: Int, prompt: String? = nil, completion: ((Bool) -> Void)? = nil) {
        queue.async { [weak self] in
            guard let self else { return }
            let ok = DesktopCore.withCoaxString(self.hostDirPath) { d in
                DesktopCore.withCoaxString(prompt ?? "") { pr in
                    oa_dc_daemon_update_foreman(d, Int64(tier), pr) != 0
                }
            }
            DispatchQueue.main.async { completion?(ok) }
        }
    }

    func listSessionMeta(completion: @escaping ([DaemonSessionMeta]) -> Void) {
        sendAsync(["schema": Self.schema, "method": "ListSessionMeta"]) { view in
            completion(view?.meta ?? [])
        }
    }

    func updateSessionMeta(sessionID: String, title: String? = nil, folder: String? = nil, pinned: Bool? = nil, archived: Bool? = nil, completion: ((Bool) -> Void)? = nil) {
        guard !sessionID.isEmpty else { completion?(false); return }
        let pinFlag: Int64 = pinned == nil ? -1 : (pinned! ? 1 : 0)
        let archFlag: Int64 = archived == nil ? -1 : (archived! ? 1 : 0)
        queue.async { [weak self] in
            guard let self else { return }
            let ok = DesktopCore.withCoaxString(self.hostDirPath) { d in
                DesktopCore.withCoaxString(sessionID) { s in
                    DesktopCore.withCoaxString(title ?? "") { t in
                        DesktopCore.withCoaxString(folder ?? "") { f in
                            oa_dc_daemon_update_meta(d, s, t, f, pinFlag, archFlag) != 0
                        }
                    }
                }
            }
            DispatchQueue.main.async { completion?(ok) }
        }
    }

    var runsDir: URL { hostDir.appendingPathComponent("runs", isDirectory: true) }
    private(set) var ownsRuns = false
    static var spawnedPID: pid_t = 0

    @discardableResult func ensureDaemon(wait: TimeInterval = 4) -> Bool {
        let prog = Bundle.main.path(forResource: "coder-host", ofType: nil) ?? ""
        var pidOut: [Int64] = [0]
        let ok = DesktopCore.withCoaxString(hostDirPath) { d in
            DesktopCore.withCoaxString(prog) { p in
                DesktopCore.withMutableS64(&pidOut) { po in
                    oa_dc_daemon_ensure(d, p, Int64(wait * 1000), po) != 0
                }
            }
        }
        if pidOut[0] > 0 { Self.spawnedPID = pid_t(pidOut[0]) }
        ownsRuns = ok
        return ok
    }

    func startRun(_ id: String) -> String? {
        var whyBuf = [UInt8](repeating: 0, count: 1024)
        let rc = DesktopCore.withCoaxString(hostDirPath) { d in
            DesktopCore.withCoaxString(id) { rid in
                DesktopCore.withMutableBytes(&whyBuf) { w in
                    oa_dc_daemon_start_run(d, rid, w)
                }
            }
        }
        if rc == 0 { return nil }
        let why = DesktopCore.cString(from: whyBuf)
        return why.isEmpty ? "The background service didn't answer." : why
    }

    func startRunAsync(_ id: String, completion: @escaping (String?) -> Void) {
        queue.async { [weak self] in
            guard let self else { return }
            let err = self.startRun(id)
            DispatchQueue.main.async { completion(err) }
        }
    }

    func stopRun(_ id: String, then: (() -> Void)? = nil) {
        queue.async { [weak self] in
            guard let self else { return }
            _ = DesktopCore.withCoaxString(self.hostDirPath) { d in
                DesktopCore.withCoaxString(id) { rid in
                    oa_dc_daemon_stop_run(d, rid)
                }
            }
            DispatchQueue.main.async { then?() }
        }
    }

    func forgetRun(_ id: String) {
        queue.async { [weak self] in
            guard let self else { return }
            _ = DesktopCore.withCoaxString(self.hostDirPath) { d in
                DesktopCore.withCoaxString(id) { rid in
                    oa_dc_daemon_forget_run(d, rid)
                }
            }
        }
    }

    func listRuns() -> [DaemonRun]? {
        guard let view = sendSync(["schema": Self.schema, "method": "ListRuns"]), view.ok == true else { return nil }
        return view.runs ?? []
    }

    private func sendAsync(_ payload: [String: Any], completion: @escaping (DaemonView?) -> Void) {
        queue.async { [weak self] in
            guard let self else { return }
            let result = self.sendSync(payload)
            DispatchQueue.main.async { completion(result) }
        }
    }

    private func sendSync(_ payload: [String: Any]) -> DaemonView? {
        guard let jsonData = try? JSONSerialization.data(withJSONObject: payload, options: []),
              let reqStr = String(data: jsonData, encoding: .utf8) else { return nil }
        var respBuf = [UInt8](repeating: 0, count: 1024 * 1024)
        let n: Int64 = DesktopCore.withCoaxString(hostDirPath) { d in
            DesktopCore.withCoaxString(reqStr) { req in
                DesktopCore.withMutableBytes(&respBuf) { out in
                    oa_dc_daemon_rpc(d, req, out)
                }
            }
        }
        guard n > 0 else { return nil }
        let data = Data(respBuf.prefix(Int(n)))
        return try? JSONDecoder().decode(DaemonView.self, from: data)
    }

    /// Forwards one NIP-SESS direct sealed request event (one NDJSON line without trailing newline)
    /// to the resident daemon over service.sock and returns the sealed reply event line, or nil.
    func forwardDirect(_ eventLine: Data) -> Data? {
        let path = socketPath
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        guard !eventLine.isEmpty, eventLine.count < 262_144 else { return nil }
        var line = eventLine
        line.append(0x0A)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        defer { close(fd) }

        var tv = timeval(tv_sec: 22, tv_usec: 0)
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
        var buf = [UInt8](repeating: 0, count: 16_384)
        while response.count < 524_288 {
            let r = Darwin.read(fd, &buf, buf.count)
            if r <= 0 { break }
            if let nl = buf[0..<r].firstIndex(of: 0x0A) {
                response.append(contentsOf: buf[0..<nl])
                break
            }
            response.append(contentsOf: buf[0..<r])
        }
        return response.isEmpty ? nil : response
    }
}
