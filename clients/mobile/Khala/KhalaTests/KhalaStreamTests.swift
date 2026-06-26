import Foundation
@testable import Khala
import XCTest

final class KhalaStreamTests: XCTestCase {
    override func tearDown() {
        StreamMockURLProtocol.requestHandler = nil
        super.tearDown()
    }

    // MARK: - SSE line parsing (pure)

    func testDeltaParsesContentFromStreamingChoice() {
        let line = #"data: {"choices":[{"delta":{"content":"Hel"}}]}"#
        let delta = KhalaClient.delta(fromSSELine: line)
        XCTAssertEqual(delta?.content, "Hel")
        XCTAssertEqual(delta?.isDone, false)
    }

    func testDeltaRecognizesDoneTerminator() {
        XCTAssertEqual(KhalaClient.delta(fromSSELine: "data: [DONE]")?.isDone, true)
    }

    func testDeltaIgnoresBlankAndCommentLines() {
        XCTAssertNil(KhalaClient.delta(fromSSELine: ""))
        XCTAssertNil(KhalaClient.delta(fromSSELine: ": keep-alive"))
        XCTAssertNil(KhalaClient.delta(fromSSELine: "event: message"))
    }

    func testDeltaSkipsRoleOnlyFrame() {
        // First OpenAI chunk often carries role but empty/no content.
        let line = #"data: {"choices":[{"delta":{"role":"assistant"}}]}"#
        XCTAssertNil(KhalaClient.delta(fromSSELine: line)?.content)
    }

    // MARK: - Multi-turn request shape

    func testStreamSendsFullHistoryAndStreamFlag() async throws {
        let session = makeSession()
        let apiKey = "oa_agent_test_key"

        StreamMockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.absoluteString, "https://openagents.com/api/v1/chat/completions")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer \(apiKey)")

            let body = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["model"] as? String, "openagents/khala")
            XCTAssertEqual(json["stream"] as? Bool, true)

            let messages = try XCTUnwrap(json["messages"] as? [[String: String]])
            XCTAssertEqual(messages.count, 4)
            XCTAssertEqual(messages[0]["role"], "system")
            XCTAssertEqual(messages[1]["role"], "user")
            XCTAssertEqual(messages[1]["content"], "first")
            XCTAssertEqual(messages[2]["role"], "assistant")
            XCTAssertEqual(messages[3]["content"], "second")

            return (Self.ok(request), Self.sse(["data: [DONE]"]))
        }

        let stream = KhalaClient.streamCompletion(
            messages: [
                .init(role: "user", content: "first"),
                .init(role: "assistant", content: "reply"),
                .init(role: "user", content: "second"),
            ],
            apiKey: apiKey,
            system: "You are Khala.",
            session: session
        )
        for try await _ in stream {}
    }

    func testStreamYieldsDeltasAndAssemblesFinalMessage() async throws {
        let session = makeSession()

        StreamMockURLProtocol.requestHandler = { request in
            (
                Self.ok(request),
                Self.sse([
                    #"data: {"choices":[{"delta":{"role":"assistant"}}]}"#,
                    #"data: {"choices":[{"delta":{"content":"Hello"}}]}"#,
                    #"data: {"choices":[{"delta":{"content":", "}}]}"#,
                    #"data: {"choices":[{"delta":{"content":"world"}}]}"#,
                    "data: [DONE]",
                ])
            )
        }

        var deltas: [String] = []
        let result = try await KhalaClient.streamAssembled(
            messages: [.init(role: "user", content: "hi")],
            apiKey: "oa_agent_test_key",
            session: session,
            onDelta: { deltas.append($0) }
        )

        XCTAssertEqual(deltas, ["Hello", ", ", "world"])
        XCTAssertEqual(result.content, "Hello, world")
        XCTAssertEqual(result.role, "assistant")
    }

    func testStreamMapsHTTP402ToQuotaExceeded() async throws {
        let session = makeSession()
        StreamMockURLProtocol.requestHandler = { request in
            (Self.status(request, 402), Data(#"{"error":"quota"}"#.utf8))
        }

        do {
            for try await _ in KhalaClient.streamCompletion(
                messages: [.init(role: "user", content: "hi")],
                apiKey: "oa_agent_test_key",
                session: session
            ) {}
            XCTFail("Expected quotaExceeded")
        } catch let error as KhalaClient.KhalaError {
            guard case .quotaExceeded = error else { return XCTFail("Got \(error)") }
        }
    }

    func testStreamMapsMissingKey() async throws {
        do {
            for try await _ in KhalaClient.streamCompletion(
                messages: [.init(role: "user", content: "hi")],
                apiKey: "   "
            ) {}
            XCTFail("Expected missingKey")
        } catch let error as KhalaClient.KhalaError {
            guard case .missingKey = error else { return XCTFail("Got \(error)") }
        }
    }

    func testCancellationStopsStreamCleanly() async throws {
        let session = makeSession()
        StreamMockURLProtocol.requestHandler = { request in
            (
                Self.ok(request),
                Self.sse((0..<200).map { #"data: {"choices":[{"delta":{"content":"\#($0) "}}]}"# })
            )
        }

        let task = Task<Int, Error> {
            var count = 0
            for try await _ in KhalaClient.streamCompletion(
                messages: [.init(role: "user", content: "hi")],
                apiKey: "oa_agent_test_key",
                session: session
            ) {
                count += 1
                if count == 3 { break } // consumer stops -> stream terminates
            }
            return count
        }

        let count = try await task.value
        XCTAssertLessThanOrEqual(count, 3)
    }

    // MARK: - Helpers

    private func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StreamMockURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private static func ok(_ request: URLRequest) -> HTTPURLResponse {
        status(request, 200)
    }

    private static func status(_ request: URLRequest, _ code: Int) -> HTTPURLResponse {
        HTTPURLResponse(
            url: request.url!,
            statusCode: code,
            httpVersion: nil,
            headerFields: ["Content-Type": "text/event-stream"]
        )!
    }

    private static func sse(_ lines: [String]) -> Data {
        Data((lines.joined(separator: "\n") + "\n").utf8)
    }

    private static func data(from stream: InputStream) -> Data {
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 1_024)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count > 0 { data.append(buffer, count: count) } else { break }
        }
        return data
    }
}

private final class StreamMockURLProtocol: URLProtocol {
    static var requestHandler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

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
