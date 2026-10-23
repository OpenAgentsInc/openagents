import XCTest

final class MedievalTownTests: XCTestCase {
    override func setUp() {
        super.setUp()
        continueAfterFailure = false
        XCUIDevice.shared.orientation = .portrait
    }

    override func tearDown() {
        XCUIDevice.shared.orientation = .portrait
        super.tearDown()
    }

    func testMenuOpensNativeTownAndMovementDrawsFrames() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.openDrawer())
        let entry = app.buttons["sidebar-medieval-town"]
        XCTAssertTrue(entry.waitForExistence(timeout: 5))
        entry.tap()

        let surface = app.otherElements["medieval-town-surface"]
        XCTAssertTrue(surface.waitForExistence(timeout: 10))
        let metric = app.staticTexts["medieval-town-metrics"]
        try waitForMetrics(metric, timeout: 180) { values in
            (values["ready"] as? Bool) == true && self.number(values, "completedFrames") >= 30
        }
        let before = try position(readMetrics(metric))
        let stick = app.otherElements["medieval-town-move"]
        XCTAssertTrue(stick.exists)
        let center = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let forward = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.16))
        center.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 1.5)
        let releasedFrames = number(try readMetrics(metric), "completedFrames")
        try waitForMetrics(metric, timeout: 10) { self.number($0, "completedFrames") >= releasedFrames + 30 }
        try waitForMetrics(metric, timeout: 10) { values in
            guard let point = try? self.position(values) else { return false }
            return hypot(point[0] - before[0], point[2] - before[2]) > 0.2
        }
        try waitForMetrics(metric, timeout: 15) { self.number($0, "completedFrames") >= 120 }
        var values = try readMetrics(metric)
        XCTAssertGreaterThan(number(values, "fps"), 0)
        XCTAssertGreaterThan(number(values, "triangles"), 100)
        XCTAssertGreaterThan(number(values, "drawCalls"), 0)
        saveMetrics(values, name: "native-town-moving-frames")

        let yawBeforeLook = number(values, "yaw")
        let look = app.otherElements["medieval-town-look"]
        XCTAssertTrue(look.exists)
        let lookStart = look.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.45))
        let lookEnd = look.coordinate(withNormalizedOffset: CGVector(dx: 0.8, dy: 0.45))
        lookStart.press(forDuration: 0.05, thenDragTo: lookEnd, withVelocity: .slow, thenHoldForDuration: 0.05)
        try waitForMetrics(metric, timeout: 10) { values in
            let delta = self.number(values, "yaw") - yawBeforeLook
            return abs(atan2(sin(delta), cos(delta))) > 0.05
        }
        values = try readMetrics(metric)
        saveMetrics(values, name: "native-town-look-gesture")

        let beforeBackground = try position(values)
        let completedBeforeBackground = number(values, "completedFrames")
        XCUIDevice.shared.press(.home)
        let backgrounded = NSPredicate { _, _ in app.state == .runningBackground || app.state == .runningBackgroundSuspended }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: backgrounded, object: app)], timeout: 10), .completed)
        app.activate()
        try waitForMetrics(metric, timeout: 10) { self.number($0, "completedFrames") > completedBeforeBackground + 10 }
        let resumed = try readMetrics(metric)
        let afterBackground = try position(resumed)
        XCTAssertLessThan(hypot(afterBackground[0] - beforeBackground[0], afterBackground[2] - beforeBackground[2]), 0.2, "Returning to the app keeps the player in place")
        saveMetrics(resumed, name: "native-town-resumed-frames")
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = "native-medieval-town"
        screenshot.lifetime = .keepAlways
        add(screenshot)

        app.buttons["medieval-town-close"].tap()
        let closed = NSPredicate { _, _ in !surface.exists }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: closed, object: app)], timeout: 10), .completed)
        XCTAssertTrue(app.buttons["drawer-menu"].exists || app.buttons["sidebar-medieval-town"].exists)
    }

    /// Run in a Release simulator or on a device after the asset cache is warm.
    func testNativeTownCompletedFrameCadence() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-route", "medieval-town", "-town-bench"]
        app.launch()
        let metric = app.staticTexts["medieval-town-metrics"]
        XCTAssertTrue(metric.waitForExistence(timeout: 10))
        try waitForMetrics(metric, timeout: 210) { $0["benchmarkComplete"] as? Bool == true }
        let after = try readMetrics(metric)
        saveMetrics(after, name: "native-town-completed-frame-cadence")
        XCTAssertGreaterThanOrEqual(number(after, "fps"), 59, "Actual Rust GPU-completed frames per second")
        let completed = number(after, "completedFrames")
        XCTAssertGreaterThan(completed, 100)
        XCTAssertLessThan(number(after, "missedFramesOver25Ms") / max(1, completed), 0.01)
        app.buttons["medieval-town-close"].tap()
    }

    private func number(_ values: [String: Any], _ key: String) -> Double {
        (values[key] as? NSNumber)?.doubleValue ?? 0
    }

    private func readMetrics(_ element: XCUIElement) throws -> [String: Any] {
        try XCTUnwrap(availableMetrics(element))
    }

    private func availableMetrics(_ element: XCUIElement) -> [String: Any]? {
        guard element.exists, let string = element.value as? String,
              let data = string.data(using: .utf8),
              let values = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return values
    }

    private func position(_ values: [String: Any]) throws -> [Double] {
        let point = try XCTUnwrap(values["playerPosition"] as? [NSNumber])
        XCTAssertEqual(point.count, 3)
        return point.map(\.doubleValue)
    }

    private func waitForMetrics(_ element: XCUIElement, timeout: TimeInterval, condition: @escaping ([String: Any]) -> Bool) throws {
        let predicate = NSPredicate { [weak self] _, _ in
            guard let self, let values = availableMetrics(element) else { return false }
            return condition(values)
        }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: predicate, object: element)], timeout: timeout), .completed)
    }

    private func saveMetrics(_ values: [String: Any], name: String) {
        guard let data = try? JSONSerialization.data(withJSONObject: values, options: [.sortedKeys, .prettyPrinted]) else { return }
        let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
