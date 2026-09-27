import XCTest

struct VerseTestObservation: Decodable {
    let frames: UInt64
    let position: [Double]
    let highest_observed_y: Double
    let camera: [Double]
    let camera_distance: Double
    let motion_needed: Bool
    let gym_active: Bool
    let gym_ready: Bool
    let gym_target: [Double]
    let world_preference: String
    let world_configuration: VerseTestWorldConfiguration
    let world_connection: VerseTestWorldConnection
    let world_public_key: String
    let remote_entities: UInt64
    let live_remote_entities: UInt64
    let presented_remote_vertices: UInt64
    let computer_ready: Bool
    let computer_target: [Double]
    let computer_open: Bool
    let computer_page: String
    let computer_hud: VerseTestComputerHud
    let map: VerseTestMap
    let companion: VerseTestCompanion
    let doors: VerseTestDoors
    let zone: VerseTestZone
    let door_preferences_revision: UInt64
    let door_storage_writes: UInt64
}

struct VerseTestComputerHud: Decodable {
    let visible: Bool
    let page: String
    let busy: Bool
    let scroll: Double
    let max_scroll: Double
    let body: [Double]
    let input: String
    let items: [VerseTestHudItem]
}

struct VerseTestHudItem: Decodable {
    let key: String
    let label: String
    let role: String
    let enabled: Bool
    let frame: [Double]
}

struct VerseTestWorldConfiguration: Decodable {
    let world_relay: String?
    let world_offline: Bool?
}

struct VerseTestWorldConnection: Decodable {
    let state: String
    let relay: String
}

struct VerseTestZone: Decodable {
    let id: String
    let state: String
    let progress: Double
    let error: String?
    let caption: String
    let combat: VerseTestCombat?
    let portal: VerseTestZonePortal
    let hud: VerseTestZoneHud
}
struct VerseTestCombat: Decodable {
    let elapsed: Double
    let player: VerseTestPlayer
    let abilities: [VerseTestAbility]
    let actors: [VerseTestActor]
    let projectiles: [VerseTestProjectile]
    let counters: VerseTestCombatCounters
}
struct VerseTestPlayer: Decodable {
    let hp: Int32
    let max_hp: Int32
    let mana: Int32
    let max_mana: Int32
}
struct VerseTestAbility: Decodable {
    let id: String
    let ready: Bool
    let cooldown_remaining: Double
}
struct VerseTestActor: Decodable {
    let id: UInt32
    let kind: String
    let faction: String
    let pos: [Double]
    let hp: Int32
    let alive: Bool
}
struct VerseTestProjectile: Decodable {
    let id: UInt32
    let kind: String
    let pos: [Double]
}
struct VerseTestCombatCounters: Decodable {
    let casts: UInt64
    let projectiles: UInt64
    let hits: UInt64
}
struct VerseTestZonePortal: Decodable {
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
}
struct VerseTestZoneHud: Decodable {
    let visible: Bool
    let frame: [Double]
    let buttons: [VerseTestZoneButton]
    let captured_pointers: [UInt64]
}
struct VerseTestZoneButton: Decodable {
    let id: String
    let label: String
    let action: String
    let enabled: Bool
    let frame: [Double]
}

struct VerseTestDoors: Decodable {
    let held: String
    let doors: [VerseTestDoor]
    let hud: VerseTestDoorHud
    let error: String?
}
struct VerseTestDoor: Decodable {
    let id: String
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let state: String
    let destination: String?
    let remembered: String?
}
struct VerseTestDoorHud: Decodable {
    let visible: Bool
    let door: String?
    let caption: String
    let frame: [Double]
    let buttons: [VerseTestDoorButton]
    let captured_pointers: [UInt64]
}
struct VerseTestDoorButton: Decodable {
    let id: String
    let frame: [Double]
    let enabled: Bool
    let selected: Bool
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

    /// Open the world computer on its native Chats page.
    func openWorldComputer(beforeTap: (() -> Void)? = nil) {
        openWorldComputerHud(beforeTap: beforeTap)
        let chats = buttons["hud-tab-chats"]
        XCTAssertTrue(chats.waitForExistence(timeout: 10))
        chats.tap()
        XCTAssertTrue(buttons["computer-close"].waitForExistence(timeout: 10))
    }

