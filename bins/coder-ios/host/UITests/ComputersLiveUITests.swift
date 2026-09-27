// The world computer's Computers screens against a real host on this Mac.
// Start the fixture first (see `serve_a_host_for_a_device_run` in
// crates/coder-mobile), then pass its invitation as
// TEST_RUNNER_CODER_LIVE_INVITATION. Without it, the test is skipped. The
// world and reader stay synthetic; the Computers surface uses the live
// service with loopback relays admitted for this launch only.
//
// With TEST_RUNNER_CODER_LIVE_ORDER=1 (and the fixture's
// CODER_COMPUTERS_REVOKE_AFTER=never), the run orders a task, follows it in
// Activity, steers it, and stops it. Otherwise it waits for the fixture's
// other devices to add activity and revoke this one.
import XCTest

final class ComputersLiveUITests: XCTestCase {
    private var app: XCUIApplication!

    func testEnrollSelectOrderAndFollowOnARealHost() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let invitation = environment["CODER_LIVE_INVITATION"], invitation.hasPrefix("coder-host:") else {
            throw XCTSkip("No live host invitation; start the fixture first.")
        }
        let order = environment["CODER_LIVE_ORDER"] == "1"
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic", "--loopback-test"]
        app.launch()
        app.openWorldComputerHud()
        XCTAssertTrue(app.hudElement("first-run-title").exists)

        // Enroll by pasting the invitation into the native keyboard.
        app.hudTap("invite-paste")
        app.hudTap("hud-input-type")
        let field = app.descendants(matching: .any)["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText(invitation)
        app.buttons["computers-submit"].tap()
        XCTAssertTrue(app.waitForHud("notice", containing: "Added Computer", timeout: 30))
        app.hudTap("first-run-continue")
        XCTAssertTrue(app.waitForHud("host-0-status", containing: "Online", timeout: 60))
        attachScreenshot("Live host online in the world HUD")

        // Select it: presence, route, rights, and the workspaces it shares.
        app.hudTap("host-0-open")
        XCTAssertTrue(app.waitForHud("host-status", containing: "Online", timeout: 20))
        XCTAssertTrue(app.waitForHud("host-workspaces", containing: "checkout, scratch", timeout: 60))
        attachScreenshot("Live host selected")

        if order {
            app.hudTap("host-order")
            app.hudTap("order-workspace-1")
            XCTAssertTrue(app.waitForHud("order-workspace-1", containing: "[x] scratch", timeout: 10))
            app.hudTap("order-prompt")
            type("Fix the flaky parser test")
            app.hudTap("order-submit")
            XCTAssertTrue(app.waitForHud("notice", containing: "Sent \"Fix the flaky parser test\"", timeout: 30))
            // The host's summary streams back into Activity.
            XCTAssertTrue(app.waitForHud("activity-0-subject", containing: "Task, queued", timeout: 60))
            XCTAssertTrue(app.waitForHud("activity-0-time", containing: "Revision 1.", timeout: 30))
            attachScreenshot("Live task queued in Activity")
            app.hudTap("activity-0-steer")
            type("Only the parser module.")
            XCTAssertTrue(app.waitForHud("activity-0-time", containing: "Revision 2.", timeout: 60))
            attachScreenshot("Live task steered")
            app.hudTap("activity-0-cancel")
            app.hudTap("activity-0-cancel-yes")
            XCTAssertTrue(app.waitForHud("activity-0-subject", containing: "Task, cancelled", timeout: 60))
            attachScreenshot("Live task stopped")
            app.hudScrollToTop()
            app.hudTap("tab-computers")
            app.hudTap("host-0-open")
            app.hudTap("host-terminal")
            XCTAssertTrue(app.waitForHud("terminal-title", containing: "Terminal on", timeout: 20))
            app.hudTap("terminal-close")
            return
        }

        // Activity after another device creates a task.
        app.hudScrollToTop()
        app.hudTap("tab-activity")
        XCTAssertTrue(app.waitForHud("activity-0-headline", containing: "Task queued", timeout: 90))
        attachScreenshot("Live activity")

        // Another device revokes this one.
        app.hudTap("tab-computers")
        XCTAssertTrue(app.waitForHud("host-0-status", containing: "Revoked", timeout: 150))
        attachScreenshot("Live host revoked")
    }

    private func type(_ text: String) {
        app.hudTap("hud-input-type")
        let field = app.descendants(matching: .any)["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText(text)
        app.buttons["computers-submit"].tap()
        let gone = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: field)
        XCTAssertEqual(XCTWaiter.wait(for: [gone], timeout: 10), .completed)
    }

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
