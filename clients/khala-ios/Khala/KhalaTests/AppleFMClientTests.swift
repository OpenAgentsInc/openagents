import Foundation
@testable import Khala
import XCTest

final class AppleFMClientTests: XCTestCase {
    override func tearDown() {
        AppleFMMockURLProtocol.requestHandler = nil
        super.tearDown()
    }

    func testBaseURLResolutionUsesProbeThenOpenAgentsThenDefault() {
        XCTAssertEqual(
            AppleFMClient.resolvedBaseURL(environment: [
                "PROBE_APPLE_FM_BASE_URL": "http://127.0.0.1:19001",
                "OPENAGENTS_APPLE_FM_BASE_URL": "http://127.0.0.1:19002",
            ]).absoluteString,
            "http://127.0.0.1:19001"
        )
        XCTAssertEqual(
            AppleFMClient.resolvedBaseURL(environment: [
                "OPENAGENTS_APPLE_FM_BASE_URL": "http://127.0.0.1:19002",
            ]).absoluteString,
            "http://127.0.0.1:19002"
        )
        XCTAssertEqual(AppleFMClient.resolvedBaseURL(environment: [:]), AppleFMClient.defaultBaseURL)
    }

    func testHealthMapsReadyBridge() async throws {
        let session = makeSession()
        AppleFMMockURLProtocol.requestHandler = { request in
            XCTAssertEqual(request.url?.path, "/health")
            return (
                Self.status(request, 200),
                Data("""
                {
                  "ready": true,
                  "model": "apple-foundation-model",
                  "message": "ready",
                  "platform": "macOS"
                }
                """.utf8)
            )
        }

        let status = await AppleFMClient.health(baseURL: Self.baseURL, session: session)

        XCTAssertEqual(status.availability, .ready)
        XCTAssertTrue(status.isReady)
        XCTAssertEqual(status.platform, "macOS")
    }

    func testHealthMapsUnsupportedAsNonFailure() async throws {
        let session = makeSession()
        AppleFMMockURLProtocol.requestHandler = { request in
            (
                Self.status(request, 200),
                Data("""
                {
                  "state": "unsupported",
                  "message": "Apple Intelligence is disabled.",
                  "blockerRefs": ["blocker.apple_fm.apple_intelligence_disabled"]
                }
                """.utf8)
            )
        }

        let status = await AppleFMClient.health(baseURL: Self.baseURL, session: session)

        XCTAssertEqual(status.availability, .unsupported)
        XCTAssertFalse(status.isReady)
        XCTAssertEqual(status.blockerRefs, ["blocker.apple_fm.apple_intelligence_disabled"])
    }

    func testSnapshotCompletionChecksHealthBeforeInferenceAndCarriesUsageTruth() async throws {
        let session = makeSession()
        var paths: [String] = []
        AppleFMMockURLProtocol.requestHandler = { request in
            paths.append(request.url?.path ?? "")
            if request.url?.path == "/health" {
                return (Self.status(request, 200), Data(#"{ "ready": true }"#.utf8))
            }
            XCTAssertEqual(request.url?.path, "/v1/chat/completions")
            let body = try XCTUnwrap(request.httpBodyStream.flatMap(Self.data(from:)))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["model"] as? String, AppleFMClient.model)
            XCTAssertEqual(json["stream"] as? Bool, false)

            return (
                Self.status(request, 200),
                Data("""
                {
                  "choices": [
                    { "message": { "content": "local snapshot" } }
                  ],
                  "usage": { "truth": "estimated" }
                }
                """.utf8)
            )
        }

        var snapshots: [AppleFMClient.Snapshot] = []
        for try await snapshot in AppleFMClient.streamSnapshotCompletion(
            messages: [.init(role: "user", content: "hello")],
            baseURL: Self.baseURL,
            session: session
        ) {
            snapshots.append(snapshot)
        }

        XCTAssertEqual(paths, ["/health", "/v1/chat/completions"])
        XCTAssertEqual(snapshots, [.init(content: "local snapshot", usageTruth: .estimated)])
    }

    func testSnapshotCompletionRefusesBeforeInferenceWhenHealthIsNotReady() async throws {
        let session = makeSession()
        var paths: [String] = []
        AppleFMMockURLProtocol.requestHandler = { request in
            paths.append(request.url?.path ?? "")
            return (
                Self.status(request, 200),
                Data("""
                {
                  "ready": false,
                  "message": "helper missing",
                  "blockers": ["blocker.apple_fm.helper_missing"]
                }
                """.utf8)
            )
        }

        do {
            for try await _ in AppleFMClient.streamSnapshotCompletion(
                messages: [.init(role: "user", content: "hello")],
                baseURL: Self.baseURL,
                session: session
            ) {}
            XCTFail("Expected notReady")
        } catch let error as AppleFMClient.AppleFMError {
            guard case .notReady(let status) = error else {
                return XCTFail("Expected notReady, got \(error)")
            }
            XCTAssertEqual(status.availability, .unavailable)
            XCTAssertEqual(status.blockerRefs, ["blocker.apple_fm.helper_missing"])
        }

        XCTAssertEqual(paths, ["/health"])
    }

    private static let baseURL = URL(string: "http://apple-fm.test")!

    private func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AppleFMMockURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private static func status(_ request: URLRequest, _ code: Int) -> HTTPURLResponse {
        HTTPURLResponse(
            url: request.url!,
            statusCode: code,
            httpVersion: nil,
            headerFields: ["Content-Type": "application/json"]
        )!
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

private final class AppleFMMockURLProtocol: URLProtocol {
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
