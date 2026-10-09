// These tests open only the Rust-generated synthetic reader fixture.
import XCTest

final class ReaderUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        app.openWorldComputer()
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 20))
    }

    func testNativeChatSelectionAndExactSource() throws {
        XCTAssertFalse(app.staticTexts["reader-status"].exists)
        app.buttons["chat-0"].tap()
        XCTAssertTrue(app.buttons["back"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["chat-title"].exists)
        XCTAssertTrue(app.switches["timeline-follow"].exists)

        XCTAssertFalse(app.staticTexts["history-progress"].exists)
        XCTAssertFalse(app.staticTexts["page-position"].exists)
        app.buttons["details"].tap()
        // The renderer exposes the exact bytes as a separate Rust action,
        // independent of its selectable Markdown presentation.
        let source = app.buttons.matching(NSPredicate(format: "identifier ENDSWITH '-raw'")).firstMatch
        reveal(source)
        XCTAssertTrue(source.exists)
        source.tap()
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label == 'Show readable text'")).firstMatch
            .waitForExistence(timeout: 10))
        XCTAssertFalse(app.textFields["Message"].exists)
        XCTAssertFalse(app.buttons["Send"].exists)
        attachScreenshot("Synthetic native transcript and source")

        app.buttons["back"].tap()
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 10))
    }

    func testFollowCanPauseAndResume() throws {
        app.buttons["chat-0"].tap()
        let follow = app.switches["timeline-follow"]
        XCTAssertTrue(follow.waitForExistence(timeout: 10))
        XCTAssertEqual(follow.value as? String, "1")
        follow.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        XCTAssertTrue(follow.waitForValue("0", timeout: 3))
        app.buttons["reader-refresh"].tap()
        XCTAssertEqual(follow.value as? String, "0")
        follow.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        XCTAssertTrue(follow.waitForValue("1", timeout: 3))
        attachScreenshot("Synthetic native follow control")
    }

    func testSyntheticDeviceIdentitySurvivesRelaunch() throws {
        let first = publicKey()
        XCTAssertEqual(first.count, 64)
        XCTAssertTrue(first.allSatisfy { $0.isHexDigit })
        app.terminate()
        app.launch()
        app.openWorldComputer()
        attachScreenshot("Synthetic reader after relaunch")
        XCTAssertEqual(publicKey(), first)
        app.buttons["computer-close"].tap()
        XCTAssertFalse(app.buttons["computer-interact"].exists)
        XCTAssertTrue(app.otherElements["verse-surface"].exists)
    }

    func testPageSelectionPinsUntilLatestIsRequested() throws {
        app.buttons["chat-0"].tap()
        app.buttons["details"].tap()
        let position = app.staticTexts["page-position"]
        XCTAssertTrue(position.waitForExistence(timeout: 10))
        XCTAssertTrue(position.label.contains("Page 2 of 2"))
        app.buttons["earlier"].tap()
        XCTAssertTrue(position.waitForLabel(containing: "Page 1 of 2", timeout: 5))
        XCTAssertTrue(app.switches["timeline-follow"].waitForValue("0", timeout: 3))
        app.buttons["reader-refresh"].tap()
        XCTAssertTrue(position.label.contains("Page 1 of 2"))
        app.buttons["later"].tap()
        XCTAssertTrue(position.waitForLabel(containing: "Page 2 of 2", timeout: 5))
        XCTAssertTrue(app.switches["timeline-follow"].waitForValue("0", timeout: 3))
        app.buttons["follow"].tap()
        XCTAssertTrue(app.switches["timeline-follow"].waitForValue("1", timeout: 3))
        attachScreenshot("Synthetic earlier-page navigation")
    }

    private func publicKey() -> String {
        let settings = app.buttons.matching(identifier: "computer-settings").firstMatch
        XCTAssertTrue(settings.waitForExistence(timeout: 10))
        settings.tap()
        let details = app.buttons.matching(identifier: "Device details").firstMatch
        XCTAssertTrue(details.waitForExistence(timeout: 10))
        details.tap()
        let key = app.staticTexts.matching(identifier: "reader-public-key").firstMatch
        XCTAssertTrue(key.waitForExistence(timeout: 10))
        let loaded = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label MATCHES '[0-9a-fA-F]{64}'"), object: key)
        XCTAssertEqual(XCTWaiter.wait(for: [loaded], timeout: 10), .completed)
        let value = key.label
        settings.tap()
        return value
    }

    private func reveal(_ element: XCUIElement) {
        for _ in 0..<12 where !element.isHittable {
            app.swipeDown()
        }
    }

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}

private extension XCUIElement {
    func waitForValue(_ expected: String, timeout: TimeInterval) -> Bool {
        let match = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", expected),
                                              object: self)
        return XCTWaiter.wait(for: [match], timeout: timeout) == .completed
    }

    func waitForLabel(containing expected: String, timeout: TimeInterval) -> Bool {
        let match = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", expected),
                                              object: self)
        return XCTWaiter.wait(for: [match], timeout: timeout) == .completed
    }
}
