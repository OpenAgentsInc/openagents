// Firm reason: 6 (XCTest exercises the installed GPU login, the keyboard, the accessibility tree and the native account transport).
import XCTest

/// The compiled login screen (components/gpu-components.md, stage G1; #13645) in the installed
/// app, against the independent account oracle:
///   python3 apps/openagents-ios/Tests/email_fixture.py X --api-only --port 13571
/// Every query here goes through the accessibility tree, so it also checks the bridge from the
/// screen's semantic rows. TEST_RUNNER_OA_LOGIN_SHOTS names a folder for pictures.
final class LoginUITests:XCTestCase {
    private let origin="http://127.0.0.1:13571"
    private func launch(_ extra:[String]=[])->XCUIApplication {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin",origin,"-OAUpdatesOrigin","http://127.0.0.1:9"]+extra
        app.launch()
        if !app.staticTexts["Log in"].waitForExistence(timeout:8) {app.showAppSettings();app.logOutAccount()}
        XCTAssertTrue(app.staticTexts["Log in"].waitForExistence(timeout:15))
        return app
    }
    private func shot(_ app:XCUIApplication,_ name:String) {
        let picture=app.screenshot(),attachment=XCTAttachment(screenshot:picture);attachment.name=name;attachment.lifetime = .keepAlways;add(attachment)
        if let folder=ProcessInfo.processInfo.environment["OA_LOGIN_SHOTS"],!folder.isEmpty {
            try? FileManager.default.createDirectory(atPath:folder,withIntermediateDirectories:true)
            try? picture.pngRepresentation.write(to:URL(fileURLWithPath:folder).appendingPathComponent(name+".png"))
        }
    }
    private func stats(_ path:String)->[String:Int] {
        guard let text=try? String(contentsOfFile:path,encoding:.utf8) else {return [:]}
        var out:[String:Int]=[:]
        for part in text.split(separator:" ") {let pair=part.split(separator:"=");if pair.count==2,let value=Int(pair[1]) {out[String(pair[0])]=value}}
        return out
    }
    private func pause(_ seconds:TimeInterval) {let done=expectation(description:"pause");DispatchQueue.main.asyncAfter(deadline:.now()+seconds) {done.fulfill()};wait(for:[done],timeout:seconds+5)}

