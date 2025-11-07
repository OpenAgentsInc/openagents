#if os(macOS)
import XCTest
@testable import OpenAgentsCore

final class JsonRpcRouterTests: XCTestCase {
    var sut: JsonRpcRouter!
    var receivedMessages: [String] = []

    override func setUp() async throws {
        try await super.setUp()
        sut = JsonRpcRouter()
        receivedMessages = []
    }

    override func tearDown() async throws {
        sut = nil
        receivedMessages = []
        try await super.tearDown()
    }

    // MARK: - Parsing Tests

    func testParse_ValidRequest() {
        // Given
        let json = """
        {"jsonrpc":"2.0","method":"test/method","id":"123","params":{"key":"value"}}
        """

        // When
        let result = sut.parse(text: json)

        // Then
        if case .request(let method, let id, let params, _) = result {
            XCTAssertEqual(method, "test/method")
            XCTAssertEqual(id.value, "123")
            XCTAssertEqual(params?["key"] as? String, "value")
        } else {
            XCTFail("Expected request, got \(result)")
        }
    }

    func testParse_RequestWithIntId() {
        // Given
        let json = """
        {"jsonrpc":"2.0","method":"test/method","id":42}
        """

        // When
        let result = sut.parse(text: json)

        // Then
        if case .request(let method, let id, _, _) = result {
            XCTAssertEqual(method, "test/method")
            XCTAssertEqual(id.value, "42")
        } else {
            XCTFail("Expected request")
        }
    }

    func testParse_RequestWithoutParams() {
        // Given
        let json = """
        {"jsonrpc":"2.0","method":"test/method","id":"123"}
        """

        // When
        let result = sut.parse(text: json)

        // Then
        if case .request(let method, let id, let params, _) = result {
            XCTAssertEqual(method, "test/method")
            XCTAssertEqual(id.value, "123")
            XCTAssertNil(params)
        } else {
            XCTFail("Expected request")
        }
    }

    func testParse_Notification() {
        // Given
        let json = """
        {"jsonrpc":"2.0","method":"test/notification","params":{"key":"value"}}
        """

        // When
        let result = sut.parse(text: json)

        // Then
        if case .notification(let method, let params) = result {
            XCTAssertEqual(method, "test/notification")
            XCTAssertEqual(params?["key"] as? String, "value")
        } else {
            XCTFail("Expected notification")
        }
    }

    func testParse_InvalidJson() {
        // Given
        let json = "not valid json"

        // When
        let result = sut.parse(text: json)

        // Then
        if case .invalidJson = result {
            // Success
        } else {
            XCTFail("Expected invalidJson")
        }
    }

    func testParse_NotJsonRpc() {
        // Given
        let json = """
        {"method":"test","id":"123"}
        """

        // When
        let result = sut.parse(text: json)

        // Then
        if case .notJsonRpc = result {
            // Success
        } else {
            XCTFail("Expected notJsonRpc")
        }
    }

    func testParse_MissingMethod() {
        // Given
        let json = """
        {"jsonrpc":"2.0","id":"123"}
        """

        // When
        let result = sut.parse(text: json)

        // Then
        if case .invalidJson = result {
            // Success
        } else {
            XCTFail("Expected invalidJson")
        }
    }

    // MARK: - Handler Registration Tests

    func testRegister_HandlerRegistered() {
        // Given/When
        var handlerCalled = false
        sut.register(method: "test/method") { _, _, _ in
            handlerCalled = true
        }

        // Then
        XCTAssertTrue(sut.hasHandler(for: "test/method"))
    }

    func testUnregister_HandlerRemoved() {
        // Given
        sut.register(method: "test/method") { _, _, _ in }
        XCTAssertTrue(sut.hasHandler(for: "test/method"))

        // When
        sut.unregister(method: "test/method")

        // Then
        XCTAssertFalse(sut.hasHandler(for: "test/method"))
    }

    func testHasHandler_NonExistentMethod() {
        // When/Then
        XCTAssertFalse(sut.hasHandler(for: "nonexistent/method"))
    }

    // MARK: - Routing Tests

    func testRoute_CallsRegisteredHandler() async {
        // Given
        var capturedId: JSONRPC.ID?
        var capturedParams: [String: Any]?

        sut.register(method: "test/method") { id, params, _ in
            capturedId = id
            capturedParams = params
        }

        let json = """
        {"jsonrpc":"2.0","method":"test/method","id":"123","params":{"key":"value"}}
        """

        // When
        let handled = await sut.route(text: json)

        // Then
        XCTAssertTrue(handled)
        XCTAssertEqual(capturedId?.value, "123")
        XCTAssertEqual(capturedParams?["key"] as? String, "value")
    }

    func testRoute_UnhandledMethod_CallsOnUnhandled() async {
        // Given
        var onUnhandledCalled = false
        var capturedMethod: String?
        var capturedId: JSONRPC.ID?

        let json = """
        {"jsonrpc":"2.0","method":"unhandled/method","id":"123"}
        """

        // When
        let handled = await sut.route(text: json) { method, id in
            onUnhandledCalled = true
            capturedMethod = method
            capturedId = id
        }

        // Then
        XCTAssertFalse(handled)
        XCTAssertTrue(onUnhandledCalled)
        XCTAssertEqual(capturedMethod, "unhandled/method")
        XCTAssertEqual(capturedId?.value, "123")
    }

    func testRoute_InvalidJson_ReturnsFalse() async {
        // Given
        let json = "not valid json"

        // When
        let handled = await sut.route(text: json)

        // Then
        XCTAssertFalse(handled)
    }

    func testRoute_Notification_ReturnsFalse() async {
        // Given
        let json = """
        {"jsonrpc":"2.0","method":"test/notification"}
        """

        // When
        let handled = await sut.route(text: json)

        // Then
        XCTAssertFalse(handled)
    }

