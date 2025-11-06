import XCTest
@testable import OpenAgentsCore

final class PathUtilsTests: XCTestCase {
    let root = "/Users/alice/code/openagents"

    func testAliasesGoToRoot() {
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "."), ".")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/"), ".")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "workspace"), ".")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/workspace"), ".")
    }

    func testWorkspacePrefixIsStripped() {
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/workspace/openagents"), ".")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/workspace/openagents/ios"), "ios")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/workspace/ios"), "ios")
    }

    func testAbsoluteInsideRootBecomesRelative() {
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/Users/alice/code/openagents"), ".")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/Users/alice/code/openagents/ios"), "ios")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "/Users/alice/code/openagents/ios/OpenAgents"), "ios/OpenAgents")
    }

    func testLeadingWorkspaceNameIsStripped() {
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "openagents"), ".")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "openagents/ios"), "ios")
    }

    func testCleanupDotsAndSlashes() {
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "./ios/"), "ios")
        XCTAssertEqual(PathUtils.normalizeToWorkspaceRelative(workspaceRoot: root, inputPath: "ios/"), "ios")
    }
}

