import Foundation
#if canImport(MetricKit)
import MetricKit
#endif
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// Native crash logs (#13379, docs/deployment/crash-reports.md), one file for the iPhone and Mac apps.
///
/// The system keeps a diagnostic of each crash and hands it to the app on a later launch (MetricKit).
/// This file writes that diagnostic as a dump, the lines `appdump 1` describes in
/// `proto/verse/crash-report-v3.json`, and posts it to `POST /api/playtest/app-crash`. The API makes
/// the crash report from it (`coax/web/room/room-crash.coax`, `crash_app_report`): what a report is,
/// its signature and its bucket are decided there, not here.
///
/// Nothing leaves the device before the person says yes. The first dump raises one question; the
/// answer is kept, and a no also drops the dumps. Dumps wait in `folder/pending` (8 at most), and
/// the answer is the file `folder/consent`.
///
/// A dump holds the app's version and build, the system's version, the signal, the termination
/// reason, and the crashed thread's frames as a binary's name, its UUID and an offset. It holds no
/// account, no device name and nothing typed or said.
enum CrashDump {
    static let maxFrames = 64
    static let maxBytes = 16000   // the route takes 16 KB (rooms.c APP_CRASH_MAX)

    /// The dump of one crash diagnostic (`MXCrashDiagnostic.jsonRepresentation()`), or nil when the
    /// JSON is not one. endedMs: the end of the day the diagnostic covers. exe: the app's own binary.
    static func text(diagnostic json: Data, endedMs: Int64, app: String, exe: String) -> String? {
        guard let root = (try? JSONSerialization.jsonObject(with: json)) as? [String: Any] else { return nil }
        let meta = root["diagnosticMetaData"] as? [String: Any] ?? [:]
        var out = "appdump 1\nt \(max(endedMs, 0))\napp \(app)\n"
        func add(_ key: String, _ value: Any?) {
            let text: String
            if let s = value as? String { text = line(s) } else if let n = value as? NSNumber { text = n.stringValue } else { return }
            if !text.isEmpty { out += "\(key) \(text)\n" }
        }
        add("os", meta["osVersion"]); add("version", meta["appVersion"]); add("buildno", meta["appBuildVersion"])
        add("exe", exe)
        add("signal", meta["signal"]); add("exception", meta["exceptionType"]); add("reason", meta["terminationReason"])
        // The crashed thread: the stack marked threadAttributed, else the first. Its root frame is the
        // innermost, and each frame's first subFrame is its caller.
        let stacks = (root["callStackTree"] as? [String: Any])?["callStacks"] as? [[String: Any]] ?? []
        let stack = stacks.first { ($0["threadAttributed"] as? Bool) == true } ?? stacks.first
        var frame = (stack?["callStackRootFrames"] as? [[String: Any]])?.first
        var count = 0
        while let f = frame, count < maxFrames {
            if let uuid = f["binaryUUID"] as? String, let offset = (f["offsetIntoBinaryTextSegment"] as? NSNumber)?.int64Value, offset >= 0 {
                let name = line(f["binaryName"] as? String ?? "")
                let a = "a \(line(uuid).replacingOccurrences(of: " ", with: "")) \(offset) \(name.isEmpty ? "?" : name)\n"
                if out.utf8.count + a.utf8.count > maxBytes { break }
                out += a; count += 1
            }
            frame = (f["subFrames"] as? [[String: Any]])?.first
        }
        return out
    }
    /// One line of printable ASCII, 200 bytes at most: anything else is a space.
    static func line(_ s: String) -> String {
        let bytes = s.utf8.prefix(200).map { $0 >= 32 && $0 <= 126 ? $0 : UInt8(32) }
        return String(decoding: bytes, as: UTF8.self).trimmingCharacters(in: .whitespaces)
    }
}

final class CrashReports: NSObject, @unchecked Sendable {
    /// Posts one dump; answers the HTTP status, or 0 when the API was not reached.
    typealias Send = @Sendable (_ dump: String, _ done: @escaping @Sendable (Int) -> Void) -> Void
    /// Asks the question, on the main thread; answers yes or no.
    typealias Ask = @Sendable (_ answer: @escaping @Sendable (Bool) -> Void) -> Void

    static let maxPending = 8
    private let folder: URL
    private let send: Send
    private let ask: Ask
    private let lock = NSLock()
    private var asking = false

    init(folder: URL, send: @escaping Send, ask: @escaping Ask) {
        self.folder = folder; self.send = send; self.ask = ask
    }

    private var pendingFolder: URL { folder.appendingPathComponent("pending", isDirectory: true) }
    private var consentFile: URL { folder.appendingPathComponent("consent") }
    /// "yes", "no", or nil while the person has not been asked.
    var consent: String? {
        guard let s = try? String(contentsOf: consentFile, encoding: .utf8) else { return nil }
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        return t == "yes" || t == "no" ? t : nil
    }
    var pending: [URL] {
        ((try? FileManager.default.contentsOfDirectory(at: pendingFolder, includingPropertiesForKeys: nil)) ?? [])
            .filter { $0.pathExtension == "txt" }.sorted { $0.lastPathComponent < $1.lastPathComponent }
    }

