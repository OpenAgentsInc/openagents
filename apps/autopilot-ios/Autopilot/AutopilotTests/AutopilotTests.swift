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
            "data: {\"seq\":41,\"eventType\":\"worker.event\",\"payload\":{\"source\":\"autopilot-ios\",\"method\":\"ios/handshake\",\"handshake_id\":\"hs-123\",\"device_id\":\"ios-device\",\"occurred_at\":\"2026-02-20T00:00:00Z\"}}",
            "",
            "event: codex.worker.event",
            "id: 42",
            "data: {\"seq\":42,\"eventType\":\"worker.event\",\"payload\":{\"source\":\"autopilot-desktop\",\"method\":\"desktop/handshake_ack\",\"handshake_id\":\"hs-123\",\"desktop_session_id\":\"session-42\",\"occurred_at\":\"2026-02-20T00:00:02Z\"}}",
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
                    "source": .string("autopilot-desktop"),
                    "method": .string("desktop/handshake_ack"),
                    "handshake_id": .string("hs-123"),
                    "desktop_session_id": .string("session-42"),
                    "occurred_at": .string("2026-02-20T00:00:02Z"),
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

    @Test("proto handshake decoder rejects incomplete desktop ack envelope")
    func protoHandshakeDecoderRejectsIncompleteAck() {
        let invalidAckEvent = RuntimeCodexStreamEvent(
            id: 80,
            event: "codex.worker.event",
            payload: .object([
                "eventType": .string("worker.event"),
                "payload": .object([
                    "source": .string("autopilot-desktop"),
                    "method": .string("desktop/handshake_ack"),
                    "handshake_id": .string("hs-abc"),
                    // desktop_session_id intentionally omitted
                    "occurred_at": .string("2026-02-20T00:00:02Z"),
                ]),
            ]),
            rawData: "{}"
        )

        #expect(CodexHandshakeMatcher.ackHandshakeID(from: invalidAckEvent) == nil)
    }

    @Test("cursor resume keeps matching ack after reconnect boundary")
    func cursorResumeKeepsMatchingAckAfterReconnectBoundary() {
        let raw = [
            "event: codex.worker.event",
            "id: 41",
            "data: {\"seq\":41,\"eventType\":\"worker.event\",\"payload\":{\"source\":\"autopilot-ios\",\"method\":\"ios/handshake\",\"handshake_id\":\"hs-xyz\",\"device_id\":\"ios-device\",\"occurred_at\":\"2026-02-20T00:00:00Z\"}}",
            "",
            "event: codex.worker.event",
            "id: 42",
            "data: {\"seq\":42,\"eventType\":\"worker.event\",\"payload\":{\"source\":\"autopilot-desktop\",\"method\":\"desktop/handshake_ack\",\"handshake_id\":\"hs-xyz\",\"desktop_session_id\":\"session-42\",\"occurred_at\":\"2026-02-20T00:00:02Z\"}}",
            "",
        ].joined(separator: "\n")

        let events = RuntimeCodexClient.parseSSEEvents(raw: raw)
        let reconnectCursor = 41
        let replayEvents = events.filter { ($0.cursorHint ?? 0) > reconnectCursor }

        #expect(replayEvents.count == 1)
        #expect(CodexHandshakeMatcher.isMatchingAck(event: replayEvents[0], handshakeID: "hs-xyz"))
    }
}
