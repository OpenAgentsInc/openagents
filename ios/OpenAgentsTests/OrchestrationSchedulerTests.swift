import XCTest
@testable import OpenAgents
import OpenAgentsCore
import Combine

@MainActor
final class OrchestrationSchedulerTests: XCTestCase {
    func testConfigSetActivateStatusAndRunNow() async throws {
        #if os(macOS)
        let server = DesktopWebSocketServer()
        // Attach temp DB for configs/history
        let dbURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("OrchSched-").appendingPathExtension("sqlite")
        server.setTinyvexDb(path: dbURL.path)

        let rpc = LocalJsonRpcClient(server: server)

        // Build a minimal config rooted at repo folder
        let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath).path
        var cfg = OrchestrationConfig.createDefault(workspaceRoot: root)
        cfg.id = "test"
        cfg.goals = ["Sanity orchestration"]

        struct SetReq: Codable { let config: OrchestrationConfig }
        let setExp = expectation(description: "config.set")
        rpc.sendJSONRPC(method: ACPRPC.orchestrateConfigSet, params: SetReq(config: cfg), id: "cfg.set") { (_: [String: AnyCodable]?) in
            setExp.fulfill()
        }
        await fulfillment(of: [setExp], timeout: 2.0)

        struct ActivateReq: Codable { let id: String; let workspace_root: String }
        let actExp = expectation(description: "config.activate")
        rpc.sendJSONRPC(method: ACPRPC.orchestrateConfigActivate, params: ActivateReq(id: cfg.id, workspace_root: root), id: "cfg.activate") { (resp: [String:String]?) in
            actExp.fulfill()
        }
        await fulfillment(of: [actExp], timeout: 2.0)

        // Status should expose active config id and a next_wake_time
        struct Status: Codable { let running: Bool; let active_config_id: String?; let next_wake_time: Int?; let message: String }
        let statusExp = expectation(description: "status")
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerStatus, params: [String:String](), id: "sched.status") { (resp: Status?) in
            XCTAssertEqual(resp?.active_config_id, cfg.id)
            statusExp.fulfill()
        }
        await fulfillment(of: [statusExp], timeout: 2.0)

        // Subscribe to notifications to detect an update from run_now
        let gotUpdate = expectation(description: "session/update from run_now")
        let cancellable = server.notificationPublisher.sink { evt in
            if evt.method == ACPRPC.sessionUpdate { gotUpdate.fulfill() }
        }
        defer { cancellable.cancel() }

        // Trigger run_now
        struct RunNow: Codable { let started: Bool; let session_id: String? }
        let runExp = expectation(description: "run_now")
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerRunNow, params: [String:String](), id: "sched.run") { (resp: RunNow?) in
            XCTAssertEqual(resp?.started, true)
            runExp.fulfill()
        }
        await fulfillment(of: [runExp, gotUpdate], timeout: 10.0)
        #else
        throw XCTSkip("macOS-only")
        #endif
    }
}
