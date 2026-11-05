// Firm reason: 6 (URLSession tasks and FIFO delivery; Coax owns all admission).
import Foundation

@MainActor
final class ForgeMetalNetwork: NSObject, @preconcurrency URLSessionDataDelegate, @preconcurrency URLSessionWebSocketDelegate {
    struct Owner: Equatable { let id, generation, engine: Int64 }
    final class Request {
        let owner: Owner
        let task: URLSessionDataTask
        let capacity: Int
        var bytes = Data()
        var failed = false
        init(_ owner: Owner, _ task: URLSessionDataTask, _ capacity: Int) { self.owner = owner; self.task = task; self.capacity = capacity }
    }
    final class Socket {
        let owner: Owner
        let task: URLSessionWebSocketTask
        var receiver: Task<Void, Never>?
        init(_ owner: Owner, _ task: URLSessionWebSocketTask) { self.owner = owner; self.task = task }
    }
    var host: UnsafeMutableRawPointer?
    var session: URLSession!
    var requests: [Int: Request] = [:]
    var sockets: [Int: Socket] = [:]
    var delivered = 0
    var sent = 0
    var changed: (() -> Void)?
    var frame: (() async -> String?)?
    var playerKey = ""
    init(host: UnsafeMutableRawPointer) {
        self.host = host
        super.init()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = true
        session = URLSession(configuration: configuration, delegate: self, delegateQueue: .main)
    }
    func owner(_ effect: [String: Any]) -> Owner {
        func n(_ key: String) -> Int64 { (effect[key] as? NSNumber)?.int64Value ?? 0 }
        return Owner(id: n("id"), generation: n("generation"), engine: n("engine"))
    }
    func receive(_ socket: Socket) {
        socket.receiver = Task { [weak self, weak socket] in
            guard let socket else { return }
            while !Task.isCancelled {
                do {
                    let message = try await socket.task.receive()
                    guard let self, let host = self.host, sockets[socket.task.taskIdentifier] === socket else { return }
                    let bytes: Data
                    switch message { case .string(let text): bytes = Data(text.utf8); case .data: continue; @unknown default: continue }
                    bytes.withUnsafeBytes { mf_socket_event(host, socket.owner.id, socket.owner.generation, socket.owner.engine, 1, $0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
                    delivered += 1; changed?()
                } catch {
                    guard let self, let host = self.host, sockets[socket.task.taskIdentifier] === socket else { return }
                    mf_socket_event(host, socket.owner.id, socket.owner.generation, socket.owner.engine, 3, nil, 0)
                    mf_socket_event(host, socket.owner.id, socket.owner.generation, socket.owner.engine, 2, nil, 0)
                    sockets.removeValue(forKey: socket.task.taskIdentifier); changed?(); return
                }
            }
        }
    }
    func execute(_ effect: [String: Any]) {
        guard let host else { return }
        let o = owner(effect)
        switch effect["kind"] as? String {
        case "socket-open":
            guard let text = effect["url"] as? String, let url = URL(string: text) else { return }
            let task = session.webSocketTask(with: url), socket = Socket(o, task)
            sockets[task.taskIdentifier] = socket; task.resume(); receive(socket)
        case "socket-send":
            guard let socket = sockets.values.first(where: { $0.owner == o }), let text = effect["body"] as? String else { return }
            sent += 1
            Task { [weak self, weak socket] in
                guard let socket else { return }
                do { try await socket.task.send(.string(text)) }
                catch {
                    guard let self, let host = self.host, sockets[socket.task.taskIdentifier] === socket else { return }
                    mf_socket_event(host, o.id, o.generation, o.engine, 3, nil, 0); changed?()
                }
            }
        case "socket-close":
            if let socket = sockets.values.first(where: { $0.owner == o }) {
                socket.receiver?.cancel(); socket.task.cancel(with: .normalClosure, reason: nil); sockets.removeValue(forKey: socket.task.taskIdentifier)
            }
            // The import has returned before this FIFO terminal event runs.
            Task { [weak self] in guard let self, let host = self.host else { return }; mf_socket_event(host, o.id, o.generation, o.engine, 2, nil, 0); changed?() }
        case "http":
            let flags = (effect["flags"] as? NSNumber)?.intValue ?? 0
            let timeout = (effect["timeout"] as? NSNumber)?.int64Value ?? 0
            if effect["bounded"] as? Bool == true, o.id >= 0 {
                Task { [weak self] in
                    try? await Task.sleep(for:.milliseconds(timeout))
                    guard let self, let host = self.host else { return }
                    mf_http_timeout(host,o.id,o.generation,o.engine)
                    for (id,r) in requests where r.owner == o { r.task.cancel(); requests.removeValue(forKey:id) }
                    changed?()
                }
            }
            Task { [weak self] in
                guard let self else { return }
                let image = flags & 32 != 0 ? await frame?() : nil
                guard let current = self.host, current == host else { return }
                if o.id >= 0 && !mf_http_chunk(current,o.id,o.generation,o.engine,0) { return }
                beginHTTP(effect,image:image)
            }
        case "http-cancel":
            for (id, r) in requests where r.owner.id == o.id { r.task.cancel(); requests.removeValue(forKey: id) }
        default: break
        }
    }
    func beginHTTP(_ effect: [String:Any], image: String?) {
            guard let host else { return }
            let o = owner(effect)
            guard let text = effect["url"] as? String, let url = URL(string: text) else { return }
            let flags = (effect["flags"] as? NSNumber)?.intValue ?? 0, bounded = effect["bounded"] as? Bool ?? false
            var request = URLRequest(url: url)
            request.cachePolicy = .reloadIgnoringLocalCacheData
            request.httpMethod = flags & 1 != 0 ? "POST" : "GET"
            var body = effect["body"] as? String ?? ""
            if flags & 16 != 0, let newline = body.firstIndex(of: "\n") {
                request.setValue(String(body[..<newline]), forHTTPHeaderField: "Authorization")
                body = String(body[body.index(after: newline)...])
            }
            if flags & 8 != 0, !playerKey.isEmpty { request.setValue(playerKey, forHTTPHeaderField: "X-Verse-Player") }
            if flags & 32 != 0, flags & 1 != 0, !body.isEmpty, let image, !image.isEmpty {
                let input = Data(body.utf8), jpeg = Data(image.utf8)
                var output = Data(count:input.count+jpeg.count+64)
                let count = output.withUnsafeMutableBytes { out in input.withUnsafeBytes { bytes in jpeg.withUnsafeBytes { shot in
                    mf_http_image(bytes.bindMemory(to:UInt8.self).baseAddress,bytes.count,shot.bindMemory(to:UInt8.self).baseAddress,shot.count,out.bindMemory(to:UInt8.self).baseAddress,out.count)
                } } }
                guard count >= 0 else { mf_http_finish(host,o.id,o.generation,o.engine,nil,0,false,true); changed?(); return }
                body = String(decoding:output.prefix(Int(count)),as:UTF8.self)
            }
            if flags & 1 != 0 { request.httpBody = Data(body.utf8); request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
            let timeout = (effect["timeout"] as? NSNumber)?.doubleValue ?? 0
            request.timeoutInterval = bounded ? max(0.001, timeout / 1000) : 35
            let task = session.dataTask(with: request)
            if o.id >= 0 { requests[task.taskIdentifier] = Request(o, task, (effect["capacity"] as? NSNumber)?.intValue ?? 0) }
            task.resume()
    }
    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        guard let host, let socket = sockets[webSocketTask.taskIdentifier] else { return }
        let o = socket.owner; mf_socket_event(host, o.id, o.generation, o.engine, 0, nil, 0); changed?()
    }
    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        guard let host, let socket = sockets.removeValue(forKey: webSocketTask.taskIdentifier) else { return }
        let o = socket.owner; socket.receiver?.cancel(); mf_socket_event(host, o.id, o.generation, o.engine, 2, nil, 0); changed?()
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard let host, let r = requests[dataTask.taskIdentifier] else { completionHandler(.allow); return }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0, prefix = Data("\(status)\n".utf8)
        guard mf_http_chunk(host, r.owner.id, r.owner.generation, r.owner.engine, Int64(prefix.count)) else { r.failed = true; completionHandler(.cancel); return }
        r.bytes = prefix; completionHandler(.allow)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard let host, let r = requests[dataTask.taskIdentifier] else { return }
        guard mf_http_chunk(host, r.owner.id, r.owner.generation, r.owner.engine, Int64(data.count)) else { r.failed = true; dataTask.cancel(); return }
        // Ordinary responses preserve the browser's inbox-sized prefix.
        let remaining = max(0, r.capacity - r.bytes.count)
        r.bytes.append(data.prefix(remaining))
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let host, let r = requests.removeValue(forKey: task.taskIdentifier) else { return }
        r.bytes.withUnsafeBytes { mf_http_finish(host, r.owner.id, r.owner.generation, r.owner.engine, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, error == nil && !r.failed, String(data: r.bytes, encoding: .utf8) != nil) }
        changed?()
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        let a = task.originalRequest?.url, b = request.url
        let same = a?.scheme == b?.scheme && a?.host?.lowercased() == b?.host?.lowercased() && a?.port == b?.port
        completionHandler(same ? request:nil)
    }
    func close() {
        host = nil
        for socket in sockets.values { socket.receiver?.cancel(); socket.task.cancel(with: .goingAway, reason: nil) }
        for r in requests.values { r.task.cancel() }
        sockets.removeAll(); requests.removeAll(); session.invalidateAndCancel()
    }
}
