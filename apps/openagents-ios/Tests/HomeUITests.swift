import XCTest

final class HomeUITests: XCTestCase {
    /// Start's dolly (verse/camera.coax) has landed behind you.
    private func arrived(_ app: XCUIApplication, within timeout: TimeInterval = 15) -> Bool {
        let grid = app.otherElements["grid.world"]
        let landed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            grid.exists && (grid.value as? String).map { $0 != "arriving" } == true
        }, object: nil)
        return XCTWaiter.wait(for: [landed], timeout: timeout) == .completed
    }

    func testStartOpensTheGridAndTheStickWalks() {
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO"]  // every test starts from the first-launch overview
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        XCTAssertEqual(start.value as? String, "0")
        XCTAssertFalse(app.staticTexts["Home unavailable"].exists)
        // Dragging off Start cancels it.
        let outside = app.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.2))
        start.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.1, thenDragTo: outside)
        XCTAssertEqual(start.value as? String, "0")
        // The home is the Grid from high above, under Start; no balance yet.
        XCTAssertFalse(app.buttons["wallet.open"].exists)
        let home = XCTAttachment(screenshot: app.screenshot())
        home.name = "The Grid from above"; home.lifetime = .keepAlways; add(home)

        start.tap()
        let grid = app.otherElements["grid.world"]
        XCTAssertTrue(grid.waitForExistence(timeout: 5))
        XCTAssertFalse(start.exists)
        XCTAssertEqual(grid.value as? String, "arriving")
        Thread.sleep(forTimeInterval: 3.5)
        let swoop = XCTAttachment(screenshot: app.screenshot())
        swoop.name = "The dolly coming down"; swoop.lifetime = .keepAlways; add(swoop)
        // It takes its time, then lands behind you, where you play.
        XCTAssertEqual(grid.value as? String, "arriving")
        XCTAssertTrue(arrived(app))
        XCTAssertEqual(grid.value as? String, "0.0 0.0")
        let balance = app.buttons["wallet.open"]
        XCTAssertTrue(balance.exists)
        XCTAssertGreaterThan(balance.frame.minX, app.frame.width / 2, "the balance sits top right")
        XCTAssertLessThan(balance.frame.minY, 120)
        // Your character stands idle, animated by verse/cast.coax.
        let you = app.otherElements["grid.you"]
        XCTAssertTrue(you.waitForExistence(timeout: 5))
        XCTAssertEqual(you.value as? String, "idle")
        let arrival = XCTAttachment(screenshot: app.screenshot())
        arrival.name = "Arriving in the Grid"; arrival.lifetime = .keepAlways; add(arrival)

        // The left stick pushed up walks you forward, along +Z.
        let thumb = app.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.8))
        thumb.press(forDuration: 0.05, thenDragTo: thumb.withOffset(CGVector(dx: 0, dy: -80)),
                    withVelocity: .default, thenHoldForDuration: 2)
        // Let go: the walk fades back to idle.
        let stood = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in you.value as? String == "idle" }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [stood], timeout: 5), .completed, "you: \(you.value ?? "")")
        let walked = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            guard let parts = (grid.value as? String)?.split(separator: " "), parts.count == 2,
                  let x = Double(parts[0]), let z = Double(parts[1]) else { return false }
            return z > 2 && abs(x) < 1
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [walked], timeout: 5), .completed, "at \(grid.value ?? "")")
        let world = XCTAttachment(screenshot: app.screenshot())
        world.name = "The Grid"; world.lifetime = .keepAlways; add(world)
        // A tap on the right jumps: the jump's clips, then idle again.
        var clips = Set<String>()
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.5)).tap()
        let landed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            if let clip = you.value as? String { clips.insert(clip) }
            return clips.contains("landing") && you.value as? String == "idle"
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [landed], timeout: 5), .completed, "clips: \(clips)")

        XCUIDevice.shared.orientation = .landscapeLeft
        let rotated = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in grid.frame.width > 600 }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [rotated], timeout: 5), .completed)
        XCUIDevice.shared.orientation = .portrait
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(grid.waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["Home unavailable"].exists)
        app.terminate()
    }

    func testTheHotbarDancesAndTheTerminalStandsAhead() {
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO"]  // every test starts from the first-launch overview
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        start.tap()
        let you = app.otherElements["grid.you"]
        XCTAssertTrue(you.waitForExistence(timeout: 5))
        // A tap during the dolly hurries it down.
        Thread.sleep(forTimeInterval: 1)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        XCTAssertTrue(arrived(app, within: 4), "a tap hurries the dolly")
        let terminal = XCTAttachment(screenshot: app.screenshot())
        terminal.name = "The terminal ahead"; terminal.lifetime = .keepAlways; add(terminal)
        // The hotbar's one slot dances, like the robots.
        let dance = app.buttons["grid.dance"]
        XCTAssertTrue(dance.waitForExistence(timeout: 5))
        dance.tap()
        let dancing = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in you.value as? String == "dancing" }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [dancing], timeout: 5), .completed, "you: \(you.value ?? "")")
        Thread.sleep(forTimeInterval: 1)
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "Dancing"; shot.lifetime = .keepAlways; add(shot)
        XCTAssertEqual(you.value as? String, "dancing")
        // Walking ahead ends the dance; the terminal's desk stops you short of it.
        let grid = app.otherElements["grid.world"]
        let thumb = app.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.8))
        thumb.press(forDuration: 0.05, thenDragTo: thumb.withOffset(CGVector(dx: 0, dy: -80)),
                    withVelocity: .default, thenHoldForDuration: 3)
        let stood = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in you.value as? String == "idle" }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [stood], timeout: 5), .completed, "you: \(you.value ?? "")")
        let parts = (grid.value as? String)?.split(separator: " ").compactMap { Double($0) } ?? []
        XCTAssertEqual(parts.count, 2)
        if parts.count == 2 { XCTAssertGreaterThan(parts[1], 2.5); XCTAssertLessThan(parts[1], 3.3, "through the desk: \(grid.value ?? "")") }
        let desk = XCTAttachment(screenshot: app.screenshot())
        desk.name = "At the terminal"; desk.lifetime = .keepAlways; add(desk)
        app.terminate()
    }

    func testTheForemanStandsByTheStart() {
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO"]  // every test starts from the first-launch overview
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        start.tap()
        let you = app.otherElements["grid.you"]
        XCTAssertTrue(you.waitForExistence(timeout: 5))
        XCTAssertTrue(arrived(app))
        // Look a little left, toward the Foreman at (2.8, 2.5).
        let look = app.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.5))
        look.press(forDuration: 0.05, thenDragTo: look.withOffset(CGVector(dx: -70, dy: 0)))
        XCTAssertEqual(you.value as? String, "idle")
        let robots = XCTAttachment(screenshot: app.screenshot())
        robots.name = "The Foreman"; robots.lifetime = .keepAlways; add(robots)

        // Turn to its left leg, at (2.78, 3.62), 0.66 rad round from ahead
        // (0.006 rad a point), and walk at it: the leg stops you short of it.
        look.press(forDuration: 0.05, thenDragTo: look.withOffset(CGVector(dx: -30, dy: 0)))
        let grid = app.otherElements["grid.world"]
        var nearest = Double.infinity
        let leg = (x: 2.78, z: 3.62)
        let watch = { () -> Void in
            if let parts = (grid.value as? String)?.split(separator: " "), parts.count == 2,
               let x = Double(parts[0]), let z = Double(parts[1]) {
                nearest = min(nearest, ((x - leg.x) * (x - leg.x) + (z - leg.z) * (z - leg.z)).squareRoot())
            }
        }
        let thumb = app.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.8))
        // A walk (inside the run ring), long enough to reach it with time
        // to spare against it.
        thumb.press(forDuration: 0.05, thenDragTo: thumb.withOffset(CGVector(dx: 0, dy: -50)),
                    withVelocity: .default, thenHoldForDuration: 1.8)
        watch()
        let stood = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in watch(); return you.value as? String == "idle" }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [stood], timeout: 5), .completed)
        watch()
        // It came close (the value is to 0.1 m), and never inside the leg.
        XCTAssertGreaterThan(nearest, 0.45, "at \(grid.value ?? "")")
        XCTAssertLessThan(nearest, 1.3, "never reached the leg: at \(grid.value ?? "")")
        let blocked = XCTAttachment(screenshot: app.screenshot())
        blocked.name = "Stopped by the Foreman"; blocked.lifetime = .keepAlways; add(blocked)
        app.terminate()
    }
}
