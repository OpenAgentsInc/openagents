// Firm reason: 6 (XCTest walks the installed UIKit app for a look review; it asserts no layout).
import XCTest

/// Walks every screen of the app against the local fixture (Tests/email_fixture.py
/// --environment-transcript) so a person can review the look (#13580). At each
/// stop it writes NAME.want into the directory named by OA_LOOK_DIR and waits
/// for NAME.png, which `xcrun simctl io DEVICE screenshot` writes from the Mac.
/// Without OA_LOOK_DIR it keeps the pictures as attachments.
final class LookUITests:XCTestCase {
    private let directory=ProcessInfo.processInfo.environment["OA_LOOK_DIR"]
    private func stop(_ app:XCUIApplication,_ name:String) {
        Thread.sleep(forTimeInterval:0.8)
        guard let directory else {let shot=XCTAttachment(screenshot:app.screenshot());shot.name=name;shot.lifetime = .keepAlways;add(shot);return}
        let want=directory+"/"+name+".want",png=directory+"/"+name+".png"
        try? FileManager.default.removeItem(atPath:png)
        FileManager.default.createFile(atPath:want,contents:Data())
        let end=Date().addingTimeInterval(30)
        while FileManager.default.fileExists(atPath:want) && Date()<end {Thread.sleep(forTimeInterval:0.1)}
    }
    func testWalkEveryScreen() {
        let origin=ProcessInfo.processInfo.environment["OA_LOOK_ORIGIN"] ?? "http://127.0.0.1:13493"
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin",origin,"-OAUpdatesOrigin","http://127.0.0.1:9","-OABalanceFixture","12345"]
        app.launch()
        let email=app.textFields["Email address"]
        if !email.waitForExistence(timeout:8) {
            app.showAppSettings()
            let logout=app.buttons["Log out"];XCTAssertTrue(logout.waitForExistence(timeout:15));logout.tap()
        }
        XCTAssertTrue(email.waitForExistence(timeout:15));stop(app,"01-login")
        email.tap();email.typeText("ada@example.com");stop(app,"02-login-typed")
        app.buttons["Send code"].tap()
        let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));stop(app,"03-code")
        code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        let newChat=app.buttons["nav.compose"];XCTAssertTrue(newChat.waitForExistence(timeout:15));stop(app,"04-chats")
        app.openAppExtras();XCTAssertTrue(app.buttons["Bitcoin balance"].waitForExistence(timeout:10));stop(app,"05-more")
        app.buttons["Bitcoin balance"].tap();XCTAssertTrue(app.staticTexts["12345 sats"].waitForExistence(timeout:10));stop(app,"06-balance")
        app.buttons["Close"].tap()
        XCTAssertTrue(newChat.waitForExistence(timeout:10));app.newAccountChat()
        let editor=app.textViews["chat.composer"];XCTAssertTrue(editor.waitForExistence(timeout:10));stop(app,"07-chat-empty")
        editor.tap();editor.typeText("Hello from iPhone");stop(app,"08-chat-typed")
        app.buttons["Send message"].tap()
        let reply=app.textViews.matching(NSPredicate(format:"value == %@","Hello from the account agent.")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout:20))
        app.scrollViews.firstMatch.coordinate(withNormalizedOffset:CGVector(dx:0.02,dy:0.4)).tap();stop(app,"09-chat")
        app.swipeUp();stop(app,"10-chat-end")
        app.showChatHistory();XCTAssertTrue(newChat.waitForExistence(timeout:15))
        stop(app,"11-computers")
        let thread=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@ AND label == %@","nav.chat.","Transcript fixture")).firstMatch
        XCTAssertTrue(thread.waitForExistence(timeout:20));stop(app,"12-sessions");thread.tap()
        let read=app.buttons["Read"];XCTAssertTrue(read.waitForExistence(timeout:10));stop(app,"13-transcript")
        read.tap();stop(app,"14-tool-open");read.tap()
        if ProcessInfo.processInfo.environment["OA_LOOK_WORLDS"]=="1" {
            app.terminate();app.launch()
            app.showChatHistory()
        let worlds=app.buttons["world.library"];XCTAssertTrue(worlds.waitForExistence(timeout:20));worlds.tap()
            let open=app.buttons["worlds.splice"];XCTAssertTrue(open.waitForExistence(timeout:30));stop(app,"15-worlds")
            let enabled=XCTNSPredicateExpectation(predicate:NSPredicate(format:"enabled == true"),object:open);XCTAssertEqual(XCTWaiter.wait(for:[enabled],timeout:30),.completed);open.tap()
            stop(app,"16-world-loading")
            let canvas=app.descendants(matching:.any).matching(identifier:"world.canvas").firstMatch
            XCTAssertTrue(canvas.waitForExistence(timeout:240))
            let presented=XCTNSPredicateExpectation(predicate:NSPredicate(format:"value == %@","Ready"),object:canvas);XCTAssertEqual(XCTWaiter.wait(for:[presented],timeout:30),.completed)
            stop(app,"17-world")
            let menu=app.buttons["world.menu"]
            if menu.exists {menu.tap();XCTAssertTrue(app.buttons["Close world"].waitForExistence(timeout:5));stop(app,"18-world-menu");app.buttons["Close world"].tap()}
            else {app.buttons["world.close"].tap()}
        }
    }
}
