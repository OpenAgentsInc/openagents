// Firm reason: 6 (real UIKit/XCTest finger taps across the system button/GPU boundary).
import XCTest

final class SidebarTests:XCTestCase {
    override func setUpWithError() throws {continueAfterFailure=false}
    @MainActor func testGPUConversationMenuRecentsAndKeyboard() {
        let app=XCUIApplication();app.launchArguments=["-OAAPIOrigin","http://127.0.0.1:9","-OAReduceMotion","YES"]
        app.launch();let menu=app.buttons["nav.menu"];XCTAssertTrue(menu.waitForExistence(timeout:20))
        let options=app.buttons["chat.options"];XCTAssertTrue(options.waitForExistence(timeout:10));options.tap()
        let recents=app.buttons["thread.recents"];XCTAssertTrue(recents.waitForExistence(timeout:10));recents.tap()
        // An inactive GPU field is an accessible activation target. UIKit's
        // editor exists only after it takes focus.
        let searchTarget=app.buttons["history.search"];XCTAssertTrue(searchTarget.waitForExistence(timeout:10));searchTarget.tap()
        let search=app.textFields["history.search"];XCTAssertTrue(search.waitForExistence(timeout:10));search.typeText("mobile")
        XCTAssertEqual(search.value as? String,"mobile")
        let picture=XCTAttachment(screenshot:app.screenshot());picture.name="GPU Recents and native search editor";picture.lifetime = .keepAlways;add(picture)
        menu.tap();let chat=app.buttons["drawer.chat"];XCTAssertTrue(chat.waitForExistence(timeout:10));chat.tap()
        XCTAssertTrue(options.waitForExistence(timeout:10));XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout:10))
        let composer=app.textViews["chat.composer"];XCTAssertTrue(composer.waitForExistence(timeout:10));composer.tap();composer.typeText("A local draft")
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout:10));options.tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout:10));XCTAssertTrue(recents.waitForExistence(timeout:10))
        XCTAssertFalse(app.buttons["chat.composer.model"].exists)
        let menuPicture=XCTAttachment(screenshot:app.screenshot());menuPicture.name="GPU conversation menu with preserved draft";menuPicture.lifetime = .keepAlways;add(menuPicture)
        app.buttons["thread.computer"].tap();XCTAssertTrue(app.buttons["env.close"].waitForExistence(timeout:10));app.buttons["env.close"].tap()
        XCTAssertTrue(options.waitForExistence(timeout:10));XCTAssertEqual(composer.value as? String,"A local draft")
    }
    @MainActor func testGPUConversationLibraryFilters() {
        let app=XCUIApplication()
        let library="openagents.chat.library.local.v1\naccount/cnv_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\t1\tPinned fixture\naccount/cnv_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\t2\tArchived fixture\n"
        app.launchArguments=["-OAAPIOrigin","http://127.0.0.1:9","-OAReduceMotion","YES","-OALibraryTrace","YES"]
        app.launchEnvironment["OA_CHAT_LIBRARY_FIXTURE"]=Data(library.utf8).base64EncodedString()
        app.launch();let options=app.buttons["chat.options"];XCTAssertTrue(options.waitForExistence(timeout:20));options.tap();app.buttons["thread.recents"].tap()
        let filter=app.buttons["history.filter"];XCTAssertTrue(filter.waitForExistence(timeout:10));filter.tap()
        let pinned=app.buttons["history.filter.pinned"];XCTAssertTrue(pinned.waitForExistence(timeout:10));pinned.tap()
        let first=app.buttons["cnv_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"];XCTAssertTrue(first.waitForExistence(timeout:10));XCTAssertEqual(first.label,"Pinned fixture")
        XCTAssertFalse(app.buttons["cnv_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"].exists)
        filter.tap();let archived=app.buttons["history.filter.archived"];XCTAssertTrue(archived.waitForExistence(timeout:10));archived.tap()
        let second=app.buttons["cnv_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"];XCTAssertTrue(second.waitForExistence(timeout:10));XCTAssertEqual(second.label,"Archived fixture")
        XCTAssertFalse(first.exists)
        let picture=XCTAttachment(screenshot:app.screenshot());picture.name="Native GPU archived library filter";picture.lifetime = .keepAlways;add(picture)
        XCUIDevice.shared.press(.home);app.activate();XCTAssertTrue(second.waitForExistence(timeout:10));XCTAssertEqual(second.label,"Archived fixture")
        app.terminate();app.launchEnvironment.removeValue(forKey:"OA_CHAT_LIBRARY_FIXTURE");app.launch()
        XCTAssertTrue(options.waitForExistence(timeout:20));options.tap();app.buttons["thread.recents"].tap();filter.tap();app.buttons["history.filter.archived"].tap()
        XCTAssertTrue(second.waitForExistence(timeout:10));XCTAssertEqual(second.label,"Archived fixture")
    }

    @MainActor func testNativeTapOpensAndClosesSidebarAndSettingsLoginBack() {
        let app=XCUIApplication()
        // Foundation's launch-argument domain overrides only this fixture's
        // saved email. Repeated runs start empty without editing app storage.
        app.launchArguments=["-OAAPIOrigin","http://127.0.0.1:9","-openagents.login-draft",""]
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
        XCTAssertTrue((email.value as? String ?? "").isEmpty || email.value as? String == "you@example.com")
        email.typeText("fixture@example.test")
        XCTAssertEqual(email.value as? String,"fixture@example.test")
        back.tap()
        XCTAssertTrue(login.waitForExistence(timeout:10))
        let usage=app.buttons["usage.open"];XCTAssertTrue(usage.waitForExistence(timeout:10));usage.tap()
        XCTAssertTrue(app.buttons["usage.back"].waitForExistence(timeout:10))
        let usagePicture=XCTAttachment(screenshot:app.screenshot());usagePicture.name="GPU Usage on the regular page";usagePicture.lifetime = .keepAlways;add(usagePicture)
        app.buttons["usage.back"].tap();XCTAssertTrue(login.waitForExistence(timeout:10))
        app.buttons["settings.back"].tap()
        XCTAssertTrue(menu.waitForExistence(timeout:10))
        menu.tap();XCTAssertTrue(settings.waitForExistence(timeout:10))
        let picture=XCTAttachment(screenshot:app.screenshot());picture.name="Sidebar opened by a real UIKit tap";picture.lifetime = .keepAlways;add(picture)
    }
    @MainActor func testRendererCompositionAndBack() {
        let app=XCUIApplication();app.launchArguments=["-OAAPIOrigin","http://127.0.0.1:9","-OAReduceMotion","YES"]
        app.launch();let menu=app.buttons["nav.menu"];XCTAssertTrue(menu.waitForExistence(timeout:20));menu.tap()
        let settings=app.buttons["drawer.settings"];XCTAssertTrue(settings.waitForExistence(timeout:10));settings.tap()
        let renderer=app.buttons["renderer.open"];XCTAssertTrue(renderer.waitForExistence(timeout:10));renderer.tap()
        let transformed=app.buttons["renderer.transformed"];XCTAssertTrue(transformed.waitForExistence(timeout:15));transformed.tap();XCTAssertEqual(transformed.label,"Tapped")
        XCTAssertTrue(app.buttons["renderer.sharp"].exists)
        let picture=XCTAttachment(screenshot:app.screenshot());picture.name="Native GPU images affine isolated opacity and blur";picture.lifetime = .keepAlways;add(picture)
        app.buttons["renderer.back"].tap();XCTAssertTrue(renderer.waitForExistence(timeout:10))
    }

}
