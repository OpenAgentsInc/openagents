import Foundation

enum CodexHandshakeMatcher {
    static func ackHandshakeID(from event: RuntimeCodexStreamEvent) -> String? {
        guard let envelope = RuntimeCodexProto.decodeHandshakeEnvelope(from: event.payload),
              envelope.kind == .desktopHandshakeAck else {
            return nil
        }

        return envelope.handshakeID
    }

    static func isMatchingAck(event: RuntimeCodexStreamEvent, handshakeID: String) -> Bool {
        ackHandshakeID(from: event) == handshakeID
    }
}
