import XCTest

final class HomeUITests: XCTestCase {
    func testStartOpensTheGridAndTheStickWalks() {
        let app = XCUIApplication()
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        XCTAssertEqual(start.value as? String, "0")
        XCTAssertFalse(app.staticTexts["Home unavailable"].exists)
        // Dragging off Start cancels it.
        let outside = app.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.2))
        start.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.1, thenDragTo: outside)
        XCTAssertEqual(start.value as? String, "0")
        let home = XCTAttachment(screenshot: app.screenshot())
        home.name = "Coax home"; home.lifetime = .keepAlways; add(home)

        start.tap()
        let grid = app.otherElements["grid.world"]
        XCTAssertTrue(grid.waitForExistence(timeout: 5))
        XCTAssertFalse(start.exists)
        XCTAssertEqual(grid.value as? String, "0.0 0.0")

        // The left stick pushed up walks you forward, along +Z.
        let thumb = app.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.8))
        thumb.press(forDuration: 0.05, thenDragTo: thumb.withOffset(CGVector(dx: 0, dy: -80)),
                    withVelocity: .default, thenHoldForDuration: 2)
        let walked = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            guard let parts = (grid.value as? String)?.split(separator: " "), parts.count == 2,
                  let x = Double(parts[0]), let z = Double(parts[1]) else { return false }
            return z > 2 && abs(x) < 1
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [walked], timeout: 5), .completed, "at \(grid.value ?? "")")
        let world = XCTAttachment(screenshot: app.screenshot())
        world.name = "The Grid"; world.lifetime = .keepAlways; add(world)

        XCUIDevice.shared.orientation = .landscapeLeft
        let rotated = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in grid.frame.width > 600 }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [rotated], timeout: 5), .completed)
        XCUIDevice.shared.orientation = .portrait
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(grid.waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["Home unavailable"].exists)
        app.terminate()
    }
}
