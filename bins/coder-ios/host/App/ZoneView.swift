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
    let combat: VerseZoneCombat?
    let portal: VerseZonePortal
    let controls: [VerseZoneControl]
    let hud: VerseZoneHud

    var valid: Bool {
        ["plaza", "ruins", "lagrange1", "physics_lab", "everglade"].contains(id) && label.utf8.count <= 128 &&
        ["idle", "loading", "failed"].contains(state) && progress.isFinite &&
        (0...1).contains(progress) && (error?.utf8.count ?? 0) <= 2048 &&
        caption.utf8.count <= 2048 && (combat?.valid ?? true) && portal.valid && controls.count <= 16 && controls.allSatisfy(\.valid) && hud.valid
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
                          "knob_prev", "knob_next", "decrease", "increase", "reset", "pause", "step"]
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

// This is a bounded projection of the Rust simulation, not native game state.
struct VerseZoneCombat: Codable {
    let elapsed: Double
    let player: VerseZonePlayer
    let abilities: [VerseZoneAbility]
    let actors: [VerseZoneActor]
    let projectiles: [VerseZoneProjectile]
    let effects: [VerseZoneEffect]
    let counters: VerseZoneCounters

    static let spells = ["firebolt", "magic_missile", "fireball"]
    static func vector(_ values: [Double]) -> Bool {
        values.count == 3 && values.allSatisfy { $0.isFinite }
    }
    var valid: Bool {
        elapsed.isFinite && elapsed >= 0 && player.valid &&
        abilities.count == 3 && Set(abilities.map(\.id)) == Set(Self.spells) && abilities.allSatisfy(\.valid) &&
        actors.count <= 512 && actors.allSatisfy(\.valid) &&
        projectiles.count <= 1024 && projectiles.allSatisfy(\.valid) &&
        effects.count <= 1024 && effects.allSatisfy(\.valid)
    }
}

struct VerseZonePlayer: Codable {
    let hp: Int32
    let max_hp: Int32
    let mana: Int32
    let max_mana: Int32
    var valid: Bool { max_hp > 0 && hp <= max_hp && max_mana >= 0 && mana >= 0 && mana <= max_mana }
}

struct VerseZoneAbility: Codable {
    let id: String
    let label: String
    let cost: Int32
    let ready: Bool
    let cooldown_remaining: Double
    var valid: Bool {
        VerseZoneCombat.spells.contains(id) && label.utf8.count <= 256 && cost >= 0 &&
        cooldown_remaining.isFinite && cooldown_remaining >= 0
    }
}

struct VerseZoneActor: Codable {
    let id: UInt32
    let kind: String
    let faction: String
    let pos: [Double]
    let yaw: Double
    let hp: Int32
    let max_hp: Int32
    let alive: Bool
    var valid: Bool {
        ["wizard", "zombie", "boss"].contains(kind) &&
        ["player", "wizards", "undead", "neutral"].contains(faction) &&
        VerseZoneCombat.vector(pos) && yaw.isFinite && max_hp > 0 && hp <= max_hp
    }
}

struct VerseZoneProjectile: Codable {
    let id: UInt32
    let kind: String
    let pos: [Double]
    let vel: [Double]
    var valid: Bool {
        VerseZoneCombat.spells.contains(kind) && VerseZoneCombat.vector(pos) && VerseZoneCombat.vector(vel)
    }
}

struct VerseZoneEffect: Codable {
    let kind: UInt8
    let pos: [Double]
    var valid: Bool { VerseZoneCombat.vector(pos) }
}

struct VerseZoneCounters: Codable {
    let casts: UInt64
    let projectiles: UInt64
    let hits: UInt64
}
