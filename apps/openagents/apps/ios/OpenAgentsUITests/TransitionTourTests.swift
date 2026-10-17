import XCTest

/// Drives every glass transition slowly (for screen recordings / visual
/// review): open + close a session, the drawer and its pages, the
/// new-session sheet, and drawer scrolling.
final class TransitionTourTests: XCTestCase {
    func testTour() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.openDrawer())
        sleep(1)
        // Push a session, then pop.
        app.cells.matching(identifier: "session-chat-cjk").firstMatch.tap()
        XCTAssertTrue(app.scrollViews["transcript"].waitForExistence(timeout: 5))
        sleep(2)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        sleep(2)
        // Drawer pages.
        for page in ["sidebar-settings", "sidebar-home"] {
            app.openDrawer()
            app.buttons[page].tap()
            sleep(1)
        }
        app.openDrawer()
        sleep(1)
        // Scroll the drawer's list, then back.
        let list = app.collectionViews.firstMatch
        list.swipeUp(velocity: .slow)
        sleep(1)
        list.swipeDown(velocity: .slow)
        sleep(1)
        // New-session sheet in and out.
        app.buttons["new-session"].tap()
        sleep(2)
        app.navigationBars["New Session"].buttons.firstMatch.tap()
        sleep(2)
    }
}

extension TransitionTourTests {
    /// Just open + leave a session (fast iteration on the capsule handoff).
    func testPushPop() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.openDrawer())
        sleep(1)
        app.cells.matching(identifier: "session-chat-cjk").firstMatch.tap()
        sleep(2)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        sleep(2)
    }
}
