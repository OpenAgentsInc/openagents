import XCTest

final class StarterUITests:XCTestCase {
    // Select explicitly with a published feed matching this build's resources.
    func testIdenticalLivePackageDoesNotPrompt() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES"];app.launch()
        let check=app.buttons["Check for updates"]
        XCTAssertTrue(check.waitForExistence(timeout:20))
        XCTAssertFalse(app.alerts["Update ready"].waitForExistence(timeout:10))
        check.tap()
        XCTAssertTrue(app.staticTexts["You have the latest compatible update."].waitForExistence(timeout:20))
        XCTAssertFalse(app.alerts["Update ready"].exists)
        app.terminate();app.launch()
        XCTAssertTrue(check.waitForExistence(timeout:20));check.tap()
        XCTAssertTrue(app.staticTexts["You have the latest compatible update."].waitForExistence(timeout:20))
        XCTAssertFalse(app.alerts["Update ready"].exists)
    }
    func testEmbeddedCoaxAccountModuleRuns() {
        let app=XCUIApplication(); app.launchArguments=["-OAAutomation","YES"]; app.launch()
        XCTAssertTrue(app.buttons["Check for updates"].waitForExistence(timeout:20))
        XCTAssertTrue(app.buttons["Build details"].exists)
        XCTAssertTrue(app.buttons["More"].exists)
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Embedded Coax account surface";shot.lifetime = .keepAlways;add(shot)
    }
    // Select explicitly after publishing the live demonstration package.
    func testLiveSignedCoaxUpdateChangesInstalledApp() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES"];app.launch()
        XCTAssertTrue(app.staticTexts["Start here."].waitForExistence(timeout:20))
        let apply=app.alerts.buttons["Apply update"]
        XCTAssertTrue(apply.waitForExistence(timeout:90));apply.tap()
        XCTAssertTrue(app.staticTexts["Updates are live."].waitForExistence(timeout:20))
        app.buttons["TRY THE STARTER"].tap()
        XCTAssertTrue(app.staticTexts["1"].waitForExistence(timeout:5))
        let health=expectation(description:"foreground health window")
        DispatchQueue.main.asyncAfter(deadline:.now()+31) {health.fulfill()}
        wait(for:[health],timeout:35)
        app.terminate();app.launch()
        XCTAssertTrue(app.staticTexts["Updates are live."].waitForExistence(timeout:20))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Live Coax OTA, same native build";shot.lifetime = .keepAlways;add(shot)
    }
}
