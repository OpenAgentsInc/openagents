// All door and key choices use real projected world/HUD taps in a local world.
import XCTest

final class DoorUITests: XCTestCase {
    private var app: XCUIApplication!
    private var surface: XCUIElement { app.otherElements["verse-surface"] }

    func testDoorChoicesRefuseIncompatibleItemsAndPersistAcrossRelaunch() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic", "--door-scope", UUID().uuidString]
        launch()
        XCTAssertEqual(try observation().doors.held, "prism")
        try approach("spark")
        try tapDoor("spark")
        waitFor { $0.doors.doors.first { $0.id == "spark" }?.remembered == "prism" }
        waitFor { $0.doors.doors.first { $0.id == "spark" }?.state == "selected" }
        attach("Spark remembers Prism")

        try tapItem("ring")
        try tapDoor("spark")
        waitFor { $0.doors.hud.caption.contains("does not fit") }
        XCTAssertEqual(try door("spark").remembered, "prism")
        XCTAssertEqual(try observation().doors.held, "ring")
        XCTAssertNotEqual(try observation().map.state, "Walking")
        attach("Incompatible Ring keeps the Spark memory")

        try tapItem("empty")
        try tapDoor("spark")
        waitFor { $0.doors.doors.first { $0.id == "spark" }?.state == "selected" }
        XCTAssertEqual(try door("spark").destination, "Library")
        let beforeWalk = try observation().position
        try tapDoor("spark")
        waitFor { $0.map.state == "Walking" }
        waitFor { hypot($0.position[0] - beforeWalk[0], $0.position[2] - beforeWalk[2]) > 0.3 }
        let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.88))
        let end = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.78))
        start.press(forDuration: 0.1, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.1)
        waitFor { $0.map.state == "Walk stopped" }

        try approach("halo")
        try tapItem("ring")
        try tapDoor("halo")
        waitFor { $0.doors.doors.first { $0.id == "halo" }?.remembered == "ring" }
        waitFor { $0.doors.doors.first { $0.id == "halo" }?.state == "selected" }
        XCTAssertEqual(try door("halo").destination, "Oracle")
        attach("Halo remembers Ring")
        let retained = try observation()
        XCTAssertGreaterThan(retained.door_storage_writes, 0)
        waitFor { $0.frames > retained.frames + 20 }
        XCTAssertEqual(try observation().door_storage_writes, retained.door_storage_writes,
                       "Unchanged rendered frames must not rewrite preferences.")
        XCTAssertFalse(app.staticTexts["door-storage-error"].exists)

        app.terminate()
        launch()
        XCTAssertEqual(try observation().doors.held, "ring")
        XCTAssertEqual(try door("spark").remembered, "prism")
        XCTAssertEqual(try door("halo").remembered, "ring")
        XCTAssertEqual(try door("spark").state, "idle")
        XCTAssertEqual(try door("halo").state, "idle")
        XCTAssertNotEqual(try observation().map.state, "Walking")
        XCTAssertEqual(try observation().door_storage_writes, 0, "Restoring a document does not rewrite it.")

        try approach("halo")
        try tapItem("reset")
        waitFor { $0.doors.doors.first { $0.id == "halo" }?.remembered == nil }
        XCTAssertEqual(try door("spark").remembered, "prism")
        app.terminate()
        launch()
        XCTAssertNil(try door("halo").remembered)
        XCTAssertEqual(try door("spark").remembered, "prism")
        XCTAssertEqual(try observation().doors.held, "ring")
        attach("Door Reset persists without clearing the other door")
    }

    private func launch() {
        app.launch()
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        waitFor { $0.frames > 0 && $0.map.visible }
    }
    private func approach(_ id: String) throws {
        let compact = try observation().map
        tap(compact.frame[0] + compact.frame[2] / 2, compact.frame[1] + 12)
        waitFor { $0.map.expanded }
        let map = try observation().map
        let target = try XCTUnwrap(map.landmarks.first { $0.id == id })
        tap(map.plot[0] + ((target.x - map.center[0]) / map.half_extent + 1) * map.plot[2] / 2,
            map.plot[1] + (1 - (target.z - map.center[1]) / map.half_extent) * map.plot[3] / 2)
        waitFor { $0.map.state == "Arrived" }
        for _ in 0..<16 {
            if try door(id).near && door(id).visible && observation().doors.hud.visible { break }
            let yaw = try observation().camera[0]
            let wrapped = atan2(sin(yaw), cos(yaw))
            if abs(wrapped) < 0.05 { break }
            // Both approach points face +Z. Turn along the shortest arc using
            // actual right-side drags, never a synthetic pose change.
            let points = max(-60, min(60, wrapped / 0.004))
            let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.48))
            let end = start.withOffset(CGVector(dx: points, dy: 0))
            start.press(forDuration: 0.1, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0)
        }
        waitFor { $0.doors.hud.visible && $0.doors.hud.door == id }
    }
    private func tapDoor(_ id: String) throws {
        let target = try door(id)
        XCTAssertTrue(target.near && target.visible)
        surface.coordinate(withNormalizedOffset: CGVector(dx: target.screen_x, dy: target.screen_y)).tap()
    }
    private func tapItem(_ id: String) throws {
        let hud = try observation().doors.hud
        XCTAssertTrue(hud.visible)
        let button = try XCTUnwrap(hud.buttons.first { $0.id == id && $0.enabled })
        tap(button.frame[0] + button.frame[2] / 2, button.frame[1] + button.frame[3] / 2)
        if id != "reset" { waitFor { $0.doors.held == id } }
    }
    private func observation() throws -> VerseTestObservation { try XCTUnwrap(app.verseObservation()) }
    private func door(_ id: String) throws -> VerseTestDoor { try XCTUnwrap(observation().doors.doors.first { $0.id == id }) }
    private func tap(_ x: Double, _ y: Double) {
        surface.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)).tap()
    }
    private func waitFor(_ predicate: @escaping (VerseTestObservation) -> Bool) {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            self.app.verseObservation().map(predicate) ?? false
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: 25), .completed)
    }
    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
