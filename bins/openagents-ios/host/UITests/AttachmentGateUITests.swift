// The attachment gate (build 58): signed in with a saved session, open one
// of the account's web chats, attach a photo from the Photos picker and a
// text file from the Files picker with the paperclip inside the reply
// composer, send a question about both, and wait for the answer to use
// them. Live and opt-in, like AccountGateUITests:
//
// - `TEST_RUNNER_OPENAGENTS_UITEST_LIVE=1`, `TEST_RUNNER_OPENAGENTS_UITEST_SHOTS=DIR`
// - `TEST_RUNNER_OPENAGENTS_UITEST_SESSION_FILE=PATH` (a saved session; never
//   signed out here)
// - `TEST_RUNNER_OPENAGENTS_UITEST_CHAT=ID` (the web chat to reply in)
// - `TEST_RUNNER_OPENAGENTS_UITEST_ORIGIN=https://staging.openagents.com`
//   (a debug Rust library only)
// - `TEST_RUNNER_OPENAGENTS_UITEST_EXPECT=WORD` (a word the answer must
//   hold, from the photo or the file)
//
// The simulator needs a photo in its library (`xcrun simctl addmedia`) and
// a text file named `codeword.txt` in Files > On My iPhone.
import XCTest

final class AttachmentGateUITests: XCTestCase {
    private var environment: [String: String] { ProcessInfo.processInfo.environment }
    private var directory: URL? {
        environment["OPENAGENTS_UITEST_SHOTS"].map { URL(fileURLWithPath: $0) }
    }

    /// `dump: false` while a system picker shows: its elements come and go
    /// under a query.
    private func record(_ app: XCUIApplication, _ name: String, dump: Bool = true) {
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let directory else { return }
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try? screenshot.pngRepresentation.write(to: directory.appendingPathComponent("\(name).png"))
        guard dump else { return }
        let words = app.staticTexts.allElementsBoundByIndex.map(\.label).filter { !$0.isEmpty }
        let buttons = app.buttons.allElementsBoundByIndex.map { "[\($0.identifier)] \($0.label)" }
        let text = (words + ["--- buttons"] + buttons).joined(separator: "\n")
        try? text.write(to: directory.appendingPathComponent("\(name).txt"), atomically: true, encoding: .utf8)
    }

    private func launched(_ extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        if let origin = environment["OPENAGENTS_UITEST_ORIGIN"] {
            app.launchArguments += ["--account-origin", origin]
        }
        app.launchArguments += extra
        if let path = environment["OPENAGENTS_UITEST_SESSION_FILE"],
           let session = try? String(contentsOfFile: path, encoding: .utf8) {
            app.launchEnvironment["OPENAGENTS_UITEST_ACCOUNT_SESSION"] = session
        }
        app.launch()
        return app
    }

    /// A new chat in each look: the composer has no paperclip (the chat
    /// with OpenAgents takes words only), and the cards are gray.
    func testNewChatInEachLook() throws {
        try XCTSkipUnless(environment["OPENAGENTS_UITEST_LIVE"] != nil, "Set OPENAGENTS_UITEST_LIVE.")
        for look in ["dark", "light"] {
            let app = launched(["--appearance", look])
            sleep(4)
            record(app, "00-new-chat-\(look)")
            XCTAssertFalse(app.buttons.matching(NSPredicate(format: "identifier ENDSWITH '-attach'")).firstMatch.exists)
            XCTAssertFalse(app.buttons["coder-attach"].exists)
            app.terminate()
        }
    }

