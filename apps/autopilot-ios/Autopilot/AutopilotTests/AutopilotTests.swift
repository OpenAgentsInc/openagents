//
//  AutopilotTests.swift
//  AutopilotTests
//
//  Created by Christopher David on 2/19/26.
//

import Testing
@testable import Autopilot

struct AutopilotTests {
    @Test("parseSSEEvents parses id/event/data frames")
    func parseSSEEventsParsesFrames() {
        let raw = [
            "event: codex.worker.event",
            "id: 41",
            "data: {\"seq\":41,\"eventType\":\"worker.event\",\"payload\":{\"method\":\"ios/handshake\"}}",
            "",
            "event: codex.worker.event",
            "id: 42",
            "data: {\"seq\":42,\"eventType\":\"worker.event\",\"payload\":{\"method\":\"desktop/handshake_ack\",\"handshake_id\":\"hs-123\"}}",
            "",
        ].joined(separator: "\n")

        let events = RuntimeCodexClient.parseSSEEvents(raw: raw)

        #expect(events.count == 2)
        #expect(events[0].id == 41)
        #expect(events[0].event == "codex.worker.event")
        #expect(events[1].id == 42)
        #expect(events[1].event == "codex.worker.event")
    }

    @Test("handshake ack matcher correlates by handshake_id")
    func handshakeMatcherCorrelatesHandshakeID() {
        let ackEvent = RuntimeCodexStreamEvent(
            id: 77,
            event: "codex.worker.event",
            payload: .object([
                "eventType": .string("worker.event"),
                "payload": .object([
                    "method": .string("desktop/handshake_ack"),
                    "handshake_id": .string("hs-123"),
                ]),
            ]),
            rawData: "{}"
        )

        let unrelatedEvent = RuntimeCodexStreamEvent(
            id: 78,
            event: "codex.worker.event",
            payload: .object([
                "eventType": .string("worker.event"),
                "payload": .object([
                    "method": .string("thread/updated"),
                ]),
            ]),
            rawData: "{}"
        )

        #expect(CodexHandshakeMatcher.ackHandshakeID(from: ackEvent) == "hs-123")
        #expect(CodexHandshakeMatcher.isMatchingAck(event: ackEvent, handshakeID: "hs-123"))
        #expect(!CodexHandshakeMatcher.isMatchingAck(event: ackEvent, handshakeID: "hs-999"))
        #expect(CodexHandshakeMatcher.ackHandshakeID(from: unrelatedEvent) == nil)
    }
}
