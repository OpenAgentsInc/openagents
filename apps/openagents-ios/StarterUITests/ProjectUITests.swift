// Firm reason: 6 (installed UIKit flow against the independent project HTTP oracle).
import XCTest
final class ProjectUITests:XCTestCase {
    func testPrivateProjectInstructionsKnowledgeAndSavedChatThroughNativeHTTP() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13496","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:5) {email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap();let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()}
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:15));app.buttons["nav.toggle"].tap();app.buttons["nav.projects"].tap()
        let create=app.buttons["projects.new"];XCTAssertTrue(create.waitForExistence(timeout:10));create.tap()
        let name=app.textFields["private.name"];XCTAssertTrue(name.waitForExistence(timeout:5));name.tap();name.typeText("Private iPhone Project")
        var text=app.textViews["private.text"];text.tap();text.typeText("Be precise.");finishEditing(app);app.buttons["private.create"].tap()
        let instructions=app.buttons["private.instructions"];XCTAssertTrue(instructions.waitForExistence(timeout:10));instructions.tap()
        text=app.textViews["private.text"];XCTAssertTrue(text.waitForExistence(timeout:5));text.tap();text.typeText(String(repeating:XCUIKeyboardKey.delete.rawValue,count:11));text.typeText("Cite Project knowledge.");finishEditing(app);app.buttons["private.save.instructions"].tap()
        let knowledge=app.buttons["private.knowledge"];XCTAssertTrue(knowledge.waitForExistence(timeout:10));knowledge.tap()
        let title=app.textFields["private.knowledge.title"];XCTAssertTrue(title.waitForExistence(timeout:5));title.tap();title.typeText("Launch notes")
        text=app.textViews["private.text"];XCTAssertEqual(text.value as? String,"");text.tap();text.typeText("The code is violet.");finishEditing(app);app.buttons["private.add.text"].tap()
        XCTAssertTrue(app.buttons["private.source.0"].waitForExistence(timeout:10));attach(app,"Private Project knowledge")
        app.buttons["private.close.edit"].tap();let newChat=app.buttons["private.chat.new"];XCTAssertTrue(newChat.waitForExistence(timeout:10));newChat.tap()
        let composer=app.textViews["chat.composer"];XCTAssertTrue(composer.waitForExistence(timeout:10));composer.tap();composer.typeText("What is the code?");app.buttons["chat.composer.send"].tap()
        let answer=app.textViews.matching(NSPredicate(format:"value == %@","The code is violet.")).firstMatch;XCTAssertTrue(answer.waitForExistence(timeout:20));attach(app,"Private Project saved answer")
        app.buttons["project.chat.back"].tap()
        let saved=app.buttons["private.chat.0"];XCTAssertTrue(saved.waitForExistence(timeout:10));saved.tap();XCTAssertTrue(answer.waitForExistence(timeout:10));XCTAssertEqual(app.webViews.count,0);attach(app,"Reopened private Project chat")
    }
    func testManageDraftPublishAndOpenProjectChatsInNativeViews() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13496","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:5) {email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap();let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()}
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:15));app.showChatHistory();app.buttons["nav.projects"].tap()
        let project=app.buttons["projects.item.0"];XCTAssertTrue(project.waitForExistence(timeout:10));project.tap()
        XCTAssertTrue(app.staticTexts["Native project fixture"].waitForExistence(timeout:10));app.buttons["project.chat"].tap()
        let message=app.textViews["project.message"];XCTAssertTrue(message.waitForExistence(timeout:10));message.tap();message.typeText("Move the milestone to doing");app.buttons["project.message.send"].tap()
        XCTAssertTrue(app.textViews.matching(NSPredicate(format:"value CONTAINS %@","Your project is updated.")).firstMatch.waitForExistence(timeout:15))
        let tool=app.buttons.matching(identifier:"project.tool.3").firstMatch
        if tool.exists {tool.tap()}
        attach(app,"Native project agent conversation")
        app.buttons["project.back"].tap();XCTAssertTrue(app.buttons["projects.new"].waitForExistence(timeout:10));app.buttons["projects.publish.new"].tap()
        let resume=app.buttons["project.resume.0"];XCTAssertTrue(resume.waitForExistence(timeout:10));resume.tap()
        let name=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@ AND label BEGINSWITH %@","project.field.","Name")).firstMatch;XCTAssertTrue(name.waitForExistence(timeout:10));name.tap()
        let field=app.textViews["project.edit"];XCTAssertTrue(field.waitForExistence(timeout:5));XCTAssertEqual(field.value as? String,"Moon Villa");field.tap();field.typeText(String(repeating:XCUIKeyboardKey.delete.rawValue,count:10));field.typeText("iPhone project");app.buttons["project.field.save"].tap()
        let publish=app.buttons["project.publish"];XCTAssertTrue(publish.waitForExistence(timeout:10));attach(app,"Native complete draft review");publish.tap();app.buttons["project.confirm"].tap()
        let chats=app.buttons["project.chats"];XCTAssertTrue(chats.waitForExistence(timeout:15));chats.tap();app.buttons["project.chat.new"].tap()
        let composer=app.textViews["chat.composer"];XCTAssertTrue(composer.waitForExistence(timeout:10));composer.tap();composer.typeText("What should we build first?");app.buttons["chat.composer.send"].tap()
        XCTAssertTrue(app.textViews.matching(NSPredicate(format:"value == %@","The project chat stays in the app.")).firstMatch.waitForExistence(timeout:20))
        app.buttons["project.chat.back"].tap();let saved=app.buttons["project.saved.0"];XCTAssertTrue(saved.waitForExistence(timeout:10));saved.tap()
        XCTAssertTrue(composer.waitForExistence(timeout:10));XCTAssertEqual(app.webViews.count,0);XCTAssertNotEqual(XCUIApplication(bundleIdentifier:"com.apple.mobilesafari").state,.runningForeground);attach(app,"Reopened native project chat")
    }
    private func finishEditing(_ app:XCUIApplication) {if app.keyboards.firstMatch.exists {app.staticTexts["project.title"].tap();XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout:5))}}
    private func attach(_ app:XCUIApplication,_ name:String) {let a=XCTAttachment(screenshot:app.screenshot());a.name=name;a.lifetime = .keepAlways;add(a)}
}
