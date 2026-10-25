import XCTest

final class GridOnlyTests: XCTestCase {
    override func setUp() { continueAfterFailure = false; XCUIDevice.shared.orientation = .portrait }
    private func fields(_ app: XCUIApplication, _ key: String) -> [String:Any] {
        guard app.staticTexts[key].exists, let value = app.staticTexts[key].value as? String, let data = value.data(using:.utf8), let fields = try? JSONSerialization.jsonObject(with:data) as? [String:Any] else { return [:] }
        return fields
    }
    private func wait(_ app: XCUIApplication, _ key: String, _ predicate: @escaping ([String:Any])->Bool) {
        let e = XCTNSPredicateExpectation(predicate:NSPredicate { _,_ in predicate(self.fields(app,key)) },object:nil)
        XCTAssertEqual(XCTWaiter.wait(for:[e],timeout:90),.completed,"State: \(fields(app,key))")
    }
    private func capture(_ app:XCUIApplication,_ name:String) {
        let a=XCTAttachment(screenshot:app.screenshot());a.name=name;a.lifetime = .keepAlways;add(a)
    }
    func testAethyrHomeAndRootGridOwnTheScreenAndGestures() {
        let app=XCUIApplication()
        app.launchArguments=["-grid-probe","-grid-offline"]
        app.launch()
        wait(app,"grid-home-metrics") { $0["rendered"] as? Bool == true }
        XCTAssertEqual(fields(app,"grid-home-metrics")["backend"] as? String,"Aethyr af-home")
        let enter=app.webViews.buttons["Enter the Grid"]
        XCTAssertTrue(enter.exists)
        XCTAssertGreaterThan(enter.frame.midY,app.frame.height*0.75)
        XCTAssertFalse(app.buttons["new-session"].exists)
        app.webViews.firstMatch.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.5)).press(forDuration:1.5)
        XCTAssertFalse(app.menuItems["Copy"].exists)
        capture(app,"grid-only-aethyr-home")
        enter.tap()
        wait(app,"grid-metrics") { ($0["fps"] as? Double ?? 0)>0 }
        var state=fields(app,"grid-metrics")
        XCTAssertEqual(state["root"] as? Bool,true)
        XCTAssertEqual(state["textInteraction"] as? Bool,false)
        XCTAssertFalse(app.buttons["grid-sessions"].exists)
        let camera=state["camera"] as? [String:Any] ?? [:]
        XCTAssertEqual(camera["mode"] as? Int,1)
        XCTAssertLessThan(camera["y"] as? Double ?? 100,5)
        let web=app.webViews.firstMatch
        XCTAssertEqual(web.frame,app.frame)
        capture(app,"grid-only-behind-player-on-entry")
        web.pinch(withScale:1.5,velocity:1)
        wait(app,"grid-metrics") { $0["scale"] as? Double == 1 }
        web.buttons["Play"].tap()
        wait(app,"grid-metrics") { $0["joined"] as? Bool == true }
        let before=fields(app,"grid-metrics")
        let stick=web.otherElements["Move"]
        stick.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.5)).press(forDuration:0.2,thenDragTo:stick.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.15)),withVelocity:.slow,thenHoldForDuration:2)
        wait(app,"grid-metrics") { hypot(($0["x"] as? Double ?? 0)-(before["x"] as? Double ?? 0),($0["z"] as? Double ?? 0)-(before["z"] as? Double ?? 0))>0.5 }
        web.buttons.matching(NSPredicate(format:"label BEGINSWITH 'Players'")).firstMatch.press(forDuration:1.5)
        XCTAssertFalse(app.menuItems["Copy"].exists)
        XCTAssertFalse(app.menuItems["Look Up"].exists)
        state=fields(app,"grid-metrics")
        XCTAssertEqual(state["scale"] as? Double,1)
        XCTAssertEqual(state["selection"] as? String,"")
        if web.buttons["Close panel"].exists { web.buttons["Close panel"].tap() }
        web.buttons["Jump"].tap()
        capture(app,"grid-only-fullscreen-touch")
        XCUIDevice.shared.press(.home)
        app.activate()
        wait(app,"grid-metrics") { $0["joined"] as? Bool == false && ($0["fps"] as? Double ?? 0)>0 }
        app.buttons["grid-close"].tap()
        wait(app,"grid-home-metrics") { $0["rendered"] as? Bool == true }
        XCTAssertTrue(app.webViews.buttons["Enter the Grid"].exists)
        XCTAssertFalse(app.buttons["new-session"].exists)
        capture(app,"grid-only-returned-home")
    }
}
