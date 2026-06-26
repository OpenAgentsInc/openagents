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
        } catch KhalaClient.KhalaError.quotaExceeded {
            // Expected.
        } catch {
            XCTFail("Expected quotaExceeded, got \(error)")
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