    func testAttachAPhotoAndAFileToAWebChatReply() throws {
        try XCTSkipUnless(environment["OPENAGENTS_UITEST_LIVE"] != nil, "Set OPENAGENTS_UITEST_LIVE.")
        let chat = try XCTUnwrap(environment["OPENAGENTS_UITEST_CHAT"], "Set OPENAGENTS_UITEST_CHAT.")
        let app = launched()
        sleep(3)

        // Settings > your account > your chats > the chat.
        app.buttons["shell-menu"].firstMatch.tap()
        let settings = app.buttons["shell-place-settings"].firstMatch
        XCTAssertTrue(settings.waitForExistence(timeout: 10))
        settings.tap()
        let accountRow = app.buttons["account-link"].firstMatch
        XCTAssertTrue(accountRow.waitForExistence(timeout: 10))
        accountRow.tap()
        let chats = app.buttons["link-open-chats"].firstMatch
        XCTAssertTrue(chats.waitForExistence(timeout: 30))
        chats.tap()
        let row = app.buttons["link-thread-\(chat)"].firstMatch
        for _ in 0..<12 where !(row.exists && row.isHittable) {
            sleep(2)
            if !row.exists || !row.isHittable { app.swipeUp() }
        }
        XCTAssertTrue(row.waitForExistence(timeout: 10), "The chat isn't listed.")
        row.tap()
        sleep(4)
        record(app, "10-web-chat")

        let attach = app.buttons["link-composer-attach"].firstMatch
        XCTAssertTrue(attach.waitForExistence(timeout: 20), "No paperclip in the reply composer.")
        let send = app.buttons["link-composer-send"].firstMatch
        XCTAssertTrue(send.exists)
        // The paperclip leads the field on the send control's line.
        XCTAssertLessThan(abs(attach.frame.midY - send.frame.midY), 4)
        XCTAssertLessThan(attach.frame.maxX, send.frame.minX)

        // A photo from the library.
        attach.tap()
        let library = app.buttons["Photo Library"].firstMatch
        XCTAssertTrue(library.waitForExistence(timeout: 10))
        record(app, "11-attach-choices", dump: false)
        library.tap()
        let photo = app.images.matching(NSPredicate(format: "label BEGINSWITH 'Photo'")).firstMatch
        XCTAssertTrue(photo.waitForExistence(timeout: 30), "The photo picker shows no photo.")
        sleep(1)
        record(app, "12-photo-picker", dump: false)
        // The picker runs in another process: tap where the photo is.
        photo.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(app.buttons["link-photo-0"].firstMatch.waitForExistence(timeout: 30),
                      "The photo didn't join the reply.")

        // A text file from Files.
        app.buttons["link-composer-attach"].firstMatch.tap()
        let file = app.buttons["Choose File"].firstMatch
        XCTAssertTrue(file.waitForExistence(timeout: 10))
        file.tap()
        sleep(3)
        record(app, "13-file-picker", dump: false)
        let named = NSPredicate(format: "label CONTAINS 'codeword'")
        var item = app.descendants(matching: .any).matching(named).firstMatch
        if !item.waitForExistence(timeout: 5) {
            let browse = app.buttons["Browse"].firstMatch
            if browse.exists { browse.tap(); sleep(2) }
            let mine = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'On My iPhone'")).firstMatch
            if mine.waitForExistence(timeout: 5) { mine.tap(); sleep(2) }
            item = app.descendants(matching: .any).matching(named).firstMatch
        }
        XCTAssertTrue(item.waitForExistence(timeout: 15), "Files shows no codeword.txt.")
        item.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        let open = app.buttons["Open"].firstMatch
        if open.waitForExistence(timeout: 3) { open.tap() }
        XCTAssertTrue(app.buttons["link-photo-1"].firstMatch.waitForExistence(timeout: 30),
                      "The file didn't join the reply.")
        sleep(1)
        record(app, "14-attached")

        let field = app.textViews.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText("What two words are in the photo, and what is the code word in the text file? Answer in one line.")
        record(app, "15-typed")
        app.buttons["link-composer-send"].firstMatch.tap()

        let expected = environment["OPENAGENTS_UITEST_EXPECT"] ?? "HERON"
        let answer = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", expected)).firstMatch
        let replied = answer.waitForExistence(timeout: 180)
        record(app, "20-reply")
        XCTAssertTrue(replied, "No answer naming \(expected).")
        XCTAssertFalse(app.buttons["link-photo-0"].exists, "The sent files stayed on the draft.")

        // Running and Settings, signed in.
        app.buttons["shell-menu"].firstMatch.tap()
        let running = app.buttons["shell-place-running"].firstMatch
        if running.waitForExistence(timeout: 10) {
            running.tap()
            sleep(6)
            record(app, "30-running")
            app.buttons["shell-menu"].firstMatch.tap()
        }
        let settingsAgain = app.buttons["shell-place-settings"].firstMatch
        XCTAssertTrue(settingsAgain.waitForExistence(timeout: 10))
        settingsAgain.tap()
        sleep(2)
        record(app, "40-settings")
    }
}
