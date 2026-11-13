import Foundation

/// The check of CrashReports.swift (#13379): crash-reports-check.sh runs it.
///   crash-reports-check FIXTURE.json PROFILE.json FOLDER
/// 1. MetricKit's diagnostic (the fixture, in the shape `jsonRepresentation()` gives) makes the
///    dump the profile's first app fixture holds: the same bytes the codec's check turns into a report.
/// 2. Nothing is sent before a yes; a yes sends what waits and is remembered; a no drops it.
@main
enum CrashReportsCheck {
    static func fail(_ why: String) -> Never { FileHandle.standardError.write(("crash-reports-check: " + why + "\n").data(using: .utf8)!); exit(1) }

    final class Log: @unchecked Sendable {
        let lock = NSLock(); var sent: [String] = []; var asked = 0; var status = 200; var say = true
        func add(_ d: String) -> Int { lock.lock(); defer { lock.unlock() }; sent.append(d); return status }
    }

    static func main() {
        let a = CommandLine.arguments
        guard a.count == 4, let fixture = FileManager.default.contents(atPath: a[1]), let profile = FileManager.default.contents(atPath: a[2]) else { fail("usage: crash-reports-check FIXTURE.json PROFILE.json FOLDER") }
        guard let p = (try? JSONSerialization.jsonObject(with: profile)) as? [String: Any], let apps = p["apps"] as? [[String: Any]], let want = apps.first?["dump"] as? String else { fail("the profile has no app fixture") }
        guard let dump = CrashDump.text(diagnostic: fixture, endedMs: 1762990000000, app: "ios", exe: "OpenAgents") else { fail("the fixture made no dump") }
        if dump != want { fail("the fixture's dump is not the profile's:\n" + dump) }
        if CrashDump.text(diagnostic: Data("[1]".utf8), endedMs: 1, app: "ios", exe: "E") != nil || CrashDump.text(diagnostic: Data("no".utf8), endedMs: 1, app: "ios", exe: "E") != nil { fail("what is no diagnostic made a dump") }
        // The least a diagnostic holds, and a line that could break the dump's lines.
        let bare = CrashDump.text(diagnostic: Data(#"{"diagnosticMetaData":{"terminationReason":"a\nb\tc"}}"#.utf8), endedMs: -5, app: "mac", exe: "E")
        if bare != "appdump 1\nt 0\napp mac\nexe E\nreason a b c\n" { fail("the bare diagnostic's dump: \(bare ?? "nil")") }
        // More frames than the dump takes.
        var deep = #"{"binaryUUID":"U","offsetIntoBinaryTextSegment":1,"binaryName":"E"}"#
        for _ in 0..<200 { deep = #"{"binaryUUID":"U","offsetIntoBinaryTextSegment":1,"binaryName":"E","subFrames":["# + deep + "]}" }
        let many = CrashDump.text(diagnostic: Data((#"{"callStackTree":{"callStacks":[{"callStackRootFrames":["# + deep + "]}]}}").utf8), endedMs: 1, app: "mac", exe: "E") ?? ""
        if many.components(separatedBy: "\na ").count - 1 != CrashDump.maxFrames || many.utf8.count > CrashDump.maxBytes { fail("the frames are not capped") }

        let folder = URL(fileURLWithPath: a[3], isDirectory: true)
        let log = Log()
        let r = CrashReports(folder: folder, send: { d, done in done(log.add(d)) }, ask: { answer in log.asked += 1; if log.say { answer(true) } })
        r.flush()
        if log.asked != 0 || !log.sent.isEmpty { fail("with nothing waiting it asked or sent") }
        // The question is out and unanswered: nothing is sent, and it is asked once.
        log.say = false
        r.keep(dump); r.flush(); r.keep(dump); r.flush()
        if log.asked != 1 || !log.sent.isEmpty || r.pending.count != 2 || r.consent != nil { fail("before an answer: asked \(log.asked), sent \(log.sent.count), waiting \(r.pending.count)") }
        // A new launch, and a yes: both are sent, the answer is kept, nothing waits.
        log.say = true
        let r2 = CrashReports(folder: folder, send: { d, done in done(log.add(d)) }, ask: { answer in log.asked += 1; answer(true) })
        r2.flush()
        if log.asked != 2 || log.sent != [dump, dump] || r2.consent != "yes" || !r2.pending.isEmpty { fail("after a yes: asked \(log.asked), sent \(log.sent.count), waiting \(r2.pending.count)") }
        // Later crashes are sent with no question; one the API can't take now waits; one it refuses goes.
        r2.keep(dump); r2.flush()
        if log.asked != 2 || log.sent.count != 3 || !r2.pending.isEmpty { fail("a later crash was not sent without a question") }
        for (status, left) in [(0, 1), (429, 1), (503, 1), (400, 0)] {
            log.status = status; r2.flush()
            if left == 1 && r2.pending.isEmpty { r2.keep(dump); r2.flush() }
            if r2.pending.count != left { fail("after status \(status), \(r2.pending.count) wait") }
        }
        if FileManager.default.fileExists(atPath: folder.appendingPathComponent("pending").path) {
            for i in 0..<12 { _ = i; r2.keep(dump) }
            if r2.pending.count != CrashReports.maxPending { fail("more than \(CrashReports.maxPending) wait") }
        }
        // A no: what waits is dropped, nothing is sent, and later dumps are not kept.
        let other = folder.appendingPathComponent("no", isDirectory: true)
        let sentBefore = log.sent.count
        let r3 = CrashReports(folder: other, send: { d, done in done(log.add(d)) }, ask: { answer in answer(false) })
        r3.keep(dump); r3.flush()
        if log.sent.count != sentBefore || r3.consent != "no" || !r3.pending.isEmpty { fail("a no sent or kept a dump") }
        r3.keep(dump)
        if !r3.pending.isEmpty { fail("after a no a dump was kept") }
        print("crash-reports-check: ok")
    }
}
