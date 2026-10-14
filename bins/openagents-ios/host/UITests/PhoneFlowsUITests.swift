// The build gate before an external TestFlight build (#11573): the flows a
// tester and the owner use, live on the site, on a fresh install, with a
// screenshot and the screen's text for each step.
//
// 1. A chat with OpenAgents: an answer streams in, and Stop stops one
//    mid-answer.
// 2. A relaunch opens the last chat (#11557).
// 3. A real device-code sign-in: the code is written to `DIR/code.txt` for
//    a signed-in browser (or a script) to approve; the test waits up to ten
//    minutes. This phone's chats stay on the phone.
// 4. The account's chats, newest first with their projects (#11541), and
//    Running.
// 5. Projects (#11558): the list, a new project from a name alone
//    (`OPENAGENTS_UITEST_PROJECT`, default `phone-gate`; not `zz-gate-*`,
//    which the harness gate clears), and a chat
//    started inside it with one question, answered.
// 6. Sign out, ending the session on the site.
//
// `TEST_RUNNER_OPENAGENTS_UITEST_SESSION_FILE=PATH` (simulator only) signs
// in with a saved session instead of a code, and then never signs out.
//
// Live and opt-in: `TEST_RUNNER_OPENAGENTS_UITEST_LIVE=1` and
// `TEST_RUNNER_OPENAGENTS_UITEST_SHOTS=DIR`. On an account people use,
// delete the project and its chat afterwards.
import XCTest

final class PhoneFlowsUITests: XCTestCase {
    private var environment: [String: String] { ProcessInfo.processInfo.environment }
    private var directory: URL? {
        environment["OPENAGENTS_UITEST_SHOTS"].map { URL(fileURLWithPath: $0) }
    }

    private func record(_ app: XCUIApplication, _ name: String) {
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let directory else { return }
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try? screenshot.pngRepresentation.write(to: directory.appendingPathComponent("\(name).png"))
        // One snapshot: the screen may change while a reply streams.
        var words: [String] = []
        var buttons: [String] = []
        func walk(_ element: XCUIElementSnapshot) {
            if element.elementType == .staticText, !element.label.isEmpty { words.append(element.label) }
            if element.elementType == .button { buttons.append("[\(element.identifier)] \(element.label)") }
            element.children.forEach(walk)
        }
        if let snapshot = try? app.snapshot() { walk(snapshot) }
        let text = (words + ["--- buttons"] + buttons).joined(separator: "\n")
        try? text.write(to: directory.appendingPathComponent("\(name).txt"), atomically: true, encoding: .utf8)
    }