    /// Keeps a dump until it may be sent. After a no, and past 8 waiting, it is dropped.
    func keep(_ dump: String) {
        lock.lock(); defer { lock.unlock() }
        guard consent != "no", pending.count < CrashReports.maxPending else { return }
        try? FileManager.default.createDirectory(at: pendingFolder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let name = "\(Int64(Date().timeIntervalSince1970 * 1000))-\(UUID().uuidString.prefix(8)).txt"
        try? dump.data(using: .utf8)?.write(to: pendingFolder.appendingPathComponent(name), options: .atomic)
    }

    /// Sends what waits when the answer is yes, drops it when it is no, and asks once when there is
    /// none yet. A dump leaves the folder once the API answered it (kept, or refused as no dump);
    /// one the API could not take now (no network, 429, a 5xx) waits for the next launch.
    func flush() {
        lock.lock()
        let waiting = pending, said = consent
        if waiting.isEmpty { lock.unlock(); return }
        if said == "no" { for f in waiting { try? FileManager.default.removeItem(at: f) }; lock.unlock(); return }
        if said == nil {
            let first = !asking; asking = true
            lock.unlock()
            if first {
                ask { [self] yes in
                    lock.lock()
                    try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
                    try? (yes ? "yes\n" : "no\n").data(using: .utf8)?.write(to: consentFile, options: .atomic)
                    asking = false
                    lock.unlock()
                    flush()
                }
            }
            return
        }
        lock.unlock()
        for f in waiting {
            guard let dump = try? String(contentsOf: f, encoding: .utf8) else { try? FileManager.default.removeItem(at: f); continue }
            send(dump) { status in
                if status != 0 && status != 429 && status < 500 { try? FileManager.default.removeItem(at: f) }
            }
        }
    }

    /// The API's route.
    static func network(origin: String) -> Send {
        return { dump, done in
            guard let url = URL(string: origin + "/api/playtest/app-crash") else { done(0); return }
            var q = URLRequest(url: url, timeoutInterval: 30)
            q.httpMethod = "POST"
            q.setValue("text/plain", forHTTPHeaderField: "Content-Type")
            q.httpBody = dump.data(using: .utf8)
            URLSession.shared.dataTask(with: q) { _, answer, _ in done((answer as? HTTPURLResponse)?.statusCode ?? 0) }.resume()
        }
    }

    static let question = "OpenAgents quit unexpectedly"
    static let detail = "Send us the crash report? It holds the app's version, your system's version and where the app stopped. It holds nothing you typed or said. We remember your answer."
}

#if canImport(MetricKit)
extension CrashReports: MXMetricManagerSubscriber {
    /// The one reporter of a running app (set by `start`).
    nonisolated(unsafe) private static var live: CrashReports?
    nonisolated(unsafe) private static var liveApp = ""

    /// Starts listening for the system's crash diagnostics and sends what already waits.
    /// app: "ios" or "mac". folder: where the answer and the waiting dumps are kept.
    static func start(app: String, folder: URL, origin: String) {
        guard live == nil else { return }
        let r = CrashReports(folder: folder, send: network(origin: origin), ask: { answer in DispatchQueue.main.async { present(answer) } })
        live = r; liveApp = app
        MXMetricManager.shared.add(r)
        r.flush()
    }

    func didReceive(_ payloads: [MXDiagnosticPayload]) {
        let exe = Bundle.main.object(forInfoDictionaryKey: "CFBundleExecutable") as? String ?? ProcessInfo.processInfo.processName
        for p in payloads {
            let ended = Int64(p.timeStampEnd.timeIntervalSince1970 * 1000)
            for c in p.crashDiagnostics ?? [] {
                if let dump = CrashDump.text(diagnostic: c.jsonRepresentation(), endedMs: ended, app: CrashReports.liveApp, exe: exe) { keep(dump) }
            }
        }
        flush()
    }
    #if os(iOS)
    func didReceive(_ payloads: [MXMetricPayload]) {}
    #endif

    /// The question, over whatever is up.
    @MainActor private static func present(_ answer: @escaping @Sendable (Bool) -> Void) {
        #if os(iOS)
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        guard var top = scene?.windows.first(where: { $0.isKeyWindow })?.rootViewController ?? scene?.windows.first?.rootViewController else {
            // No window yet (the diagnostic came before the first screen): ask once it is up.
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { present(answer) }
            return
        }
        while let shown = top.presentedViewController { top = shown }
        let alert = UIAlertController(title: question, message: detail, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Don't Send", style: .cancel) { _ in answer(false) })
        alert.addAction(UIAlertAction(title: "Send", style: .default) { _ in answer(true) })
        top.present(alert, animated: true)
        #elseif os(macOS)
        let alert = NSAlert()
        alert.messageText = question
        alert.informativeText = detail
        alert.addButton(withTitle: "Send")
        alert.addButton(withTitle: "Don't Send")
        answer(alert.runModal() == .alertFirstButtonReturn)
        #endif
    }
}

#if os(macOS)
/// main.c's call (account-mac.h): the Mac app's crash logs, kept under `~/.openagents/desktop/crash`.
@_cdecl("oa_crash_reports_start")
func oa_crash_reports_start() {
    let folder = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".openagents", isDirectory: true).appendingPathComponent("desktop", isDirectory: true).appendingPathComponent("crash", isDirectory: true)
    CrashReports.start(app: "mac", folder: folder, origin: "https://openagents.com")
}
#endif
#endif
