import Foundation
import UIKit

/// The app's own events, sent the way every openagents.com page sends its
/// (apps/openagents-web/analytics.coax): batches of {"sid", "events"} to
/// /api/analytics/events, page "app". The web pages in the app send their
/// own events; the server knows them as the app's by the User-Agent
/// ("OpenAgentsApp/<build>"). No words, no names: ids the app chose.
/// A batch goes at 20 events, 5 s after its first event, or when the app
/// leaves the screen. Tests and automated runs mark their batches ("auto"),
/// so they are never counted as usage.
final class AppAnalytics {
    static let shared = AppAnalytics()
    static let url = URL(string: "https://openagents.com/api/analytics/events")!

    private let queue = DispatchQueue(label: "openagents.analytics")
    private let sid = "a" + AppAnalytics.hex(8)
    private var events: [[String: Any]] = []
    private var timer: DispatchWorkItem?
    private var activeSince = Date()
    private var once: Set<String> = []   // what counts once each time the app is on screen (the gyro)
    private let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "0"
    private let automated: String? = {
        let env = ProcessInfo.processInfo.environment
        if env["XCTestConfigurationFilePath"] != nil || env["XCTestBundlePath"] != nil { return "xctest" }
        if env["OA_AUTOMATION"] != nil || ProcessInfo.processInfo.arguments.contains("-OAAutomation") { return "xctest" }
        if ProcessInfo.processInfo.arguments.contains(where: { $0.hasPrefix("-OA") }) { return "xctest" }   // UI tests launch with -OA… settings
        return nil
    }()

    var userAgent: String { "Mozilla/5.0 (\(UIDevice.current.model); CPU OS \(UIDevice.current.systemVersion.replacingOccurrences(of: ".", with: "_")) like Mac OS X) OpenAgentsApp/\(build)" }

    private init() {
        let center = NotificationCenter.default
        center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in self?.background() }
        center.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { [weak self] _ in self?.foreground() }
    }

    /// The anonymous device id: random, kept on this phone, nothing else in it.
    static var pid: String {
        let key = "OAAnalyticsPid"
        if let p = UserDefaults.standard.string(forKey: key), p.count == 16 { return p }
        let p = hex(8); UserDefaults.standard.set(p, forKey: key); return p
    }
    private static func hex(_ bytes: Int) -> String {
        (0..<bytes).map { _ in String(format: "%02x", UInt8.random(in: 0...255)) }.joined()
    }

    // MARK: the events

    func launch() {
        activeSince = Date()
        track("app-launch", ["pid": Self.pid, "build": build, "os": "iOS " + UIDevice.current.systemVersion,
                             "device": UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone", "cold": true])
    }
    func gridFallback(_ why: String) { track("grid-fallback", ["id": why]) }
    func loginOpen() { track("login-open") }
    func loginApproved(_ kind: String) { track("login-approved", ["id": kind]) }
    func walletOpen() { track("wallet-open") }
    func voiceOn() { track("voice-on") }
    /// The phone's turn moved the camera: once each time the app is on screen.
    func gyro(_ what: String) {
        queue.async { [self] in
            guard !once.contains("gyro") else { return }
            once.insert("gyro")
            add("gyro", ["id": what])
        }
    }
    func worldEnter(_ world: String) { track("world-enter", ["world": world]) }
    func fail(_ what: String, _ status: Int = 0) { track("fail", ["id": what, "n": status]) }

    func track(_ ev: String, _ props: [String: Any] = [:]) { queue.async { [self] in add(ev, props) } }

    // MARK: batching (on the queue)

    private func add(_ ev: String, _ props: [String: Any]) {
        var p: [String: Any] = [:]
        for (k, v) in props where p.count < 15 && Self.goodKey(k) {
            if let s = v as? String { p[k] = String(s.prefix(80)) } else { p[k] = v }
        }
        events.append(["ev": ev, "page": "app", "ct": Int64(Date().timeIntervalSince1970 * 1000), "p": p])
        if events.count >= 20 { send() }
        else if timer == nil {
            let t = DispatchWorkItem { [weak self] in self?.send() }
            timer = t; queue.asyncAfter(deadline: .now() + 5, execute: t)
        }
    }
    private static func goodKey(_ k: String) -> Bool {
        guard let f = k.unicodeScalars.first, k.count <= 24, CharacterSet.letters.contains(f), f.isASCII else { return false }
        return k.unicodeScalars.allSatisfy { $0.isASCII && (CharacterSet.alphanumerics.contains($0) || $0 == "_") }
    }
    private func send(done: (() -> Void)? = nil) {
        timer?.cancel(); timer = nil
        guard !events.isEmpty else { done?(); return }
        var batch: [String: Any] = ["sid": sid, "events": events]
        if let automated { batch["auto"] = automated }
        events = []
        guard let body = try? JSONSerialization.data(withJSONObject: batch) else { done?(); return }
        var request = URLRequest(url: Self.url)
        request.httpMethod = "POST"
        request.httpBody = body
        request.setValue("text/plain", forHTTPHeaderField: "Content-Type")
        request.setValue(userAgent, forHTTPHeaderField: "User-Agent")
        if let automated { request.setValue(automated, forHTTPHeaderField: "X-Verse-Automation") }
        URLSession.shared.dataTask(with: request) { _, _, _ in done?() }.resume()
    }

    private func background() {
        let seconds = Int(Date().timeIntervalSince(activeSince))
        var task = UIBackgroundTaskIdentifier.invalid
        task = UIApplication.shared.beginBackgroundTask { UIApplication.shared.endBackgroundTask(task) }
        queue.async { [self] in
            add("app-background", ["s": seconds])
            send { DispatchQueue.main.async { UIApplication.shared.endBackgroundTask(task) } }
        }
    }
    private func foreground() {
        activeSince = Date()
        queue.async { [self] in once.removeAll(); add("app-foreground", [:]) }
    }
}
