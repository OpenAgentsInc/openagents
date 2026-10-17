import XCTest

/// The existing wallet operations through focused screens, using only the
/// in-memory wallet fixture. These tests never open a funded wallet.
final class WalletDesignTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func tap(_ app: XCUIApplication, _ key: String) {
        let button = app.buttons["wallet-\(key)"]
        XCTAssertTrue(button.waitForExistence(timeout: 10), key)
        for _ in 0..<6 {
            if button.isHittable { break }
            let scroll = app.scrollViews["wallet-scroll"]
            if button.frame.midY < scroll.frame.minY {
                scroll.swipeDown()
            } else {
                scroll.swipeUp()
            }
        }
        button.tap()
    }

    private func launch(_ options: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-wallet-fixture", "-route", "wallet"] + options
        app.launch()
        XCTAssertTrue(app.staticTexts["wallet-balance"].waitForExistence(timeout: 20))
        return app
    }

    func testExistingReceiveSendAndActivityFlows() {
        let app = launch()
        capture(app, "wallet-home")
        XCTAssertFalse(app.textViews["wallet-send-to"].exists)
        tap(app, "receive-open")
        XCTAssertTrue(app.staticTexts["wallet-code"].waitForExistence(timeout: 10))
        capture(app, "wallet-receive-spark")
        tap(app, "receive-bitcoin")
        capture(app, "wallet-receive-bitcoin")
        tap(app, "receive-lightning")
        let amount = app.textViews["wallet-invoice-amount"]
        amount.tap()
        amount.typeText("2100")
        app.toolbars.buttons["Done"].tap()
        tap(app, "invoice")
        XCTAssertTrue(app.staticTexts["wallet-code"].waitForExistence(timeout: 10))
        capture(app, "wallet-receive-lightning")
        app.scrollViews["wallet-scroll"].swipeDown()
        tap(app, "home")
        tap(app, "send-open")
        let destination = app.textViews["wallet-send-to"]
        destination.tap()
        destination.typeText("lnbc1000n1pfixture")
        app.toolbars.buttons["Done"].tap()
        capture(app, "wallet-send-entry")
        tap(app, "review")
        XCTAssertTrue(app.staticTexts["wallet-review-total"].waitForExistence(timeout: 10))
        XCTAssertFalse(destination.exists)
        capture(app, "wallet-send-review")
        tap(app, "send-cancel")
        XCTAssertEqual(destination.value as? String, "lnbc1000n1pfixture")
        tap(app, "review")
        tap(app, "pay")
        XCTAssertTrue(app.staticTexts["wallet-sent"].waitForExistence(timeout: 10))
        capture(app, "wallet-payment-result")
        tap(app, "send-done")
        tap(app, "activity-open")
        XCTAssertTrue(app.staticTexts["wallet-history-heading"].exists)
        capture(app, "wallet-activity")
        tap(app, "home")
        tap(app, "trust")
        capture(app, "wallet-about")
        tap(app, "trust-ok")
        XCTAssertTrue(app.staticTexts["wallet-balance"].exists)
    }

    func testDarkAppearanceAndLargeText() {
        let app = launch(["-appearance", "2",
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"])
        let receive = app.buttons["wallet-receive-open"]
        let send = app.buttons["wallet-send-open"]
        XCTAssertGreaterThan(send.frame.minY, receive.frame.minY)
        XCTAssertGreaterThan(receive.frame.width, app.frame.width * 0.7)
        capture(app, "wallet-home-dark-large-text")
        tap(app, "receive-open")
        XCTAssertTrue(app.staticTexts["wallet-code"].waitForExistence(timeout: 10))
        capture(app, "wallet-receive-dark-large-text")
        tap(app, "home")
        tap(app, "send-open")
        XCTAssertTrue(app.textViews["wallet-send-to"].exists)
        capture(app, "wallet-send-dark-large-text")
    }
}
