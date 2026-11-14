// Firm reason: 6 (XCTest exercises the actual embedded Metal view and UIKit lifecycle).
import XCTest

final class ComponentsUITests:XCTestCase {
    func testEmbeddedGallerySectionsMotionAndLifecycle() {
        let app=XCUIApplication();app.launchArguments=["-OAComponents","YES"];app.launch()
        XCTAssertTrue(app.navigationBars["Components"].waitForExistence(timeout:30))
        let canvas=app.otherElements["Coax GPU component playground"]
        XCTAssertTrue(canvas.waitForExistence(timeout:30))
        func picture(_ name:String) {let a=XCTAttachment(screenshot:app.screenshot());a.name=name;a.lifetime = .keepAlways;add(a)}
        picture("native-motion")
        func pick(_ name:String) {app.buttons["Gallery options"].tap();app.buttons[name].tap()}
        pick("Controls");canvas.swipeUp();picture("native-controls")
        pick("ARWES");canvas.swipeUp();picture("native-arwes")
        pick("Motion");canvas.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.4)).tap()
        XCUIDevice.shared.press(.home);app.activate()
        XCTAssertTrue(canvas.waitForExistence(timeout:10));picture("native-resumed")
    }
}
