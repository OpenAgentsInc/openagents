// Firm reason: 6 (URLSession: the platform's HTTP and TLS for the compiled app).
// HTTP for a compiled Coax app (GitHub #13645). The app's Coax decides what may be asked: it
// gives the policy (`admit`, the limits) and this file enforces it, performs the request with
// URLSession and answers with the status and the body. It holds no route and no rule of its own.
//
// What it guarantees:
//   - one origin. A request names a path; the URL is the origin plus that path, and a path that
//     would change the host is refused before anything is sent. A redirect is never followed: the
//     3xx answer is returned as it is;
//   - the account session is attached here (`authorize`, AppAuth). No token enters Coax;
//   - no cookies, no cache, no stored credentials, and no header the caller did not ask for
//     except Content-Type, Accept, Origin on a POST and Authorization. Nothing reaches another
//     origin;
//   - bounded: a request body over the policy's limit is refused, and an answer over its limit
//     ends the request;
//   - cancel by operation or by owner generation. A cancelled request never answers;
//   - an answer says whether the request can have reached the server: `failed` means it was
//     never sent, `unknown` means it may have been (the caller must not repeat it by itself).
// Every public call and every answer runs on the main thread.
//   apps/shared/native/app-services-check.sh
import Foundation

final class AppHTTP: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    /// What the app's Coax allows. `admit` is called on the main thread before a request is built.
    struct Policy {
        var origin: String
        var admit: (_ method: String, _ path: String, _ bodyBytes: Int) -> Bool
        var maxResponse = 1 << 20
        var timeout: TimeInterval = 60
    }
    struct Request {
        var method = "GET"
        var path = ""
        var body = Data()
        var contentType = "application/json"
        /// Attach the account session when there is one.
        var account = true
        /// Answer each part of the body as it arrives (`part`), then the end.
        var stream = false
        /// This request's own response limit in bytes and timeout in milliseconds, each at most
        /// the policy's; 0 takes the policy's.
        var limit = 0
        var timeoutMs = 0
    }
    /// Why a request did not answer.
    enum Reason: Int32 { case refused = 1, login = 2, offline = 3, tooLarge = 4, lost = 5 }
    enum Outcome: Equatable {
        /// The whole answer (in a stream: the status, with the bytes not yet given as parts).
        case answer(status: Int, body: Data, contentType: String)
        /// A part of a streamed body, in order.
        case part(Data)
        /// Never sent.
        case failed(Reason)
        /// Sent, or perhaps sent; the answer did not arrive whole.
        case unknown(Reason)
        var final: Bool { if case .part = self { return false }; return true }
    }
    /// Why the account session could not be attached: renewing it did not reach the server, or
    /// the login ended.
    enum Blocked: Error { case offline, login }
    /// The request with the account session, nil when nobody is logged in. Throws `Blocked`.
    typealias Authorize = @Sendable (URLRequest) async throws -> URLRequest?

    private let policy: Policy
    private let authorize: Authorize
    private var session: URLSession!
    private struct Entry { var generation: UInt64; var task: URLSessionTask?; var stream: Bool; var limit: Int; var body = Data(); var received = 0; var status = 0; var type = ""; var over = false; var answer: (Outcome) -> Void }
    /// Main thread only.
    private var entries: [UInt64: Entry] = [:]
    private var ops: [Int: UInt64] = [:]

    /// `protocols`: URLProtocol classes that stand in for the network in the checks.
    init(policy: Policy, authorize: @escaping Authorize, protocols: [AnyClass] = []) {
        self.policy = policy; self.authorize = authorize
        super.init()
        let c = URLSessionConfiguration.ephemeral
        c.httpCookieStorage = nil; c.httpShouldSetCookies = false; c.httpCookieAcceptPolicy = .never
        c.urlCache = nil; c.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        c.urlCredentialStorage = nil
        c.timeoutIntervalForRequest = policy.timeout
        c.waitsForConnectivity = false
        if !protocols.isEmpty { c.protocolClasses = protocols }
        session = URLSession(configuration: c, delegate: self, delegateQueue: .main)
    }
    deinit { session?.invalidateAndCancel() }

    /// How many requests have not answered.
    var active: Int { entries.count }

    /// The URL a path names, or nil when it would leave the origin.
    static func url(origin: String, path: String) -> URL? {
        guard path.hasPrefix("/"), !path.hasPrefix("//"), !path.contains("\\"), let base = URL(string: origin), let host = base.host, !host.isEmpty,
              let url = URL(string: origin + path), url.host == host, url.scheme == base.scheme, url.port == base.port, url.user == nil, url.password == nil else { return nil }
        return url
    }

    /// Starts a request. `answer` runs later on the main thread, never inside this call: parts in
    /// order when the request streams, then one final outcome. `op` names the request for
    /// `cancel`; a second request with a live op is refused.
    func start(op: UInt64, generation: UInt64, _ r: Request, answer: @escaping (Outcome) -> Void) {
        let later: (Outcome) -> Void = { o in DispatchQueue.main.async { answer(o) } }
        guard entries[op] == nil, ["GET", "POST"].contains(r.method), policy.admit(r.method, r.path, r.body.count),
              let url = AppHTTP.url(origin: policy.origin, path: r.path) else { later(.failed(.refused)); return }
        let timeout = r.timeoutMs > 0 ? min(policy.timeout, TimeInterval(r.timeoutMs) / 1000) : policy.timeout
        let limit = r.limit > 0 ? min(policy.maxResponse, r.limit) : policy.maxResponse
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: timeout)
        request.httpMethod = r.method
        request.httpShouldHandleCookies = false
        request.setValue(r.stream ? "text/event-stream" : "application/json", forHTTPHeaderField: "Accept")
        if r.method == "POST" {
            request.httpBody = r.body
            request.setValue(r.contentType, forHTTPHeaderField: "Content-Type")
            request.setValue(policy.origin, forHTTPHeaderField: "Origin")
        }
        entries[op] = Entry(generation: generation, task: nil, stream: r.stream, limit: limit, answer: answer)
        let plain = request, wants = r.account, authorize = self.authorize
        Task { @MainActor [weak self] in
            var ready = plain
            if wants {
                do { if let with = try await authorize(plain) { ready = with } }
                catch {
                    // The login ended, or renewing it did not reach the server. Nothing was sent.
                    guard let self, let e = self.entries.removeValue(forKey: op), e.generation == generation else { return }
                    e.answer(.failed((error as? Blocked) == .offline ? .offline : .login)); return
                }
            }
            guard let self, var e = self.entries[op], e.generation == generation, e.task == nil else { return }
            // The session may only go to the origin: an authorizer cannot move the request.
            guard ready.url == plain.url else { self.entries[op] = nil; e.answer(.failed(.refused)); return }
            let task = self.session.dataTask(with: ready)
            e.task = task; self.entries[op] = e; self.ops[task.taskIdentifier] = op
            task.resume()
        }
    }

    /// Ends one request. It never answers.
    func cancel(op: UInt64) {
        guard let e = entries.removeValue(forKey: op) else { return }
        if let t = e.task { ops[t.taskIdentifier] = nil; t.cancel() }
    }
    /// Ends every request of an owner generation (0: every request).
    func cancel(generation: UInt64) {
        for (op, e) in entries where generation == 0 || e.generation == generation { _ = e; cancel(op: op) }
    }

    // MARK: URLSession (its delegate queue is the main queue)

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard let op = ops[dataTask.taskIdentifier], var e = entries[op] else { completionHandler(.cancel); return }
        e.status = (response as? HTTPURLResponse)?.statusCode ?? 0; e.type = response.mimeType ?? ""
        entries[op] = e
        completionHandler(.allow)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard let op = ops[dataTask.taskIdentifier], var e = entries[op], !e.over else { return }
        e.received += data.count
        if e.received > e.limit { e.over = true; e.body = Data(); entries[op] = e; dataTask.cancel(); return }
        // A streamed 2xx body is given as it arrives; an error's body is kept for the answer.
        if e.stream, (200..<300).contains(e.status) { entries[op] = e; e.answer(.part(data)); return }
        e.body.append(data); entries[op] = e
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let op = ops.removeValue(forKey: task.taskIdentifier), let e = entries.removeValue(forKey: op) else { return }
        if e.over { e.answer(.unknown(.tooLarge)); return }
        if let error {
            e.answer(AppHTTP.neverSent(error) ? .failed(.offline) : .unknown(.lost)); return
        }
        e.answer(.answer(status: e.status, body: e.body, contentType: e.type))
    }

    /// Whether a transport error means the request cannot have reached the server.
    static func neverSent(_ error: Error) -> Bool {
        guard let u = error as? URLError else { return false }
        switch u.code {
        case .notConnectedToInternet, .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed, .secureConnectionFailed, .serverCertificateUntrusted,
             .serverCertificateHasBadDate, .serverCertificateNotYetValid, .serverCertificateHasUnknownRoot, .clientCertificateRejected, .clientCertificateRequired,
             .appTransportSecurityRequiresSecureConnection, .dataNotAllowed, .internationalRoamingOff, .unsupportedURL, .badURL: return true
        default: return false
        }
    }
}
