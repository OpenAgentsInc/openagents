import XCTest

extension XCUIApplication {
    func openWorldComputer() {
        let surface = otherElements["verse-surface"]
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        let button = buttons["computer-interact"]
        XCTAssertTrue(button.waitForExistence(timeout: 15))
        if !button.isEnabled {
            let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.66))
            let forward = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.42))
            start.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 0.7)
        }
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == true"), object: button)
        XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 10), .completed)
        button.tap()
        XCTAssertTrue(buttons["computer-close"].waitForExistence(timeout: 10))
    }
}
