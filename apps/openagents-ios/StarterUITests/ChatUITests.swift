// Firm reason: 6 (XCTest exercises UIKit, native account transport and Keychain).
import XCTest

final class ChatUITests:XCTestCase {
    func testDurableChatContinuesWhileBackgroundedReopensAndStops() {
        let app=XCUIApplication(),suffix=String(UUID().uuidString.prefix(8)),question="Background durable reply "+suffix
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491","-OAUpdatesOrigin","http://127.0.0.1:9"];app.launch()
        if !app.textFields["Email address"].waitForExistence(timeout:5) {app.showAppSettings();app.logOutAccount()}
        loginIfNeeded(app,email:"ada@example.com");app.newAccountChat()
        let editor=app.textViews["chat.composer"];XCTAssertTrue(editor.waitForExistence(timeout:10));editor.tap();editor.typeText(question);app.buttons["chat.composer.send"].tap()
        XCTAssertTrue(app.textViews.matching(NSPredicate(format:"value == %@","A partial answer.")).firstMatch.waitForExistence(timeout:15))
        XCUIDevice.shared.press(.home);XCTAssertTrue(app.wait(for:.runningBackground,timeout:5))
        let completed=waitForServer(question);XCTAssertEqual(completed["state"] as? String,"completed");XCTAssertEqual(completed["dispatches"] as? Int,1)
        app.terminate();app.launch();XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:15));app.showChatHistory()
        let saved=app.buttons.matching(NSPredicate(format:"label CONTAINS %@",question)).firstMatch;XCTAssertTrue(saved.waitForExistence(timeout:15));saved.tap()
        XCTAssertTrue(app.textViews.matching(NSPredicate(format:"value == %@","Saved after background.")).firstMatch.waitForExistence(timeout:15))
        let stopQuestion="Stop this answer "+suffix;editor.tap();editor.typeText(stopQuestion);app.buttons["chat.composer.send"].tap()
        XCTAssertTrue(app.textViews.matching(NSPredicate(format:"value == %@","Stop fixture partial.")).firstMatch.waitForExistence(timeout:15));app.buttons["chat.composer.send"].tap()
        let cancelled=waitForServer(stopQuestion);XCTAssertEqual(cancelled["state"] as? String,"cancelled");XCTAssertEqual(cancelled["dispatches"] as? Int,1);XCTAssertEqual(cancelled["stop_requests"] as? Int,1)
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format:"label CONTAINS %@","cancelled")).firstMatch.waitForExistence(timeout:10))
        app.showAppSettings();app.logOutAccount();loginIfNeeded(app,email:"grace@example.com");app.showChatHistory()
        XCTAssertFalse(app.buttons.matching(NSPredicate(format:"label CONTAINS %@",question)).firstMatch.exists)
        XCTAssertFalse(app.textViews.matching(NSPredicate(format:"value CONTAINS %@","Stop fixture partial.")).firstMatch.exists)
        XCTAssertEqual(app.webViews.count,0)
    }
    private func loginIfNeeded(_ app:XCUIApplication,email address:String) {
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:5) {email.tap();email.typeText(address);app.buttons["Send code"].tap();let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()}
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:15))
    }
    private func waitForServer(_ question:String)->[String:Any] {
        var url=URLComponents(string:"http://127.0.0.1:13491/report/durable/wait")!;url.queryItems=[URLQueryItem(name:"question",value:question)]
        let done=expectation(description:"Independent server reaches a terminal state"),result=DurableHTTPResult()
        URLSession.shared.dataTask(with:url.url!) {data,_,error in
            XCTAssertNil(error);if let data {result.value=(try? JSONSerialization.jsonObject(with:data)) as? [String:Any] ?? [:]};done.fulfill()
        }.resume()
        wait(for:[done],timeout:20);return result.value
    }
    func testEmailAccountSendsAndReopensConversationWithNativeComposer() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491","-OAUpdatesOrigin","http://127.0.0.1:9"]
        app.launch()
        let email=app.textFields["Email address"]
        if !email.waitForExistence(timeout:5) {
            app.showAppSettings()
            app.logOutAccount()
        }
        XCTAssertTrue(email.waitForExistence(timeout:15));email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
        let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        app.newAccountChat()
        let editor=app.textViews["chat.composer"],send=app.buttons["Send message"]
        XCTAssertTrue(editor.waitForExistence(timeout:10));XCTAssertFalse(send.isEnabled)
        XCTAssertTrue(app.buttons["nav.toggle"].exists)
        XCTAssertEqual(app.webViews.count,0)
        editor.tap();editor.typeText("Hello from iPhone")
        XCTAssertTrue(send.isEnabled)
        let compact=XCTAttachment(screenshot:app.screenshot());compact.name="T3 compact native composer";compact.lifetime = .keepAlways;add(compact)
        send.tap()
        let reply=app.textViews.matching(NSPredicate(format:"value == %@","Hello from the account agent.")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout:20));XCTAssertEqual(editor.value as? String,"");XCTAssertTrue(app.keyboards.firstMatch.exists)
        app.scrollViews.firstMatch.coordinate(withNormalizedOffset:CGVector(dx:0.02,dy:0.4)).tap()
        let dismissed=XCTNSPredicateExpectation(predicate:NSPredicate(format:"exists == false"),object:app.keyboards.firstMatch)
        XCTAssertEqual(XCTWaiter.wait(for:[dismissed],timeout:5),.completed)
        let copy=app.buttons.matching(NSPredicate(format:"identifier ENDSWITH %@",".assistant.copy")).firstMatch;XCTAssertTrue(copy.exists);copy.tap()
        XCTAssertTrue(app.buttons.matching(NSPredicate(format:"identifier ENDSWITH %@ AND label == %@",".assistant.copy","Copied")).firstMatch.waitForExistence(timeout:5))
        let rich=XCTAttachment(screenshot:app.screenshot());rich.name="Coax Markdown native conversation";rich.lifetime = .keepAlways;add(rich)
        app.showChatHistory()
        let saved=app.buttons["Hello from iPhone"];XCTAssertTrue(saved.waitForExistence(timeout:15))
        app.terminate();app.launch();XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:15));app.showChatHistory();XCTAssertTrue(saved.waitForExistence(timeout:15));saved.tap()
        XCTAssertTrue(reply.waitForExistence(timeout:15))
        editor.tap();editor.typeText("First line\nSecond line\nThird line\nFourth line\nFifth line")
        XCTAssertLessThanOrEqual(editor.frame.height,105)
        XCTAssertGreaterThan(editor.frame.height,70)
        XCTAssertLessThanOrEqual(send.frame.maxY,app.keyboards.firstMatch.frame.minY+1)
        let expanded=XCTAttachment(screenshot:app.screenshot());expanded.name="T3 expanded keyboard-docked composer";expanded.lifetime = .keepAlways;add(expanded)
        app.firstMatch.pinch(withScale:2,velocity:1)
        XCTAssertLessThanOrEqual(editor.frame.height,105)
    }
}
private final class DurableHTTPResult:@unchecked Sendable {var value:[String:Any]=[:]}
