import XCTest

/// One-tap sign-in (#11980): the sign-in screen opened from the signed-out
/// chat offers Sign in with Apple beside the code flow. Points the account
/// at a closed local port so no server is reached; Apple's own sheet and the
/// web approval sheet need a device and are checked there.
final class SignInSheetTests: XCTestCase {
    func testSignInOffersSignInWithApple() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-signedout", "-no-auth-browser",
                               "-account-api", "http://127.0.0.1:9/v1", "-account-site", "http://127.0.0.1:9"]
        app.launch()
        let signIn = app.buttons["account-sign-in"]
        XCTAssertTrue(signIn.waitForExistence(timeout: 10))
        signIn.tap()
        XCTAssertTrue(app.buttons["account-sign-in-apple"].waitForExistence(timeout: 10))
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "sign-in-with-apple"
        shot.lifetime = .keepAlways
        add(shot)
        if let dir = ProcessInfo.processInfo.environment["SIGN_IN_SHOTS"] {
            try? app.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("sign-in.png"))
        }
    }
}
