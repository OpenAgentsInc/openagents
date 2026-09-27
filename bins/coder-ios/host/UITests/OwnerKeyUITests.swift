// Enter owner key on a phone, over the Rust synthetic fixture. The fixture
// contacts no host or relay; its owner key is a public test value, not a
// credential. This checks the masked native field and the Rust answer.
import XCTest

final class OwnerKeyUITests: XCTestCase {
    private var app: XCUIApplication!
    private let ownerKey = String(repeating: "0e", count: 32)
    private let otherKey = String(repeating: "0f", count: 32)

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        app.openWorldComputer()
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 20))
    }

    func testOwnerKeyIsMaskedAndAcceptedOnlyWhenAGrantNamesIt() throws {
        app.buttons["computers-toggle"].tap()
        tap("first-run-continue")
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 10))
        app.buttons["computers-toggle"].tap()
        XCTAssertTrue(app.staticTexts["computers-title"].waitForExistence(timeout: 10))
        tap("directory-owner-key")

        // A secret request is a secure field, never a plain text field.
        let field = app.secureTextFields["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        XCTAssertFalse(app.textFields["computers-input"].exists)
        reveal(field)
        field.tap()
        field.typeText(otherKey)
        XCTAssertNotEqual(field.value as? String, otherKey, "The typed key is masked.")
        attachScreenshot("Owner key masked")
        tap("computers-submit")
        XCTAssertTrue(app.staticTexts["notice"].waitForLabel(containing: "isn't the owner key", timeout: 10))
        XCTAssertFalse(anyText(containing: otherKey), "A refused key is never echoed.")

        let again = app.secureTextFields["computers-input"]
        XCTAssertTrue(again.waitForExistence(timeout: 10))
        reveal(again)
        again.tap()
        again.typeText(ownerKey)
        tap("computers-submit")
        XCTAssertTrue(app.staticTexts["notice"].waitForLabel(containing: "now holds your owner key", timeout: 10))
        XCTAssertTrue(app.staticTexts["directory-status"].waitForLabel(containing: "Your directory is empty", timeout: 10))
        XCTAssertFalse(app.buttons["directory-owner-key"].exists)
        XCTAssertFalse(app.secureTextFields["computers-input"].exists)
        XCTAssertFalse(anyText(containing: ownerKey), "The accepted key is never echoed.")
        attachScreenshot("Owner key accepted")
    }

    private func anyText(containing value: String) -> Bool {
        let predicate = NSPredicate(format: "label CONTAINS %@ OR value CONTAINS %@", value, value)
        return app.descendants(matching: .any).matching(predicate).count > 0
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
