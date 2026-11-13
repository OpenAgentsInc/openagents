import XCTest

final class StarterUITests:XCTestCase {
    func testEmbeddedCoaxAccountModuleRuns() {
        let app=XCUIApplication(); app.launchArguments=["-OAAutomation","YES"]; app.launch()
        XCTAssertTrue(app.webViews.buttons["CHECK FOR UPDATES"].waitForExistence(timeout:20))
        XCTAssertTrue(app.webViews.buttons["BUILD DETAILS"].exists)
        XCTAssertTrue(app.webViews.buttons["MORE"].exists)
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Embedded Coax account surface";shot.lifetime = .keepAlways;add(shot)
    }
    // Select explicitly after publishing the live demonstration package.
    func testLiveSignedCoaxUpdateChangesInstalledApp() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES"];app.launch()
        XCTAssertTrue(app.webViews.staticTexts["Start here."].waitForExistence(timeout:20))
        let apply=app.alerts.buttons["Apply update"]
        XCTAssertTrue(apply.waitForExistence(timeout:90));apply.tap()
        XCTAssertTrue(app.webViews.staticTexts["Updates are live."].waitForExistence(timeout:20))
        app.webViews.buttons["TRY THE STARTER"].tap()
        XCTAssertTrue(app.webViews.staticTexts["1"].waitForExistence(timeout:5))
        let health=expectation(description:"foreground health window")
        DispatchQueue.main.asyncAfter(deadline:.now()+31) {health.fulfill()}
        wait(for:[health],timeout:35)
        app.terminate();app.launch()
        XCTAssertTrue(app.webViews.staticTexts["Updates are live."].waitForExistence(timeout:20))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Live Coax OTA, same native build";shot.lifetime = .keepAlways;add(shot)
    }
}
