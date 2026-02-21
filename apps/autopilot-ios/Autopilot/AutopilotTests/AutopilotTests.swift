//
//  AutopilotTests.swift
//  AutopilotTests
//
//  Created by Christopher David on 2/19/26.
//

import Testing
import Foundation
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

    @Test("khala reconnect policy uses bounded exponential backoff with jitter")
    func khalaReconnectPolicyUsesBoundedBackoff() {
        let policy = KhalaReconnectPolicy.default

        let first = policy.delayMs(attempt: 1, jitterUnit: 0.0)
        let second = policy.delayMs(attempt: 2, jitterUnit: 0.0)
        let third = policy.delayMs(attempt: 3, jitterUnit: 0.0)
        let capped = policy.delayMs(attempt: 32, jitterUnit: 0.0)
        let jittered = policy.delayMs(attempt: 4, jitterUnit: 1.0)

        #expect(first == 250)
        #expect(second == 500)
        #expect(third == 1_000)
        #expect(capped == 8_000)
        #expect(jittered == 3_000)
    }

    @Test("khala reconnect classifier maps failure classes consistently")
    func khalaReconnectClassifierMapsFailureClasses() {
        let unauthorized = RuntimeCodexApiError(message: "unauthorized", code: .auth, status: 401)
        let staleCursor = RuntimeCodexApiError(message: "stale_cursor", code: .conflict, status: 409)
        let streamClosed = RuntimeCodexApiError(message: "khala_stream_closed", code: .network, status: nil)
        let gatewayRestart = NSError(domain: NSURLErrorDomain, code: NSURLErrorNetworkConnectionLost)

        #expect(KhalaReconnectClassifier.classify(unauthorized) == .unauthorized)
        #expect(KhalaReconnectClassifier.classify(staleCursor) == .staleCursor)
        #expect(KhalaReconnectClassifier.classify(streamClosed) == .streamClosed)
        #expect(KhalaReconnectClassifier.classify(gatewayRestart) == .gatewayRestart)
    }

    @Test("rust bridge normalization matches shared core semantics when symbols are loaded")
    func rustBridgeNormalizationParity() {
        let email = RustClientCoreBridge.normalizeEmail("  ChrIS@OpenAgents.com ")
        let code = RustClientCoreBridge.normalizeVerificationCode("Code: 123 456.")
        let message = RustClientCoreBridge.normalizeMessageText("  who are you?  ")

        if RustClientCoreBridge.isAvailable {
            #expect(email == "chris@openagents.com")
            #expect(code == "123456")
            #expect(message == "who are you?")
        } else {
            #expect(email == nil)
            #expect(code == nil)
            #expect(message == nil)
        }
    }

    @Test("rust bridge khala parser preserves frame contract when symbols are loaded")
    func rustBridgeKhalaParserParity() {
        let raw = "[\"1\",\"2\",\"sync:v1\",\"sync:heartbeat\",{\"watermarks\":[{\"topic\":\"runtime.codex_worker_events\",\"watermark\":33}]}]"
        let parsed = RustClientCoreBridge.parseKhalaFrame(raw: raw)

        if RustClientCoreBridge.isAvailable {
            #expect(parsed != nil)
            #expect(parsed?.topic == "sync:v1")
            #expect(parsed?.event == "sync:heartbeat")
            #expect(parsed?.payload.objectValue?["watermarks"]?.arrayValue?.count == 1)
        } else {
            #expect(parsed == nil)
        }
    }
}
