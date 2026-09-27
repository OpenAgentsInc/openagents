// Native portal taps load the reviewed pack over HTTPS or from the verified
// device cache. Artwork is never an app resource.
import XCTest

final class ZoneUITests: XCTestCase {
    private var app: XCUIApplication!
    private var surface: XCUIElement { app.otherElements["verse-surface"] }

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        waitFor { $0.frames > 0 && $0.map.visible }
    }

    func testPortalLoadsRealtimeForestAndHotbarThenReturnsToPlaza() throws {
        let initial = try observation()
        XCTAssertEqual(initial.zone.id, "plaza")
        XCTAssertEqual(initial.zone.state, "idle")
        XCTAssertEqual(initial.zone.progress, 0)
        waitFor { $0.frames > initial.frames + 12 }
        XCTAssertEqual(try observation().zone.progress, 0, "Starting Verse does not load the forest.")
        try approachPortal()
        let plaza = try observation()
        XCTAssertTrue(plaza.zone.portal.near && plaza.zone.portal.visible)
        surface.coordinate(withNormalizedOffset: CGVector(dx: plaza.zone.portal.screen_x,
                                                          dy: plaza.zone.portal.screen_y)).tap()
        waitFor(timeout: 45) { $0.zone.id == "forest" && $0.zone.state == "idle" }
        let forest = try observation()
        XCTAssertFalse(forest.gym_active)
        XCTAssertFalse(forest.computer_ready)
        XCTAssertFalse(forest.doors.hud.visible)
        XCTAssertFalse(forest.map.landmarks.contains { $0.id == "gym" })
        XCTAssertTrue(forest.map.landmarks.contains { $0.id == "return" })
        XCTAssertTrue(forest.map.landmarks.allSatisfy { abs($0.x) <= forest.map.half_extent && abs($0.z) <= forest.map.half_extent })
        XCTAssertNil(forest.zone.error)
        attach("Runtime-loaded Atlantis forest")

        let initialCombat = try XCTUnwrap(forest.zone.combat)
        XCTAssertEqual(Set(initialCombat.abilities.map(\.id)), Set(["firebolt", "magic_missile", "fireball"]))
        XCTAssertFalse(forest.zone.hud.buttons.contains { ["start_encounter", "cast", "end_turn", "reset_encounter"].contains($0.action) })
        XCTAssertTrue(initialCombat.actors.contains { $0.kind == "zombie" })
        XCTAssertTrue(initialCombat.actors.contains { $0.kind == "wizard" && $0.faction != "player" })
        let positions = Dictionary(uniqueKeysWithValues: initialCombat.actors.filter { $0.faction != "player" }.map { ($0.id, $0.pos) })
        waitFor { value in
            guard let combat = value.zone.combat, combat.elapsed > initialCombat.elapsed + 0.5 else { return false }
            return combat.actors.contains { actor in
                guard let old = positions[actor.id] else { return false }
                return hypot(actor.pos[0] - old[0], actor.pos[2] - old[2]) > 0.1
            }
        }

        // Walking and casting share the Rust simulation. The route remains
        // active while this test taps the rendered ability, with no turn gate.
        try walkToLandmark("grove")
        let beforeCast = try observation()
        let beforeCombat = try XCTUnwrap(beforeCast.zone.combat)
        XCTAssertGreaterThanOrEqual(beforeCombat.player.mana, 5)
        var firedObservation: VerseTestObservation?
        try tapAction("fireball")
        waitFor(timeout: 4) { value in
            guard let combat = value.zone.combat,
                  let ability = combat.abilities.first(where: { $0.id == "fireball" }) else { return false }
            let fired = combat.counters.casts > beforeCombat.counters.casts &&
                combat.player.mana < beforeCombat.player.mana && ability.cooldown_remaining > 0 && !ability.ready
            if fired { firedObservation = value }
            return fired
        }
        let observedFireball = try XCTUnwrap(firedObservation)
        let fired = try XCTUnwrap(observedFireball.zone.combat)
        XCTAssertGreaterThan(fired.counters.projectiles, beforeCombat.counters.projectiles)
        let fireball = try XCTUnwrap(fired.abilities.first { $0.id == "fireball" })
        let receipt: [String: Any] = [
            "before_frame": beforeCast.frames, "after_frame": observedFireball.frames,
            "before_position": beforeCast.position, "after_position": observedFireball.position,
            "before_mana": beforeCombat.player.mana, "after_mana": fired.player.mana,
            "before_cast_requests": beforeCombat.counters.casts, "after_cast_requests": fired.counters.casts,
            "before_all_projectiles": beforeCombat.counters.projectiles, "after_all_projectiles": fired.counters.projectiles,
            "fireball_cooldown_seconds": fireball.cooldown_remaining, "fireball_ready": fireball.ready,
        ]
        let receiptData = try JSONSerialization.data(withJSONObject: receipt, options: [.prettyPrinted, .sortedKeys])
        let receiptAttachment = XCTAttachment(data: receiptData, uniformTypeIdentifier: "public.json")
        receiptAttachment.name = "Player Fireball input, mana, and cooldown receipt"
        receiptAttachment.lifetime = .keepAlways
        add(receiptAttachment)
        attach("Fireball hotbar during cooldown")
        waitFor { value in
            hypot(value.position[0] - beforeCast.position[0], value.position[2] - beforeCast.position[2]) > 0.5
        }
        waitFor { value in
            value.zone.combat?.abilities.first(where: { $0.id == "fireball" })?.ready == true
        }
        attach("Real-time forest and GPU ability hotbar")
        attachObservation("Real-time combat after Fireball")
        try tapAction("return")
        waitFor { $0.zone.id == "plaza" && $0.zone.state == "idle" }
        let returned = try observation()
        XCTAssertEqual(returned.position[0], plaza.position[0], accuracy: 0.1)
        XCTAssertEqual(returned.position[2], plaza.position[2], accuracy: 0.1)
        XCTAssertTrue(returned.map.landmarks.contains { $0.id == "gym" })
        XCTAssertNil(returned.zone.combat)
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        attach("Returned to the amber plaza")
    }

    func testRulesAndArtworkNoticesShipWithoutLoadingForest() throws {
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        let about = app.buttons.matching(NSPredicate(format: "identifier == %@ OR label == %@", "verse-about", "About Verse")).firstMatch
        for _ in 0..<4 {
            if about.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(about.waitForExistence(timeout: 10))
        about.tap()
        let credits = app.staticTexts["verse-credits"]
        XCTAssertTrue(credits.waitForExistence(timeout: 10))
        let loaded = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            credits.label.contains("Wizards of the Coast LLC") && credits.label.contains("Apache License")
        }, object: credits)
        XCTAssertEqual(XCTWaiter.wait(for: [loaded], timeout: 10), .completed)
        XCTAssertEqual(try observation().zone.id, "plaza")
        XCTAssertEqual(try observation().zone.progress, 0)
        attach("Bundled Verse rules and artwork notices")
    }

    private func approachPortal() throws {
        let compact = try observation().map
        tap(compact.frame[0] + compact.frame[2] / 2, compact.frame[1] + 12)
        waitFor { $0.map.expanded }
        let map = try observation().map
        let destination = try XCTUnwrap(map.landmarks.first { $0.id == "forest" })
        tap(map.plot[0] + ((destination.x - map.center[0]) / map.half_extent + 1) * map.plot[2] / 2,
            map.plot[1] + (1 - (destination.z - map.center[1]) / map.half_extent) * map.plot[3] / 2)
        waitFor { $0.map.state == "Arrived" }
        for _ in 0..<16 {
            let current = try observation()
            if current.zone.portal.near && current.zone.portal.visible && current.zone.hud.visible { break }
            let angle = atan2(sin(current.camera[0]), cos(current.camera[0]))
            let points = max(-60, min(60, angle / 0.004))
            let start = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.48))
            start.press(forDuration: 0.1, thenDragTo: start.withOffset(CGVector(dx: points, dy: 0)),
                        withVelocity: .slow, thenHoldForDuration: 0)
        }
        waitFor { $0.zone.portal.near && $0.zone.portal.visible && $0.zone.hud.visible }
    }

    private func walkToLandmark(_ id: String) throws {
        let compact = try observation().map
        tap(compact.frame[0] + compact.frame[2] / 2, compact.frame[1] + 12)
        waitFor { $0.map.expanded }
        let map = try observation().map
        let destination = try XCTUnwrap(map.landmarks.first { $0.id == id })
        tap(map.plot[0] + ((destination.x - map.center[0]) / map.half_extent + 1) * map.plot[2] / 2,
            map.plot[1] + (1 - (destination.z - map.center[1]) / map.half_extent) * map.plot[3] / 2)
        waitFor { $0.map.destination != nil }
    }

    private func attachObservation(_ name: String) {
        guard let value = surface.value as? String else { return }
        let attachment = XCTAttachment(string: value)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func tapAction(_ action: String) throws {
        let zone = try observation().zone
        XCTAssertTrue(zone.hud.visible)
        let control = try XCTUnwrap(zone.hud.buttons.first { $0.action == action && $0.enabled })
        tap(control.frame[0] + control.frame[2] / 2, control.frame[1] + control.frame[3] / 2)
    }
    private func observation() throws -> VerseTestObservation { try XCTUnwrap(app.verseObservation()) }
    private func tap(_ x: Double, _ y: Double) {
        surface.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)).tap()
    }
    private func waitFor(timeout: TimeInterval = 25, _ predicate: @escaping (VerseTestObservation) -> Bool) {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            self.app.verseObservation().map(predicate) ?? false
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed)
    }
    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
