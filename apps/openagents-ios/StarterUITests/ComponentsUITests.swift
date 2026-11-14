// Firm reason: 6 (XCTest exercises the actual embedded Metal view and UIKit lifecycle).
import XCTest

final class ComponentsUITests:XCTestCase {
    func testSettingsOpensMetalGallery() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13567","-OAUpdatesOrigin","http://127.0.0.1:9"];app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:8) {
            email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
            let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        }
        XCTAssertTrue(app.textViews["chat.composer"].waitForExistence(timeout:20))
        app.showAppSettings();let components=app.buttons["app.components"];XCTAssertTrue(components.waitForExistence(timeout:10));components.tap()
        XCTAssertTrue(app.navigationBars["Components"].waitForExistence(timeout:20))
        XCTAssertTrue(app.otherElements["Coax GPU component playground"].exists)
        app.navigationBars["Components"].buttons.firstMatch.tap()
        XCTAssertTrue(app.buttons["nav.settings.close"].waitForExistence(timeout:10));app.buttons["nav.settings.close"].tap()
        XCTAssertTrue(app.textViews["chat.composer"].waitForExistence(timeout:10))
    }
    func testEmbeddedGallerySectionsMotionAndLifecycle() {
        let app=XCUIApplication();app.launchArguments=["-OAComponents","YES"];app.launch()
        XCTAssertTrue(app.navigationBars["Components"].waitForExistence(timeout:30))
        let canvas=app.otherElements["Coax GPU component playground"]
        XCTAssertTrue(canvas.waitForExistence(timeout:30))
        func picture(_ name:String) {let a=XCTAttachment(screenshot:app.screenshot());a.name=name;a.lifetime = .keepAlways;add(a)}
        picture("native-motion")
        func pick(_ name:String) {app.buttons["Gallery options"].tap();app.buttons[name].firstMatch.tap()}
        pick("Controls");canvas.swipeUp();picture("native-controls")
        pick("ARWES");canvas.swipeUp();picture("native-arwes")
        pick("Motion");canvas.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.4)).tap()
        XCUIDevice.shared.press(.home);app.activate()
        XCTAssertTrue(canvas.waitForExistence(timeout:10));picture("native-resumed")
    }
}
