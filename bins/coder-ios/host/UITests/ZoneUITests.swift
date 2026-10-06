// Native portal taps load the reviewed pack over HTTPS or from the verified
// device cache. Artwork is never an app resource.
import XCTest

final class ZoneUITests: XCTestCase {
    private var app: XCUIApplication!
    private var surface: XCUIElement { app.otherElements["verse-surface"] }

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        waitFor { $0.frames > 0 && $0.map.visible }
    }

    func testLagrangePortalEntersStationFliesAndReturns() throws {
        try approachPortal("lagrange1")
        let plaza = try observation()
        XCTAssertTrue(plaza.zone.hud.buttons.contains { $0.action == "enter" && $0.label == "Enter L1" })
        surface.coordinate(withNormalizedOffset: CGVector(dx: plaza.zone.portal.screen_x,
                                                          dy: plaza.zone.portal.screen_y)).tap()
        waitFor { $0.zone.id == "lagrange1" && $0.zone.state == "idle" }
        let station = try observation()
        XCTAssertFalse(station.gym_active)
        XCTAssertTrue(station.map.landmarks.contains { $0.id == "jig" })
        XCTAssertTrue(station.zone.hud.buttons.contains { $0.action == "grab" })
        XCTAssertTrue(station.zone.caption.hasPrefix("Earth 1."))
        attach("Lagrange 1 construction station")
        // A map tap sets an EVA pack autopilot target; thrust is gradual.
        try walkToLandmark("depot")
        waitFor(timeout: 40) { value in
            hypot(value.position[0] - station.position[0], value.position[2] - station.position[2]) > 3
        }
        attach("EVA pack flight toward the parts depot")
        try tapAction("return")
        waitFor { $0.zone.id == "plaza" && $0.zone.state == "idle" }
        let returned = try observation()
        XCTAssertEqual(returned.position[0], plaza.position[0], accuracy: 0.1)
        XCTAssertEqual(returned.position[2], plaza.position[2], accuracy: 0.1)
    }

    func testEvergladePortalEntersGladeAndReturns() throws {
        try approachPortal("everglade")
        let plaza = try observation()
        XCTAssertTrue(plaza.zone.hud.buttons.contains { $0.action == "enter" && $0.label == "Enter Everglade" })
        surface.coordinate(withNormalizedOffset: CGVector(dx: plaza.zone.portal.screen_x,
                                                          dy: plaza.zone.portal.screen_y)).tap()
        waitFor { $0.zone.id == "everglade" && $0.zone.state == "idle" }
        let glade = try observation()
        XCTAssertFalse(glade.gym_active)
        XCTAssertTrue(glade.map.landmarks.contains { $0.id == "task_wall" })
        XCTAssertTrue(glade.zone.caption.hasPrefix("Everglade"))
        attach("Everglade greybox glade")
        try tapAction("return")
        waitFor { $0.zone.id == "plaza" && $0.zone.state == "idle" }
        let returned = try observation()
        XCTAssertEqual(returned.position[0], plaza.position[0], accuracy: 0.1)
        XCTAssertEqual(returned.position[2], plaza.position[2], accuracy: 0.1)
    }

    func testRulesAndArtworkNoticesShipWithoutLoadingAZone() throws {
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        let about = app.buttons.matching(NSPredicate(format: "identifier == %@ OR label == %@", "verse-about", "About Verse")).firstMatch
        for _ in 0..<4 {
            if about.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(about.waitForExistence(timeout: 10))
        about.tap()
        let credits = app.staticTexts["verse-credits"]
        XCTAssertTrue(credits.waitForExistence(timeout: 10))
        let loaded = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            credits.label.contains("Wizards of the Coast LLC")
        }, object: credits)
        XCTAssertEqual(XCTWaiter.wait(for: [loaded], timeout: 10), .completed)
        XCTAssertEqual(try observation().zone.id, "plaza")
        XCTAssertEqual(try observation().zone.progress, 0)
        attach("Bundled Verse rules and artwork notices")
    }

    private func approachPortal(_ id: String) throws {
        let compact = try observation().map
        tap(compact.frame[0] + compact.frame[2] / 2, compact.frame[1] + 12)
        waitFor { $0.map.expanded }
        let map = try observation().map
        let destination = try XCTUnwrap(map.landmarks.first { $0.id == id })
        tap(map.plot[0] + ((destination.x - map.center[0]) / map.half_extent + 1) * map.plot[2] / 2,
            map.plot[1] + (1 - (destination.z - map.center[1]) / map.half_extent) * map.plot[3] / 2)
        waitFor { $0.map.state == "Arrived" }
        for _ in 0..<16 {
            let current = try observation()
            if current.zone.portal.near && current.zone.portal.visible && current.zone.hud.visible { break }
            let angle = atan2(sin(current.camera[0]), cos(current.camera[0]))
            let points = max(-60, min(60, angle / 0.004))
            let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.48))
            start.press(forDuration: 0.1, thenDragTo: start.withOffset(CGVector(dx: points, dy: 0)),
                        withVelocity: .slow, thenHoldForDuration: 0)
        }
        waitFor { $0.zone.portal.near && $0.zone.portal.visible && $0.zone.hud.visible }
    }

    private func walkToLandmark(_ id: String) throws {
        let compact = try observation().map
        tap(compact.frame[0] + compact.frame[2] / 2, compact.frame[1] + 12)
        waitFor { $0.map.expanded }
        let map = try observation().map
        let destination = try XCTUnwrap(map.landmarks.first { $0.id == id })
        tap(map.plot[0] + ((destination.x - map.center[0]) / map.half_extent + 1) * map.plot[2] / 2,
            map.plot[1] + (1 - (destination.z - map.center[1]) / map.half_extent) * map.plot[3] / 2)
        waitFor { $0.map.destination != nil || !$0.map.expanded }
    }

    private func attachObservation(_ name: String) {
        guard let value = surface.value as? String else { return }
        let attachment = XCTAttachment(string: value)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func tapAction(_ action: String) throws {
        let zone = try observation().zone
        XCTAssertTrue(zone.hud.visible)
        let control = try XCTUnwrap(zone.hud.buttons.first { $0.action == action && $0.enabled })
        tap(control.frame[0] + control.frame[2] / 2, control.frame[1] + control.frame[3] / 2)
    }
    private func observation() throws -> VerseTestObservation { try XCTUnwrap(app.verseObservation()) }
    private func tap(_ x: Double, _ y: Double) {
        surface.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)).tap()
    }
    private func waitFor(timeout: TimeInterval = 25, _ predicate: @escaping (VerseTestObservation) -> Bool) {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            self.app.verseObservation().map(predicate) ?? false
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed)
    }
    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
