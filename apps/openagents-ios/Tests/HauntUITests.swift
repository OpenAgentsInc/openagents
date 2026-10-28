import XCTest

final class HauntUITests: XCTestCase {
    private func enter(_ app: XCUIApplication) {
        let start = app.buttons["home.start"]
        if start.waitForExistence(timeout: 10) {
            start.tap()
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5,dy: 0.5)).tap()
            let landed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
                let grid = app.otherElements["grid.world"]
                return grid.exists && (grid.value as? String) != "arriving"
            },object:nil)
            XCTAssertEqual(XCTWaiter.wait(for:[landed],timeout:20),.completed)
        }
        let portal = app.buttons["grid.haunt"]
        XCTAssertTrue(portal.waitForExistence(timeout: 10))
        portal.tap()
        XCTAssertTrue(app.otherElements["haunt.world"].waitForExistence(timeout: 20))
        XCTAssertFalse(app.alerts["Haunted House"].exists, "A failed shader/resource must not pass as a working scene")
    }
    private func row(_ title: String,_ app: XCUIApplication) { let cell=app.cells["haunt.row."+title];XCTAssertTrue(cell.waitForExistence(timeout:5));cell.tap() }
    func testPortalHouseConversationEditingAndReturn() {
        let app=XCUIApplication();app.launchArguments+=["-OAGridSeen","NO"];app.launch();enter(app)
        let world=app.otherElements["haunt.world"]
        let initial=world.value as? String ?? ""
        XCTAssertTrue(initial.contains("Yard"))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Haunted House from the Grid";shot.lifetime = .keepAlways;add(shot)
        XCTAssertTrue(app.buttons["haunt.interact"].exists)
        app.buttons["haunt.talk"].tap()
        let question=app.alerts.textFields["haunt.question"]
        XCTAssertTrue(question.waitForExistence(timeout:5));question.tap();question.typeText("Who are you?")
        app.alerts.buttons["Ask"].tap()
        XCTAssertTrue(app.alerts["Mortimer"].waitForExistence(timeout:5))
        XCTAssertTrue(app.alerts.staticTexts["Mortimer"].exists)
        app.alerts.buttons["Bye"].tap()
        app.buttons["haunt.view"].tap();app.buttons["haunt.view"].tap();app.buttons["haunt.view"].tap()
        app.buttons["haunt.edit"].tap();row("Add",app)
        // Barrel is the first catalogue entry; the placed asset is the real
        // kit mesh and appears two metres ahead, inside the yard.
        row("Barrel",app)
        let placed=XCTNSPredicateExpectation(predicate:NSPredicate { _, _ in
            let value=world.value as? String ?? ""
            return value != initial && value.contains("objects")
        },object:nil)
        XCTAssertEqual(XCTWaiter.wait(for:[placed],timeout:5),.completed)
        let saved=world.value as? String ?? ""
        app.buttons["haunt.edit"].tap();row("Done",app)
        app.buttons["haunt.exit"].tap()
        XCTAssertTrue(app.otherElements["grid.world"].waitForExistence(timeout:10))
        enter(app)
        XCTAssertEqual(app.otherElements["haunt.world"].value as? String,saved)
        XCUIDevice.shared.orientation = .landscapeLeft
        let rotated = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.frame.width > app.frame.height && world.frame.width > app.frame.width * 0.9
        }, object:nil)
        XCTAssertEqual(XCTWaiter.wait(for:[rotated],timeout:8),.completed)
        XCTAssertTrue(app.buttons["haunt.map"].waitForExistence(timeout:5));app.buttons["haunt.map"].tap()
        XCTAssertTrue(app.navigationBars["Map"].waitForExistence(timeout:5));app.navigationBars.buttons["Done"].tap()
        let landscape=XCTAttachment(screenshot:XCUIScreen.main.screenshot());landscape.name="House landscape";landscape.lifetime = .keepAlways;add(landscape)
        XCUIDevice.shared.orientation = .portrait
        let portrait = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.frame.width < app.frame.height && app.frame.contains(app.buttons["haunt.exit"].frame) && app.buttons["haunt.exit"].isHittable
        }, object:nil)
        XCTAssertEqual(XCTWaiter.wait(for:[portrait],timeout:8),.completed)
        // Reset is exercised through the same public controls. This test
        // screen was introduced with an empty simulator save for this port.
        app.buttons["haunt.edit"].tap();row("World",app);row("Reset",app)
        XCTAssertTrue(app.alerts["Reset?"].waitForExistence(timeout:5));app.alerts.buttons["Reset"].tap()
        app.buttons["haunt.exit"].tap()
    }
}
