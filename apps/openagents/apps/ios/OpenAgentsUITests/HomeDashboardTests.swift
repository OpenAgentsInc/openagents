import XCTest

final class HomeDashboardTests: XCTestCase {
    private func reveal(_ element: XCUIElement, in app: XCUIApplication) {
        for _ in 0..<20 {
            if element.isHittable { return }
            app.swipeUp()
        }
        XCTAssertTrue(element.isHittable)
    }

    func testSessionsStayVisibleAndOpenTheOriginalList() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast"]
        app.launch()
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 15))
        XCTAssertFalse(app.buttons["import"].exists)
        XCTAssertFalse(app.buttons["new-dashboard"].exists)
        for _ in 0..<3 { app.swipeUp() }
        XCTAssertTrue(app.buttons["drawer-menu"].isHittable)
        XCTAssertTrue(app.openDrawer())
        XCTAssertTrue(app.cells["section-pinned"].waitForExistence(timeout: 5))
        app.buttons["sidebar-home"].tap()
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 5))
        app.buttons["drawer-menu"].tap()
        XCTAssertTrue(app.cells["section-pinned"].waitForExistence(timeout: 5))
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = "original-sessions-restored"
        attachment.lifetime = .keepAlways
        add(attachment)
        let session = app.cells.matching(NSPredicate(format: "identifier BEGINSWITH 'session-'")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 5))
        reveal(session, in: app)
        session.tap()
        XCTAssertTrue(app.textViews["composer-input"].waitForExistence(timeout: 10))
    }

    func testHomeEditingDefaultAndRestart() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 10))
        app.buttons["manage"].tap()
        let create = app.buttons["new-dashboard"]
        reveal(create, in: app)
        create.tap()
        let name = app.textFields["name"]
        for _ in 0..<15 { if name.isHittable { break }; app.swipeDown() }
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.press(forDuration: 1.2)
        if app.menuItems["Select All"].exists { app.menuItems["Select All"].tap() }
        name.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 30))
        let title = "Dashboard \(Int(Date().timeIntervalSince1970))"
        name.typeText(title)
        app.buttons["done"].tap()
        XCTAssertTrue(app.staticTexts[title].waitForExistence(timeout: 5))
        app.buttons["manage"].tap()
        let makeDefault = app.buttons["default"]
        reveal(makeDefault, in: app)
        makeDefault.tap()
        app.terminate()
        app.launch()
        XCTAssertTrue(app.staticTexts[title].waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["edit"].exists)
        app.buttons["edit"].tap()
        let add = app.buttons["add-computers"]
        reveal(add, in: app)
        add.tap()
        for _ in 0..<20 { if app.buttons["cancel"].isHittable { break }; app.swipeDown() }
        app.buttons["cancel"].tap()
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 5))
    }
}
