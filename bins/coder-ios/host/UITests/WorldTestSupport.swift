import XCTest

extension XCUIApplication {
    func openWorldComputer(beforeTap: (() -> Void)? = nil) {
        let surface = otherElements["verse-surface"]
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        XCTAssertFalse(buttons["computer-interact"].exists, "The computer is drawn in the world, not as a native button.")
        for _ in 0..<5 {
            if computerTarget(surface)?.ready == true { break }
            let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.50))
            let forward = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.32))
            start.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 0.7)
        }
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            self.computerTarget(surface)?.ready == true
        }, object: surface)
        XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 10), .completed)
        guard let target = computerTarget(surface) else {
            XCTFail("The Rust world did not expose a projected monitor target.")
            return
        }
        beforeTap?()
        surface.coordinate(withNormalizedOffset: CGVector(dx: target.x, dy: target.y)).tap()
        XCTAssertTrue(buttons["computer-close"].waitForExistence(timeout: 10))
    }

    private func computerTarget(_ surface: XCUIElement) -> (ready: Bool, x: Double, y: Double)? {
        guard let value = surface.value as? String else { return nil }
        let fields = value.split(separator: ":")
        guard fields.count == 4, fields[0] == "computer",
              let x = Double(fields[2]), let y = Double(fields[3]), x.isFinite, y.isFinite else { return nil }
        return (fields[1] == "ready", x, y)
    }
}
