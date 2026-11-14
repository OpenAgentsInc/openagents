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
            } else {
                let top=canvas.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0))
                top.withOffset(CGVector(dx:0,dy:max(16,(canvas.frame.height-380)/2)+341)).tap()
                let playing=XCTAttachment(screenshot:app.screenshot());playing.name="Everglade entered through native touch";playing.lifetime = .keepAlways;add(playing)
            }
            app.buttons["world.close"].tap()
        }
    }
}
