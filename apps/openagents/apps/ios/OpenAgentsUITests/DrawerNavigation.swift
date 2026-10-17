import XCTest

/// The iPhone shell's drawer: Sessions, search, Home and Settings, opened
/// from the page's menu button.
extension XCUIApplication {
    @discardableResult
    func openDrawer(timeout: TimeInterval = 10) -> Bool {
        let settings = buttons["sidebar-settings"]
        if settings.exists, settings.isHittable { return true }
        let menu = buttons["drawer-menu"]
        guard menu.waitForExistence(timeout: timeout) else { return false }
        menu.tap()
        return settings.waitForExistence(timeout: 5)
    }

    /// A session row in the open drawer, scrolled into view when it sits
    /// below the fold.
    func drawerCell(_ identifier: String, timeout: TimeInterval = 10) -> XCUIElement {
        let cell = cells[identifier]
        _ = cells["section-pinned"].waitForExistence(timeout: timeout)
        for _ in 0..<4 where !(cell.exists && cell.isHittable) {
            collectionViews.firstMatch.swipeUp()
        }
        return cell
    }

    func openSettings(timeout: TimeInterval = 10) {
        XCTAssertTrue(openDrawer(timeout: timeout), "drawer opens")
        buttons["sidebar-settings"].tap()
    }
}
