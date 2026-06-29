import Foundation
import XCTest
@testable import Khala

final class PylonSupervisorTests: XCTestCase {
    override func tearDown() {
        PylonMockURLProtocol.handler = nil
        super.tearDown()
    }

    @MainActor
    func testAttachesToExistingPylonWhenHealthAndTokenArePresent() async throws {
        let root = try makeTempDirectory()
        let existingHome = root.appendingPathComponent("existing", isDirectory: true)
        try FileManager.default.createDirectory(at: existingHome, withIntermediateDirectories: true)
        try "0123456789abcdef".write(to: existingHome.appendingPathComponent("control-token"), atomically: true, encoding: .utf8)
        let launcher = FakePylonLauncher()
        PylonMockURLProtocol.handler = { request in
            if request.url?.path == "/health" {
                return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data(#"{"ok":true}"#.utf8))
            }
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer 0123456789abcdef")
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data(#"{"ok":true,"result":{"refs":["capacity.inference.apple_fm.ready=1","capacity.inference.apple_fm.available=1"]}}"#.utf8))
        }

        let supervisor = PylonSupervisor(
            configuration: configuration(root: root, existingHome: existingHome),
            launcher: launcher,
            session: mockSession()
        )

        await supervisor.start()

        XCTAssertEqual(supervisor.snapshot.mode, .connectedExisting)
        XCTAssertEqual(supervisor.snapshot.pylonHome, existingHome)
        XCTAssertEqual(launcher.launchCount, 0)
        XCTAssertEqual(supervisor.snapshot.providerStatusText, "Online")
    }

    @MainActor
    func testBootsBundledPylonWithAppManagedHomeWhenNoExistingNodeIsReachable() async throws {
        let root = try makeTempDirectory()
        let launcher = FakePylonLauncher()
        PylonMockURLProtocol.handler = { request in
            if request.url?.path == "/health" {
                throw URLError(.cannotConnectToHost)
            }
            return (HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!, Data())
        }

        let supervisor = PylonSupervisor(
            configuration: configuration(root: root),
            launcher: launcher,
            session: mockSession()
        )

        await supervisor.start()

        XCTAssertEqual(supervisor.snapshot.mode, .bundledRunning)
        XCTAssertEqual(launcher.launchCount, 1)
        XCTAssertEqual(launcher.lastConfiguration?.appManagedPylonHome.path, root.appendingPathComponent("app-home").path)
        XCTAssertFalse(supervisor.snapshot.pylonHome.path.contains(".codex"))
    }

    @MainActor
    func testAttachCanUseInjectedControlTokenWithoutReadingHomeToken() async throws {
        let root = try makeTempDirectory()
        var config = configuration(root: root)
        config.controlTokenOverride = "override-token-123456"
        PylonMockURLProtocol.handler = { request in
            if request.url?.path == "/health" {
                return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data(#"{"ok":true}"#.utf8))
            }
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer override-token-123456")
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data(#"{"ok":true,"result":{"accounts":[]}}"#.utf8))
        }

        let supervisor = PylonSupervisor(configuration: config, launcher: FakePylonLauncher(), session: mockSession())

        await supervisor.start()

        XCTAssertEqual(supervisor.snapshot.mode, .connectedExisting)
    }

    @MainActor
    func testStopTerminatesOnlyBundledChild() async throws {
        let root = try makeTempDirectory()
        let launcher = FakePylonLauncher()
        PylonMockURLProtocol.handler = { request in
            if request.url?.path == "/health" { throw URLError(.cannotConnectToHost) }
            return (HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!, Data())
        }
        let supervisor = PylonSupervisor(configuration: configuration(root: root), launcher: launcher, session: mockSession())
        await supervisor.start()

        supervisor.stop()

        XCTAssertEqual(supervisor.snapshot.mode, .stopped)
        XCTAssertEqual(launcher.child.terminateCount, 1)
    }

    @MainActor
    func testCrashRecoveryRelaunchesBundledChildOncePerExit() async throws {
        let root = try makeTempDirectory()
        let launcher = FakePylonLauncher()
        PylonMockURLProtocol.handler = { request in
            if request.url?.path == "/health" { throw URLError(.cannotConnectToHost) }
            return (HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!, Data())
        }
        let supervisor = PylonSupervisor(configuration: configuration(root: root), launcher: launcher, session: mockSession())
        await supervisor.start()

        launcher.child.terminationHandler?()
        try await Task.sleep(nanoseconds: 50_000_000)

        XCTAssertEqual(launcher.launchCount, 2)
        XCTAssertEqual(supervisor.snapshot.mode, .bundledRunning)
    }

    @MainActor
    func testProjectionSummariesRedactTokens() async throws {
        let root = try makeTempDirectory()
        let existingHome = root.appendingPathComponent("existing", isDirectory: true)
        try FileManager.default.createDirectory(at: existingHome, withIntermediateDirectories: true)
        try "0123456789abcdef".write(to: existingHome.appendingPathComponent("control-token"), atomically: true, encoding: .utf8)
        PylonMockURLProtocol.handler = { request in
            if request.url?.path == "/health" {
                return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data(#"{"ok":true}"#.utf8))
            }
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data(#"{"ok":true,"result":{"token":"oa_agent_secret","header":"Bearer abcdefghijklmnop"}}"#.utf8))
        }
        let supervisor = PylonSupervisor(configuration: configuration(root: root, existingHome: existingHome), launcher: FakePylonLauncher(), session: mockSession())

        await supervisor.start()

        XCTAssertFalse(supervisor.snapshot.accountsSummary.contains("oa_agent_secret"))
        XCTAssertFalse(supervisor.snapshot.accountsSummary.contains("Bearer abcdefghijklmnop"))
        XCTAssertTrue(supervisor.snapshot.accountsSummary.contains("[redacted]"))
    }

    private func configuration(root: URL, existingHome: URL? = nil) -> PylonSupervisorConfiguration {
        PylonSupervisorConfiguration(
            controlURL: URL(string: "http://127.0.0.1:4716")!,
            bundledPylonEntry: root.appendingPathComponent("pylon-node/index.js"),
            bundledBunExecutable: root.appendingPathComponent("bun"),
            appManagedPylonHome: root.appendingPathComponent("app-home", isDirectory: true),
            existingPylonHome: existingHome ?? root.appendingPathComponent("existing", isDirectory: true),
            appleFmBridgePath: root.appendingPathComponent("foundation-bridge"),
            openAgentsBaseURL: "https://openagents.com",
            controlTokenOverride: nil
        )
    }

    private func makeTempDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func mockSession() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [PylonMockURLProtocol.self]
        return URLSession(configuration: config)
    }
}

private final class PylonMockURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        do {
            guard let handler = Self.handler else { throw URLError(.badServerResponse) }
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

private final class FakePylonLauncher: PylonProcessLaunching {
    let child = FakePylonChild()
    var launchCount = 0
    var lastConfiguration: PylonSupervisorConfiguration?

    func launchPylonNode(configuration: PylonSupervisorConfiguration) throws -> PylonChildProcess {
        launchCount += 1
        lastConfiguration = configuration
        try FileManager.default.createDirectory(at: configuration.appManagedPylonHome, withIntermediateDirectories: true)
        try "fedcba9876543210".write(to: configuration.appManagedPylonHome.appendingPathComponent("control-token"), atomically: true, encoding: .utf8)
        return child
    }
}

private final class FakePylonChild: PylonChildProcess {
    var isRunning = true
    var terminationHandler: (() -> Void)?
    var terminateCount = 0

    func terminate() {
        terminateCount += 1
        isRunning = false
    }
}
