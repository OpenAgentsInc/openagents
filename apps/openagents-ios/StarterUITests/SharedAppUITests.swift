import XCTest

/// The book's chapter 6 independent check (#13516): the login case of openagents/examples/cases/login.ops, done
/// by hand through XCUITest and the accessibility tree, with no coax drive. The app opens the shared app with
/// `-OASharedApp YES`, so its fake services answer on their own half a second after each request. What the
/// case's trace says after login (the route's shell and `y...@example.com`) must be what a person sees here.
final class SharedAppUITests: XCTestCase {
    func testLoginByHandMatchesTheLoginCase() {
        let app = XCUIApplication(); app.launchArguments = ["-OASharedApp", "YES"]; app.launch()
        let login = app.buttons["home.login"]
        XCTAssertTrue(login.waitForExistence(timeout: 20)); login.tap()
        let email = app.textFields["account.email"]
        XCTAssertTrue(email.waitForExistence(timeout: 5)); email.tap(); email.typeText("you@example.com")
        app.buttons["account.submit"].tap()
        let code = app.textFields["account.code"]
        XCTAssertTrue(code.waitForExistence(timeout: 5)); code.tap(); code.typeText("123456")
        app.buttons["account.verify"].tap()
        let who = app.staticTexts["shell.who"]
        XCTAssertTrue(who.waitForExistence(timeout: 5))
        XCTAssertEqual(who.label, "y...@example.com")
        XCTAssertTrue(app.buttons["shell.new"].exists && app.buttons["shell.logout"].exists)
        // the refusal: a wrong code keeps the login screen and says why
        app.buttons["shell.logout"].tap()
        XCTAssertTrue(app.buttons["home.login"].waitForExistence(timeout: 5)); app.buttons["home.login"].tap()
        XCTAssertTrue(email.waitForExistence(timeout: 5)); email.tap(); email.typeText("you@example.com")
        app.buttons["account.submit"].tap()
        XCTAssertTrue(code.waitForExistence(timeout: 5)); code.tap(); code.typeText("000000")
        app.buttons["account.verify"].tap()
        XCTAssertTrue(app.staticTexts["That code isn't right."].waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["shell.who"].exists)
    }
}