    /// Open the world computer. It shows the Computers screens in the world
    /// HUD, which Rust draws and hit-tests.
    func openWorldComputerHud(beforeTap: (() -> Void)? = nil) {
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
        let shown = buttons["hud-close"].waitForExistence(timeout: 10)
        let state = verseObservation().map {
            "open \($0.computer_open), page \($0.computer_page), hud \($0.computer_hud.visible), items \($0.computer_hud.items.count)"
        } ?? "no observation"
        XCTAssertTrue(shown, "The computer's HUD did not open: \(state)")
        XCTAssertFalse(buttons["computer-close"].exists, "Computers draw in the world, not a native panel.")
    }

    /// Tap a control the HUD laid out, scrolling its body until it is fully
    /// in view. The tap is a real touch on the world surface.
    func hudTap(_ key: String, timeout: TimeInterval = 20) {
        let control = hudElement(key, timeout: timeout)
        XCTAssertTrue(control.isEnabled, "\(key) is disabled")
        control.tap()
    }

    /// Wait for a HUD element, scrolling its body down, then up, to find it.
    @discardableResult
    func hudElement(_ key: String, timeout: TimeInterval = 20) -> XCUIElement {
        let element = descendants(matching: .any)[key]
        let deadline = Date().addingTimeInterval(timeout)
        var direction = 1.0
        while !element.exists && Date() < deadline {
            if let hud = verseObservation()?.computer_hud, hud.max_scroll > 0 {
                if hud.scroll >= hud.max_scroll - 1 { direction = -1 }
                if hud.scroll <= 1 { direction = 1 }
                hudSwipe(direction, hud: hud)
            } else {
                _ = element.waitForExistence(timeout: 0.5)
            }
        }
        XCTAssertTrue(element.exists, "\(key) is not on the computer's screen")
        return element
    }

    /// Drag the HUD body: positive moves further down the screen's content.
    func hudSwipe(_ direction: Double, hud: VerseTestComputerHud) {
        let surface = otherElements["verse-surface"]
        let frame = surface.frame
        guard hud.body.count == 4, frame.width > 0, frame.height > 0 else { return }
        let x = (hud.body[0] + hud.body[2] * 0.5) / frame.width
        let top = (hud.body[1] + hud.body[3] * 0.25) / frame.height
        let bottom = (hud.body[1] + hud.body[3] * 0.75) / frame.height
        let from = surface.coordinate(withNormalizedOffset: CGVector(dx: x, dy: direction > 0 ? bottom : top))
        let to = surface.coordinate(withNormalizedOffset: CGVector(dx: x, dy: direction > 0 ? top : bottom))
        from.press(forDuration: 0.05, thenDragTo: to, withVelocity: .default, thenHoldForDuration: 0.1)
    }

    /// Scroll the HUD back to its top.
    func hudScrollToTop() {
        for _ in 0..<12 {
            guard let hud = verseObservation()?.computer_hud, hud.scroll > 1 else { return }
            hudSwipe(-1, hud: hud)
        }
    }

    /// The label of a HUD text or control, once it exists.
    func hudLabel(_ key: String, timeout: TimeInterval = 20) -> String {
        hudElement(key, timeout: timeout).label
    }

    /// Wait until a HUD element's label contains `expected`, polling as the
    /// HUD's screen refreshes.
    func waitForHud(_ key: String, containing expected: String, timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let element = descendants(matching: .any)[key]
            if element.exists && element.label.contains(expected) { return true }
            _ = element.waitForExistence(timeout: 1)
        }
        return false
    }

    private func computerTarget(_ surface: XCUIElement) -> (ready: Bool, x: Double, y: Double)? {
        guard let value = verseObservation(), value.computer_target.count == 2,
              value.computer_target.allSatisfy(\.isFinite) else { return nil }
        return (value.computer_ready, value.computer_target[0], value.computer_target[1])
    }
}
