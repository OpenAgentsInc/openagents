// Exercise normal startup, including device identity and live world setup.
// Run against the packaged Release app as well as the synthetic UI checks.
import XCTest

final class ProductionLaunchUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = []
        app.launchEnvironment = [:]
    }

    func testProductionWorldLoadsResumesAndRelaunches() throws {
        app.launch()
        assertWorldIsActive()
        assertWorldStaysActive()
        attach("Production Verse after normal launch")

        XCUIDevice.shared.press(.home)
        XCTAssertTrue(wait {
            self.app.state == .runningBackground || self.app.state == .runningBackgroundSuspended
        }, "The app enters the background before the resume check.")
        app.activate()
        assertWorldIsActive()
        assertWorldStaysActive()
        attach("Production Verse after background resume")

        app.terminate()
        XCTAssertTrue(app.wait(for: .notRunning, timeout: 10))
        app.launch()
        assertWorldIsActive()
        assertWorldStaysActive()
        attach("Production Verse after cold relaunch")
    }

    private func assertWorldIsActive(file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30), file: file, line: line)
        XCTAssertTrue(app.otherElements["verse-surface"].waitForExistence(timeout: 30), file: file, line: line)
        XCTAssertTrue(wait { self.worldIsActive() },
                      "Normal startup must mount the Metal world and publish active Rust state.",
                      file: file, line: line)
        XCTAssertFalse(app.staticTexts["verse-error"].exists, file: file, line: line)
        XCTAssertFalse(app.staticTexts["reader-error"].exists, file: file, line: line)
        XCTAssertFalse(app.staticTexts["verse-frames"].exists, file: file, line: line)
        XCTAssertFalse(app.buttons["verse-motion-sample"].exists, file: file, line: line)
        XCTAssertFalse(app.buttons["verse-jump"].exists, file: file, line: line)
        XCTAssertFalse(app.switches["Sprint"].exists, file: file, line: line)
        XCTAssertFalse(app.buttons["Zoom in"].exists, file: file, line: line)
        XCTAssertFalse(app.buttons["Zoom out"].exists, file: file, line: line)
        XCTAssertFalse(app.staticTexts["verse-status"].exists, file: file, line: line)
        XCTAssertFalse(app.staticTexts["Coder"].exists, file: file, line: line)
        XCTAssertFalse(app.staticTexts["gym-interest"].exists, file: file, line: line)
        XCTAssertFalse(app.staticTexts["verse-motion-needed"].exists, file: file, line: line)
        XCTAssertTrue(app.buttons["verse-camera-mode"].exists, file: file, line: line)
        XCTAssertFalse(app.buttons["computer-close"].exists, file: file, line: line)
    }

    private func assertWorldStaysActive(file: StaticString = #filePath, line: UInt = #line) {
        let stopped = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            !self.worldIsActive() || self.app.staticTexts["verse-error"].exists
                || self.app.staticTexts["reader-error"].exists
        }, object: app)
        stopped.isInverted = true
        XCTAssertEqual(XCTWaiter.wait(for: [stopped], timeout: 5), .completed,
                       "The normal world must remain active without a startup or renderer error.",
                       file: file, line: line)
    }

    private func worldIsActive() -> Bool {
        guard app.state == .runningForeground else { return false }
        let surface = app.otherElements["verse-surface"]
        guard surface.exists, let value = surface.value as? String else { return false }
        return value == "Exploring Verse" || value == "Computer in reach"
    }

    private func wait(_ condition: @escaping () -> Bool) -> Bool {
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: app)
        return XCTWaiter.wait(for: [ready], timeout: 30) == .completed
    }

    private func attach(_ name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
