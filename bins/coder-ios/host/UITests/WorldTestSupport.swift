import XCTest

struct VerseTestObservation: Decodable {
    let frames: UInt64
    let position: [Double]
    let highest_observed_y: Double
    let camera: [Double]
    let camera_distance: Double
    let motion_needed: Bool
    let gym_active: Bool
    let computer_ready: Bool
    let computer_target: [Double]
    let map: VerseTestMap
    let companion: VerseTestCompanion
}

struct VerseTestCompanion: Decodable {
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let reacting: Bool
    let cooldown_seconds: Double
    let pet_count: UInt64
}

struct VerseTestMap: Decodable {
    let visible: Bool
    let expanded: Bool
    let state: String
    let destination: [Double]?
    let captured_pointers: [UInt64]
    let frame: [Double]
    let plot: [Double]
    let center: [Double]
    let half_extent: Double
    let landmarks: [VerseTestLandmark]
}

struct VerseTestLandmark: Decodable {
    let id: String
    let label: String
    let x: Double
    let z: Double
}

extension XCUIApplication {
    func verseObservation() -> VerseTestObservation? {
        guard let value = otherElements["verse-surface"].value as? String,
              let data = value.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(VerseTestObservation.self, from: data)
    }

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
        guard let value = verseObservation(), value.computer_target.count == 2,
              value.computer_target.allSatisfy(\.isFinite) else { return nil }
        return (value.computer_ready, value.computer_target[0], value.computer_target[1])
    }
}
