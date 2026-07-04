import Foundation
@testable import Khala
import XCTest

final class KhalaClientTests: XCTestCase {
    override func tearDown() {
        MockURLProtocol.requestHandler = nil
        super.tearDown()
    }

    func testCompletePostsDeterministicHandshakeAndReturnsAssistantContent() async throws {
        let session = makeSession()
        let prompt = "Explain the latest public issue status."
        let apiKey = "oa_agent_test_key"

        MockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/v1/chat/completions")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer \(apiKey)")

            let body = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["model"] as? String, "openagents/khala")

            let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
            XCTAssertEqual(messages.count, 1)
            XCTAssertEqual(messages.first?["role"] as? String, "user")
            XCTAssertEqual(messages.first?["content"] as? String, prompt)

            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data("""
                {
                  "choices": [
                    { "message": { "content": "offline assistant response" } }
                  ]
                }
                """.utf8)
            )
        }

        let content = try await KhalaClient.complete(prompt: prompt, apiKey: apiKey, session: session)

        XCTAssertEqual(content, "offline assistant response")
    }

    func testCompleteMapsHTTP402ToQuotaExceeded() async throws {
        let session = makeSession()

        MockURLProtocol.requestHandler = { request in
            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 402,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{ "error": "quota exceeded" }"#.utf8)
            )
        }

        do {
            _ = try await KhalaClient.complete(
                prompt: "hello",
                apiKey: "oa_agent_test_key",
                session: session
            )
            XCTFail("Expected quotaExceeded")
        } catch let error as KhalaClient.KhalaError {
            guard case .quotaExceeded = error else {
                return XCTFail("Expected quotaExceeded, got \(error)")
            }
            XCTAssertEqual(error.recoveryTitle, "Free quota reached")
            XCTAssertFalse(error.isRetryable)
        } catch {
            XCTFail("Expected quotaExceeded, got \(error)")
        }
    }

    func testCompleteMapsHTTP401ToUnauthorized() async throws {
        let session = makeSession()

        MockURLProtocol.requestHandler = { request in
            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 401,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{ "error": "invalid key" }"#.utf8)
            )
        }

        do {
            _ = try await KhalaClient.complete(
                prompt: "hello",
                apiKey: "oa_agent_bad_key",
                session: session
            )
            XCTFail("Expected unauthorized")
        } catch let error as KhalaClient.KhalaError {
            guard case .unauthorized = error else {
                return XCTFail("Expected unauthorized, got \(error)")
            }
            XCTAssertEqual(error.recoveryTitle, "Key rejected")
            XCTAssertFalse(error.isRetryable)
            XCTAssertTrue(error.requiresKeyAttention)
        }
    }

    func testCompleteRejectsMissingKeyBeforeNetwork() async throws {
        MockURLProtocol.requestHandler = { _ in
            XCTFail("Missing keys should fail before creating a request")
            throw URLError(.badURL)
        }

        do {
            _ = try await KhalaClient.complete(prompt: "hello", apiKey: "   ", session: makeSession())
            XCTFail("Expected missingKey")
        } catch let error as KhalaClient.KhalaError {
            guard case .missingKey = error else { return XCTFail("Got \(error)") }
            XCTAssertEqual(error.recoveryTitle, "Missing API key")
            XCTAssertTrue(error.requiresKeyAttention)
        }
    }

    func testCompleteMapsHTTP403ToUnauthorized() async throws {
        let session = makeSession()
        MockURLProtocol.requestHandler = { request in
            (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 403, httpVersion: nil, headerFields: nil
                )!,
                Data()
            )
        }
        do {
            _ = try await KhalaClient.complete(prompt: "hi", apiKey: "k", session: session)
            XCTFail("Expected unauthorized")
        } catch let error as KhalaClient.KhalaError {
            guard case .unauthorized = error else { return XCTFail("Got \(error)") }
        }
    }

    func testCompleteMapsHTTP500ToRetryableServerError() async throws {
        let session = makeSession()

        MockURLProtocol.requestHandler = { request in
            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 503,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{ "error": "unavailable" }"#.utf8)
            )
        }

        do {
            _ = try await KhalaClient.complete(
                prompt: "hello",
                apiKey: "oa_agent_test_key",
                session: session
            )
            XCTFail("Expected http 503")
        } catch let error as KhalaClient.KhalaError {
            guard case .http(let code, _) = error else {
                return XCTFail("Expected http 503, got \(error)")
            }
            XCTAssertEqual(code, 503)
            XCTAssertEqual(error.recoveryTitle, "Temporary Khala error")
            XCTAssertTrue(error.isRetryable)
        } catch {
            XCTFail("Expected http 503, got \(error)")
        }
    }

    func testCompleteMapsTransportFailureToRetryableError() async throws {
        let session = makeSession()

        MockURLProtocol.requestHandler = { _ in
            throw URLError(.notConnectedToInternet)
        }

        do {
            _ = try await KhalaClient.complete(
                prompt: "hello",
                apiKey: "oa_agent_test_key",
                session: session
            )
            XCTFail("Expected transport error")
        } catch let error as KhalaClient.KhalaError {
            guard case .transport = error else {
                return XCTFail("Expected transport error, got \(error)")
            }
            XCTAssertEqual(error.recoveryTitle, "Connection interrupted")
            XCTAssertTrue(error.isRetryable)
        } catch {
            XCTFail("Expected transport error, got \(error)")
        }
    }

    func testMintFreeKeyPostsToFreeKeyEndpointAndDecodesCredentialToken() async throws {
        let session = makeSession()

        MockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/keys/free")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")

            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data("""
                {
                  "credential": {
                    "token": "oa_agent_free_contract_token"
                  },
                  "dataSharing": {
                    "scope": "free-tier"
                  }
                }
                """.utf8)
            )
        }

        let token = try await KhalaClient.mintFreeKey(session: session)

        XCTAssertEqual(token, "oa_agent_free_contract_token")
    }

    func testRequestCodexTaskPostsTypedDelegationAndParsesDurableHeaders() async throws {
        let session = makeSession()
        let prompt = "Implement public issue #6849 and run xcodebuild test."
        let pylonRef = "pylon.a1469b9cdf6965a57530"
        let apiKey = "oa_agent_codex_delegate"

        MockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/v1/chat/completions")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer \(apiKey)")

            let body = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["model"] as? String, "openagents/khala")
            XCTAssertEqual(json["workflowClass"] as? String, "codex_agent_task")
            XCTAssertEqual(json["targetPylonRef"] as? String, pylonRef)
            XCTAssertEqual(json["stream"] as? Bool, true)

            let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
            XCTAssertEqual(messages.first?["role"] as? String, "user")
            XCTAssertEqual(messages.first?["content"] as? String, prompt)

            let openagents = try XCTUnwrap(json["openagents"] as? [String: Any])
            XCTAssertEqual(openagents["workflowClass"] as? String, "codex_agent_task")
            let coding = try XCTUnwrap(openagents["coding"] as? [String: Any])
            XCTAssertEqual(coding["targetPylonRef"] as? String, pylonRef)

            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: [
                        "openagents-coding-assignment-ref": "assign_6849",
                        "openagents-durable-stream-url": "/v1/chat/completions/durable/request_6849",
                        "stream-next-offset": "7",
                        "stream-closed": "true",
                        "stream-up-to-date": "true",
                    ]
                )!,
                Data("""
                data: {"choices":[{"delta":{"content":"delegated "}}]}

                data: {"choices":[{"message":{"content":"accepted"}}]}

                data: [DONE]

                """.utf8)
            )
        }

        let result = try await KhalaClient.requestCodexTask(
            prompt: "  \(prompt)  ",
            pylonRef: "  \(pylonRef)  ",
            apiKey: apiKey,
            session: session
        )

        XCTAssertEqual(result.assignmentRef, "assign_6849")
        XCTAssertEqual(result.durableRequestId, "request_6849")
        XCTAssertEqual(result.durableStreamURL, "/v1/chat/completions/durable/request_6849")
        XCTAssertEqual(result.nextOffset, "7")
        XCTAssertTrue(result.streamClosed)
        XCTAssertTrue(result.streamUpToDate)
        XCTAssertEqual(result.text, "delegated accepted")
        XCTAssertTrue(result.displayText.contains("Assignment: assign_6849"))
    }

    func testRequestCodexTaskRejectsUnsafePromptBeforeNetwork() async throws {
        MockURLProtocol.requestHandler = { _ in
            XCTFail("Unsafe prompts should fail before creating a request")
            throw URLError(.badURL)
        }

        do {
            _ = try await KhalaClient.requestCodexTask(
                prompt: "Use this bearer token in the prompt.",
                pylonRef: "pylon.good-ref",
                apiKey: "oa_agent_codex_delegate",
                session: makeSession()
            )
            XCTFail("Expected invalidCodingRequest")
        } catch let error as KhalaClient.KhalaError {
            guard case .invalidCodingRequest(let reason) = error else {
                return XCTFail("Got \(error)")
            }
            XCTAssertTrue(reason.contains("public-safe"))
            XCTAssertFalse(error.isRetryable)
        }
    }

    func testRequestCodexTaskRejectsInvalidPylonRefBeforeNetwork() async throws {
        MockURLProtocol.requestHandler = { _ in
            XCTFail("Invalid refs should fail before creating a request")
            throw URLError(.badURL)
        }

        do {
            _ = try await KhalaClient.requestCodexTask(
                prompt: "Implement public issue #6849.",
                pylonRef: "../private",
                apiKey: "oa_agent_codex_delegate",
                session: makeSession()
            )
            XCTFail("Expected invalidCodingRequest")
        } catch let error as KhalaClient.KhalaError {
            guard case .invalidCodingRequest(let reason) = error else {
                return XCTFail("Got \(error)")
            }
            XCTAssertTrue(reason.contains("Pylon ref"))
        }
    }

    func testRequestCodexTaskMapsHTTP402ToQuotaExceeded() async throws {
        let session = makeSession()
        MockURLProtocol.requestHandler = { request in
            (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 402,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{ "error": "quota exceeded" }"#.utf8)
            )
        }

        do {
            _ = try await KhalaClient.requestCodexTask(
                prompt: "Implement public issue #6849.",
                pylonRef: "pylon.good-ref",
                apiKey: "oa_agent_codex_delegate",
                session: session
            )
            XCTFail("Expected quotaExceeded")
        } catch let error as KhalaClient.KhalaError {
            guard case .quotaExceeded = error else { return XCTFail("Got \(error)") }
        }
    }

    func testPushChatSyncTurnPostsCreateAndAppendMutations() async throws {
        let session = makeSession()
        let store = makeChatSyncStateStore()
        let apiKey = "oa_agent_chat_sync"
        let threadId = "ios.thread.abc"
        let messageId = "ios.message.def"

        MockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/sync/push")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer \(apiKey)")

            let data = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            XCTAssertEqual(json["protocolVersion"] as? Int, 1)
            XCTAssertEqual(json["schemaVersion"] as? Int, 1)
            XCTAssertTrue((json["clientGroupId"] as? String)?.hasPrefix("khala_ios_cg_") == true)
            XCTAssertTrue((json["clientId"] as? String)?.hasPrefix("khala_ios_client_") == true)

            let mutations = try XCTUnwrap(json["mutations"] as? [[String: Any]])
            XCTAssertEqual(mutations.count, 2)
            XCTAssertEqual(mutations[0]["mutationId"] as? Int, 1)
            XCTAssertEqual(mutations[0]["name"] as? String, "chat.createThread")
            XCTAssertEqual(mutations[1]["mutationId"] as? Int, 2)
            XCTAssertEqual(mutations[1]["name"] as? String, "chat.appendMessage")

            let createArgsData = try XCTUnwrap((mutations[0]["argsJson"] as? String)?.data(using: .utf8))
            let createArgs = try XCTUnwrap(JSONSerialization.jsonObject(with: createArgsData) as? [String: Any])
            XCTAssertEqual(createArgs["threadId"] as? String, threadId)
            XCTAssertEqual(createArgs["title"] as? String, "Owner dogfood")

            let appendArgsData = try XCTUnwrap((mutations[1]["argsJson"] as? String)?.data(using: .utf8))
            let appendArgs = try XCTUnwrap(JSONSerialization.jsonObject(with: appendArgsData) as? [String: Any])
            XCTAssertEqual(appendArgs["threadId"] as? String, threadId)
            XCTAssertEqual(appendArgs["messageId"] as? String, messageId)
            XCTAssertEqual(appendArgs["body"] as? String, "hello from phone")

            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data("""
                {
                  "protocolVersion": 1,
                  "results": [
                    { "mutationId": 1, "status": "applied" },
                    { "mutationId": 2, "status": "applied" }
                  ],
                  "lastMutationId": 2
                }
                """.utf8)
            )
        }

        let result = try await KhalaClient.pushChatSyncTurn(
            threadId: threadId,
            title: "  Owner dogfood  ",
            messageId: messageId,
            body: "hello from phone",
            apiKey: apiKey,
            stateStore: store,
            session: session
        )

        XCTAssertEqual(result.routeRef, "route.khala_sync.push.v0_1")
        XCTAssertEqual(result.mutationIds, [1, 2])
        XCTAssertEqual(result.lastMutationId, 2)
        XCTAssertEqual(store.load().lastMutationId, 2)
        XCTAssertTrue(store.load().syncedThreadIds.contains(threadId))
    }

    func testPushChatSyncTurnSkipsCreateForKnownSyncedThread() async throws {
        let session = makeSession()
        let store = makeChatSyncStateStore()
        var state = store.load()
        state.lastMutationId = 2
        state.syncedThreadIds.insert("ios.thread.known")
        store.save(state)

        MockURLProtocol.requestHandler = { request in
            let data = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let mutations = try XCTUnwrap(json["mutations"] as? [[String: Any]])
            XCTAssertEqual(mutations.count, 1)
            XCTAssertEqual(mutations[0]["mutationId"] as? Int, 3)
            XCTAssertEqual(mutations[0]["name"] as? String, "chat.appendMessage")

            return (
                HTTPURLResponse(url: try XCTUnwrap(request.url), statusCode: 200, httpVersion: nil, headerFields: nil)!,
                Data("""
                {
                  "protocolVersion": 1,
                  "results": [{ "mutationId": 3, "status": "applied" }],
                  "lastMutationId": 3
                }
                """.utf8)
            )
        }

        let result = try await KhalaClient.pushChatSyncTurn(
            threadId: "ios.thread.known",
            title: "Known",
            messageId: "ios.message.next",
            body: "phone follow-up",
            apiKey: "oa_agent_chat_sync",
            stateStore: store,
            session: session
        )

        XCTAssertEqual(result.mutationIds, [3])
        XCTAssertEqual(store.load().lastMutationId, 3)
    }

    func testFetchChatSyncThreadsBootstrapsOwnerScopeAndDecodesThreadRows() async throws {
        let session = makeSession()
        let store = makeChatSyncStateStore()
        let apiKey = "oa_agent_chat_sync"

        MockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/sync/bootstrap")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer \(apiKey)")

            let data = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            XCTAssertEqual(json["protocolVersion"] as? Int, 1)
            XCTAssertEqual(json["schemaVersion"] as? Int, 1)
            XCTAssertEqual(json["scope"] as? String, "scope.user.user.public.owner")
            XCTAssertEqual(json["pageSize"] as? Int, 200)

            let postImageJson = """
            {
              "threadId": "thread.remote.kh-202",
              "ownerUserId": "user.public.owner",
              "title": "Remote device thread",
              "status": "active",
              "messageCount": 2,
              "lastMessageAt": "2026-07-04T17:03:00.000Z",
              "createdAt": "2026-07-04T17:00:00.000Z",
              "updatedAt": "2026-07-04T17:03:00.000Z"
            }
            """
            let body: [String: Any] = [
                "protocolVersion": 1,
                "scope": "scope.user.user.public.owner",
                "cursor": 7,
                "entities": [
                    [
                        "entityType": "chat_thread",
                        "entityId": "thread.remote.kh-202",
                        "postImageJson": postImageJson,
                    ],
                ],
            ]
            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                try JSONSerialization.data(withJSONObject: body)
            )
        }

        let result = try await KhalaClient.fetchChatSyncThreads(
            ownerUserId: "user.public.owner",
            apiKey: apiKey,
            stateStore: store,
            session: session
        )

        XCTAssertEqual(result.routeRef, "route.khala_sync.bootstrap.v0_1")
        XCTAssertEqual(result.scope, "scope.user.user.public.owner")
        XCTAssertEqual(result.cursor, 7)
        XCTAssertEqual(result.threads.map(\.threadId), ["thread.remote.kh-202"])
        XCTAssertEqual(result.threads.first?.messageCount, 2)
        XCTAssertTrue(store.load().syncedThreadIds.contains("thread.remote.kh-202"))
    }

    func testFetchFleetInspectorStatusUsesBearerAuthAndDecodesPublicRefs() async throws {
        let session = makeSession()
        let apiKey = "oa_agent_fleet_status_test"

        MockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/operator/fleet/status")
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer \(apiKey)")

            return (
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data("""
                {
                  "identity": {
                    "userRef": "user.public.owner",
                    "email": "owner@example.com"
                  },
                  "pylon": {
                    "pylonRef": "pylon.owner.codex",
                    "status": "ready",
                    "lastHeartbeatAt": "2026-06-28T12:00:00Z",
                    "heartbeatFresh": true,
                    "capacityRefs": [
                      "capacity.coding.codex.ready=2",
                      "capacity.coding.codex.available=1"
                    ],
                    "loadRefs": [
                      "load.coding.codex.busy=1",
                      "load.coding.codex.queued=0"
                    ]
                  },
                  "providerAccounts": [
                    { "provider": "codex", "accountRef": "codex", "readiness": "ready" }
                  ],
                  "appleFM": {
                    "status": "blocked",
                    "blockerRefs": ["blocker.apple_fm.backend_missing"]
                  },
                  "recentCloseoutRefs": [
                    "assignment.public.khala_coding.fixture",
                    "closeout.public.khala_coding.fixture"
                  ],
                  "proofRefs": ["proof.public.khala_coding.fixture"],
                  "rawPrompt": "do not display me",
                  "localPath": "/Users/example/.codex/auth.json",
                  "token": "oa_agent_should_not_render"
                }
                """.utf8)
            )
        }

        let status = try await KhalaClient.fetchFleetInspectorStatus(apiKey: apiKey, session: session)

        XCTAssertEqual(status.connectedIdentity, "user.public.owner")
        XCTAssertEqual(status.localAgentIdentity, "pylon.owner.codex")
        XCTAssertEqual(status.pylonRef, "pylon.owner.codex")
        XCTAssertEqual(status.pylonReadiness, .available)
        XCTAssertEqual(status.heartbeatObservedAt, "2026-06-28T12:00:00Z")
        XCTAssertEqual(status.heartbeatFresh, true)
        XCTAssertEqual(status.providerAccounts, [
            FleetInspectorStatus.ProviderAccount(provider: "codex", ref: "codex", readiness: .available, detail: "ready"),
        ])
        XCTAssertEqual(status.appleFM.readiness, .blocked)
        XCTAssertEqual(status.capacityRefs, [
            "capacity.coding.codex.ready=2",
            "capacity.coding.codex.available=1",
        ])
        XCTAssertEqual(status.loadRefs, [
            "load.coding.codex.busy=1",
            "load.coding.codex.queued=0",
        ])
        XCTAssertEqual(status.recentRefs.map(\.value), [
            "assignment.public.khala_coding.fixture",
            "closeout.public.khala_coding.fixture",
        ])
        XCTAssertEqual(status.proofRefs, ["proof.public.khala_coding.fixture"])
        XCTAssertFalse(status.capacityRefs.contains { $0.contains("oa_agent_should_not_render") })
        XCTAssertFalse(status.proofRefs.contains { $0.contains("/Users/") })
    }

    func testFleetInspectorRedactsSensitiveDisplayStrings() {
        XCTAssertEqual(FleetInspectorStatus.redactedForDisplay("oa_agent_secret"), "[redacted]")
        XCTAssertEqual(FleetInspectorStatus.redactedForDisplay("/Users/me/.codex/auth.json"), "[redacted]")
        XCTAssertEqual(FleetInspectorStatus.redactedForDisplay("person@example.com", key: "email"), "[redacted-email]")
        XCTAssertEqual(FleetInspectorStatus.redactedForDisplay("capacity.coding.codex.available=2"), "capacity.coding.codex.available=2")
    }

    private func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MockURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private func makeChatSyncStateStore() -> KhalaChatSyncStateStore {
        let suite = "KhalaChatSyncTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defaults.removePersistentDomain(forName: suite)
        return KhalaChatSyncStateStore(defaults: defaults, key: "state")
    }

    private static func data(from stream: InputStream) -> Data {
        stream.open()
        defer { stream.close() }

        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 1_024)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count > 0 {
                data.append(buffer, count: count)
            } else {
                break
            }
        }
        return data
    }
}

private final class MockURLProtocol: URLProtocol {
    static var requestHandler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let requestHandler = Self.requestHandler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }

        do {
            let (response, data) = try requestHandler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
