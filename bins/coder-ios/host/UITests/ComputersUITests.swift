// The world computer's Computers screens, drawn by Rust in the world HUD,
// over the Rust synthetic fixture. The fixture contacts no host or relay;
// this checks the in-world layout, real touches on the world surface, the
// native keyboard for input requests, and the Chats page behind it.
import XCTest

final class ComputersUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        app.openWorldComputerHud()
    }

    func testHostsOrderWorkSteerStopTerminalAndChatsInTheWorld() throws {
        XCTAssertTrue(app.hudElement("first-run-title").exists)
        XCTAssertFalse(app.buttons["verse-camera-mode"].exists, "The world controls hide while the computer is open.")
        attachScreenshot("Computers first run in the world HUD")
        app.hudTap("first-run-continue")

        // Linked hosts with presence and route.
        XCTAssertTrue(app.waitForHud("computers-title", containing: "Computers", timeout: 20))
        for (key, words) in [
            ("host-0-status", "Online over the local network"), ("host-1-status", "Connecting"),
            ("host-2-status", "Offline"), ("host-3-status", "Out of date"),
        ] {
            XCTAssertTrue(app.hudLabel(key).contains(words), "\(key): \(app.hudLabel(key))")
        }
        app.hudScrollToTop()
        attachScreenshot("Computers statuses in the world HUD")

        // Select a host.
        app.hudTap("host-0-open")
        XCTAssertTrue(app.waitForHud("host-title", containing: "Studio Mac", timeout: 20))
        XCTAssertTrue(app.hudLabel("host-status").contains("Online over the local network"))
        XCTAssertEqual(app.hudLabel("host-workspaces"), "Workspaces: openagents, scratch.")
        XCTAssertTrue(app.hudElement("host-terminal").isEnabled)
        attachScreenshot("Host screen in the world HUD")

        // Order work: a workspace the host lists and a typed prompt.
        app.hudTap("host-order")
        XCTAssertTrue(app.waitForHud("order-title", containing: "Order work on Studio Mac", timeout: 20))
        XCTAssertFalse(app.hudElement("order-submit").isEnabled)
        app.hudTap("order-workspace-1")
        XCTAssertTrue(app.waitForHud("order-workspace-1", containing: "[x] scratch", timeout: 10))
        app.hudTap("order-prompt")
        type("Fix the flaky parser test")
        XCTAssertTrue(app.waitForHud("order-prompt-text", containing: "Fix the flaky parser test", timeout: 10))
        attachScreenshot("Order form in the world HUD")
        app.hudTap("order-submit")
        XCTAssertTrue(app.waitForHud("notice", containing: "Sent \"Fix the flaky parser test\"", timeout: 20))
        attachScreenshot("Task sent, following in Activity")

        // Follow it in Activity, steer it, then stop it.
        let row = try activityRow(headline: "Fix the flaky parser test")
        XCTAssertTrue(app.hudLabel("\(row)-subject").contains("Task, queued"))
        app.hudTap("\(row)-steer")
        type("Only the parser module.")
        XCTAssertTrue(app.waitForHud("notice", containing: "Sent new instructions", timeout: 20))
        XCTAssertTrue(app.waitForHud("\(row)-time", containing: "Revision 2.", timeout: 20))
        app.hudTap("\(row)-cancel")
        XCTAssertTrue(app.hudElement("\(row)-cancel-confirm").exists)
        app.hudTap("\(row)-cancel-yes")
        XCTAssertTrue(app.waitForHud("\(row)-subject", containing: "cancelled", timeout: 20))
        attachScreenshot("Task stopped in Activity")

        // The Terminal entry point opens the terminal page and returns.
        app.hudScrollToTop()
        app.hudTap("tab-computers")
        app.hudTap("host-0-open")
        app.hudTap("host-terminal")
        XCTAssertTrue(app.waitForHud("terminal-title", containing: "Terminal on Studio Mac", timeout: 20))
        XCTAssertEqual(app.verseObservation()?.computer_page, "terminal")
        attachScreenshot("Terminal entry point in the world HUD")
        app.hudTap("terminal-close")
        XCTAssertTrue(app.waitForHud("host-title", containing: "Studio Mac", timeout: 20))

        // Enroll by pasting an invitation.
        app.hudScrollToTop()
        app.hudTap("tab-add")
        XCTAssertFalse(app.descendants(matching: .any)["ssh-connect"].exists)
        app.hudTap("invite-paste")
        app.hudTap("hud-input-type")
        let field = app.descendants(matching: .any)["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText("coder-host:simulator")
        app.buttons["computers-submit"].tap()
        XCTAssertTrue(app.waitForHud("notice", containing: "Added New computer.", timeout: 20))

        // The chat reader stays reachable as the Chats page.
        app.hudTap("hud-tab-chats")
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 20))
        attachScreenshot("Chats page")
        app.buttons["computers-toggle"].tap()
        XCTAssertTrue(app.buttons["hud-close"].waitForExistence(timeout: 10))
        app.hudTap("hud-close")
        let closed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            self.app.verseObservation()?.computer_open == false
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [closed], timeout: 10), .completed)
        XCTAssertTrue(app.buttons["verse-camera-mode"].waitForExistence(timeout: 10))
    }

    func testOwnerKeyInputIsMaskedAndClearedOnCancel() throws {
        app.hudTap("first-run-continue")
        app.hudTap("directory-owner-key")
        app.hudTap("hud-input-type")
        let field = app.secureTextFields["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        XCTAssertFalse(app.textFields["computers-input"].exists)
        field.tap()
        // This deliberately invalid marker is never submitted or persisted.
        let marker = "not-a-real-owner-key"
        field.typeText(marker)
        XCTAssertFalse((field.value as? String ?? "").contains(marker))
        app.buttons["computers-cancel"].tap()
        let cleared = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: field)
        XCTAssertEqual(XCTWaiter.wait(for: [cleared], timeout: 10), .completed)

        app.hudTap("directory-owner-key")
        app.hudTap("hud-input-type")
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        let value = field.value as? String
        XCTAssertTrue(value == "" || value == field.placeholderValue,
                      "Cancelled key text must not return")
        app.buttons["computers-cancel"].tap()
    }

    /// Answer the HUD's current input request with the native keyboard.
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

    /// The key prefix of the Activity row with `headline`, scrolling the
    /// HUD body to find it.
    private func activityRow(headline: String) throws -> String {
        for _ in 0..<16 {
            if let item = app.verseObservation()?.computer_hud.items.first(where: {
                $0.key.hasPrefix("activity-") && $0.key.hasSuffix("-headline") && $0.label == headline
            }) {
                return String(item.key.dropLast("-headline".count))
            }
            if let hud = app.verseObservation()?.computer_hud { app.hudSwipe(1, hud: hud) }
        }
        XCTFail("no Activity row for \(headline)")
        throw MissingRow()
    }

    private struct MissingRow: Error {}

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
