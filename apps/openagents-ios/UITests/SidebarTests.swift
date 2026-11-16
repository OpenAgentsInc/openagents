// Firm reason: 6 (real UIKit/XCTest finger taps across the system button/GPU boundary).
import XCTest

final class SidebarTests:XCTestCase {
    override func setUpWithError() throws {continueAfterFailure=false}
    @MainActor func testNativeTapOpensAndClosesSidebarAndSettingsLoginBack() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAPIOrigin","http://127.0.0.1:9"]
        app.launch()
        let menu=app.buttons["nav.menu"],settings=app.buttons["drawer.settings"]
        XCTAssertTrue(menu.waitForExistence(timeout:20))
        for _ in 0..<2 {
            menu.tap()
            XCTAssertTrue(settings.waitForExistence(timeout:10))
            XCTAssertEqual(menu.label,"Close sidebar")
            menu.tap()
            XCTAssertTrue(settings.waitForNonExistence(timeout:10))
            XCTAssertEqual(menu.label,"Open sidebar")
        }
        menu.tap();XCTAssertTrue(settings.waitForExistence(timeout:10))
        app.buttons["drawer.projects"].tap()
        XCTAssertTrue(app.buttons["projects.login"].waitForExistence(timeout:10))
        XCTAssertFalse(app.buttons["project.new"].isEnabled)
        menu.tap();XCTAssertTrue(settings.waitForExistence(timeout:10));settings.tap()
        let login=app.buttons["settings.login"]
        XCTAssertTrue(login.waitForExistence(timeout:10))
        login.tap()
        let back=app.buttons["login.back"]
        XCTAssertTrue(back.waitForExistence(timeout:10))
        let email=app.textFields["login.email"]
        XCTAssertTrue(email.waitForExistence(timeout:10));email.tap()
        if let held=email.value as? String {
            email.typeText(String(repeating:XCUIKeyboardKey.rightArrow.rawValue,count:held.count))
            email.typeText(String(repeating:XCUIKeyboardKey.delete.rawValue,count:held.count))
        }
        email.typeText("fixture@example.test")
        XCTAssertEqual(email.value as? String,"fixture@example.test")
        back.tap()
        XCTAssertTrue(login.waitForExistence(timeout:10))
        app.buttons["settings.back"].tap()
        XCTAssertTrue(menu.waitForExistence(timeout:10))
        menu.tap();XCTAssertTrue(settings.waitForExistence(timeout:10))
        let picture=XCTAttachment(screenshot:app.screenshot());picture.name="Sidebar opened by a real UIKit tap";picture.lifetime = .keepAlways;add(picture)
    }
}