    func testRoute_MultipleHandlers() async {
        // Given
        var handler1Called = false
        var handler2Called = false

        sut.register(method: "method1") { _, _, _ in
            handler1Called = true
        }

        sut.register(method: "method2") { _, _, _ in
            handler2Called = true
        }

        let json1 = """
        {"jsonrpc":"2.0","method":"method1","id":"1"}
        """

        let json2 = """
        {"jsonrpc":"2.0","method":"method2","id":"2"}
        """

        // When
        await sut.route(text: json1)
        await sut.route(text: json2)

        // Then
        XCTAssertTrue(handler1Called)
        XCTAssertTrue(handler2Called)
    }

    // MARK: - Helper Method Tests

    func testExtractId_FromInt() {
        // Given
        let idAny: Any = 42

        // When
        let id = JsonRpcRouter.extractId(from: idAny)

        // Then
        XCTAssertEqual(id.value, "42")
    }

    func testExtractId_FromString() {
        // Given
        let idAny: Any = "test-id"

        // When
        let id = JsonRpcRouter.extractId(from: idAny)

        // Then
        XCTAssertEqual(id.value, "test-id")
    }

    func testExtractId_FromUnknownType() {
        // Given
        let idAny: Any = ["array"]

        // When
        let id = JsonRpcRouter.extractId(from: idAny)

        // Then
        XCTAssertEqual(id.value, "1") // Fallback
    }

    func testSendResponse_EncodesAndSends() {
        // Given
        struct TestResult: Codable {
            let message: String
        }
        let result = TestResult(message: "success")
        let id = JSONRPC.ID("test-id")

        // When
        JsonRpcRouter.sendResponse(id: id, result: result) { text in
            self.receivedMessages.append(text)
        }

        // Then
        XCTAssertEqual(receivedMessages.count, 1)
        XCTAssertTrue(receivedMessages[0].contains("\"result\""))
        XCTAssertTrue(receivedMessages[0].contains("success"))
        XCTAssertTrue(receivedMessages[0].contains("test-id"))
    }

    func testSendError_EncodesAndSends() {
        // Given
        let id = JSONRPC.ID("error-id")

        // When
        JsonRpcRouter.sendError(id: id, code: -32603, message: "Internal error") { text in
            self.receivedMessages.append(text)
        }

        // Then
        XCTAssertEqual(receivedMessages.count, 1)
        XCTAssertTrue(receivedMessages[0].contains("\"error\""))
        XCTAssertTrue(receivedMessages[0].contains("-32603"))
        XCTAssertTrue(receivedMessages[0].contains("Internal error"))
        XCTAssertTrue(receivedMessages[0].contains("error-id"))
    }

    func testSendNotification_EncodesAndSends() {
        // Given
        struct TestParams: Codable {
            let key: String
        }
        let params = TestParams(key: "value")

        // When
        JsonRpcRouter.sendNotification(method: "test/notification", params: params) { text in
            self.receivedMessages.append(text)
        }

        // Then
        XCTAssertEqual(receivedMessages.count, 1)
        XCTAssertTrue(receivedMessages[0].contains("\"method\""))
        XCTAssertTrue(receivedMessages[0].contains("test/notification"))
        XCTAssertTrue(receivedMessages[0].contains("value"))
        XCTAssertFalse(receivedMessages[0].contains("\"id\"")) // Notifications don't have id
    }

    // MARK: - Complex Parameter Tests

    func testRoute_WithComplexParams() async {
        // Given
        var capturedParams: [String: Any]?

        sut.register(method: "complex/method") { _, params, _ in
            capturedParams = params
        }

        let json = """
        {
            "jsonrpc":"2.0",
            "method":"complex/method",
            "id":"123",
            "params":{
                "string":"value",
                "number":42,
                "bool":true,
                "nested":{"key":"nested_value"}
            }
        }
        """

        // When
        await sut.route(text: json)

        // Then
        XCTAssertNotNil(capturedParams)
        XCTAssertEqual(capturedParams?["string"] as? String, "value")
        XCTAssertEqual(capturedParams?["number"] as? Int, 42)
        XCTAssertEqual(capturedParams?["bool"] as? Bool, true)

        if let nested = capturedParams?["nested"] as? [String: Any] {
            XCTAssertEqual(nested["key"] as? String, "nested_value")
        } else {
            XCTFail("Expected nested params")
        }
    }

    func testRoute_RawDictAccessible() async {
        // Given
        var capturedRawDict: [String: Any]?

        sut.register(method: "raw/method") { _, _, rawDict in
            capturedRawDict = rawDict
        }

        let json = """
        {"jsonrpc":"2.0","method":"raw/method","id":"123","custom_field":"custom_value"}
        """

        // When
        await sut.route(text: json)

        // Then
        XCTAssertNotNil(capturedRawDict)
        XCTAssertEqual(capturedRawDict?["method"] as? String, "raw/method")
        XCTAssertEqual(capturedRawDict?["custom_field"] as? String, "custom_value")
    }

    // MARK: - Concurrent Access Tests

    func testRoute_ConcurrentHandlers() async {
        // Given
        let expectation = XCTestExpectation(description: "All handlers called")
        expectation.expectedFulfillmentCount = 10

        sut.register(method: "concurrent/method") { _, _, _ in
            expectation.fulfill()
        }

        let json = """
        {"jsonrpc":"2.0","method":"concurrent/method","id":"1"}
        """

        // When
        await withTaskGroup(of: Void.self) { group in
            for _ in 1...10 {
                group.addTask {
                    await self.sut.route(text: json)
                }
            }
        }

        // Then
        wait(for: [expectation], timeout: 2.0)
    }
}
#endif
