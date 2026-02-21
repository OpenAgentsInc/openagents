import Foundation

enum CodexHandshakeMatcher {
    private static let ackMethodToken = "\"method\":\"desktop/handshake_ack\""

    static func ackHandshakeID(from event: RuntimeCodexStreamEvent) -> String? {
        if let handshakeID = RustClientCoreBridge.extractDesktopHandshakeAckID(payloadJSON: event.rawData) {
            return handshakeID
        }

        guard let envelope = RuntimeCodexProto.decodeHandshakeEnvelope(from: event.payload),
              envelope.kind == .desktopHandshakeAck else {
            return nil
        }

        return envelope.handshakeID
    }

    static func isMatchingAck(event: RuntimeCodexStreamEvent, handshakeID: String) -> Bool {
        if ackHandshakeID(from: event) == handshakeID {
            return true
        }

        return rawPayloadLooksLikeMatchingAck(event: event, handshakeID: handshakeID)
    }

    private static func rawPayloadLooksLikeMatchingAck(
        event: RuntimeCodexStreamEvent,
        handshakeID: String
    ) -> Bool {
        let compactRaw = event.rawData.replacingOccurrences(of: " ", with: "")
        guard compactRaw.contains(ackMethodToken) else {
            return false
        }

        if compactRaw.contains("\"handshake_id\":\"\(handshakeID)\"") {
            return true
        }

        if compactRaw.contains("\"handshakeId\":\"\(handshakeID)\"") {
            return true
        }

        return false
    }
}
