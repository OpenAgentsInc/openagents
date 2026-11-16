// Firm reason: 6 (installed account screens, native fields and interactive navigation oracle).
import XCTest

final class SettingsUITests:XCTestCase {
    func testArcadeRailAndEveryAccountScreen() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13567","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:5) {email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap();let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()}
        let menu=app.buttons["nav.toggle"];XCTAssertTrue(menu.waitForExistence(timeout:20));menu.tap()
        let projects=app.buttons["nav.projects"];XCTAssertTrue(projects.waitForExistence(timeout:5));XCTAssertGreaterThan(projects.frame.width,200)
        XCTAssertTrue(app.staticTexts["Recents"].exists);capture(app,"Navigation drawer")
        app.buttons["nav.settings"].tap();let profile=app.buttons["settings.profile"];XCTAssertTrue(profile.waitForExistence(timeout:10));waitLoaded(app)
        capture(app,"Account settings")
        profile.tap();app.buttons["settings.edit"].tap()
        let name=app.textFields["settings.profile.0"];XCTAssertTrue(name.waitForExistence(timeout:5));name.tap();let old=name.value as? String ?? "";name.typeText(String(repeating:XCUIKeyboardKey.delete.rawValue,count:old.count)+"Ada Arcade")
        reveal(app,"settings.save");app.buttons["settings.save"].tap();waitLoaded(app);XCTAssertTrue(app.staticTexts["Ada Arcade"].firstMatch.waitForExistence(timeout:10))
        app.buttons["settings.profile"].tap();capture(app,"Native profile");app.buttons["settings.back"].tap()
        app.buttons["settings.relays"].tap();let relay=app.textFields["settings.relay.url"];XCTAssertTrue(relay.waitForExistence(timeout:5));relay.tap();relay.typeText("wss://relay.damus.io");app.buttons["settings.relay.add"].tap();waitLoaded(app)
        app.buttons["settings.relays"].tap();let second=app.buttons["settings.relay.remove.1"];XCTAssertTrue(second.waitForExistence(timeout:5));second.tap();app.buttons["settings.cancel"].tap();XCTAssertTrue(second.exists)
        second.tap();app.buttons["settings.confirm"].tap();waitLoaded(app);app.buttons["settings.privacy"].tap();app.buttons["settings.privacy.images"].tap();capture(app,"Privacy switches");app.buttons["settings.save"].tap();waitLoaded(app)
        app.buttons["settings.notifications"].tap();waitLoaded(app);XCTAssertTrue(app.buttons["settings.notifications.system"].exists);capture(app,"Notification settings");app.buttons["settings.back"].tap();waitLoaded(app)
        app.buttons["settings.keys"].tap();XCTAssertTrue(app.buttons["settings.passkey-add"].exists);capture(app,"Identity and login methods");reveal(app,"settings.import");app.buttons["settings.import"].tap()
        let secret=app.secureTextFields["settings.secret.input"];XCTAssertTrue(secret.waitForExistence(timeout:5));secret.tap();secret.typeText("invalid-key");app.buttons["settings.key-import"].tap();waitLoaded(app);XCTAssertTrue(app.staticTexts["Use a valid nsec or hex private key"].waitForExistence(timeout:5));app.buttons["settings.back"].tap();waitLoaded(app)
        app.buttons["settings.backup"].tap();XCTAssertTrue(app.buttons["settings.backup-open"].exists);capture(app,"Protected identity backup");app.buttons["settings.back"].tap();waitLoaded(app)
        app.buttons["settings.devices"].tap();XCTAssertTrue(app.buttons["settings.device.remove.0"].waitForExistence(timeout:5));capture(app,"Account sessions");app.buttons["settings.back"].tap();waitLoaded(app)
        reveal(app,"settings.changelog");app.buttons["settings.changelog"].tap();XCTAssertTrue(app.staticTexts["1.0.0 · 1088"].exists);capture(app,"OpenAgents changelog")
        app.buttons["nav.settings.close"].tap();XCTAssertTrue(menu.waitForExistence(timeout:5));XCTAssertEqual(app.webViews.count,0)
    }
    private func waitLoaded(_ app:XCUIApplication) {let done=NSPredicate(format:"exists == false");expectation(for:done,evaluatedWith:app.staticTexts["settings.busy"]);waitForExpectations(timeout:20)}
    private func reveal(_ app:XCUIApplication,_ id:String) {app.revealAppButton(id)}
    private func capture(_ app:XCUIApplication,_ name:String) {let shot=XCTAttachment(screenshot:app.screenshot());shot.name=name;shot.lifetime = .keepAlways;add(shot)}
}
