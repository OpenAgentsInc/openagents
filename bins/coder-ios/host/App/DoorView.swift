// Typed projection only. Rust owns compatibility, selection, routes, and memory.
import Foundation

struct VerseDoors: Codable {
    let held: String
    let doors: [VerseDoor]
    let hud: VerseDoorHud
    let error: String?

    var valid: Bool {
        Self.items.contains(held) && doors.count <= 8 && doors.allSatisfy(\.valid) && hud.valid &&
        (error?.utf8.count ?? 0) <= 1024
    }
    var observation: [String: Any] {
        guard let data = try? JSONEncoder().encode(self),
              let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return value
    }
    static let items = ["prism", "ring", "bolt", "empty"]
    static let ids = ["spark", "halo"]
    static func validFrame(_ frame: [Double]) -> Bool {
        frame.count == 4 && frame.allSatisfy(\.isFinite) && frame[2] >= 0 && frame[3] >= 0
    }
}

struct VerseDoor: Codable {
    let id: String
    let label: String
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let distance: Double
    let state: String
    let destination: String?
    let remembered: String?

    var valid: Bool {
        VerseDoors.ids.contains(id) && label.utf8.count <= 128 &&
        screen_x.isFinite && screen_y.isFinite && (0...1).contains(screen_x) && (0...1).contains(screen_y) &&
        distance.isFinite && distance >= 0 && ["idle", "reacting", "selected", "cooldown"].contains(state) &&
        (destination?.utf8.count ?? 0) <= 128 && (remembered == nil || VerseDoors.items.contains(remembered!))
    }
}

struct VerseDoorHud: Codable {
    let visible: Bool
    let door: String?
    let held: String
    let caption: String
    let frame: [Double]
    let buttons: [VerseDoorButton]
    let captured_pointers: [UInt64]

    var valid: Bool {
        (door == nil || VerseDoors.ids.contains(door!)) && VerseDoors.items.contains(held) &&
        caption.utf8.count <= 512 && VerseDoors.validFrame(frame) && buttons.count <= 8 &&
        buttons.allSatisfy(\.valid) && captured_pointers.count <= 8 &&
        Set(captured_pointers).count == captured_pointers.count
    }
}

struct VerseDoorButton: Codable {
    let id: String
    let label: String
    let action: VerseDoorAction
    let frame: [Double]
    let selected: Bool
    let enabled: Bool

    var valid: Bool {
        id.utf8.count <= 64 && label.utf8.count <= 128 && VerseDoors.validFrame(frame) && action.valid
    }
}

struct VerseDoorAction: Codable {
    let action: String
    let item: String?
    let door: String?

    var valid: Bool {
        switch action {
        case "door_hold": return item.map(VerseDoors.items.contains) == true && door == nil
        case "door_reset": return door.map(VerseDoors.ids.contains) == true && item == nil
        default: return false
        }
    }
    var request: [String: Any] {
        if action == "door_hold", let item { return ["action": action, "item": item] }
        if action == "door_reset", let door { return ["action": action, "door": door] }
        return [:]
    }
}
