import XCTest

/// The wallet's flows against Spark mainnet with a throwaway wallet: random
/// words held in memory for this launch (debug builds only), so it is empty
/// and no one's money is near it. It proves the SDK opens, every kind of
/// request is made and shown, and sends are parsed and quoted; an empty
/// wallet can't send, so nothing leaves it.
final class WalletUITests: XCTestCase {
    private func shot(_ app: XCUIApplication, _ name: String) {
        let a = XCTAttachment(screenshot: app.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a)
    }
    private func wait(_ timeout: TimeInterval, _ test: @escaping () -> Bool) -> Bool {
        XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in test() }, object: nil)], timeout: timeout) == .completed
    }

    func testReceiveAndQuoteWithAThrowawayWallet() {
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO", "-OANativeHome", "YES"]  // every test starts from the first-launch overview
        app.launchEnvironment["OPENAGENTS_THROWAWAY_WALLET"] = "1"
        app.launch()
        // The balance shows once you are in the Grid.
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        start.tap()
        let open = app.buttons["wallet.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 20))
        XCTAssertTrue(wait(30) { !open.label.contains(",") && open.value.map { "\($0)".hasPrefix("₿") } == true }, "\(open.value ?? "")")
        XCTAssertTrue(wait(90) { open.isEnabled }, "balance: \(app.staticTexts["wallet.balance"].label)")
        open.tap()
        let total = app.buttons["wallet.total"]
        XCTAssertTrue(total.waitForExistence(timeout: 10))
        XCTAssertEqual(total.label, "₿0")
        XCTAssertTrue(app.staticTexts["No payments yet"].waitForExistence(timeout: 20))
        shot(app, "Wallet")

        // Receive: one QR for Lightning and on-chain, then each alone.
        app.buttons["wallet.receive"].tap()
        let amount = app.textFields["receive.amount"]
        XCTAssertTrue(amount.waitForExistence(timeout: 5))
        amount.tap(); amount.typeText("2100")
        app.textFields["receive.note"].tap(); app.textFields["receive.note"].typeText("Coffee")
        app.buttons["receive.create"].tap()
        let text = app.staticTexts["receive.text"]
        XCTAssertTrue(text.waitForExistence(timeout: 30))
        XCTAssertTrue(text.label.hasPrefix("bitcoin:bc1"), text.label)
        XCTAssertTrue(text.label.contains("amount=0.000021") && text.label.contains("lightning=lnbc21"), text.label)
        XCTAssertEqual(app.staticTexts["receive.status"].label, "Waiting for payment…")
        XCTAssertTrue(app.images["receive.qr"].exists)
        shot(app, "Receive: Lightning and on-chain")
        let kind = app.segmentedControls["receive.kind"]
        kind.buttons["Lightning"].tap()
        app.buttons["receive.create"].tap()
        XCTAssertTrue(wait(30) { text.exists && text.label.hasPrefix("lnbc21") }, text.label)
        let invoice = text.label
        kind.buttons["On-chain"].tap()
        XCTAssertTrue(wait(30) { text.exists && text.label.hasPrefix("bc1") }, text.label)
        shot(app, "Receive: on-chain")
        kind.buttons["Spark"].tap()
        XCTAssertTrue(wait(30) { text.exists && text.label.hasPrefix("spark1") }, text.label)
        app.navigationBars.buttons["Wallet"].tap()

        // Send: a request that isn't one, our own invoice, then an address.
        app.buttons["wallet.send"].tap()
        let input = app.textFields["send.input"]
        let about = app.staticTexts["send.about"]
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        input.tap(); input.typeText("hello\n")
        XCTAssertTrue(wait(20) { about.label.contains("isn't a payment request") }, about.label)
        input.clearAndType(invoice + "\n")
        XCTAssertTrue(wait(20) { about.label.hasPrefix("Lightning invoice for ₿2,100") }, about.label)
        shot(app, "Send: an invoice")
        app.buttons["send.continue"].tap()
        // An empty wallet: quoted with a shortfall noted, or refused outright.
        let review = app.staticTexts["send.total"]
        XCTAssertTrue(wait(30) { review.exists || app.alerts.count > 0 })
        shot(app, "Send: review")
        if review.exists {
            XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Your wallet has ₿0'")).count > 0)
            app.buttons["send.confirm"].tap()
            XCTAssertTrue(app.alerts.firstMatch.waitForExistence(timeout: 5))
            XCTAssertTrue(app.alerts.firstMatch.label.contains("enough") || app.alerts.firstMatch.staticTexts.containing(NSPredicate(format: "label CONTAINS 'enough'")).count > 0)
            app.alerts.firstMatch.buttons["OK"].tap()
            app.navigationBars.buttons["Send"].tap()
        } else {
            app.alerts.firstMatch.buttons["OK"].tap()
        }
        input.clearAndType("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq\n")
        XCTAssertTrue(wait(20) { about.label.hasPrefix("Bitcoin address") }, "\(about.label) [\(input.value ?? "")]")
        let sats = app.textFields["send.amount"]
        XCTAssertTrue(sats.waitForExistence(timeout: 5))
        sats.tap(); sats.typeText("5000")
        app.buttons["send.continue"].tap()
        XCTAssertTrue(wait(30) { review.exists || app.alerts.count > 0 })
        shot(app, "Send: on-chain")
        // Refusals are in words, never the SDK's own text.
        if app.alerts.count > 0 {
            let words = app.alerts.firstMatch.staticTexts.allElementsBoundByIndex.map(\.label).joined(separator: " ")
            XCTAssertFalse(words.contains("SdkError") || words.contains("Spark error"), words)
        }
        app.terminate()
    }
}

private extension XCUIElement {
    func clearAndType(_ text: String) {
        if buttons["Clear text"].exists { buttons["Clear text"].tap() }
        tap(); typeText(text)
    }
}
