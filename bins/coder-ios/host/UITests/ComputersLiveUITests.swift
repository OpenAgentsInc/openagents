// The Computers screens against a real host on this Mac. Start the fixture
// first (see `serve_a_host_for_a_device_run` in crates/coder-mobile), then
// pass its invitation as TEST_RUNNER_CODER_LIVE_INVITATION. Without it, the
// test is skipped. The world and reader stay synthetic; the Computers
// surface uses the live service with loopback relays admitted for this
// launch only.
import XCTest

final class ComputersLiveUITests: XCTestCase {
    private var app: XCUIApplication!

    func testEnrollOnlineInviteActivityAndRevocation() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let invitation = environment["CODER_LIVE_INVITATION"], invitation.hasPrefix("coder-host:") else {
            throw XCTSkip("No live host invitation; start the fixture first.")
        }
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic", "--loopback-test"]
        app.launch()
        app.openWorldComputer()
        tap("computer-settings")
        tap("computers-toggle")
        XCTAssertTrue(app.staticTexts["first-run-title"].waitForExistence(timeout: 20))

        // Enroll by pasting the invitation.
        tap("invite-paste")
        let field = app.descendants(matching: .any)["computers-input"]
        reveal(field)
        field.tap()
        field.typeText(invitation)
        tap("computers-submit")
        XCTAssertTrue(app.staticTexts["notice"].waitForLabel(containing: "Added Computer", timeout: 30))
        scrollToTop()
        tap("first-run-continue")
        tap("computer-settings")
        tap("computers-toggle")
        XCTAssertTrue(app.staticTexts["computers-title"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["host-0-status"].waitForLabel(containing: "Online", timeout: 60))
        attachScreenshot("Live host online")

        // The host's device list with its own last-seen time.
        tap("host-0-access")
        XCTAssertTrue(app.staticTexts["device-0-label"].waitForLabel(containing: "last seen", timeout: 30))
        XCTAssertFalse(app.staticTexts["device-0-label"].label.contains("unknown"))
        attachScreenshot("Live access with last seen")

        // An invitation with narrowed rights, shown as a QR code.
        tap("share-right-terminal")
        tap("share-right-review")
        tap("share-create")
        let qr = app.descendants(matching: .any)["computers-qr"]
        XCTAssertTrue(qr.waitForExistence(timeout: 30))
        XCTAssertTrue(app.staticTexts["share-code-detail"].label.contains("View sessions and tasks, Run and steer tasks."))
        reveal(qr)
        attachScreenshot("Live narrowed invitation QR code")
        tap("share-done")

        // Activity after another device creates a task.
        scrollToTop()
        tap("tab-activity")
        XCTAssertTrue(app.staticTexts["activity-0-headline"].waitForLabel(containing: "Task queued", timeout: 90))
        attachScreenshot("Live activity")

        // Another device revokes this one.
        tap("tab-computers")
        XCTAssertTrue(app.staticTexts["host-0-status"].waitForLabel(containing: "Revoked", timeout: 150))
        XCTAssertTrue(app.staticTexts["host-0-status"].label.contains("removed this device's access"))
        attachScreenshot("Live host revoked")
    }

    private func tap(_ key: String) {
        let button = app.buttons[key]
        XCTAssertTrue(button.waitForExistence(timeout: 20), key)
        reveal(button)
        button.tap()
    }

    private func reveal(_ element: XCUIElement) {
        for _ in 0..<12 where !element.isHittable {
            app.scrollViews.firstMatch.swipeUp()
        }
    }

    private func scrollToTop() {
        for _ in 0..<6 { app.scrollViews.firstMatch.swipeDown() }
    }

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}

private extension XCUIElement {
    func waitForLabel(containing expected: String, timeout: TimeInterval) -> Bool {
        let match = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", expected),
                                              object: self)
        return XCTWaiter.wait(for: [match], timeout: timeout) == .completed
    }
}
