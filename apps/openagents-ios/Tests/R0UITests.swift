import XCTest

final class R0UITests: XCTestCase {
    func testRealGridFramesProduceBenchmarkReport() {
        let app = XCUIApplication(); app.launchArguments += ["-OABenchmark"]; app.launch()
        let start = app.buttons["home.start"]; XCTAssertTrue(start.waitForExistence(timeout: 30)); start.tap()
        let grid = app.otherElements["grid.world"]
        let done = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in grid.exists && (grid.value as? String) == "benchmark.complete" }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [done], timeout: 90), .completed)
        XCTAssertTrue(app.otherElements["grid.you"].exists)
        let screenshot = XCTAttachment(screenshot: app.screenshot()); screenshot.name = "R0 live Metal frame"; screenshot.lifetime = .keepAlways; add(screenshot)
    }
}
