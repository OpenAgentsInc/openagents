import XCTest

final class BundledGridTests: XCTestCase {
    override func setUp() { continueAfterFailure = false; XCUIDevice.shared.orientation = .portrait }
    override func tearDown() { XCUIDevice.shared.orientation = .portrait; super.tearDown() }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-route", "zones", "-zone", "grid", "-grid-offline", "-grid-probe"]
        app.launch()
        return app
    }
    private func metrics(_ app: XCUIApplication) -> [String: Any] {
        guard let value = app.staticTexts["grid-metrics"].value as? String, let data = value.data(using: .utf8), let result = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return result
    }
    private func wait(_ app: XCUIApplication, timeout: TimeInterval = 30, _ condition: @escaping ([String: Any]) -> Bool) {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition(self.metrics(app)) }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed, "Grid state: \(metrics(app))")
    }
    private func number(_ values: [String: Any], _ key: String) -> Double { (values[key] as? NSNumber)?.doubleValue ?? 0 }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot()); attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    private func ready(_ app: XCUIApplication) {
        wait(app, timeout: 120) { self.number($0,"fps") > 0 && self.number($0,"landmarks") == 7 && self.number($0,"colliders") == 8 }
    }

    func testBundledGridRendersMovesAndSurvivesBackground() {
        let app = launch(); ready(app)
        let web = app.webViews.firstMatch
        XCTAssertTrue(web.waitForExistence(timeout: 5))
        web.buttons["Play"].tap()
        wait(app) { $0["joined"] as? Bool == true }
        let before = metrics(app)
        let stick = web.otherElements["Move"]
        XCTAssertTrue(stick.exists)
        let center = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let forward = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.15))
        center.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 1.5)
        wait(app) { hypot(self.number($0,"x")-self.number(before,"x"), self.number($0,"z")-self.number(before,"z")) > 0.4 }
        let firstPerson = web.buttons.matching(NSPredicate(format: "label CONTAINS 'First person'")).firstMatch
        firstPerson.tap()
        wait(app) { $0["firstPerson"] as? Bool == true }
        firstPerson.tap()
        wait(app) { $0["firstPerson"] as? Bool == false }
        web.buttons["Jump"].tap()
        wait(app) { self.number($0,"maxY") > 0.1 }
        capture(app,"aethyr-grid-touch-world")

        XCUIDevice.shared.press(.home)
        let backgrounded = XCTNSPredicateExpectation(predicate: NSPredicate { _,_ in app.state == .runningBackground || app.state == .runningBackgroundSuspended }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for:[backgrounded],timeout:10),.completed)
        app.activate()
        wait(app) { $0["joined"] as? Bool == false && self.number($0,"fps") > 0 }
        XCTAssertTrue(web.buttons["Play"].exists)
        capture(app,"aethyr-grid-resumed")
        app.buttons["grid-close"].tap()
        XCTAssertTrue(app.buttons["open:grid"].waitForExistence(timeout: 10))
        app.buttons["open:grid"].tap(); ready(app)
        app.buttons["grid-sessions"].tap()
        XCTAssertTrue(app.buttons["new-session"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.webViews.firstMatch.exists)
        capture(app,"aethyr-grid-native-sessions")
    }

    func testMapGuideAndLandscapeUseTheBundledWorld() {
        let app = launch(); ready(app)
        let web = app.webViews.firstMatch
        web.buttons["Play"].tap(); wait(app) { $0["joined"] as? Bool == true }
        web.buttons.matching(NSPredicate(format:"label CONTAINS 'Map'")).firstMatch.tap()
        capture(app,"aethyr-grid-map")
        web.buttons.matching(NSPredicate(format:"label CONTAINS 'Guide'")).firstMatch.tap()
        XCTAssertTrue(web.staticTexts["Grid guide"].waitForExistence(timeout: 5))
        web.buttons["Controls"].tap()
        XCTAssertTrue(web.staticTexts.matching(NSPredicate(format:"label CONTAINS 'WASD'")).firstMatch.waitForExistence(timeout: 5))
        web.buttons["Close panel"].tap()
        XCUIDevice.shared.orientation = .landscapeLeft
        let rotated = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            let frame = web.frame
            return frame.width > frame.height && app.buttons["grid-sessions"].isHittable
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [rotated], timeout: 10), .completed)
        wait(app) { self.number($0,"fps") > 0 }
        XCTAssertTrue(web.buttons["Jump"].isHittable)
        capture(app,"aethyr-grid-landscape")
        app.buttons["grid-close"].tap()
    }
}
