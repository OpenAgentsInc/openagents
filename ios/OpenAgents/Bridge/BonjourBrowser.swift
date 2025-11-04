import Foundation

#if os(iOS)
/// Simple Bonjour browser using NetServiceBrowser for _openagents._tcp.
final class BonjourBrowser: NSObject, NetServiceBrowserDelegate, NetServiceDelegate {
    private let browser = NetServiceBrowser()
    private var services: [NetService] = []
    private var onResolved: ((String, Int) -> Void)?
    private var onLog: ((String) -> Void)?

    func start(onResolved: @escaping (String, Int) -> Void, onLog: ((String) -> Void)? = nil) {
        self.onResolved = onResolved
        self.onLog = onLog
        browser.delegate = self
        browser.searchForServices(ofType: "_openagents._tcp.", inDomain: "local.")
    }

    func stop() {
        browser.stop()
        services.removeAll()
        onResolved = nil
    }

    // MARK: NetServiceBrowserDelegate
    func netServiceBrowser(_ browser: NetServiceBrowser, didFind service: NetService, moreComing: Bool) {
        services.append(service)
        service.delegate = self
        onLog?("found service name=\(service.name)")
        service.resolve(withTimeout: 5.0)
    }

    func netServiceBrowser(_ browser: NetServiceBrowser, didRemove service: NetService, moreComing: Bool) {
        services.removeAll { $0 == service }
    }

    // MARK: NetServiceDelegate
    func netServiceDidResolveAddress(_ sender: NetService) {
        // Prefer IPv4 address if available
        if let addr = sender.addresses?.compactMap({ BonjourBrowser.ipString(from: $0) }).first {
            onLog?("resolved host=\(addr) port=\(sender.port)")
            onResolved?(addr, sender.port)
            return
        }
        if let host = sender.hostName {
            let h = host.replacingOccurrences(of: ".local.", with: ".local")
            onLog?("resolved hostName=\(h) port=\(sender.port)")
            onResolved?(h, sender.port)
        }
    }

    func netService(_ sender: NetService, didNotResolve errorDict: [String : NSNumber]) {
        onLog?("resolve failed: \(errorDict)")
    }

    // MARK: Utilities
    private static func ipString(from data: Data) -> String? {
        return data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> String? in
            let family = raw.load(as: sa_family_t.self)
            if family == sa_family_t(AF_INET) {
                var addr = raw.bindMemory(to: sockaddr_in.self)[0]
                var buffer = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
                let p = withUnsafePointer(to: &addr.sin_addr) {
                    inet_ntop(AF_INET, $0, &buffer, socklen_t(INET_ADDRSTRLEN))
                }
                if p != nil { return String(cString: buffer) }
            } else if family == sa_family_t(AF_INET6) {
                var addr = raw.bindMemory(to: sockaddr_in6.self)[0]
                var buffer = [CChar](repeating: 0, count: Int(INET6_ADDRSTRLEN))
                let p = withUnsafePointer(to: &addr.sin6_addr) {
                    inet_ntop(AF_INET6, $0, &buffer, socklen_t(INET6_ADDRSTRLEN))
                }
                if p != nil { return String(cString: buffer) }
            }
            return nil
        }
    }
}
#endif
