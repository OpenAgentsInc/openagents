// Firm reason: 6 (macOS owner-only AF_UNIX fast-path to ~/.openagents/coder/host/service.sock and runs directory watcher; stubbed on iOS so both platforms share one Swift source tree).
import Foundation
#if os(macOS)
import Darwin
#endif

/// Local Unix-socket transport to the headless `coder-host` daemon on the same Mac.
/// Uses the exact same newline-framed `EaRequest`/`EaView`, `WsRequest`/`WsView`, and sealed
/// kind-3188 wire payloads as Tailscale TCP — zero relay, zero extra serialization.
final class LocalSocketClient: @unchecked Sendable {
    static let shared = LocalSocketClient()

    private let ioQueue = DispatchQueue(label: "com.openagents.local-socket", qos: .userInitiated)
    #if os(macOS)
    private var runsWatcher: DispatchSourceFileSystemObject?
    private var runsWatcherFD: Int32 = -1
    #endif

    var hostDirectory: URL {
        #if os(macOS)
        let envHome = ProcessInfo.processInfo.environment["CODER_HOME"]
        let base: URL
        if let envHome, !envHome.isEmpty {
            base = URL(fileURLWithPath: envHome, isDirectory: true)
        } else {
            base = FileManager.default.homeDirectoryForCurrentUser
        }
        return base.appendingPathComponent(".openagents/coder/host", isDirectory: true)
        #else
        return FileManager.default.temporaryDirectory.appendingPathComponent("openagents-host", isDirectory: true)
        #endif
    }

    var socketURL: URL {
        hostDirectory.appendingPathComponent("service.sock")
    }

    var runsDirectoryURL: URL {
        hostDirectory.appendingPathComponent("runs", isDirectory: true)
    }

    /// Probes live listener reachability via non-blocking `AF_UNIX` `connect()` instead of `fileExists`
    /// so stale `service.sock` files left after a crash never falsely show the daemon as running (#14001).
    var isSocketAvailable: Bool {
        #if os(macOS)
        return Self.probeSocket(path: socketURL.path)
        #else
        return false
        #endif
    }

    /// Sends one newline-framed payload (`EaRequest`, `WsRequest`, or sealed kind-3188 Nostr event)
    /// over `service.sock` and returns the single newline-delimited reply payload.
    func exchangeLine(_ payload: Data, timeoutSeconds: Int = 15) async -> Result<Data, TailscaleRPCError> {
        #if os(macOS)
        let path = socketURL.path
        return await withCheckedContinuation { continuation in
            ioQueue.async {
                continuation.resume(returning: Self.syncExchange(path: path, payload: payload, timeoutSeconds: timeoutSeconds))
            }
        }
        #else
        return .failure(.unreachable("Local Unix socket is only available on macOS."))
        #endif
    }

    /// Watches `~/.openagents/coder/host/runs` on macOS so local UI and remote Tailscale subscribers
    /// receive instant change triggers without polling.
    func startWatchingRuns(onChange: @escaping @Sendable () -> Void) {
        #if os(macOS)
        stopWatchingRuns()
        let dir = runsDirectoryURL
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let fd = open(dir.path, O_EVTONLY)
        guard fd >= 0 else { return }
        runsWatcherFD = fd
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .extend, .rename, .delete, .attrib],
            queue: DispatchQueue(label: "com.openagents.runs-watch", qos: .utility)
        )
        source.setEventHandler {
            onChange()
        }
        source.setCancelHandler { [fd] in
            close(fd)
        }
        runsWatcher = source
        source.resume()
        #endif
    }

    func stopWatchingRuns() {
        #if os(macOS)
        runsWatcher?.cancel()
        runsWatcher = nil
        runsWatcherFD = -1
        #endif
    }

    #if os(macOS)
    private static func probeSocket(path: String) -> Bool {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { close(fd) }

        var tv = timeval(tv_sec: 0, tv_usec: 250_000)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let maxLen = MemoryLayout.size(ofValue: addr.sun_path)
        let bytes = Array(path.utf8)
        guard bytes.count < maxLen else { return false }
        withUnsafeMutableBytes(of: &addr.sun_path) { raw in
            raw.initializeMemory(as: UInt8.self, repeating: 0)
            for (i, b) in bytes.enumerated() {
                raw[i] = b
            }
        }
        let rc = withUnsafePointer(to: &addr) { ptr -> Int32 in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                connect(fd, sa, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        return rc == 0
    }

    private static func syncExchange(path: String, payload: Data, timeoutSeconds: Int) -> Result<Data, TailscaleRPCError> {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return .failure(.unreachable("Could not open local socket")) }
        defer { close(fd) }

        var tv = timeval(tv_sec: max(2, timeoutSeconds), tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let maxLen = MemoryLayout.size(ofValue: addr.sun_path)
        let bytes = Array(path.utf8)
        guard bytes.count < maxLen else {
            return .failure(.unreachable("Local socket path too long"))
        }
        withUnsafeMutableBytes(of: &addr.sun_path) { raw in
            raw.initializeMemory(as: UInt8.self, repeating: 0)
            for (i, b) in bytes.enumerated() {
                raw[i] = b
            }
        }

        let connected = withUnsafePointer(to: &addr) { ptr -> Int32 in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                connect(fd, sa, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else {
            return .failure(.unreachable("Local OpenAgents host daemon is not running."))
        }

        var line = payload
        if line.last != 0x0A {
            line.append(0x0A)
        }
        let sentOK = line.withUnsafeBytes { raw -> Bool in
            guard let base = raw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            var off = 0
            while off < line.count {
                let wrote = Darwin.write(fd, base.advanced(by: off), line.count - off)
                if wrote <= 0 { return false }
                off += wrote
            }
            return true
        }
        guard sentOK else {
            return .failure(.unreachable("Failed writing to local host daemon."))
        }

        var response = Data()
        var chunk = [UInt8](repeating: 0, count: 16_384)
        while response.count < 524_288 {
            let got = Darwin.read(fd, &chunk, chunk.count)
            if got <= 0 { break }
            if let nl = chunk[0..<got].firstIndex(of: 0x0A) {
                response.append(contentsOf: chunk[0..<nl])
                return .success(response)
            }
            response.append(contentsOf: chunk[0..<got])
        }
        if !response.isEmpty {
            return .success(response)
        }
        return .failure(.invalidResponse)
    }
    #endif
}