    func testEmailCodeLoginOverTheSceneLandsInTheApp() {
        let file=NSTemporaryDirectory()+"oa-login-stats-\(UUID().uuidString).txt"
        let app=launch(["-OALoginStats",file])
        // The screen is the kit's rows, not UIKit controls: a title, a field, an action and the key login.
        let email=app.textFields["Email address"]
        XCTAssertTrue(email.exists);XCTAssertTrue(app.buttons["Send code"].exists);XCTAssertTrue(app.buttons["login.key"].exists)
        XCTAssertEqual(app.textFields.count,1);XCTAssertEqual(app.webViews.count,0)
        XCTAssertFalse(app.staticTexts["Sign in"].exists)
        // The scene is live behind the form: it keeps computing frames.
        pause(3);let first=stats(file);pause(2);let second=stats(file)
        XCTAssertEqual(second["scene"],1);XCTAssertGreaterThan(second["sceneFrames"] ?? 0,(first["sceneFrames"] ?? 0)+30)
        shot(app,"login-rest")
        // An empty email asks for one and starts nothing.
        app.buttons["Send code"].tap()
        XCTAssertTrue(app.staticTexts["Enter your email address."].waitForExistence(timeout:5))
        // The focused field is the one platform editor.
        XCTAssertTrue(email.waitForExistence(timeout:5));email.tap();email.typeText("ada@example.com")
        XCTAssertEqual(app.textFields.count,1);XCTAssertEqual(email.value as? String,"ada@example.com")
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout:5))
        pause(1);shot(app,"login-keyboard")
        // The form stays clear of the keyboard.
        XCTAssertLessThanOrEqual(app.buttons["Send code"].frame.maxY,app.keyboards.firstMatch.frame.minY)
        app.buttons["Send code"].tap()
        let code=app.textFields["Email code"]
        XCTAssertTrue(code.waitForExistence(timeout:10));XCTAssertTrue(app.staticTexts["Email code"].exists);XCTAssertTrue(app.staticTexts["ada@example.com"].exists)
        XCTAssertEqual(app.textFields.count,1)
        code.typeText("123456");pause(1);shot(app,"login-code")
        app.buttons["Log in"].tap()
        XCTAssertTrue(app.staticTexts["That code isn't right. 4 tries left."].waitForExistence(timeout:10))
        XCTAssertEqual(code.value as? String,"000000")   // cleared: the placeholder is what is left
        app.buttons["login.resend"].tap()
        XCTAssertTrue(app.staticTexts["We sent a code to your email."].waitForExistence(timeout:10))
        XCTAssertTrue(code.waitForExistence(timeout:5));code.tap();code.typeText("415926")
        app.buttons["Log in"].tap()
        // The app continues into the existing runtime, logged in.
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:20))
        XCTAssertFalse(app.textFields["Email address"].exists);XCTAssertFalse(app.buttons["Send code"].exists)
        pause(1);shot(app,"login-landed")
        // The session is the native adapter's: it survives a restart with no login screen.
        app.terminate();app.launch()
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:20));XCTAssertFalse(app.staticTexts["Log in"].exists)
        // Logging out in the existing runtime brings the compiled screen back.
        app.showAppSettings();app.logOutAccount()
        XCTAssertTrue(app.textFields["Email address"].waitForExistence(timeout:15));XCTAssertTrue(app.buttons["login.key"].exists)
    }

    func testReducedMotionIsAStillSceneAndAStaticScreenPresentsNothing() {
        let file=NSTemporaryDirectory()+"oa-login-stats-\(UUID().uuidString).txt"
        let app=launch(["-OAReduceMotion","YES","-OALoginStats",file])
        XCTAssertTrue(app.textFields["Email address"].exists)
        pause(3);let first=stats(file);pause(3);let second=stats(file)
        // One still frame of the scene, and no frame submitted while nothing changes.
        XCTAssertEqual(second["scene"],1);XCTAssertEqual(second["reduced"],1)
        XCTAssertEqual(second["sceneFrames"],first["sceneFrames"]);XCTAssertEqual(second["presented"],first["presented"])
        XCTAssertLessThanOrEqual(second["sceneFrames"] ?? 99,4)
        shot(app,"login-reduced-motion")
        // The form still works: typing wakes it.
        let email=app.textFields["Email address"];email.tap();email.typeText("ada@example.com")
        XCTAssertEqual(email.value as? String,"ada@example.com")
    }

    func testWithoutASceneTheFormIsOnThePlainCanvasAndLoginWorks() {
        let file=NSTemporaryDirectory()+"oa-login-stats-\(UUID().uuidString).txt"
        let app=launch(["-OALoginScene","NO","-OALoginStats",file])
        pause(2);XCTAssertEqual(stats(file)["scene"],0)
        shot(app,"login-no-scene")
        let email=app.textFields["Email address"];email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
        let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.typeText("415926");app.buttons["Log in"].tap()
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:20))
        app.showAppSettings();app.logOutAccount()
        XCTAssertTrue(app.staticTexts["Log in"].waitForExistence(timeout:15))
    }

    func testTheKeyLoginStillOpensInTheExistingRuntimeAndComesBack() {
        let app=launch()
        app.buttons["login.key"].tap()
        // The existing runtime's dialog takes over; closing it returns to the compiled screen.
        let back=app.buttons["settings.back"]
        XCTAssertTrue(back.waitForExistence(timeout:10));shot(app,"login-key");back.tap()
        XCTAssertTrue(app.buttons["Send code"].waitForExistence(timeout:10));XCTAssertEqual(app.textFields.count,1)
    }
}
