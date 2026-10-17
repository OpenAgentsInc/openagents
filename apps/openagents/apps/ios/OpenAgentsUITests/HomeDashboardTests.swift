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
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
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

    func testWidgetEditingStaysOffThePhone() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast"]
        app.launch()
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
        for key in ["edit", "manage", "scope"] { XCTAssertFalse(app.buttons[key].exists, key) }
    }
}
