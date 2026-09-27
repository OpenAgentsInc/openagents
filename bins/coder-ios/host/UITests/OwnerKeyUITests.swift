// Enter owner key on a phone, over the Rust synthetic fixture. The fixture
// contacts no host or relay; its owner key is a public test value, not a
// credential. This checks the masked native field and the Rust answer shown
// in the world computer's HUD.
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
        app.openWorldComputerHud()
    }

    func testOwnerKeyIsMaskedAndAcceptedOnlyWhenAGrantNamesIt() throws {
        app.hudTap("first-run-continue")
        XCTAssertTrue(app.waitForHud("computers-title", containing: "Computers", timeout: 10))
        app.hudTap("directory-owner-key")
        app.hudTap("hud-input-type")

        // A secret request is a secure field, never a plain text field.
        let field = app.secureTextFields["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        XCTAssertFalse(app.textFields["computers-input"].exists)
        let canvas = app.otherElements["verse-surface"].frame
        field.tap()
        field.typeText(otherKey)
        assertKeyboardKeepsWorldBounds(canvas)
        XCTAssertNotEqual(field.value as? String, otherKey, "The typed key is masked.")
        attachScreenshot("Owner key masked")
        submitFromKeyboard()
        XCTAssertTrue(app.waitForHud("notice", containing: "isn't the owner key", timeout: 10))
        XCTAssertFalse(anyText(containing: otherKey), "A refused key is never echoed.")

        XCTAssertTrue(app.hudElement("directory-owner-key").isEnabled,
                      "A refused key does not authorize the owner directory.")
        // The refused request stays open; the keyboard starts empty again.
        app.hudTap("hud-input-type")
        let again = app.secureTextFields["computers-input"]
        XCTAssertTrue(again.waitForExistence(timeout: 10))
        let empty = again.value as? String
        XCTAssertTrue(empty == "" || empty == again.placeholderValue, "Submission clears the key draft.")
        again.tap()
        again.typeText(ownerKey)
        assertKeyboardKeepsWorldBounds(canvas)
        submitFromKeyboard()
        XCTAssertTrue(app.waitForHud("notice", containing: "now holds your owner key", timeout: 10))
        app.hudElement("directory-status")
        XCTAssertTrue(app.waitForHud("directory-status", containing: "Your directory is empty", timeout: 10))
        XCTAssertFalse(app.descendants(matching: .any)["directory-owner-key"].exists)
        XCTAssertFalse(app.secureTextFields["computers-input"].exists)
        XCTAssertFalse(anyText(containing: ownerKey), "The accepted key is never echoed.")
        attachScreenshot("Owner key accepted")
    }

    private func assertKeyboardKeepsWorldBounds(_ expected: CGRect) {
        XCTAssertTrue(app.keyboards.firstMatch.exists)
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        let actual = app.otherElements["verse-surface"].frame
        XCTAssertEqual(actual.minX, expected.minX, accuracy: 0.5)
        XCTAssertEqual(actual.minY, expected.minY, accuracy: 0.5)
        XCTAssertEqual(actual.width, expected.width, accuracy: 0.5)
        XCTAssertEqual(actual.height, expected.height, accuracy: 0.5)
    }

    private func submitFromKeyboard() {
        let done = app.keyboards.buttons["Done"]
        XCTAssertTrue(done.waitForExistence(timeout: 10))
        done.tap()
        let dismissed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"),
                                                  object: app.keyboards.firstMatch)
        XCTAssertEqual(XCTWaiter.wait(for: [dismissed], timeout: 10), .completed)
    }

    private func anyText(containing value: String) -> Bool {
        let predicate = NSPredicate(format: "label CONTAINS %@ OR value CONTAINS %@", value, value)
        return app.descendants(matching: .any).matching(predicate).count > 0
    }

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
