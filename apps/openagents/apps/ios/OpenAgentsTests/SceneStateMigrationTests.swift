import XCTest
@testable import OpenAgents

/// #11889: an earlier SwiftUI app's saved window-scene state made TestFlight
/// builds open to a black screen and crash. The inherited state is removed
/// once, before UIKit starts, and never again after that.
final class SceneStateMigrationTests: XCTestCase {
    private let bundleID = "com.openagents.app.scene-migration-test"
    private var suiteName = ""
    private var defaults: UserDefaults!

    override func setUpWithError() throws {
        suiteName = "SceneStateMigrationTests.\(UUID().uuidString)"
        defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
    }

    override func tearDownWithError() throws {
        defaults.removePersistentDomain(forName: suiteName)
        try? FileManager.default.removeItem(at: savedState())
    }

    private func savedState() -> URL {
        FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Saved Application State", isDirectory: true)
            .appendingPathComponent("\(bundleID).savedState", isDirectory: true)
    }

    private func seedSavedState() throws {
        let dir = savedState()
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try Data("AppSceneDelegate".utf8).write(to: dir.appendingPathComponent("data.data"))
    }

    func testInheritedSceneStateIsRemovedOnce() throws {
        try seedSavedState()
        SceneStateMigration.discardInheritedState(defaults: defaults, bundleID: bundleID)
        XCTAssertFalse(FileManager.default.fileExists(atPath: savedState().path))
        XCTAssertTrue(defaults.bool(forKey: SceneStateMigration.doneKey))

        // State this app saves itself later is kept.
        try seedSavedState()
        SceneStateMigration.discardInheritedState(defaults: defaults, bundleID: bundleID)
        XCTAssertTrue(FileManager.default.fileExists(atPath: savedState().path))
    }

    func testFreshInstallIsMarkedDone() {
        SceneStateMigration.discardInheritedState(defaults: defaults, bundleID: bundleID)
        XCTAssertTrue(defaults.bool(forKey: SceneStateMigration.doneKey))
    }
}
