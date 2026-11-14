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
            app.buttons["world.close"].tap()
        }
    }
}
