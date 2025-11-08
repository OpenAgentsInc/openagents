import XCTest
@testable import OpenAgentsCore

final class JSONRPCRequestManagerTests: XCTestCase {
    struct Foo: Codable, Equatable { let a: Int }

    func testAddAndFulfill_Success() throws {
        let m = JSONRPCRequestManager()
        let exp = expectation(description: "fulfilled")
        var received: Foo?
        m.addExpectation(id: "1") { (resp: Foo?) in
            received = resp
            exp.fulfill()
        }
        // Simulate server JSON result object
        let obj: [String: Any] = ["a": 42]
        XCTAssertTrue(m.fulfill(id: "1", withJsonResult: obj))
        wait(for: [exp], timeout: 1.0)
        XCTAssertEqual(received, Foo(a: 42))
    }

    func testFulfill_WrongType_YieldsNil() {
        let m = JSONRPCRequestManager()
        let exp = expectation(description: "fulfilled")
        var received: Foo?
        m.addExpectation(id: "2") { (resp: Foo?) in
            received = resp
            exp.fulfill()
        }
        // Provide incompatible result shape
        let obj: [String: Any] = ["b": 10]
        XCTAssertTrue(m.fulfill(id: "2", withJsonResult: obj))
        wait(for: [exp], timeout: 1.0)
        XCTAssertNil(received)
    }

    func testRemove_CancelsPending() {
        let m = JSONRPCRequestManager()
        m.addExpectation(id: "3") { (resp: Foo?) in
            XCTFail("Should not be called")
        }
        m.remove(id: "3")
        // Fulfill after removal should return false (no handler)
        XCTAssertFalse(m.fulfill(id: "3", withJsonResult: ["a": 1]))
    }
}

