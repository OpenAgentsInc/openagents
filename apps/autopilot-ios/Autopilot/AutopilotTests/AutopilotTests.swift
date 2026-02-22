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

    @Test("rust bridge enforces ffi contract version when symbols are loaded")
    func rustBridgeFFIContractVersionGate() {
        if RustClientCoreBridge.isAvailable {
            #expect(RustClientCoreBridge.isContractVersionCompatible)
            #expect(RustClientCoreBridge.ffiContractVersion == RustClientCoreBridge.expectedContractVersion)
        } else {
            #expect(RustClientCoreBridge.ffiContractVersion == nil || !RustClientCoreBridge.isContractVersionCompatible)
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

    @Test("auth flow state keeps latest send-code challenge and drops stale completion")
    func authFlowStateDropsStaleSendCompletion() {
        var flow = CodexAuthFlowState()
        let first = flow.beginSend(email: "one@openagents.com")
        let second = flow.beginSend(email: "two@openagents.com")

        let staleResolved = flow.resolveSend(generation: first, challengeID: "challenge-1")
        let latestResolved = flow.resolveSend(generation: second, challengeID: "challenge-2")
        #expect(!staleResolved)
        #expect(latestResolved)

        let verify = flow.beginVerify()
        #expect(verify?.generation == second)
        #expect(verify?.challengeID == "challenge-2")
        #expect(verify?.email == "two@openagents.com")
    }

    @Test("auth flow state invalidation rejects stale verify responses")
    func authFlowStateRejectsStaleVerifyResponses() {
        var flow = CodexAuthFlowState()
        let generation = flow.beginSend(email: "race@openagents.com")
        let resolved = flow.resolveSend(generation: generation, challengeID: "challenge-race")
        #expect(resolved)

        let verify = flow.beginVerify()
        #expect(verify?.generation == generation)

        flow.invalidate()
        #expect(!flow.shouldAcceptResponse(generation: generation))
    }

    @Test("resume checkpoint store isolates watermarks by namespace and worker")
    func resumeCheckpointStoreNamespacesWatermarks() {
        var store = CodexResumeCheckpointStore()
        let topic = "runtime.codex_worker_events"

        store.upsert(
            namespace: "device:ios-1|user:user-1",
            workerID: "desktopw:shared",
            topic: topic,
            watermark: 12,
            sessionID: "session-1",
            updatedAt: "2026-02-21T10:00:00Z"
        )
        store.upsert(
            namespace: "device:ios-1|user:user-1",
            workerID: "desktopw:shared",
            topic: topic,
            watermark: 8,
            sessionID: "session-1",
            updatedAt: "2026-02-21T10:00:01Z"
        )
        store.upsert(
            namespace: "device:ios-1|user:user-2",
            workerID: "desktopw:shared",
            topic: topic,
            watermark: 4,
            sessionID: "session-2",
            updatedAt: "2026-02-21T10:00:02Z"
        )

        #expect(store.watermark(namespace: "device:ios-1|user:user-1", workerID: "desktopw:shared", topic: topic) == 12)
        #expect(store.watermark(namespace: "device:ios-1|user:user-2", workerID: "desktopw:shared", topic: topic) == 4)
        #expect(store.watermark(namespace: "device:ios-1|user:user-1", workerID: "desktopw:other", topic: topic) == 0)
    }

    @Test("resume checkpoint store topic reset is scoped and deterministic")
    func resumeCheckpointStoreResetTopic() {
        var store = CodexResumeCheckpointStore()
        let namespace = "device:ios-2|user:user-10"
        let workerID = "desktopw:shared"

        store.upsert(
            namespace: namespace,
            workerID: workerID,
            topic: "runtime.codex_worker_events",
            watermark: 40,
            sessionID: "session-a",
            updatedAt: "2026-02-21T10:10:00Z"
        )
        store.upsert(
            namespace: namespace,
            workerID: workerID,
            topic: "runtime.other_topic",
            watermark: 9,
            sessionID: "session-a",
            updatedAt: "2026-02-21T10:10:01Z"
        )

        store.resetTopic(
            namespace: namespace,
            workerID: workerID,
            topic: "runtime.codex_worker_events",
            updatedAt: "2026-02-21T10:10:02Z"
        )

        #expect(store.watermark(namespace: namespace, workerID: workerID, topic: "runtime.codex_worker_events") == 0)
        #expect(store.watermark(namespace: namespace, workerID: workerID, topic: "runtime.other_topic") == 9)
    }

    @Test("lifecycle resume state rejects stale foreground generations")
    func lifecycleResumeStateRejectsStaleGenerations() {
        var state = CodexLifecycleResumeState()

        let firstBackground = state.markBackground()
        #expect(state.shouldAccept(generation: firstBackground))

        let firstResume = state.beginForegroundResume()
        #expect(firstResume != nil)
        #expect(!state.shouldAccept(generation: firstBackground))

        let secondBackground = state.markBackground()
        #expect(secondBackground != firstBackground)

        let secondResume = state.beginForegroundResume()
        #expect(secondResume != nil)
        #expect(secondResume != firstResume)

        state.invalidate()
        if let secondResume {
            #expect(!state.shouldAccept(generation: secondResume))
        }
    }

    @Test("control receipt decoder parses worker.response and worker.error envelopes")
    func controlReceiptDecoderParsesTerminalEnvelopes() {
        let successPayload: JSONValue = .object([
            "eventType": .string("worker.response"),
            "payload": .object([
                "request_id": .string("iosreq-1"),
                "method": .string("turn/start"),
                "ok": .bool(true),
                "response": .object([
                    "turn": .object(["id": .string("turn-1")]),
                ]),
                "occurred_at": .string("2026-02-22T01:00:00Z"),
            ]),
        ])

        let errorPayload: JSONValue = .object([
            "eventType": .string("worker.error"),
            "payload": .object([
                "request_id": .string("iosreq-2"),
                "method": .string("turn/start"),
                "code": .string("conflict"),
                "message": .string("stale thread mapping"),
                "retryable": .bool(false),
                "occurred_at": .string("2026-02-22T01:00:01Z"),
            ]),
        ])

        let successReceipt = RuntimeCodexProto.decodeControlReceipt(from: successPayload)
        let errorReceipt = RuntimeCodexProto.decodeControlReceipt(from: errorPayload)

        #expect(successReceipt?.eventType == "worker.response")
        #expect(successReceipt?.receipt.requestID == "iosreq-1")
        #expect(successReceipt?.receipt.method == "turn/start")

        switch successReceipt?.receipt.outcome {
        case .success(let response):
            #expect(response?.objectValue?["turn"]?.objectValue?["id"]?.stringValue == "turn-1")
        default:
            #expect(Bool(false), "expected success receipt outcome")
        }

        #expect(errorReceipt?.eventType == "worker.error")
        #expect(errorReceipt?.receipt.requestID == "iosreq-2")

        switch errorReceipt?.receipt.outcome {
        case .error(let code, let message, let retryable, _):
            #expect(code == "conflict")
            #expect(message == "stale thread mapping")
            #expect(!retryable)
        default:
            #expect(Bool(false), "expected error receipt outcome")
        }
    }

    @Test("control coordinator transitions queued running success and ignores duplicate receipts")
    func controlCoordinatorTransitionsAndDedupesReceipts() {
        var coordinator = RuntimeCodexControlCoordinator()

        let request = RuntimeCodexWorkerActionRequest(
            requestID: "iosreq-control-1",
            method: .turnStart,
            params: [
                "thread_id": .string("thread-123"),
                "text": .string("continue"),
            ],
            sentAt: "2026-02-22T01:01:00Z"
        )

        let queued = coordinator.enqueue(
            workerID: "desktopw:shared",
            request: request,
            occurredAt: "2026-02-22T01:01:00Z"
        )
        #expect(queued.state == .queued)
        #expect(coordinator.queuedRequests.count == 1)

        let running = coordinator.markRunning(
            requestID: request.requestID,
            occurredAt: "2026-02-22T01:01:01Z"
        )
        #expect(running?.state == .running)
        #expect(coordinator.queuedRequests.isEmpty)

        let successReceipt = RuntimeCodexControlReceipt(
            requestID: request.requestID,
            method: "turn/start",
            occurredAt: "2026-02-22T01:01:02Z",
            outcome: .success(response: .object(["ok": .bool(true)]))
        )

        let reconciled = coordinator.reconcile(
            workerID: "desktopw:shared",
            receipt: successReceipt
        )
        #expect(reconciled?.state == .success)
        #expect(reconciled?.response?.objectValue?["ok"]?.boolValue == true)

        let duplicate = coordinator.reconcile(
            workerID: "desktopw:shared",
            receipt: successReceipt
        )
        #expect(duplicate == nil)
    }

    @Test("control coordinator keeps pending requests across disconnect and reconciles replay receipt")
    func controlCoordinatorReconcilesReplayAfterDisconnectBoundary() {
        var coordinator = RuntimeCodexControlCoordinator()
        let workerID = "desktopw:shared"
        let request = RuntimeCodexWorkerActionRequest(
            requestID: "iosreq-replay-1",
            method: .turnInterrupt,
            params: [
                "thread_id": .string("thread-123"),
                "turn_id": .string("turn-123"),
            ],
            sentAt: "2026-02-22T01:02:00Z"
        )

        _ = coordinator.enqueue(
            workerID: workerID,
            request: request,
            occurredAt: "2026-02-22T01:02:00Z"
        )
        _ = coordinator.markRunning(
            requestID: request.requestID,
            occurredAt: "2026-02-22T01:02:01Z"
        )

        // Simulate a temporary disconnect window where no receipt is observed live,
        // then the terminal receipt appears in replay.
        let replayReceipt = RuntimeCodexControlReceipt(
            requestID: request.requestID,
            method: "turn/interrupt",
            occurredAt: "2026-02-22T01:02:05Z",
            outcome: .error(
                code: "conflict",
                message: "turn already completed",
                retryable: false,
                details: nil
            )
        )

        let reconciled = coordinator.reconcile(workerID: workerID, receipt: replayReceipt)
        #expect(reconciled?.state == .error)
        #expect(reconciled?.errorCode == "conflict")
        #expect(reconciled?.errorMessage == "turn already completed")
    }

    @Test("runtime codex client request/stop APIs encode payloads and map error statuses")
    func runtimeCodexClientRequestStopApisEncodeAndMapErrors() async throws {
        let session = makeRuntimeCodexTestSession()
        defer {
            RuntimeCodexClientURLProtocol.setHandler(nil)
        }

        let client = RuntimeCodexClient(
            baseURL: URL(string: "https://openagents.com")!,
            authToken: "test-token",
            session: session
        )

        RuntimeCodexClientURLProtocol.setHandler { request in
            #expect(request.httpMethod == "POST")
            #expect(request.url?.path == "/api/runtime/codex/workers/desktopw%3Ashared/requests")

            let body = readRequestBodyData(request)
            let json = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
            let requestObject = json?["request"] as? [String: Any]

            #expect((requestObject?["request_id"] as? String) == "req-ios-1")
            #expect((requestObject?["method"] as? String) == "turn/start")
            #expect((requestObject?["request_version"] as? String) == "v1")
            #expect((requestObject?["source"] as? String) == "autopilot-ios")

            let params = requestObject?["params"] as? [String: Any]
            #expect((params?["thread_id"] as? String) == "thread-123")
            #expect((params?["text"] as? String) == "continue")

            let responseBody = """
            {"data":{"worker_id":"desktopw:shared","request_id":"req-ios-1","ok":true,"method":"turn/start","response":{"status":"accepted"}}}
            """
            let response = HTTPURLResponse(
                url: request.url ?? URL(string: "https://openagents.com/fallback")!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data(responseBody.utf8))
        }

        let request = RuntimeCodexWorkerActionRequest(
            requestID: "req-ios-1",
            method: .turnStart,
            params: [
                "thread_id": .string("thread-123"),
                "text": .string("continue"),
            ],
            sentAt: "2026-02-22T00:00:00Z"
        )

        let requestResult = try await client.requestWorkerAction(workerID: "desktopw:shared", request: request)
        #expect(requestResult.workerID == "desktopw:shared")
        #expect(requestResult.requestID == "req-ios-1")
        #expect(requestResult.ok == true)
        #expect(requestResult.method == "turn/start")

        RuntimeCodexClientURLProtocol.setHandler { request in
            #expect(request.httpMethod == "POST")
            #expect(request.url?.path == "/api/runtime/codex/workers/desktopw%3Ashared/stop")

            let body = readRequestBodyData(request)
            let json = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
            #expect((json?["reason"] as? String) == "user_requested")

            let responseBody = """
            {"data":{"worker_id":"desktopw:shared","status":"stopped","idempotent_replay":false}}
            """
            let response = HTTPURLResponse(
                url: request.url ?? URL(string: "https://openagents.com/fallback")!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data(responseBody.utf8))
        }

        let stopResult = try await client.stopWorker(workerID: "desktopw:shared", reason: "user_requested")
        #expect(stopResult.workerID == "desktopw:shared")
        #expect(stopResult.status == "stopped")
        #expect(stopResult.idempotentReplay == false)

        let cases: [(Int, RuntimeCodexApiErrorCode)] = [
            (401, .auth),
            (403, .forbidden),
            (409, .conflict),
            (422, .invalid),
        ]

        for (status, expectedCode) in cases {
            RuntimeCodexClientURLProtocol.setHandler { request in
                let response = HTTPURLResponse(
                    url: request.url ?? URL(string: "https://openagents.com/fallback")!,
                    statusCode: status,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                let body = """
                {"error":{"message":"status_\(status)"}}
                """
                return (response, Data(body.utf8))
            }

            do {
                _ = try await client.requestWorkerAction(workerID: "desktopw:shared", request: request)
                #expect(Bool(false), "expected RuntimeCodexApiError for status \(status)")
            } catch let error as RuntimeCodexApiError {
                #expect(error.code == expectedCode)
                #expect(error.status == status)
                #expect(error.message == "status_\(status)")
            }
        }
    }

    private func makeRuntimeCodexTestSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [RuntimeCodexClientURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private func readRequestBodyData(_ request: URLRequest) -> Data {
        if let body = request.httpBody {
            return body
        }

        guard let stream = request.httpBodyStream else {
            return Data()
        }

        stream.open()
        defer {
            stream.close()
        }

        var collected = Data()
        let bufferSize = 1024
        var buffer = [UInt8](repeating: 0, count: bufferSize)

        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: bufferSize)
            if read <= 0 {
                break
            }
            collected.append(buffer, count: read)
        }

        return collected
    }
}

private final class RuntimeCodexClientURLProtocol: URLProtocol {
    private static let lock = NSLock()
    private static var requestHandler: (@Sendable (URLRequest) throws -> (HTTPURLResponse, Data))?

    static func setHandler(_ handler: (@Sendable (URLRequest) throws -> (HTTPURLResponse, Data))?) {
        lock.lock()
        requestHandler = handler
        lock.unlock()
    }

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        RuntimeCodexClientURLProtocol.lock.lock()
        let handler = RuntimeCodexClientURLProtocol.requestHandler
        RuntimeCodexClientURLProtocol.lock.unlock()

        guard let handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }

        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
