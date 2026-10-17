import XCTest

/// Captures every screen of the current app for Design Lab
/// (`crates/design-lab`), on the offline demo so nothing signs in. Each shot
/// is `<area>--<screen>.png` in the folder in `DESIGN_LAB_DIR` (pass it as
/// `TEST_RUNNER_DESIGN_LAB_DIR`), or `<area>--<screen>@dark.png` when
/// `DESIGN_LAB_APPEARANCE` is `dark`; `scripts/design-lab-capture.sh` runs
/// this once per appearance and files the folder under the commit it was
/// taken from.
final class DesignLabTourTests: XCTestCase {
    override func setUp() {
        // One missing screen shouldn't lose the rest of the tour.
        continueAfterFailure = true
    }

    private func launch(_ args: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = args
        app.launch()
        return app
    }

    private func shot(_ app: XCUIApplication, _ area: String, _ screen: String) {
        // Let transitions and widgets settle.
        sleep(1)
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = "\(area)--\(screen)"
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let dir = ProcessInfo.processInfo.environment["DESIGN_LAB_DIR"], !dir.isEmpty else { return }
        let folder = URL(fileURLWithPath: dir, isDirectory: true)
        let dark = ProcessInfo.processInfo.environment["DESIGN_LAB_APPEARANCE"] == "dark"
        let file = "\(area)--\(screen)\(dark ? "@dark" : "").png"
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? screenshot.pngRepresentation.write(to: folder.appendingPathComponent(file))
    }

    func testSignedOut() {
        var app = launch(["-demo", "-fast", "-route", "welcome"])
        _ = app.staticTexts["cloud-chat-empty"].waitForExistence(timeout: 15)
        shot(app, "welcome", "first-launch")
        app.terminate()

        app = launch(["-signedout", "-no-auth-browser", "-account-fixture"])
        _ = app.buttons["account-sign-in"].waitForExistence(timeout: 15)
        shot(app, "welcome", "sign-in")
        if app.buttons["account-sign-in"].exists {
            app.buttons["account-sign-in"].tap()
            shot(app, "welcome", "sign-in-sheet")
        }
    }

    func testShell() {
        let app = launch(["-demo", "-fast"])
        _ = app.buttons["home-sessions"].waitForExistence(timeout: 15)
        shot(app, "home", "home")

        if app.openDrawer() {
            _ = app.cells["section-pinned"].waitForExistence(timeout: 5)
            shot(app, "drawer", "sessions")
            app.collectionViews.firstMatch.swipeUp()
            shot(app, "drawer", "sessions-scrolled")
        }

        for (route, area, screen) in [
            ("new", "new-session", "new-session"),
            ("more", "settings", "settings"),
            ("search", "drawer", "search"),
        ] {
            let page = launch(["-demo", "-fast", "-route", route])
            _ = page.buttons["drawer-menu"].waitForExistence(timeout: 15)
            sleep(1)
            shot(page, area, screen)
            page.terminate()
        }
    }

    func testCloudChat() {
        let app = launch(["-demo", "-fast", "-route", "cloud"])
        _ = app.otherElements["cloud-chat"].waitForExistence(timeout: 15)
        shot(app, "chat", "cloud-chat")
        let input = app.textViews["composer-input"]
        if input.waitForExistence(timeout: 5) {
            input.tap()
            if app.buttons["Continue"].waitForExistence(timeout: 1) { app.buttons["Continue"].tap() }
            shot(app, "chat", "composer-keyboard")
        }
    }

    /// Every demo session's transcript.
    func testSessions() {
        for id in [
            "chat-veil", "chat-picker", "chat-tabs", "chat-errored", "chat-deploy", "chat-ios-scroll",
            "chat-ios-keyboard", "chat-home", "chat-visuals", "chat-cjk", "chat-blog", "chat-side",
            "chat-oklch", "chat-presence",
        ] {
            let app = launch(["-demo", "-fast", "-route", "chat:\(id)"])
            _ = app.scrollViews["transcript"].waitForExistence(timeout: 15)
            shot(app, "session", String(id.dropFirst("chat-".count)))
            app.terminate()
        }
    }

    func testVoice() {
        let app = launch(["-demo", "-fast", "-voice-preview", "-voice-stage"])
        _ = app.otherElements["voice-stage"].waitForExistence(timeout: 15)
        shot(app, "voice", "voice-stage")
    }
}
