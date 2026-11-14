// Firm reason: 6 (installed UIKit controls and code entry against the independent HTTP oracle).
import XCTest
final class UsageUITests:XCTestCase {
    func testUsageAndSeparateAccountLogin() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13493","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let email=app.textFields["Email address"]
        if !email.waitForExistence(timeout:8) {app.showAppSettings();app.buttons["account.logout"].tap()}
        if email.waitForExistence(timeout:8) {email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap();let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()}
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:20));app.showAppSettings();app.buttons["usage.open"].tap()
        XCTAssertTrue(app.staticTexts["65% remaining"].firstMatch.waitForExistence(timeout:15));XCTAssertTrue(app.staticTexts["Mac"].firstMatch.exists)
        app.buttons["usage.add.0"].tap();let label=app.textFields["usage.label"];XCTAssertTrue(label.waitForExistence(timeout:5));label.tap();label.typeText("Work");app.buttons["usage.begin"].tap()
        let code=app.textFields["usage.code"];XCTAssertTrue(code.waitForExistence(timeout:15));code.tap();code.typeText("fixture-code");app.buttons["usage.complete"].tap()
        let added=app.staticTexts["work@example.com"].firstMatch;XCTAssertTrue(added.waitForExistence(timeout:15))
        let select=app.buttons["usage.account.0.1.select"];if !select.isHittable {app.swipeUp()};XCTAssertTrue(select.waitForExistence(timeout:5));select.tap()
        XCTAssertTrue(app.buttons["usage.close"].exists);XCTAssertEqual(app.webViews.count,0)
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Usage with isolated Claude accounts";shot.lifetime = .keepAlways;add(shot)
    }
}
