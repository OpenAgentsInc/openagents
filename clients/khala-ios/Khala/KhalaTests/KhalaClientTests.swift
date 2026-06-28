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

    private func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MockURLProtocol.self]
        return URLSession(configuration: configuration)
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
