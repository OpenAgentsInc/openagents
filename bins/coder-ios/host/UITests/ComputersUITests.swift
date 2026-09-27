// The Computers screens over the Rust synthetic fixture. The fixture contacts
// no host or relay; this checks native mounting, activation, and input only.
import XCTest

final class ComputersUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        app.openWorldComputer()
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 20))
    }

    func testComputersStatusesAccessAndInvitationInput() throws {
        app.buttons["computer-settings"].tap()
        app.buttons["computers-toggle"].tap()
        XCTAssertTrue(app.staticTexts["first-run-title"].waitForExistence(timeout: 20))
        attachScreenshot("Computers first run")
        tap("first-run-continue")
        // First run hands back to the existing pairing and chats flow.
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 10))

        app.buttons["computer-settings"].tap()
        app.buttons["computers-toggle"].tap()
        XCTAssertTrue(app.staticTexts["computers-title"].waitForExistence(timeout: 10))
        for (key, words) in [
            ("host-0-status", "Online"), ("host-1-status", "Connecting"),
            ("host-2-status", "Offline"), ("host-3-status", "Out of date"),
            ("host-4-status", "Not enrolled"), ("host-5-status", "Revoked"),
            ("host-6-status", "switched off"),
        ] {
            let status = app.staticTexts[key]
            reveal(status)
            XCTAssertTrue(status.label.contains(words), "\(key): \(status.label)")
        }
        attachScreenshot("Computers statuses")
        scrollToTop()

        tap("host-0-access")
        XCTAssertTrue(app.staticTexts["access-title"].waitForExistence(timeout: 10))
        let revokeSelf = app.buttons["device-0-revoke"]
        reveal(revokeSelf)
        XCTAssertFalse(revokeSelf.isEnabled)
        XCTAssertTrue(app.staticTexts["device-0-revoke-reason"].label.contains("device you're using"))
        attachScreenshot("Computers access")
        scrollToTop()

        tap("tab-computers")
        tap("host-0-switch")
        XCTAssertTrue(app.staticTexts["host-0-status"].waitForLabel(containing: "switched off", timeout: 10))

        tap("tab-add")
        // The shared Phone projection omits SSH instead of offering a
        // disabled control. Invitation enrollment remains available.
        XCTAssertTrue(app.buttons["invite-paste"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["ssh-connect"].exists)
        XCTAssertFalse(app.staticTexts["ssh-title"].exists)
        tap("invite-paste")
        let field = app.descendants(matching: .any)["computers-input"]
        reveal(field)
        field.tap()
        field.typeText("coder-host:simulator")
        tap("computers-submit")
        XCTAssertTrue(app.staticTexts["notice"].waitForLabel(containing: "Added New computer.", timeout: 10))
        attachScreenshot("Computers added by pasted invitation")

        scrollToTop()
        tap("tab-activity")
        XCTAssertTrue(app.staticTexts["activity-0-subject"].waitForLabel(containing: "Build server", timeout: 10))
        attachScreenshot("Computers activity")
    }

    func testOwnerKeyInputIsMaskedAndClearedOnCancel() throws {
        app.buttons["computer-settings"].tap()
        app.buttons["computers-toggle"].tap()
        XCTAssertTrue(app.staticTexts["first-run-title"].waitForExistence(timeout: 20))
        tap("first-run-continue")
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 10))
        app.buttons["computer-settings"].tap()
        app.buttons["computers-toggle"].tap()
        tap("directory-owner-key")

        let field = app.secureTextFields["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        XCTAssertFalse(app.textFields["computers-input"].exists)
        reveal(field)
        field.tap()
        // This deliberately invalid marker is never submitted or persisted.
        let marker = "not-a-real-owner-key"
        field.typeText(marker)
        XCTAssertFalse((field.value as? String ?? "").contains(marker))
        tap("computers-cancel")
        let cleared = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: field)
        XCTAssertEqual(XCTWaiter.wait(for: [cleared], timeout: 10), .completed)

        tap("directory-owner-key")
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        let value = field.value as? String
        XCTAssertTrue(value == "" || value == field.placeholderValue,
                      "Cancelled key text must not return")
        tap("computers-cancel")
    }

    private func tap(_ key: String) {
        let button = app.buttons[key]
        XCTAssertTrue(button.waitForExistence(timeout: 10), key)
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
