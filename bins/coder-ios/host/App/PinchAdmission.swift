// Native recognizers arbitrate contacts; Rust still owns movement and camera state.
import Foundation

struct PinchAdmission {
    private struct Contact { let began: Double; let x: Double; let y: Double; var moved = false }
    private var contacts: [UInt64: Contact] = [:]
    private(set) var reserved = false
    var allowed: Bool { reserved && contacts.count == 2 }

    mutating func down(_ id: UInt64, x: Double, y: Double, time: Double) {
        if contacts.isEmpty { reserved = false }
        if contacts.count == 1, let first = contacts.values.first,
           abs(time - first.began) <= 0.15, !first.moved {
            reserved = true
        }
        contacts[id] = Contact(began: time, x: x, y: y)
    }

    mutating func move(_ id: UInt64, x: Double, y: Double) {
        guard var contact = contacts[id] else { return }
        contact.moved = contact.moved || hypot(x - contact.x, y - contact.y) > 8
        contacts[id] = contact
    }

    mutating func up(_ id: UInt64) {
        contacts.removeValue(forKey: id)
        if contacts.isEmpty { reset() }
    }

    mutating func reset() { contacts.removeAll(); reserved = false }
}
