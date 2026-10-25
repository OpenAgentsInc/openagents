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

    private func network(_ values: [String: Any]) -> [String: Any] { values["network"] as? [String: Any] ?? [:] }
    private func players(_ values: [String: Any]) -> [[String: Any]] { network(values)["players"] as? [[String: Any]] ?? [] }

    func testMultiplayerNamesMovementReconnectAndBackgroundRejoin() throws {
        guard let relay = ProcessInfo.processInfo.environment["GRID_TEST_RELAY"] else {
            throw XCTSkip("Set TEST_RUNNER_GRID_TEST_RELAY to the loopback multiplayer fixture.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-route", "zones", "-zone", "grid", "-grid-probe", "-grid-relay", relay, "-grid-name", "iOS Multiplayer QA"]
        app.launch(); ready(app)
        wait(app, timeout: 60) { self.network($0)["ready"] as? Bool == true && self.players($0).contains { $0["name"] as? String == "Original Grid probe" } }
        XCTAssertEqual(number(network(metrics(app)), "framesPublished"), 0, "Watch listens without advertising an avatar")
        let web = app.webViews.firstMatch
        web.buttons["Play"].tap()
        wait(app) { $0["joined"] as? Bool == true && self.number(self.network($0), "framesPublished") >= 2 }
        let before = metrics(app)
        let identity = network(before)["key"] as? String
        let stick = web.otherElements["Move"]
        let center = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let forward = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.15))
        center.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 2)
        wait(app) { hypot(self.number($0,"x")-self.number(before,"x"), self.number($0,"z")-self.number(before,"z")) > 0.4 }
        capture(app, "aethyr-grid-mobile-multiplayer")
        web.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Players'")).firstMatch.tap()
        web.buttons.matching(NSPredicate(format: "label == 'Grid Test Bot'")).firstMatch.tap()
        web.buttons["Mute"].tap(); XCTAssertTrue(web.buttons["Unmute"].exists)
        web.buttons["Unmute"].tap(); web.buttons["Block"].tap()
        wait(app) { !self.players($0).contains { $0["name"] as? String == "Grid Test Bot" } }
        XCTAssertTrue(web.staticTexts["Blocked players"].exists)
        web.buttons.matching(NSPredicate(format: "label CONTAINS 'Unblock'")).firstMatch.tap()
        wait(app) { self.players($0).contains { $0["name"] as? String == "Grid Test Bot" } }
        web.buttons["Close panel"].tap()

        var control = URLComponents(string: relay)!; control.scheme = "http"; control.path = "/disconnect"
        var request = URLRequest(url: control.url!); request.httpMethod = "POST"
        let disconnected = expectation(description: "Disconnect test relay sockets")
        URLSession.shared.dataTask(with: request) { _, _, error in XCTAssertNil(error); disconnected.fulfill() }.resume()
        wait(for: [disconnected], timeout: 10)
        wait(app, timeout: 20) { self.network($0)["ready"] as? Bool == true && self.players($0).count >= 2 }
        XCTAssertEqual(number(network(metrics(app)), "refusals"), 0)

        XCUIDevice.shared.press(.home)
        XCTAssertTrue(app.wait(for: .runningBackground, timeout: 10) || app.state == .runningBackgroundSuspended)
        app.activate()
        wait(app) { $0["joined"] as? Bool == false && self.network($0)["ready"] as? Bool == true }
        web.buttons["Play"].tap()
        wait(app) { $0["joined"] as? Bool == true && self.players($0).count >= 2 }
        web.buttons["Watch"].tap()
        wait(app) { $0["joined"] as? Bool == false }
        app.buttons["grid-close"].tap()
        app.buttons["open:grid"].tap(); ready(app)
        wait(app) { self.network($0)["ready"] as? Bool == true && self.players($0).count >= 2 }
        XCTAssertEqual(network(metrics(app))["key"] as? String, identity)
        capture(app, "aethyr-grid-mobile-multiplayer-reopened")
        app.buttons["grid-close"].tap()
    }

    func testPublicRelayAuthenticatesAndPublishesMobilePresence() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-route", "zones", "-zone", "grid", "-grid-probe", "-grid-name", "iOS Staging QA"]
        app.launch(); ready(app)
        wait(app, timeout: 60) { self.network($0)["ready"] as? Bool == true }
        app.webViews.firstMatch.buttons["Play"].tap()
        wait(app, timeout: 30) { self.number(self.network($0), "framesPublished") >= 1 && self.network($0)["status"] as? String == "Online" }
        XCTAssertEqual(number(network(metrics(app)), "refusals"), 0)
        capture(app, "aethyr-grid-mobile-public-relay")
        app.webViews.firstMatch.buttons["Watch"].tap()
        app.buttons["grid-close"].tap()
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
