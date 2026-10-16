import XCTest

/// Native list, paging, full text and file controls against an isolated account.
final class OpenAgentsHistoryTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func expandSavedChats(_ app: XCUIApplication) {
        let sessions = app.tabBars.buttons["Sessions"]
        XCTAssertTrue(sessions.waitForExistence(timeout: 10))
        sessions.tap()
        let header = app.cells["section-oa-saved-history"]
        XCTAssertTrue(header.waitForExistence(timeout: 20))
        if header.value as? String == "Collapsed" { header.tap() }
    }
    func testSavedAccountHistory() throws {
        let env = ProcessInfo.processInfo.environment
        guard let base = env["CODER_HISTORY_FIXTURE"] else { throw XCTSkip("Run scripts/ios/check-history.py") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-api", base + "/v1", "-account-site", base,
                               "-account-test-credential-url", base + "/fixture-key", "-edge", base]
        app.launch()
        expandSavedChats(app)
        func cell(_ id: String) -> XCUIElement { app.descendants(matching: .any).matching(identifier: id).firstMatch }
        func tap(_ id: String) {
            let element = cell(id)
            XCTAssertTrue(element.waitForExistence(timeout: 20), id)
            if !element.isHittable { app.swipeUp() }
            element.tap()
        }
        tap("saved-more")
        // Pinned chat 204 arrives on the second page and sorts above the other chats.
        let pinned = cell("saved-chat-000000cc-1234-4234-8234-123456789abc")
        XCTAssertTrue(pinned.waitForExistence(timeout: 20))
        app.swipeDown()
        pinned.tap()
        let transcript = app.scrollViews["transcript"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 15))
        XCTAssertTrue(transcript.waitForValue("200 messages loaded", timeout: 15))
        tap("saved-messages-more")
        XCTAssertTrue(transcript.waitForValue("400 messages loaded", timeout: 15))
        tap("saved-messages-more")
        XCTAssertTrue(transcript.waitForValue("405 messages loaded", timeout: 15))
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.04)).tap()
        let question = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Saved question")).firstMatch
        for _ in 0..<40 where !question.exists { transcript.swipeDown(velocity: .fast) }
        XCTAssertTrue(question.waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["new-session"].exists)
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "normal-account-transcript"; shot.lifetime = .keepAlways; add(shot)
        app.navigationBars.buttons["saved-chat-files"].tap()
        app.buttons["fixture.txt"].tap()
        let contents = app.textViews["saved-message-body"]
        XCTAssertTrue(contents.waitForExistence(timeout: 15))
        XCTAssertTrue((contents.value as? String)?.contains("fixture file contents") == true)
        // Close the saved view, show archives and shared projects, then sign out.
        for _ in 0..<2 { app.navigationBars.buttons.element(boundBy: 0).tap() }
        tap("saved-archived")
        XCTAssertTrue(pinned.waitForNonExistence(timeout: 10))
        tap("saved-projects")
        XCTAssertTrue(app.staticTexts["Shared project"].waitForExistence(timeout: 10))
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
        XCTAssertFalse(pinned.exists)
    }
    func testSharedProjectsWithoutComputer() throws {
        guard let base = ProcessInfo.processInfo.environment["CODER_HISTORY_FIXTURE"] else { throw XCTSkip("Local fixture only") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-api", base + "/v1", "-account-site", base,
                               "-account-test-credential-url", base + "/fixture-key", "-edge", base]
        app.launch()
        let projects = app.cells["section-oa-shared-projects"]
        XCTAssertTrue(projects.waitForExistence(timeout: 20))
        if projects.value as? String == "Collapsed" { projects.tap() }
        let shared = app.cells["home-project-prj_0000000000000001"]
        let empty = app.cells["home-project-prj_0000000000000002"]
        XCTAssertTrue(shared.waitForExistence(timeout: 20))
        XCTAssertTrue(empty.exists)
        XCTAssertTrue(app.cells["saved-invitations"].exists)
        let home = XCTAttachment(screenshot: app.screenshot())
        home.name = "shared-projects-on-home-without-computer"
        home.lifetime = .keepAlways
        add(home)
        let chats = app.cells["section-oa-saved-history"]
        XCTAssertTrue(chats.waitForExistence(timeout: 10))
        if chats.value as? String != "Collapsed" { chats.tap() }
        empty.tap()
        XCTAssertTrue(app.cells["saved-notice"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["No chats here yet."].waitForExistence(timeout: 10))
        XCUIDevice.shared.press(.home)
        let joined = expectation(description: "Accept the local fixture invitation")
        URLSession.shared.dataTask(with: URL(string: base + "/join-project")!) { _, _, _ in joined.fulfill() }.resume()
        wait(for: [joined], timeout: 5)
        app.activate()
        XCTAssertTrue(app.cells["home-project-prj_0000000000000003"].waitForExistence(timeout: 15))
        shared.tap()
        let chat = app.cells["saved-chat-00000001-1234-4234-8234-123456789abc"]
        XCTAssertTrue(chat.waitForExistence(timeout: 15))
        chat.tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForExistence(timeout: 15))
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "shared-project-chat-without-computer"
        shot.lifetime = .keepAlways
        add(shot)
    }
    func testRichAccountHistory() throws {
        guard let base = ProcessInfo.processInfo.environment["CODER_HISTORY_FIXTURE"] else { throw XCTSkip("Local fixture only") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-api", base + "/v1", "-account-site", base,
                               "-account-test-credential-url", base + "/fixture-key", "-edge", base]
        app.launch()
        expandSavedChats(app)
        let row = app.cells["saved-chat-000000c6-1234-4234-8234-123456789abc"]
        XCTAssertTrue(row.waitForExistence(timeout: 20))
        row.tap()
        let transcript = app.scrollViews["transcript"]
        XCTAssertTrue(transcript.waitForValue("5 messages loaded", timeout: 15))
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Saved question")).firstMatch.waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["new-session"].exists)
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "rich-account-transcript"; shot.lifetime = .keepAlways; add(shot)
    }

    func testPublicCloudChat() throws {
        let env = ProcessInfo.processInfo.environment
        guard let raw = env["CODER_PUBLIC_CHAT_CONFIG"], let bytes = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: bytes) as? [String: String],
              let keyURL = config["key_url"], let chat = config["chat"], let prompt = config["prompt"],
              let stopPrompt = config["stop_prompt"],
              let before = config["messages"].flatMap(Int.init) else { throw XCTSkip("Dedicated-account check only") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-test-credential-url", keyURL, "-edge", "https://coder-sync-157437760789.us-central1.run.app"]
        app.launch()
        expandSavedChats(app)
        let row = app.descendants(matching: .any).matching(identifier: "saved-chat-" + chat).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 30)); row.tap()
        let input = app.textViews["composer-input"]
        XCTAssertTrue(input.waitForExistence(timeout: 20)); input.tap(); input.typeText(prompt)
        app.buttons["composer-send"].tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("\(before + 2) messages loaded", timeout: 90))
        let finished = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", "Send message"), object: app.buttons["composer-send"])
        XCTAssertEqual(XCTWaiter.wait(for: [finished], timeout: 90), .completed)
        app.swipeDown()
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "public-cloud-reply"; shot.lifetime = .keepAlways; add(shot)
        func signal(_ path: String) {
            let done = expectation(description: path)
            let url = URL(string: keyURL)!.deletingLastPathComponent().appendingPathComponent(path)
            URLSession.shared.dataTask(with: url) { _, _, _ in done.fulfill() }.resume()
            wait(for: [done], timeout: 5)
        }
        signal("reply-ready")
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("\(before + 4) messages loaded", timeout: 90))
        let webShot = XCTAttachment(screenshot: app.screenshot()); webShot.name = "public-cloud-from-web"; webShot.lifetime = .keepAlways; add(webShot)
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", "Send message"), object: app.buttons["composer-send"])
        XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 90), .completed)
        input.tap(); input.typeText(stopPrompt)
        app.buttons["composer-send"].tap()
        let accepted = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == nil OR value == %@ OR value == %@", "", "Message"), object: input)
        XCTAssertEqual(XCTWaiter.wait(for: [accepted], timeout: 15), .completed)
        XCTAssertEqual(app.buttons["composer-send"].label, "Stop response")
        app.buttons["composer-send"].tap()
        let stopped = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", "Send message"), object: app.buttons["composer-send"])
        XCTAssertEqual(XCTWaiter.wait(for: [stopped], timeout: 30), .completed)
        signal("web-seen")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
    }

    func testPublicSavedAccountHistory() throws {
        let env = ProcessInfo.processInfo.environment
        guard let raw = env["CODER_PUBLIC_HISTORY_CONFIG"], let bytes = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: bytes) as? [String: String],
              let keyURL = config["key_url"], let chat = config["chat"] else { throw XCTSkip("Dedicated-account check only") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-test-credential-url", keyURL, "-edge", "https://coder-sync-157437760789.us-central1.run.app"]
        app.launch()
        expandSavedChats(app)
        let row = app.descendants(matching: .any).matching(identifier: "saved-chat-" + chat).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 30)); row.tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("200 messages loaded", timeout: 20))
        app.descendants(matching: .any).matching(identifier: "saved-messages-more").firstMatch.tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("205 messages loaded", timeout: 20))
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "public-account-history"; shot.lifetime = .keepAlways; add(shot)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
    }

    func testLiveAccountHistory() throws {
        guard let base = ProcessInfo.processInfo.environment["CODER_HISTORY_FIXTURE"] else { throw XCTSkip("Local fixture only") }
        func change(_ path: String) {
            let done = expectation(description: path)
            URLSession.shared.dataTask(with: URL(string: base + path)!) { _, _, _ in done.fulfill() }.resume()
            wait(for: [done], timeout: 5)
        }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-api", base + "/v1", "-account-site", base,
                               "-account-test-credential-url", base + "/fixture-key", "-edge", base]
        app.launch()
        expandSavedChats(app)
        let row = app.descendants(matching: .any).matching(identifier: "saved-chat-000000c7-1234-4234-8234-123456789abc").firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 20)); row.tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("2 messages loaded", timeout: 10))
        change("/advance")
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("4 messages loaded", timeout: 12))
        change("/offline")
        let offline = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Couldn't update this chat")).firstMatch
        XCTAssertTrue(offline.waitForExistence(timeout: 12))
        XCTAssertTrue(app.scrollViews["transcript"].value as? String == "4 messages loaded")
        change("/online")
        XCTAssertTrue(offline.waitForNonExistence(timeout: 12))
        XCUIDevice.shared.press(.home)
        change("/advance")
        app.activate()
        XCTAssertTrue(app.scrollViews["transcript"].waitForValue("6 messages loaded", timeout: 12))
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "live-account-history"; shot.lifetime = .keepAlways; add(shot)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
    }

    /// Captures the real account reader while the other applications change the same chat.
    func testRecordAccountChat() throws {
        guard let raw = ProcessInfo.processInfo.environment["CODER_FILM_CONFIG"], let bytes = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: bytes) as? [String: String],
              let keyURL = config["key_url"], let chat = config["chat"], let control = config["control"] else { throw XCTSkip("Film coordinator only") }
        func exchange(_ path: String, _ body: [String: Any]? = nil) -> [String: Any] {
            let done = expectation(description: path)
            var result: [String: Any] = [:]
            var request = URLRequest(url: URL(string: control + path)!)
            if let body { request.httpMethod = "POST"; request.httpBody = try? JSONSerialization.data(withJSONObject: body); request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
            URLSession.shared.dataTask(with: request) { data, _, _ in
                if let data, let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] { result = value }
                done.fulfill()
            }.resume()
            wait(for: [done], timeout: 8)
            return result
        }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-test-credential-url", keyURL, "-edge", "https://coder-sync-157437760789.us-central1.run.app", "-appearance", "2"]
        app.launch()
        expandSavedChats(app)
        let row = app.descendants(matching: .any).matching(identifier: "saved-chat-" + chat).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 30)); row.tap()
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "saved-message-count").firstMatch.waitForExistence(timeout: 15))
        _ = exchange("/phone-ready")
        let deadline = Date().addingTimeInterval(180)
        while Date() < deadline {
            let labels = app.staticTexts.allElementsBoundByIndex.map { $0.label }
            _ = exchange("/phone-proof", ["epoch": Date().timeIntervalSince1970, "chat": chat, "count": app.tables["saved-history-page"].value as? String ?? "", "labels": labels])
            if exchange("/status")["stop"] as? Bool == true { break }
            Thread.sleep(forTimeInterval: 0.6)
        }
        XCTAssertLessThan(Date(), deadline)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
    }

}

private extension XCUIElement {
    func waitForValue(_ expected: String, timeout: TimeInterval) -> Bool {
        let predicate = NSPredicate(format: "value == %@", expected)
        return XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: predicate, object: self)], timeout: timeout) == .completed
    }
}
