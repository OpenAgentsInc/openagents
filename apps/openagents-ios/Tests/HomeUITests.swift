import XCTest

final class HomeUITests: XCTestCase {
    func testHomeAndNativeTouchPath() {
        let app = XCUIApplication()
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        XCTAssertEqual(start.value as? String, "0")
        XCTAssertFalse(app.staticTexts["Home unavailable"].exists)
        start.tap()
        expectation(for: NSPredicate(format: "value == %@", "1"), evaluatedWith: start)
        waitForExpectations(timeout: 5)
        start.tap()
        expectation(for: NSPredicate(format: "value == %@", "2"), evaluatedWith: start)
        waitForExpectations(timeout: 5)
        let outside = app.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.2))
        start.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.1, thenDragTo: outside)
        XCTAssertEqual(start.value as? String, "2")
        XCUIDevice.shared.orientation = .landscapeLeft
        let rotated = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in start.frame.width > 600 }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [rotated], timeout: 5), .completed)
        XCTAssertTrue(start.isHittable)
        start.tap()
        expectation(for: NSPredicate(format: "value == %@", "3"), evaluatedWith: start)
        waitForExpectations(timeout: 5)
        XCUIDevice.shared.orientation = .portrait
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(start.waitForExistence(timeout: 5))
        start.tap()
        expectation(for: NSPredicate(format: "value == %@", "4"), evaluatedWith: start)
        waitForExpectations(timeout: 5)
        let home = XCTAttachment(screenshot: app.screenshot())
        home.name = "Coax home"; home.lifetime = .keepAlways; add(home)
        app.terminate()
    }
}
