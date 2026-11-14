// Firm reason: 6 (XCTest exercises the installed Metal view and real public asset delivery).
import XCTest

final class WorldsUITests:XCTestCase {
    func testWorldLibraryDownloadsAndOpensTheTwoGames() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let worlds=app.buttons["world.library"];XCTAssertTrue(worlds.waitForExistence(timeout:20));worlds.tap()
        for (id,name) in [("worlds.splice","Splice Bots"),("worlds.everglade","Everglade")] {
            let open=app.buttons[id];XCTAssertTrue(open.waitForExistence(timeout:30))
            let enabled=XCTNSPredicateExpectation(predicate:NSPredicate(format:"enabled == true"),object:open);XCTAssertEqual(XCTWaiter.wait(for:[enabled],timeout:30),.completed);open.tap()
            let canvas=app.descendants(matching:.any).matching(identifier:"world.canvas").firstMatch
            XCTAssertTrue(canvas.waitForExistence(timeout:180),app.debugDescription)
            let presented=XCTNSPredicateExpectation(predicate:NSPredicate(format:"value == %@","Ready"),object:canvas);XCTAssertEqual(XCTWaiter.wait(for:[presented],timeout:30),.completed)
            XCTAssertEqual(app.webViews.count,0)
            XCTAssertFalse(app.navigationBars[name].exists)
            let menu=app.buttons["world.menu"];XCTAssertTrue(menu.exists);XCTAssertLessThanOrEqual(menu.frame.width,44);XCTAssertLessThanOrEqual(menu.frame.height,44)
            XCTAssertFalse(app.buttons["world.close"].exists)
            XCTAssertLessThanOrEqual(canvas.frame.minY,1);XCTAssertGreaterThanOrEqual(canvas.frame.maxY,app.frame.maxY-1)
            let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Installed \(name), downloaded assets";shot.lifetime = .keepAlways;add(shot)
            if id=="worlds.splice" {
                // Exercise the actual portrait touch targets on a fresh app:
                // New game, then Use this character. Keep both screens for
                // visual qualification alongside the native input oracle.
                let top=canvas.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0))
                top.withOffset(CGVector(dx:0,dy:242)).tap()
                let character=XCTAttachment(screenshot:app.screenshot());character.name="Splice portrait character selection";character.lifetime = .keepAlways;add(character)
                top.withOffset(CGVector(dx:0,dy:494)).tap()
                let playing=XCTAttachment(screenshot:app.screenshot());playing.name="Splice started through native touch";playing.lifetime = .keepAlways;add(playing)
                let look=canvas.coordinate(withNormalizedOffset:CGVector(dx:0.72,dy:0.48))
                look.press(forDuration:0.1,thenDragTo:canvas.coordinate(withNormalizedOffset:CGVector(dx:0.72,dy:0.36)))
                let up=XCTAttachment(screenshot:app.screenshot());up.name="Splice vertical camera drag up";up.lifetime = .keepAlways;add(up)
                look.press(forDuration:0.1,thenDragTo:canvas.coordinate(withNormalizedOffset:CGVector(dx:0.72,dy:0.7)))
                let down=XCTAttachment(screenshot:app.screenshot());down.name="Splice vertical camera drag down";down.lifetime = .keepAlways;add(down)
            } else {
                XCTAssertEqual(canvas.label,"Everglade")
                let start=canvas.coordinate(withNormalizedOffset:CGVector(dx:0.18,dy:0.82))
                let end=canvas.coordinate(withNormalizedOffset:CGVector(dx:0.18,dy:0.65))
                start.press(forDuration:0.15,thenDragTo:end)
                let playing=XCTAttachment(screenshot:app.screenshot());playing.name="Current Everglade II forest and path through native touch";playing.lifetime = .keepAlways;add(playing)
            }
            menu.tap();let close=app.buttons["Close world"];XCTAssertTrue(close.waitForExistence(timeout:5))
            let actions=XCTAttachment(screenshot:app.screenshot());actions.name="\(name) floating world menu";actions.lifetime = .keepAlways;add(actions)
            close.tap()
        }
    }
}
