import XCTest

/// The book's device checks (chapters 6 and 11; GitHub #13517): what a physical iPhone adds to the cases that
/// `coax drive` plays (openagents/examples/cases). Each test opens the shared app by hand with
/// `-OASharedApp YES`, so its fake services answer on their own half a second after each request, and works
/// through the accessibility tree the way VoiceOver and a person do. They run in the simulator too, but they
/// count for the chapters only from a run on a phone:
///   xcodebuild -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents -configuration Debug \
///     -destination id=UDID -only-testing:OpenAgentsUITests/SharedAppDeviceUITests test
/// Dictation can't be driven from XCUITest; chapter 6 records it as not covered.
final class SharedAppDeviceUITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    private func open(_ more: [String] = []) -> XCUIApplication {
        let app = XCUIApplication(); app.launchArguments = ["-OASharedApp", "YES"] + more; app.launch()
        XCTAssertTrue(app.buttons["home.login"].waitForExistence(timeout: 20))
        return app
    }
    private func logIn(_ app: XCUIApplication, code: String = "123456") {
        app.buttons["home.login"].tap()
        let email = app.textFields["account.email"]
        XCTAssertTrue(email.waitForExistence(timeout: 5)); email.tap(); email.typeText("you@example.com")
        app.buttons["account.submit"].tap()
        let field = app.textFields["account.code"]
        XCTAssertTrue(field.waitForExistence(timeout: 5)); field.tap(); field.typeText(code)
        app.buttons["account.verify"].tap()
    }
    private func newChat(_ app: XCUIApplication) {
        XCTAssertTrue(app.buttons["shell.new"].waitForExistence(timeout: 5)); app.buttons["shell.new"].tap()
        XCTAssertTrue(app.textFields["composer.input"].waitForExistence(timeout: 5))
    }
    /// Every button, field and text on the screen has the label VoiceOver reads, and the audit passes.
    private func checkScreen(_ app: XCUIApplication, _ name: String) throws {
        for kind in [XCUIElement.ElementType.button, .textField, .staticText] {
            for e in app.descendants(matching: kind).allElementsBoundByIndex where !e.identifier.isEmpty {
                XCTAssertFalse(e.label.trimmingCharacters(in: .whitespaces).isEmpty, "\(name): \(e.identifier) has no VoiceOver label")
            }
        }
        let keyboard = app.keyboards.firstMatch
        let system = keyboard.exists ? keyboard.frame.insetBy(dx: 0, dy: -60) : .null   // with its suggestion bar
        try app.performAccessibilityAudit { issue in
            // Two issues are not the shared screen's, and chapter 6 records both: the masked address
            // (y...@example.com), which the audit calls not human-readable though the mask is the point, and
            // the system keyboard's suggestion bar, whose empty slot has no description.
            let masked = issue.element?.identifier == "shell.who" && issue.compactDescription == "Label not human-readable"
            let keys = issue.element.map { $0.identifier.isEmpty && system.contains($0.frame) } ?? false
            print("AUDIT \(name): \(issue.compactDescription) on \(issue.element?.identifier ?? "the screen")\(masked ? " (the masked address)" : keys ? " (the system keyboard)" : "")")
            return masked || keys
        }
    }

    /// The code field is a one-time-code field with the number pad: its keys are digits, and the code typed on
    /// them logs in (the AutoFill suggestion from a text message needs a real message, so it is not checked).
    func testOneTimeCodeFieldTakesTheCodeFromTheNumberPad() {
        let app = open()
        app.buttons["home.login"].tap()
        let email = app.textFields["account.email"]
        XCTAssertTrue(email.waitForExistence(timeout: 5)); email.tap(); email.typeText("you@example.com")
        app.buttons["account.submit"].tap()
        let code = app.textFields["account.code"]
        XCTAssertTrue(code.waitForExistence(timeout: 5)); code.tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue(app.keyboards.keys["1"].exists && app.keyboards.keys["0"].exists, "the code field shows the number pad")
        XCTAssertFalse(app.keyboards.keys["q"].exists || app.keyboards.keys["Q"].exists, "the code field shows no letters")
        for digit in "123456" { app.keyboards.keys[String(digit)].tap() }
        XCTAssertEqual(code.value as? String, "123456")
        app.buttons["account.verify"].tap()
        XCTAssertTrue(app.staticTexts["shell.who"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.staticTexts["shell.who"].label, "y...@example.com")
    }

    /// Japanese through the Japanese keyboard's input method, key by key: romaji becomes marked kana, a
    /// candidate (日本語) or the confirm key commits it, and only the committed text is sent. The app is given
    /// the Japanese (Romaji) keyboard with launch arguments, its own preferences, so the phone's keyboard
    /// settings don't change. (typeText would insert the text past the input method.)
    func testJapaneseThroughTheInputMethodIsSentOnce() {
        let app = open(["-AppleKeyboards", "(\"ja_JP-Romaji@sw=QWERTY-Japanese;hw=Automatic\")", "-AppleKeyboardsExpanded", "1"])
        logIn(app); newChat(app)
        let input = app.textFields["composer.input"]
        input.tap()
        let keyboard = app.keyboards.firstMatch
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        XCTAssertTrue(keyboard.keys["数字"].exists || keyboard.keys["ー"].exists, "the Japanese keyboard is up")
        for c in "nihongo" { keyboard.keys[String(c)].tap() }
        let marked = input.value as? String ?? ""
        XCTAssertTrue(marked.hasPrefix("にほん"), "romaji became kana as it was typed, not \(marked)")
        let candidate = app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", "日本語")).firstMatch
        if candidate.waitForExistence(timeout: 3) && candidate.isHittable { candidate.tap() } else { keyboard.buttons["Return"].tap() }
        let typed = input.value as? String ?? ""
        print("JAPANESE: marked \(marked), committed \(typed)")
        XCTAssertTrue(typed == "にほんご" || typed == "日本語", "the input method made Japanese of nihongo, not \(typed)")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts["You: \(typed)"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["transcript.1"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.staticTexts["transcript.2"].exists, "one message and one reply")
    }

    /// Japanese, Chinese and an emoji typed straight into the composer reach the shared app whole (UTF-8 of
    /// three and four bytes a character) and come back in the transcript as they were typed.
    func testNonLatinTextIsSentWhole() {
        let app = open()
        logIn(app); newChat(app)
        let text = "こんにちは 你好 🙂"
        let input = app.textFields["composer.input"]
        input.tap(); input.typeText(text)
        XCTAssertEqual(input.value as? String, text)
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts["You: \(text)"].waitForExistence(timeout: 5))
    }

    /// The audit, and a VoiceOver label on every control, on each screen of the shared app.
    func testEveryScreenHasVoiceOverLabelsAndPassesTheAudit() throws {
        let app = open()
        try checkScreen(app, "home")
        app.buttons["home.login"].tap()
        XCTAssertTrue(app.textFields["account.email"].waitForExistence(timeout: 5))
        try checkScreen(app, "login, email")
        app.textFields["account.email"].tap(); app.textFields["account.email"].typeText("you@example.com")
        app.buttons["account.submit"].tap()
        XCTAssertTrue(app.textFields["account.code"].waitForExistence(timeout: 5))
        try checkScreen(app, "login, code")
        app.textFields["account.code"].tap(); app.textFields["account.code"].typeText("123456")
        app.buttons["account.verify"].tap()
        XCTAssertTrue(app.buttons["shell.new"].waitForExistence(timeout: 5))
        try checkScreen(app, "chats")
        newChat(app)
        app.textFields["composer.input"].tap(); app.textFields["composer.input"].typeText("hello")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.buttons["reply.copy"].waitForExistence(timeout: 10))
        try checkScreen(app, "chat")
    }

    /// A turn held across the background: the app goes to the Home Screen while a reply is pending and comes
    /// back to the same conversation, where the reply has finished once: one message, one whole reply, no
    /// lost or doubled part and no stop. The fakes answer every two seconds here (-OASharedAppHandSeconds), so
    /// the turn is surely still pending when the app leaves.
    func testATurnHeldInTheBackgroundFinishesOnceOnResume() {
        let app = open(["-OASharedAppHandSeconds", "2"])
        logIn(app); newChat(app)
        let input = app.textFields["composer.input"]
        input.tap(); input.typeText("hello")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.buttons["composer.stop"].exists, "the turn is live when the app leaves")
        XCUIDevice.shared.press(.home)
        XCTAssertTrue(app.wait(for: .runningBackground, timeout: 10) || app.wait(for: .runningBackgroundSuspended, timeout: 10))
        sleep(12)
        app.activate()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        print("RESUMED: the turn is \(app.buttons["composer.stop"].exists ? "still live" : "done")")
        XCTAssertTrue(app.buttons["reply.copy"].waitForExistence(timeout: 20), "the turn finishes after resume")
        XCTAssertEqual(app.staticTexts["transcript.1"].label, "Hello! This is a fixture reply.")
        XCTAssertEqual(app.staticTexts["transcript.0"].label, "You: hello")
        XCTAssertFalse(app.staticTexts["Stopped"].exists, "the turn was not cancelled")
        XCTAssertFalse(app.staticTexts["transcript.2"].exists, "one message and one reply")
    }
}
