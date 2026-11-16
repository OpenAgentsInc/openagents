// Firm reason: 6 (controlled macOS process/filesystem SDK tests; no real environment is contacted).
#if os(macOS)
import XCTest
import Foundation

@MainActor private final class MacPeerFixture: AppDeviceService {
    let key = String(repeating: "b", count: 64)
    var calls = 0
    func list() -> String { key + "\tRemote fixture\n" }
    func pair(_ link: String, done: @escaping (Bool, String) -> Void) { done(true, "fixture") }
    func call(host: String, body: Data, done: @escaping (AppCompletion.State, Data) -> Void) { calls += 1; done(.done, Data("remote fixture".utf8)) }
}
@MainActor private final class MacProcessFixture {
    let key = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    var commands: [AppMacProcess.Command] = []
    var request: Data?, requestURL: URL?
    var started = true, status: Int32 = 0
    func run(_ command: AppMacProcess.Command) async -> AppMacProcess.Result {
        commands.append(command)
        if command.arguments == ["key"] { return .init(started: true, status: 0, bytes: Data((key + "\npublic display\n").utf8)) }
        guard command.arguments.count == 3, command.arguments[0] == "local-rpc", command.arguments[1] == "--request-file" else { return .init() }
        let file = URL(fileURLWithPath: command.arguments[2]); requestURL = file; request = try? Data(contentsOf: file)
        XCTAssertEqual((try? FileManager.default.attributesOfItem(atPath: file.path)[.posixPermissions] as? NSNumber)?.intValue, 0o600)
        return .init(started: started, status: status, bytes: Data("opaque SDK reply\n".utf8))
    }
}
@MainActor final class AppMacDeviceTests: XCTestCase {
    func testProcessBoundsDeadlineAndStartFailure() async {
        let printCommand = AppMacProcess.Command(executable: URL(fileURLWithPath: "/usr/bin/printf"), arguments: ["%s", "fixture"], limit: 64, timeoutMs: 2000)
        let printed = await AppMacProcess.run(printCommand)
        XCTAssertTrue(printed.ok); XCTAssertEqual(printed.bytes, Data("fixture".utf8))
        let cut = await AppMacProcess.run(.init(executable: printCommand.executable, arguments: ["%s", String(repeating: "x", count: 128)], limit: 8, timeoutMs: 2000))
        XCTAssertTrue(cut.started && cut.cut && !cut.ok && cut.bytes.count <= 8)
        let timed = await AppMacProcess.run(.init(executable: URL(fileURLWithPath: "/bin/sleep"), arguments: ["5"], limit: 64, timeoutMs: 50))
        XCTAssertTrue(timed.started && timed.timedOut && !timed.ok)
        let missing = await AppMacProcess.run(.init(executable: URL(fileURLWithPath: "/no-such-openagents-executable"), arguments: [], limit: 64, timeoutMs: 100))
        XCTAssertFalse(missing.started)
    }
    func testDiscoveryAndLocalRPCUseOneProtectedService() async throws {
        let home = FileManager.default.temporaryDirectory.appendingPathComponent("oa-local-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: home) }
        let socket = home.appendingPathComponent(".openagents/coder/host/service.sock")
        try FileManager.default.createDirectory(at: socket.deletingLastPathComponent(), withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: socket.path, contents: Data()) // presence oracle; the actual CLI authenticates the socket peer
        let process = MacProcessFixture(), peer = MacPeerFixture()
        let device = AppMacDevice(home: home, peer: peer, run: { await process.run($0) })
        let listed = await withCheckedContinuation { continuation in device.list { continuation.resume(returning: $0) } }
        XCTAssertTrue(listed.contains(process.key + "\tThis computer\n") && listed.contains(peer.key))
        XCTAssertEqual(process.commands.count, 1)
        XCTAssertEqual(process.commands[0].environment["HOME"], home.path)
        let body = Data("opaque SDK request".utf8)
        let reply = await withCheckedContinuation { continuation in device.call(host: process.key, body: body) { continuation.resume(returning: ($0, $1)) } }
        XCTAssertEqual(reply.0, .done); XCTAssertEqual(process.request, body); XCTAssertEqual(peer.calls, 0)
        XCTAssertFalse(FileManager.default.fileExists(atPath: try XCTUnwrap(process.requestURL).path))
        process.status = 3
        let lost = await withCheckedContinuation { continuation in device.call(host: process.key, body: body) { state, _ in continuation.resume(returning: state) } }
        XCTAssertEqual(lost, .unknown); XCTAssertEqual(peer.calls, 0)
        process.started = false
        let refused = await withCheckedContinuation { continuation in device.call(host: process.key, body: body) { state, _ in continuation.resume(returning: state) } }
        XCTAssertEqual(refused, .failed)
    }
    func testNoLocalServiceKeepsRemoteCallsOnTheirExistingAdapter() async {
        let home = FileManager.default.temporaryDirectory.appendingPathComponent("oa-no-local-\(UUID().uuidString)"), peer = MacPeerFixture(), process = MacProcessFixture()
        let device = AppMacDevice(home: home, peer: peer, run: { await process.run($0) })
        let rows = await withCheckedContinuation { continuation in device.list { continuation.resume(returning: $0) } }
        XCTAssertEqual(rows, peer.list()); XCTAssertTrue(process.commands.isEmpty)
        let result = await withCheckedContinuation { continuation in device.call(host: peer.key, body: Data()) { state, _ in continuation.resume(returning: state) } }
        XCTAssertEqual(result, .done); XCTAssertEqual(peer.calls, 1); XCTAssertTrue(process.commands.isEmpty)
    }
}
#endif
