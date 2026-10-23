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
        XCTAssertGreaterThan(number(after, "materialLayers"), 100)
        XCTAssertGreaterThan(number(after, "sceneTriangles"), 4_000_000)
        XCTAssertGreaterThan(number(after, "distanceWalked"), 10)
        app.buttons["medieval-town-close"].tap()
    }

    func testTownVisualWalkthrough() throws {
        XCUIDevice.shared.orientation = .landscapeLeft
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-route", "medieval-town", "-town-metrics"]
        app.launch()
        let metric = app.staticTexts["medieval-town-metrics"]
        XCTAssertTrue(metric.waitForExistence(timeout: 10))
        try waitForMetrics(metric, timeout: 180) { values in
            values["ready"] as? Bool == true
                && self.number(values, "completedFrames") >= 60
                && self.number(values, "width") > self.number(values, "height")
        }
        let initial = try readMetrics(metric)
        XCTAssertGreaterThan(number(initial, "materialLayers"), 100)
        XCTAssertGreaterThan(number(initial, "sceneTriangles"), 4_000_000)
        try captureTownView(app, metric: metric, name: "01-arch-arrival")

        let stick = app.otherElements["medieval-town-move"]
        XCTAssertTrue(stick.exists)
        let center = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let forward = stick.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.16))
        center.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 1.8)
        try captureTownView(app, metric: metric, name: "02-arch-walking")
        let walked = try readMetrics(metric)
        XCTAssertGreaterThan(number(walked, "distanceWalked") - number(initial, "distanceWalked"), 0.5)

        try orientTownCamera(app, metric: metric, yaw: .pi / 2, pitch: 0)
        try captureTownView(app, metric: metric, name: "03-street-east")
        try orientTownCamera(app, metric: metric, yaw: -.pi / 2, pitch: 0)
        try captureTownView(app, metric: metric, name: "04-street-west")
        try orientTownCamera(app, metric: metric, yaw: -0.058, pitch: 0.27)
        try captureTownView(app, metric: metric, name: "05-castle-skyline")
        app.buttons["medieval-town-close"].tap()
    }

    private func captureTownView(_ app: XCUIApplication, metric: XCUIElement, name: String) throws {
        let frames = number(try readMetrics(metric), "completedFrames")
        try waitForMetrics(metric, timeout: 15) { self.number($0, "completedFrames") >= frames + 30 }
        let pose = try readMetrics(metric)
        XCTAssertGreaterThan(number(pose, "width"), number(pose, "height"))
        saveMetrics(pose, name: "native-town-visual-\(name)-pose")
        let screen = XCUIScreen.main.screenshot()
        XCTAssertGreaterThan(screen.image.size.width, screen.image.size.height, "The full display capture is landscape")
        let image = XCTAttachment(screenshot: screen)
        image.name = "native-town-visual-\(name)"
        image.lifetime = .keepAlways
        add(image)
    }

    private func orientTownCamera(_ app: XCUIApplication, metric: XCUIElement, yaw: Double, pitch: Double) throws {
        let look = app.otherElements["medieval-town-look"]
        XCTAssertTrue(look.exists)
        for _ in 0..<16 {
            let pose = try readMetrics(metric)
            let yawError = atan2(sin(yaw - number(pose, "yaw")), cos(yaw - number(pose, "yaw")))
            let pitchError = pitch - number(pose, "pitch")
            if abs(yawError) < 0.07 && abs(pitchError) < 0.04 { return }
            let width = Double(max(1, look.frame.width))
            let height = Double(max(1, look.frame.height))
            let dx = max(-width * 0.35, min(width * 0.35, yawError / 0.004))
            let dy = max(-height * 0.2, min(height * 0.2, -pitchError / 0.004))
            let start = look.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4))
            let end = look.coordinate(withNormalizedOffset: CGVector(dx: CGFloat(0.5 + dx / width), dy: CGFloat(0.4 + dy / height)))
            start.press(forDuration: 0.05, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.05)
            let frames = number(try readMetrics(metric), "completedFrames")
            try waitForMetrics(metric, timeout: 15) { self.number($0, "completedFrames") >= frames + 20 }
        }
        XCTFail("Look gestures did not reach the requested camera angle")
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
