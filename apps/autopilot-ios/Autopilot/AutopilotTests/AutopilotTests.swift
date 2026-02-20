//
//  AutopilotTests.swift
//  AutopilotTests
//
//  Created by Christopher David on 2/19/26.
//

import Testing
@testable import Autopilot

struct AutopilotTests {
    @Test("decodeWorkerEvent parses khala codex worker event payload shape")
    func decodeWorkerEventParsesKhalaPayload() {
        let payload: JSONValue = .object([
            "workerId": .string("desktopw:shared"),
            "seq": .int(42),
            "eventType": .string("worker.event"),
            "payload": .object([
                "source": .string("autopilot-desktop"),
                "method": .string("desktop/handshake_ack"),
                "handshake_id": .string("hs-123"),
                "desktop_session_id": .string("session-42"),
                "occurred_at": .string("2026-02-20T00:00:02Z"),
            ]),
        ])

        let event = RuntimeCodexProto.decodeWorkerEvent(from: payload)

        #expect(event?.seq == 42)
        #expect(event?.eventType == "worker.event")
        #expect(event?.payload.method == "desktop/handshake_ack")
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

    @Test("handshake matcher falls back to raw payload frame when decode fails")
    func handshakeMatcherFallsBackToRawPayloadFrame() {
        let rawEvent = RuntimeCodexStreamEvent(
            id: 90,
            event: "codex.worker.event",
            payload: .string("not-json"),
            rawData: "{\"eventType\":\"worker.event\",\"payload\":{\"source\":\"autopilot-desktop\",\"method\":\"desktop/handshake_ack\",\"handshake_id\":\"hs-raw-123\",\"desktop_session_id\":\"session-42\",\"occurred_at\":\"2026-02-20T00:00:02Z\"}}"
        )

        #expect(CodexHandshakeMatcher.isMatchingAck(event: rawEvent, handshakeID: "hs-raw-123"))
        #expect(!CodexHandshakeMatcher.isMatchingAck(event: rawEvent, handshakeID: "hs-other"))
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
        let events = [
            RuntimeCodexStreamEvent(
                id: 41,
                event: "codex.worker.event",
                payload: .object([
                    "seq": .int(41),
                    "eventType": .string("worker.event"),
                    "payload": .object([
                        "source": .string("autopilot-ios"),
                        "method": .string("ios/handshake"),
                        "handshake_id": .string("hs-xyz"),
                        "device_id": .string("ios-device"),
                        "occurred_at": .string("2026-02-20T00:00:00Z"),
                    ]),
                ]),
                rawData: "{}"
            ),
            RuntimeCodexStreamEvent(
                id: 42,
                event: "codex.worker.event",
                payload: .object([
                    "seq": .int(42),
                    "eventType": .string("worker.event"),
                    "payload": .object([
                        "source": .string("autopilot-desktop"),
                        "method": .string("desktop/handshake_ack"),
                        "handshake_id": .string("hs-xyz"),
                        "desktop_session_id": .string("session-42"),
                        "occurred_at": .string("2026-02-20T00:00:02Z"),
                    ]),
                ]),
                rawData: "{}"
            ),
        ]
        let reconnectCursor = 41
        let replayEvents = events.filter { ($0.cursorHint ?? 0) > reconnectCursor }

        #expect(replayEvents.count == 1)
        #expect(CodexHandshakeMatcher.isMatchingAck(event: replayEvents[0], handshakeID: "hs-xyz"))
    }

    @Test("system event display policy suppresses noisy thread started events")
    func systemEventDisplayPolicySuppressesThreadStarted() {
        #expect(!CodexChatEventDisplayPolicy.shouldDisplaySystemMethod("thread/started"))
        #expect(CodexChatEventDisplayPolicy.shouldDisplaySystemMethod("turn/started"))
        #expect(CodexChatEventDisplayPolicy.shouldDisplaySystemMethod("turn/completed"))
    }

    @Test("streaming text assembler preserves token order and removes overlap duplication")
    func streamingTextAssemblerPreservesOrderAndOverlap() {
        let noInjectedSpace = CodexStreamingTextAssembler.append(existing: "I'm Cod", delta: "ex,")
        #expect(noInjectedSpace == "I'm Codex,")

        let overlapMerged = CodexStreamingTextAssembler.append(existing: "Hello wor", delta: "world")
        #expect(overlapMerged == "Hello world")

        let duplicateSkipped = CodexStreamingTextAssembler.append(existing: "Hello", delta: "Hello")
        #expect(duplicateSkipped == "Hello")
    }

    @Test("assistant delta source policy prefers legacy stream when both feeds are present")
    func assistantDeltaPolicyPrefersLegacy() {
        let firstLegacy = CodexAssistantDeltaPolicy.decide(current: nil, incoming: .legacyContent)
        #expect(firstLegacy.selectedSource == .legacyContent)
        #expect(firstLegacy.shouldAccept)
        #expect(!firstLegacy.shouldReset)

        let switchFromModernToLegacy = CodexAssistantDeltaPolicy.decide(current: .modern, incoming: .legacyContent)
        #expect(switchFromModernToLegacy.selectedSource == .legacyContent)
        #expect(switchFromModernToLegacy.shouldAccept)
        #expect(switchFromModernToLegacy.shouldReset)

        let ignoreModernAfterLegacy = CodexAssistantDeltaPolicy.decide(current: .legacyContent, incoming: .modern)
        #expect(ignoreModernAfterLegacy.selectedSource == .legacyContent)
        #expect(!ignoreModernAfterLegacy.shouldAccept)
        #expect(!ignoreModernAfterLegacy.shouldReset)
    }
}
