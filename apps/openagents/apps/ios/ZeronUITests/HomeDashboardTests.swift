import XCTest

final class HomeDashboardTests: XCTestCase {
    private func reveal(_ element: XCUIElement, in app: XCUIApplication) {
        for _ in 0..<20 {
            if element.isHittable { return }
            app.swipeUp()
        }
        XCTAssertTrue(element.isHittable)
    }

    func testHomeEditingDefaultAndRestart() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.tabBars.buttons["Home"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 10))
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