    private func open(_ app: XCUIApplication, _ place: String) {
        let menu = app.buttons["shell-menu"].firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 15))
        menu.tap()
        let row = app.buttons[place].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10), place)
        row.tap()
        sleep(2)
    }

    /// A new chat, wherever the app opened: the welcome's Start a chat, a
    /// chat's New chat, or the list's.
    private func newChat(_ app: XCUIApplication) -> XCUIElement {
        let field = app.textViews["Ask OpenAgents"]
        let ways = ["shell-welcome-start", "shell-new-chat", "coder-new"].map { app.buttons[$0].firstMatch }
        let shown = NSPredicate { _, _ in field.exists || ways.contains { $0.exists } }
        wait(for: [expectation(for: shown, evaluatedWith: nil)], timeout: 30)
        if !field.exists { ways.first { $0.exists }?.tap() }
        XCTAssertTrue(field.waitForExistence(timeout: 15), "Start a chat did not open a chat.")
        return field
    }

    /// Tap a field until it has the keyboard, then type.
    private func type(_ app: XCUIApplication, _ field: XCUIElement, _ text: String) {
        sleep(1)
        for _ in 0..<5 {
            field.tap()
            if app.keyboards.firstMatch.waitForExistence(timeout: 3) { break }
        }
        field.typeText(text)
    }

    private func send(_ app: XCUIApplication, _ field: XCUIElement, _ text: String) {
        type(app, field, text)
        app.buttons["Send"].firstMatch.tap()
    }

    override func setUp() {
        continueAfterFailure = false
    }

    func testPhoneFlows() throws {
        try XCTSkipUnless(environment["OPENAGENTS_UITEST_LIVE"] != nil, "Set OPENAGENTS_UITEST_LIVE.")
        let project = environment["OPENAGENTS_UITEST_PROJECT"] ?? "phone-gate"
        // `OPENAGENTS_UITEST_SESSION_FILE`: a saved session to sign in
        // with instead of approving the code (simulator builds only). It is
        // never signed out, since another app shares it.
        let saved = environment["OPENAGENTS_UITEST_SESSION_FILE"]
            .flatMap { try? String(contentsOfFile: $0, encoding: .utf8) }
        func launched() -> XCUIApplication {
            let app = XCUIApplication()
            if let saved { app.launchEnvironment["OPENAGENTS_UITEST_ACCOUNT_SESSION"] = saved }
            app.launch()
            return app
        }
        var app = launched()
        sleep(3)
        record(app, "00-first-launch")

        // 1. Stop mid-answer, then an answer that streams to its end.
        var field = newChat(app)
        send(app, field, "Write a 400-word story about a lighthouse keeper and a storm.")
        let stop = app.buttons["Stop"].firstMatch
        XCTAssertTrue(stop.waitForExistence(timeout: 30), "No Stop while the answer streams.")
        sleep(4)
        record(app, "01-streaming")
        stop.tap()
        sleep(3)
        record(app, "02-stopped")
        func shown() -> String {
            var words: [String] = []
            func walk(_ element: XCUIElementSnapshot) {
                if element.elementType == .staticText { words.append(element.label) }
                element.children.forEach(walk)
            }
            if let snapshot = try? app.snapshot() { walk(snapshot) }
            return words.joined(separator: " ")
        }
        let stopped = shown()
        sleep(6)
        let later = shown()
        XCTAssertEqual(stopped, later, "The answer kept growing after Stop.")
        XCTAssertFalse(app.buttons["Stop"].exists, "Stop still shows after stopping.")

        if app.buttons["shell-new-chat"].exists { app.buttons["shell-new-chat"].tap() }
        field = newChat(app)
        send(app, field, "What is OpenAgents? Two sentences.")
        sleep(3)
        record(app, "03-answer-streaming")
        let done = NSPredicate { _, _ in !app.buttons["Stop"].exists }
        wait(for: [expectation(for: done, evaluatedWith: nil)], timeout: 90)
        sleep(2)
        record(app, "04-answer")

        // 2. A relaunch opens the last chat.
        app.terminate()
        app = launched()
        sleep(5)
        record(app, "05-relaunch-last-chat")
        XCTAssertTrue(
            app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "What is OpenAgents?")).firstMatch.exists,
            "The relaunch did not open the last chat.")
        open(app, "shell-place-settings")
        app.buttons["shell-menu"].firstMatch.tap()
        sleep(2)
        record(app, "06-drawer-recent-chats")
        app.buttons["shell-place-settings"].firstMatch.tap()
        sleep(2)

        // 3. Sign in with a device code another signed-in device approves.
        let accountRow = app.buttons["account-link"].firstMatch
        XCTAssertTrue(accountRow.waitForExistence(timeout: 10))
        accountRow.tap()
        let signIn = app.buttons["link-sign-in"].firstMatch
        if saved == nil {
            XCTAssertTrue(signIn.waitForExistence(timeout: 15))
            record(app, "10-sign-in")
            signIn.tap()
            let code = app.staticTexts["link-code"].firstMatch
            XCTAssertTrue(code.waitForExistence(timeout: 30))
            record(app, "11-sign-in-code")
            if let directory {
                try? code.label.write(to: directory.appendingPathComponent("code.txt"), atomically: true, encoding: .utf8)
            }
        }
        let signedIn = app.staticTexts["link-signed-in"].firstMatch
        XCTAssertTrue(signedIn.waitForExistence(timeout: 600), "The code was not approved.")
        sleep(3)
        let local = app.buttons["link-choice-local"].firstMatch
        if local.waitForExistence(timeout: 10) { local.tap(); sleep(2) }
        record(app, "12-signed-in")

        // 4. The account's chats, then Running.
        app.buttons["link-open-chats"].firstMatch.tap()
        sleep(8)
        record(app, "20-account-chats")
        app.buttons["link-chats-header-back"].firstMatch.tap()
        sleep(2)
        app.buttons["link-open-running"].firstMatch.tap()
        sleep(8)
        record(app, "21-running")
        app.buttons["link-running-header-back"].firstMatch.tap()
        sleep(2)

        // 5. Projects: the list, a new one from a name, a chat inside it.
        let projects = app.buttons["link-open-projects"].firstMatch
        XCTAssertTrue(projects.waitForExistence(timeout: 10))
        sleep(2)
        projects.tap()
        let list = app.buttons["link-projects-header-back"].firstMatch
        if !list.waitForExistence(timeout: 5) { projects.tap() }
        XCTAssertTrue(list.waitForExistence(timeout: 10), "Projects did not open.")
        sleep(5)
        record(app, "30-projects")
        let name = app.textViews.matching(NSPredicate(format: "label == %@", "New project name")).firstMatch
        XCTAssertTrue(name.waitForExistence(timeout: 10))
        type(app, name, project)
        app.buttons["link-project-composer-send"].firstMatch.tap()
        let made = app.staticTexts.matching(NSPredicate(format: "label == %@", "Made \(project).")).firstMatch
        XCTAssertTrue(made.waitForExistence(timeout: 30), "The project was not made.")
        app.swipeDown()
        sleep(1)
        record(app, "31-project-made")
        let row = app.buttons.matching(NSPredicate(format: "label == %@", project)).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 20), "The new project does not list.")
        row.tap()
        sleep(2)
        record(app, "32-project-tapped")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label == %@", "In \(project)")).firstMatch.waitForExistence(timeout: 10))
        record(app, "32-project-new-chat")
        let reply = app.textViews.firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout: 10))
        type(app, reply, "Say hello in five words.")
        app.buttons.matching(NSPredicate(format: "identifier ENDSWITH '-send'")).firstMatch.tap()
        sleep(25)
        app.swipeDown()
        record(app, "33-project-chat-answer")
        if let directory {
            try? shown().write(
                to: directory.appendingPathComponent("project-chat.txt"), atomically: true, encoding: .utf8)
        }

        // 6. Sign out (not a shared saved session).
        guard saved == nil || environment["OPENAGENTS_UITEST_SIGN_OUT"] != nil else { return }
        open(app, "shell-place-settings")
        app.buttons["account-link"].firstMatch.tap()
        let signOut = app.buttons["link-sign-out"].firstMatch
        XCTAssertTrue(signOut.waitForExistence(timeout: 10))
        signOut.tap()
        XCTAssertTrue(app.buttons["link-sign-in"].firstMatch.waitForExistence(timeout: 30))
        record(app, "40-signed-out")
    }
}
