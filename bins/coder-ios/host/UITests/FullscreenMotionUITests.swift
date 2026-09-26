// Actual Metal layout and native input with a synthetic quaternion source.
// The simulator verifies adapter integration, not a physical motion sensor.
import XCTest

final class FullscreenMotionUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
    }

    func testMetalCanvasCoversTheWindowBehindTheSystemStatusBar() throws {
        launch(motion: false)
        let window = app.windows.firstMatch.frame
        let surface = app.otherElements["verse-surface"].frame
        XCTAssertGreaterThan(window.width, 0)
        XCTAssertEqual(surface.minX, window.minX, accuracy: 1)
        XCTAssertEqual(surface.minY, window.minY, accuracy: 1)
        XCTAssertEqual(surface.maxX, window.maxX, accuracy: 1)
        XCTAssertEqual(surface.maxY, window.maxY, accuracy: 1)
        let status = app.statusBars.firstMatch.exists ? app.statusBars.firstMatch : XCUIApplication(bundleIdentifier: "com.apple.springboard").statusBars.firstMatch
        XCTAssertTrue(status.exists, "The system status bar remains visible over the world.")
        XCTAssertGreaterThan(status.frame.height, 0)
        XCTAssertLessThanOrEqual(surface.minY, status.frame.minY + 1)
        XCTAssertGreaterThanOrEqual(surface.maxY, status.frame.maxY)
        let jump = app.buttons["verse-jump"].frame
        XCTAssertGreaterThan(jump.minY, status.frame.maxY)
        XCTAssertLessThan(jump.maxY, window.maxY - 10, "The movement controls leave room for the home indicator.")
        let geometry: [String: Any] = [
            "window": rect(window), "surface": rect(surface), "status_bar": rect(status.frame), "jump": rect(jump),
            "scope": "Actual simulator Metal UIView frame and native controls; no physical device claim."
        ]
        let data = try JSONSerialization.data(withJSONObject: geometry, options: [.prettyPrinted, .sortedKeys])
        let receipt = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
        receipt.name = "Fullscreen Metal frame geometry"
        receipt.lifetime = .keepAlways
        add(receipt)
        attach("Full canvas behind system clock and home indicator")
    }

    func testTouchAndInjectedMotionModesPreserveLeftMovement() throws {
        launch(motion: true)
        let mode = app.buttons["verse-camera-mode"]
        XCTAssertTrue(mode.isEnabled)
        XCTAssertTrue(mode.label.contains("Touch look"))
        let initial = camera()
        dragRight()
        XCTAssertTrue(wait { abs(self.camera()[0] - initial[0]) > 0.01 }, "Right drag changes touch-mode yaw.")
        mode.tap()
        XCTAssertTrue(wait { mode.label.contains("Motion look") })
        XCTAssertTrue(wait { self.app.staticTexts["verse-motion-needed"].label == "Motion active" })
        let baselineFrame = frameCount()
        XCTAssertTrue(wait { self.frameCount() > baselineFrame + 8 }, "Present frames with the new motion baseline before injecting a turn.")
        let beforeMotion = camera()
        app.buttons["verse-motion-sample"].tap()
        XCTAssertTrue(wait { abs(self.camera()[0] - beforeMotion[0]) > 0.05 }, "An injected native quaternion changes the Rust camera.")
        let afterMotion = camera()
        dragRight()
        XCTAssertEqual(camera()[0], afterMotion[0], accuracy: 0.01, "Right drag cannot compete with motion control.")
        let beforeMove = position()
        app.otherElements["verse-surface"].coordinate(withNormalizedOffset: CGVector(dx: 0.16, dy: 0.43)).press(forDuration: 0.9)
        XCTAssertTrue(wait { self.distance(self.position(), beforeMove) > 0.1 }, "Holding the left side moves in motion mode.")
        app.buttons["verse-motion-recenter"].tap()
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        attach("Motion camera with full canvas and safe controls")
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(wait { mode.label.contains("Motion look") })
        XCTAssertTrue(wait { self.app.staticTexts["verse-motion-needed"].label == "Motion active" })
        mode.tap()
        XCTAssertTrue(wait { mode.label.contains("Touch look") })
        XCTAssertFalse(app.buttons["verse-motion-recenter"].exists)
        XCTAssertTrue(wait { self.app.staticTexts["verse-motion-needed"].label == "Motion idle" })
        let backToTouch = camera()
        dragRight()
        XCTAssertTrue(wait { abs(self.camera()[0] - backToTouch[0]) > 0.01 })
    }

    func testUnavailableMotionClearlyKeepsTouchControls() throws {
        #if targetEnvironment(simulator)
        launch(motion: false)
        XCTAssertTrue(app.staticTexts["verse-motion-unavailable"].exists)
        XCTAssertFalse(app.buttons["verse-camera-mode"].isEnabled)
        XCTAssertTrue(app.buttons["verse-camera-mode"].label.contains("Touch look"))
        XCTAssertFalse(app.buttons["verse-motion-sample"].exists)
        let before = camera()
        dragRight()
        XCTAssertTrue(wait { abs(self.camera()[0] - before[0]) > 0.01 })
        attach("Motion unavailable with working touch camera")
        #else
        throw XCTSkip("This control requires a simulator without a physical motion sensor.")
        #endif
    }

    private func launch(motion: Bool) {
        app.launchArguments = motion ? ["--synthetic", "--motion-preview"] : ["--synthetic"]
        app.launch()
        XCTAssertTrue(app.otherElements["verse-surface"].waitForExistence(timeout: 30))
        XCTAssertTrue(wait { self.app.staticTexts["verse-frames"].exists && self.app.staticTexts["verse-frames"].label != "Frames 0" })
        XCTAssertTrue(app.buttons["verse-camera-mode"].exists)
    }

    private func dragRight() {
        let surface = app.otherElements["verse-surface"]
        let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.84, dy: 0.42))
        let end = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.65, dy: 0.37))
        start.press(forDuration: 0.1, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.1)
    }

    private func frameCount() -> UInt64 {
        UInt64(app.staticTexts["verse-frames"].label.split(separator: " ").last ?? "0") ?? 0
    }
    private func camera() -> [Double] { values("verse-camera", count: 2) }
    private func position() -> [Double] { values("verse-position", count: 3) }
    private func values(_ id: String, count: Int) -> [Double] {
        let result = app.staticTexts[id].label.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
        XCTAssertEqual(result.count, count)
        return result.count == count ? result : Array(repeating: 0, count: count)
    }
    private func distance(_ a: [Double], _ b: [Double]) -> Double { abs(a[0]-b[0])+abs(a[2]-b[2]) }
    private func wait(_ condition: @escaping () -> Bool) -> Bool {
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: app)
        return XCTWaiter.wait(for: [ready], timeout: 10) == .completed
    }
    private func rect(_ value: CGRect) -> [Double] { [value.minX, value.minY, value.width, value.height] }
    private func attach(_ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }
}
