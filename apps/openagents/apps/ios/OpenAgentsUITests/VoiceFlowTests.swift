import XCTest

/// Voice controls around a scripted call (`-voice-preview`: no audio, host or relay).
final class VoiceFlowTests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .phone, "iPhone tab shell")
    }

    private func launch(_ args: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast", "-voice-preview"] + args
        app.launch()
        return app
    }

    /// The live call's way back: the capsule strip on pages without a composer,
    /// the orb in the navigation bar over a chat or session (the capsule hides there).
    private func liveCall(_ app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(
            format: "identifier IN {'voice-live', 'voice-bar'} AND label BEGINSWITH 'Voice call'"
        )).firstMatch
    }

    /// Stage → transcript → back: the live call control reopens the stage.
    func testLiveStripReopensTheStageAfterTheTranscript() {
        let app = launch(["-voice-stage"])
        let stage = app.otherElements["voice-stage"]
        XCTAssertTrue(stage.waitForExistence(timeout: 10))
        app.buttons["voice-transcript"].tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForExistence(timeout: 10))
        XCTAssertFalse(stage.exists)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        let call = liveCall(app)
        XCTAssertTrue(call.waitForExistence(timeout: 5))
        call.tap()
        XCTAssertTrue(stage.waitForExistence(timeout: 5), "the live call control opens the call stage")
    }

    /// Minimizing the stage lands on the chat (the first page since #12009);
    /// its navigation-bar orb brings the stage back.
    func testLiveStripReopensAMinimizedStage() {
        let app = launch(["-voice-stage"])
        let stage = app.otherElements["voice-stage"]
        XCTAssertTrue(stage.waitForExistence(timeout: 10))
        app.buttons["voice-minimize"].tap()
        let call = liveCall(app)
        XCTAssertTrue(call.waitForExistence(timeout: 5))
        sleep(1)
        call.tap()
        XCTAssertTrue(stage.waitForExistence(timeout: 5))
    }

    /// Settings → Voice lists hosts and call tips; the voice is a pop-up menu.
    func testVoiceSettings() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast", "-remote-voice", "-route", "more"]
        app.launch()
        let row = app.cells["settings-voice"]
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        row.tap()
        let style = app.cells["voice-style"]
        XCTAssertTrue(style.waitForExistence(timeout: 5))
        snapshot(app, "voice-settings")
        style.tap()
        let maple = app.buttons["Maple"]
        XCTAssertTrue(maple.waitForExistence(timeout: 5))
        snapshot(app, "voice-settings-menu")
        maple.tap()
        XCTAssertTrue(style.staticTexts["Maple"].waitForExistence(timeout: 5), "the chosen voice shows in the row")
        app.swipeUp()
        snapshot(app, "voice-settings-scrolled")
    }

    /// Settings → Voice shows the call computer's Codex usage section without a
    /// call. The demo has no voice-ready computer, so it says so instead of 0%.
    func testVoiceSettingsShowCodexUsageWithoutACall() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast", "-remote-voice", "-route", "more"]
        app.launch()
        let row = app.cells["settings-voice"]
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        row.tap()
        let status = app.cells["voice-usage-status"]
        XCTAssertTrue(status.waitForExistence(timeout: 5))
        XCTAssertTrue(status.staticTexts["Usage shows for the computer you call"].exists)
        XCTAssertTrue(app.staticTexts["Codex account usage"].exists || app.staticTexts["CODEX ACCOUNT USAGE"].exists)
        XCTAssertFalse(app.staticTexts.containing(NSPredicate(format: "label CONTAINS '% used'")).firstMatch.exists,
                       "no quota is shown without data")
        XCTAssertFalse(app.cells["voice-usage-refresh"].exists, "nothing to refresh without a computer")
        snapshot(app, "voice-settings-usage")
    }

    private func snapshot(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }
}
