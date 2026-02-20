import Foundation

enum CodexHandshakeMatcher {
    static func ackHandshakeID(from event: RuntimeCodexStreamEvent) -> String? {
        guard let object = event.payload.objectValue else {
            return nil
        }

        let eventType = object["eventType"]?.stringValue ?? object["event_type"]?.stringValue
        guard eventType == "worker.event" else {
            return nil
        }

        guard let payload = object["payload"]?.objectValue else {
            return nil
        }

        let method = payload["method"]?.stringValue
        guard method == "desktop/handshake_ack" else {
            return nil
        }

        return payload["handshake_id"]?.stringValue ?? payload["handshakeId"]?.stringValue
    }

    static func isMatchingAck(event: RuntimeCodexStreamEvent, handshakeID: String) -> Bool {
        ackHandshakeID(from: event) == handshakeID
    }
}
