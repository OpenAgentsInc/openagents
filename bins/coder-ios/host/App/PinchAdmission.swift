// Native recognizers arbitrate contacts; Rust still owns movement and camera state.
import Foundation

struct PinchAdmission {
    private struct Contact {
        let began: Double
        let x: Double
        let y: Double
        var currentX: Double
        var currentY: Double
        var moved = false
    }
    private var contacts: [UInt64: Contact] = [:]
    private var previousDistance: Double?
    private(set) var reserved = false
    var allowed: Bool { reserved && contacts.count == 2 }

    mutating func down(_ id: UInt64, x: Double, y: Double, time: Double) {
        if contacts.isEmpty { reset() }
        if contacts.count == 1, let first = contacts.values.first,
           abs(time - first.began) <= 0.15, !first.moved {
            reserved = true
        }
        contacts[id] = Contact(began: time, x: x, y: y, currentX: x, currentY: y)
    }

    mutating func move(_ id: UInt64, x: Double, y: Double) {
        guard var contact = contacts[id] else { return }
        contact.moved = contact.moved || hypot(x - contact.x, y - contact.y) > 8
        contact.currentX = x
        contact.currentY = y
        contacts[id] = contact
    }

    // Sample after each native event batch so both contacts use the same event.
    // The first sample anchors each sequence; it never applies an old scale.
    mutating func scale() -> Double? {
        guard allowed else { previousDistance = nil; return nil }
        let points = Array(contacts.values)
        let distance = hypot(points[0].currentX - points[1].currentX,
                             points[0].currentY - points[1].currentY)
        guard distance.isFinite, distance >= 1 else { previousDistance = nil; return nil }
        defer { previousDistance = distance }
        guard let previousDistance else { return nil }
        return distance / previousDistance
    }

    mutating func up(_ id: UInt64) {
        contacts.removeValue(forKey: id)
        previousDistance = nil
        if contacts.isEmpty { reset() }
    }

    mutating func reset() { contacts.removeAll(); reserved = false; previousDistance = nil }
}
