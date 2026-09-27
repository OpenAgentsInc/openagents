// This live acceptance check needs a fresh production relay preference and a
// separate Verse witness publishing nearby poses to the public relay.
import XCTest

final class WorldRelayProductionUITests: XCTestCase {
    func testFreshLaunchJoinsPublicWorldAndPresentsRemoteGeometry() throws {
        guard ProcessInfo.processInfo.environment["CODER_WORLD_LIVE_TEST"] == "1" else {
            throw XCTSkip("Set TEST_RUNNER_CODER_WORLD_LIVE_TEST=1 while a public world witness runs.")
        }
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--uitest-observe-world"]
        app.launch()
        XCTAssertTrue(app.otherElements["verse-surface"].waitForExistence(timeout: 30))
        let online = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            guard let world = app.verseObservation() else { return false }
            return world.frames > 0 && world.world_connection.state == "connected"
                && world.world_connection.relay == "wss://relay.openagents.com"
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [online], timeout: 45), .completed)
        let initial = try XCTUnwrap(app.verseObservation())
        XCTAssertEqual(initial.world_preference, "unconfigured", "Use a simulator without a saved production relay choice.")
        XCTAssertNil(initial.world_configuration.world_relay)
        XCTAssertNil(initial.world_configuration.world_offline)
        XCTAssertEqual(initial.world_public_key.count, 64)
        print("CODER_WORLD_PUBLIC_KEY=\(initial.world_public_key)")
        let remote = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            guard let world = app.verseObservation() else { return false }
            return world.live_remote_entities > 0 && world.presented_remote_vertices > 0
                && world.world_connection.state == "connected"
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [remote], timeout: 45), .completed)
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        XCTAssertFalse(app.buttons["gym-interact"].exists)
        let surface = app.otherElements["verse-surface"]
        if let json = surface.value as? String, let data = json.data(using: .utf8) {
            let record = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
            record.name = "Production world presence observation"
            record.lifetime = .keepAlways
            add(record)
        }
        let image = XCTAttachment(screenshot: app.screenshot())
        image.name = "Public world after default production launch"
        image.lifetime = .keepAlways
        add(image)
    }
}
