// Firm reason: 6 (installed UIKit app, gestures and accessibility against a local HTTP oracle).
import XCTest

final class NavigationUITests:XCTestCase {
    func testSimpleComposerCenteredProjectsAndWorldsStayInTheWorkspace() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13567","-OAUpdatesOrigin","http://127.0.0.1:9"];app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:5) {email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap();let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()}
        XCTAssertTrue(app.textViews["chat.composer"].waitForExistence(timeout:20))
        XCTAssertFalse(app.buttons["chat.options"].exists);XCTAssertFalse(app.buttons["Clear draft"].exists);XCTAssertFalse(app.buttons["Dictation: use the microphone on the keyboard"].exists)
        attach(app,"Minimal composer")
        app.buttons["nav.toggle"].tap();app.buttons["nav.projects"].tap()
        let title=app.staticTexts["projects.title"];XCTAssertTrue(title.waitForExistence(timeout:10));XCTAssertEqual(title.frame.midX,app.frame.midX,accuracy:2)
        XCTAssertFalse(app.buttons["projects.refresh"].exists);XCTAssertTrue(app.buttons["projects.new"].exists);attach(app,"Centered Projects")
        app.buttons["nav.toggle"].tap();app.buttons["world.library"].tap()
        XCTAssertTrue(app.buttons["worlds.splice"].waitForExistence(timeout:15));XCTAssertTrue(app.buttons["nav.toggle"].exists);XCTAssertFalse(app.buttons["worlds.refresh"].exists);XCTAssertEqual(app.sheets.count,0);attach(app,"Worlds in the workspace")
        app.showChatHistory();XCTAssertTrue(app.textFields["Search chats"].waitForExistence(timeout:10));XCTAssertFalse(app.buttons["nav.refresh"].exists);attach(app,"Single line history")
        app.buttons["nav.home"].tap();XCTAssertTrue(app.textViews["chat.composer"].waitForExistence(timeout:10))
    }
    func testSearchSwitchChatsAndUseComposerFromHistory() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13493","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:8) {
            email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
            let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        }
        XCTAssertTrue(app.textViews["chat.composer"].waitForExistence(timeout:20))
        XCTAssertTrue(app.buttons["home.splice"].exists);XCTAssertTrue(app.buttons["home.everglade"].exists)
        attach(app,"Quiet mobile home")
        app.showChatHistory()
        XCTAssertTrue(app.buttons["nav.projects"].exists);XCTAssertTrue(app.buttons["world.library"].exists)
        app.coordinate(withNormalizedOffset:CGVector(dx:0.96,dy:0.5)).tap()
        XCTAssertTrue(app.buttons["nav.toggle"].isHittable)
        app.showChatHistory();app.buttons["nav.projects"].tap()
        XCTAssertTrue(app.buttons["projects.item.0"].waitForExistence(timeout:10));attach(app,"Account projects")
        app.showChatHistory();app.buttons["nav.home"].tap();app.showChatHistory()
        let search=app.textFields["nav.search"];XCTAssertTrue(search.waitForExistence(timeout:20))
        let mac=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@ AND label == %@","nav.chat.","Transcript fixture")).firstMatch
        let coder=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@ AND label == %@","nav.chat.","CoderOS task")).firstMatch
        XCTAssertTrue(mac.waitForExistence(timeout:20));XCTAssertTrue(coder.waitForExistence(timeout:20))
        attach(app,"History groups and fixed navigation")
        search.tap();search.typeText("coder");XCTAssertTrue(coder.exists);XCTAssertFalse(mac.exists)
        search.tap();search.typeText(String(repeating:XCUIKeyboardKey.delete.rawValue,count:5))
        XCTAssertTrue(mac.waitForExistence(timeout:5));mac.tap()
        let field=app.textViews["env.composer"];XCTAssertTrue(field.waitForExistence(timeout:10));field.tap();field.typeText("Keep my draft")
        app.showChatHistory();XCTAssertTrue(search.waitForExistence(timeout:5));XCTAssertFalse(app.keyboards.firstMatch.exists)
        coder.tap();XCTAssertTrue(field.waitForExistence(timeout:10));XCTAssertEqual(field.value as? String,"")
        app.showChatHistory();mac.tap();XCTAssertTrue(field.waitForExistence(timeout:10));XCTAssertEqual(field.value as? String,"Keep my draft")
        XCTAssertFalse(app.buttons["Your computers"].exists);XCTAssertFalse(app.buttons["Refresh conversation"].exists)
        let read=app.buttons["Read"];XCTAssertTrue(read.waitForExistence(timeout:10));XCTAssertLessThanOrEqual(read.frame.height,32)
        XCTAssertFalse(app.buttons["env.effort"].exists);XCTAssertFalse(app.buttons["env.model"].exists);XCTAssertFalse(app.buttons["env.refresh"].exists)
        attach(app,"Compact conversation and docked model controls")
        app.buttons["nav.toggle"].tap();app.buttons["nav.settings"].tap()
        XCTAssertTrue(app.buttons["app.updates"].waitForExistence(timeout:5));app.buttons["nav.settings.close"].tap()
        let account=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@ AND label == %@","nav.account.","Account fixture")).firstMatch
        app.showChatHistory();XCTAssertTrue(account.waitForExistence(timeout:10));account.tap();XCTAssertTrue(app.textViews["chat.composer"].waitForExistence(timeout:10))
        XCTAssertEqual(app.webViews.count,0);attach(app,"Saved account conversation")
    }
    private func attach(_ app:XCUIApplication,_ name:String) {let a=XCTAttachment(screenshot:app.screenshot());a.name=name;a.lifetime = .keepAlways;add(a)}
}

// Shared navigation paths for SDK smoke checks; actions still go through Coax.
extension XCUIApplication {
    func showChatHistory() {if buttons["nav.toggle"].isHittable {buttons["nav.toggle"].tap()};if buttons["nav.recent"].waitForExistence(timeout:3) {buttons["nav.recent"].tap()}}
    func revealAppButton(_ id:String) {if keyboards.firstMatch.exists {staticTexts["settings.title"].firstMatch.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.5)).tap()};for _ in 0..<10 {if buttons[id].isHittable {return};scrollViews.containing(.button,identifier:id).firstMatch.swipeUp()};XCTAssertTrue(buttons[id].isHittable)}
    func logOutAccount() {XCTAssertTrue(staticTexts["settings.busy"].waitForNonExistence(timeout:20));let logout=buttons["settings.logout"];XCTAssertTrue(logout.waitForExistence(timeout:15));revealAppButton("settings.logout");logout.tap();let confirm=buttons["settings.confirm"];XCTAssertTrue(confirm.waitForExistence(timeout:5));confirm.tap()}
    func showAppSettings() {showChatHistory();let settings=buttons["nav.settings"];XCTAssertTrue(settings.waitForExistence(timeout:15));settings.tap()}
    func openAppExtras() {if buttons["More"].isHittable {buttons["More"].tap()}else {showAppSettings();buttons["extras.more"].tap()}}
    func newAccountChat() {showChatHistory();let compose=buttons["nav.compose"];XCTAssertTrue(compose.waitForExistence(timeout:15));compose.tap();buttons["nav.account.new"].tap()}
}
