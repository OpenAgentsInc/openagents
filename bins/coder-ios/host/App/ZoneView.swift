// Rust owns zone loading, rules, and portal admission. This projection supplies
// native accessibility for the same controls drawn and picked in the world HUD.
import Foundation

struct VerseZone: Codable {
    let id: String
    let label: String
    let state: String
    let progress: Double
    let error: String?
    let caption: String
    let portal: VerseZonePortal
    let controls: [VerseZoneControl]
    let hud: VerseZoneHud

    var valid: Bool {
        ["plaza", "lagrange1", "physics_lab", "everglade"].contains(id) && label.utf8.count <= 128 &&
        ["idle", "loading", "failed"].contains(state) && progress.isFinite &&
        (0...1).contains(progress) && (error?.utf8.count ?? 0) <= 2048 &&
        caption.utf8.count <= 2048 && portal.valid && controls.count <= 16 && controls.allSatisfy(\.valid) && hud.valid
    }
    var observation: [String: Any] {
        guard let data = try? JSONEncoder().encode(self),
              let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return value
    }
}

struct VerseZonePortal: Codable {
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let distance: Double

    var valid: Bool {
        screen_x.isFinite && screen_y.isFinite && (0...1).contains(screen_x) &&
        (0...1).contains(screen_y) && distance.isFinite && distance >= 0
    }
}

struct VerseZoneControl: Codable {
    let id: String
    let label: String
    let action: String
    let enabled: Bool

    static let intents = ["enter", "return", "cancel", "retry", "firebolt", "magic_missile", "fireball", "grab", "release", "tether", "forces", "camera",
                          "knob_prev", "knob_next", "decrease", "increase", "reset", "pause", "step", "interact"]
    var valid: Bool {
        !id.isEmpty && id.utf8.count <= 64 && label.utf8.count <= 256 && Self.intents.contains(action)
    }
}

struct VerseZoneHud: Codable {
    let visible: Bool
    let frame: [Double]
    let buttons: [VerseZoneButton]
    let captured_pointers: [UInt64]

    var valid: Bool {
        VerseDoors.validFrame(frame) && buttons.count <= 16 && buttons.allSatisfy(\.valid) &&
        captured_pointers.count <= 8 && Set(captured_pointers).count == captured_pointers.count
    }
}

struct VerseZoneButton: Codable {
    let id: String
    let label: String
    let action: String
    let enabled: Bool
    let frame: [Double]

    var valid: Bool {
        !id.isEmpty && id.utf8.count <= 64 && label.utf8.count <= 256 &&
        VerseZoneControl.intents.contains(action) && VerseDoors.validFrame(frame)
    }
}
