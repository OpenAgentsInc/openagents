import Foundation

#if os(iOS)
/// Simple Bonjour browser using NetServiceBrowser for _openagents._tcp.
final class BonjourBrowser: NSObject, NetServiceBrowserDelegate, NetServiceDelegate {
    private let browser = NetServiceBrowser()
    private var services: [NetService] = []
    private var onResolved: ((String, Int) -> Void)?

    func start(onResolved: @escaping (String, Int) -> Void) {
        self.onResolved = onResolved
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
        service.resolve(withTimeout: 5.0)
    }

    func netServiceBrowser(_ browser: NetServiceBrowser, didRemove service: NetService, moreComing: Bool) {
        services.removeAll { $0 == service }
    }

    // MARK: NetServiceDelegate
    func netServiceDidResolveAddress(_ sender: NetService) {
        guard let host = sender.hostName else { return }
        onResolved?(host.replacingOccurrences(of: ".local.", with: ".local"), sender.port)
    }

    func netService(_ sender: NetService, didNotResolve errorDict: [String : NSNumber]) {
        // ignore
    }
}
#endif

