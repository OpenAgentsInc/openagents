import XCTest

final class OpenAgentsAccountTests: XCTestCase {
    func testDedicatedAccountSignsInSeesSessionsAndSignsOut() throws {
        let env = ProcessInfo.processInfo.environment
        guard let api = env["CODER_ACCOUNT_API"], let website = env["CODER_ACCOUNT_WEBSITE"], let sync = env["CODER_ACCOUNT_SYNC"] else {
            throw XCTSkip("Run with the local OpenAgents account fixture.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-signedout", "-no-auth-browser", "-account-fixture", "-account-api", api, "-account-site", website, "-edge", sync, "-harness", "mock"]
        app.launch()
        let signIn = app.buttons["account-sign-in"]
        XCTAssertTrue(signIn.waitForExistence(timeout: 10))
        signIn.tap()
        let session = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", "Fixture sync session")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 25), "The signed-in account's sync sessions appear")
        let signedIn = XCTAttachment(screenshot: app.screenshot())
        signedIn.name = "openagents-account-signed-in"
        signedIn.lifetime = .keepAlways
        add(signedIn)
        app.openSettings()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<6 where !signOut.isHittable { app.swipeUp() }
        XCTAssertTrue(signOut.isHittable)
        signOut.tap()
        app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 10))
        XCTAssertFalse(session.exists)
        let signedOut = XCTAttachment(screenshot: app.screenshot())
        signedOut.name = "openagents-account-signed-out"
        signedOut.lifetime = .keepAlways
        add(signedOut)
    }
}
