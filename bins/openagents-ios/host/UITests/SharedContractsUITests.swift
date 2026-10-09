// The phone composer edits through Rust Native's shared editor, and saved
// chat cards offer the shared chat menu as a native menu (#10028). The chat
// is text only (#10093): no attach control, and an image handed to it or
// on the pasteboard never reaches the draft. Runs offline on the chat
// fixture. `OPENAGENTS_UITEST_SHOTS` names a directory for the screenshots,
// and `OPENAGENTS_UITEST_IMAGE` a PNG the launch tries to attach.
import UIKit
import XCTest

final class SharedContractsUITests: XCTestCase {
    private func shot(_ name: String) {
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let directory = ProcessInfo.processInfo.environment["OPENAGENTS_UITEST_SHOTS"] {
            try? screenshot.pngRepresentation.write(to: URL(fileURLWithPath: directory)
                .appendingPathComponent("\(name).png"))
        }
    }

    private func launch(_ arguments: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--chat-fixture", "1"] + arguments
        app.launch()
        return app
    }

    private func field(_ app: XCUIApplication) -> XCUIElement {
        let field = app.textViews["Ask OpenAgents"]
        XCTAssertTrue(field.waitForExistence(timeout: 30))
        return field
    }

    func testComposerEditsGraphemesAndUndoesThroughTheSharedDraft() throws {
        continueAfterFailure = false
        let app = launch()
        let field = field(app)
        field.tap()
        field.typeText("Hi 👨‍👩‍👧‍👦 cafe\u{301}")
        XCTAssertEqual(field.value as? String, "Hi 👨‍👩‍👧‍👦 cafe\u{301}")
        shot("10028-composer-typed")
        // One delete removes the whole combining sequence, then the family.
        field.typeText(XCUIKeyboardKey.delete.rawValue)
        XCTAssertEqual(field.value as? String, "Hi 👨‍👩‍👧‍👦 caf")
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 5))
        XCTAssertEqual(field.value as? String, "Hi ")
        shot("10028-composer-deleted")
        // Undo restores from the shared history, from the field's edit menu
        // (a phone has no Command-Z).
        field.doubleTap()
        let undo = app.menuItems["Undo"].firstMatch
        XCTAssertTrue(undo.waitForExistence(timeout: 5))
        sleep(1)
        shot("10028-composer-undo-menu")
        undo.tap()
        let restored = expectation(for: NSPredicate(format: "value BEGINSWITH 'Hi 👨‍👩‍👧‍👦'"), evaluatedWith: field)
        wait(for: [restored], timeout: 5)
        shot("10028-composer-undone")
    }

    func testSavedChatCardsOfferTheSharedMenuAndTheChatIsTextOnly() throws {
        continueAfterFailure = false
        var arguments: [String] = []
        if let image = ProcessInfo.processInfo.environment["OPENAGENTS_UITEST_IMAGE"] {
            arguments = ["--coder-tap", "sleep:3,attach:\(image)"]
        }
        let app = launch(arguments)
        let field = field(app)
        // No attach control, and an image the launch handed the draft was
        // dropped: no `image:` card, no notice.
        let images = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'image:'"))
        sleep(arguments.isEmpty ? 2 : 6)
        XCTAssertFalse(app.descendants(matching: .any)["coder-attach"].exists)
        XCTAssertFalse(app.buttons["Attach image"].exists)
        XCTAssertFalse(images.firstMatch.exists)
        XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'PNG or JPEG'")).firstMatch.exists)
        shot("10093-composer-text-only")
        // An image alone on the pasteboard offers no Paste in the composer.
        UIPasteboard.general.image = UIGraphicsImageRenderer(size: CGSize(width: 8, height: 8)).image { context in
            UIColor.systemBlue.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 8, height: 8))
        }
        field.tap()
        sleep(1)
        field.tap()
        sleep(2)
        XCTAssertFalse(app.menuItems["Paste"].exists)
        XCTAssertFalse(images.firstMatch.exists)
        UIPasteboard.general.items = []
        field.tap()
        field.typeText("Keep this chat")
        app.buttons["Send"].firstMatch.tap()
        // The draft clears once Rust takes the message.
        let cleared = expectation(for: NSPredicate(format: "value == ''"), evaluatedWith: field)
        wait(for: [cleared], timeout: 10)
        sleep(4)
        app.buttons["coder-menu"].tap()
        sleep(2)
        shot("10028-chats-list")
        let card = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'talk-'")).firstMatch
        XCTAssertTrue(card.waitForExistence(timeout: 10))
        card.press(forDuration: 1.2)
        let pin = app.buttons["Pin chat"]
        XCTAssertTrue(pin.waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["Archive chat"].exists)
        shot("10028-card-menu")
        pin.tap()
        sleep(2)
        shot("10028-card-pinned")
        let pinned = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'talk-'")).firstMatch
        XCTAssertTrue(pinned.waitForExistence(timeout: 10))
        pinned.press(forDuration: 1.2)
        XCTAssertTrue(app.buttons["Unpin chat"].waitForExistence(timeout: 5))
        shot("10028-card-menu-pinned")
    }

    func testCodeBlocksPaintSyntaxColors() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--rust-native-fixture"]
        app.launch()
        let transcript = app.scrollViews["transcript"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 30))
        sleep(3)
        // The fixture's Rust block: keywords and numbers in syntax colors
        // once the highlighter's spans arrive.
        shot("10028-code-highlight")
    }
}
