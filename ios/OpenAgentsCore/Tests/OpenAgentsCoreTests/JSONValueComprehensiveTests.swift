import XCTest
@testable import OpenAgentsCore

final class JSONValueComprehensiveTests: XCTestCase {
    var encoder: JSONEncoder!
    var decoder: JSONDecoder!

    override func setUp() {
        super.setUp()
        encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        decoder = JSONDecoder()
    }

    override func tearDown() {
        encoder = nil
        decoder = nil
        super.tearDown()
    }

    // MARK: - Null Tests

    func testNull_Encoding() throws {
        let value = JSONValue.null
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data)
        XCTAssertTrue(json is NSNull)
    }

    func testNull_Decoding() throws {
        let json = "null".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        XCTAssertEqual(value, .null)
    }

    func testNull_RoundTrip() throws {
        let original = JSONValue.null
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(original, decoded)
    }

    // MARK: - Bool Tests

    func testBool_TrueEncoding() throws {
        let value = JSONValue.bool(true)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Bool
        XCTAssertEqual(json, true)
    }

    func testBool_FalseEncoding() throws {
        let value = JSONValue.bool(false)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Bool
        XCTAssertEqual(json, false)
    }

    func testBool_TrueDecoding() throws {
        let json = "true".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        XCTAssertEqual(value, .bool(true))
    }

    func testBool_FalseDecoding() throws {
        let json = "false".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        XCTAssertEqual(value, .bool(false))
    }

    func testBool_RoundTrip() throws {
        let original = JSONValue.bool(true)
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(original, decoded)
    }

    // MARK: - Number Tests

    func testNumber_IntegerEncoding() throws {
        let value = JSONValue.number(42)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Double
        XCTAssertEqual(json, 42.0)
    }

    func testNumber_FloatEncoding() throws {
        let value = JSONValue.number(3.14)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Double
        XCTAssertEqual(json, 3.14, accuracy: 0.001)
    }

    func testNumber_NegativeEncoding() throws {
        let value = JSONValue.number(-100.5)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Double
        XCTAssertEqual(json, -100.5, accuracy: 0.001)
    }

    func testNumber_ZeroEncoding() throws {
        let value = JSONValue.number(0)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Double
        XCTAssertEqual(json, 0.0)
    }

    func testNumber_LargeEncoding() throws {
        let value = JSONValue.number(1e308)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Double
        XCTAssertNotNil(json)
    }

    func testNumber_SmallEncoding() throws {
        let value = JSONValue.number(1e-308)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? Double
        XCTAssertNotNil(json)
    }

    func testNumber_Decoding() throws {
        let json = "42.5".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        XCTAssertEqual(value, .number(42.5))
    }

    func testNumber_RoundTrip() throws {
        let original = JSONValue.number(123.456)
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        if case .number(let n) = decoded {
            XCTAssertEqual(n, 123.456, accuracy: 0.001)
        } else {
            XCTFail("Expected number")
        }
    }

    // MARK: - String Tests

    func testString_SimpleEncoding() throws {
        let value = JSONValue.string("hello")
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? String
        XCTAssertEqual(json, "hello")
    }

    func testString_EmptyEncoding() throws {
        let value = JSONValue.string("")
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? String
        XCTAssertEqual(json, "")
    }

    func testString_SpecialCharactersEncoding() throws {
        let value = JSONValue.string("Line1\nLine2\tTabbed")
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? String
        XCTAssertEqual(json, "Line1\nLine2\tTabbed")
    }

    func testString_QuotesEncoding() throws {
        let value = JSONValue.string("She said \"hello\"")
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? String
        XCTAssertEqual(json, "She said \"hello\"")
    }

    func testString_UnicodeEncoding() throws {
        let value = JSONValue.string("Hello 世界 🚀")
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? String
        XCTAssertEqual(json, "Hello 世界 🚀")
    }

    func testString_LargeEncoding() throws {
        let longString = String(repeating: "a", count: 100000)
        let value = JSONValue.string(longString)
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? String
        XCTAssertEqual(json?.count, 100000)
    }

    func testString_Decoding() throws {
        let json = "\"test string\"".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        XCTAssertEqual(value, .string("test string"))
    }

    func testString_RoundTrip() throws {
        let original = JSONValue.string("test\nvalue")
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(original, decoded)
    }

    // MARK: - Array Tests

    func testArray_EmptyEncoding() throws {
        let value = JSONValue.array([])
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? [Any]
        XCTAssertEqual(json?.count, 0)
    }

    func testArray_SingleElement() throws {
        let value = JSONValue.array([.string("test")])
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? [Any]
        XCTAssertEqual(json?.count, 1)
    }

    func testArray_MultipleElements() throws {
        let value = JSONValue.array([
            .string("a"),
            .number(1),
            .bool(true),
            .null
        ])
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? [Any]
        XCTAssertEqual(json?.count, 4)
    }

    func testArray_Nested() throws {
        let value = JSONValue.array([
            .array([.string("nested"), .number(1)]),
            .array([.bool(true), .null])
        ])
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(value, decoded)
    }

    func testArray_LargeArray() throws {
        let elements = (0..<1000).map { JSONValue.number(Double($0)) }
        let value = JSONValue.array(elements)
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        if case .array(let arr) = decoded {
            XCTAssertEqual(arr.count, 1000)
        } else {
            XCTFail("Expected array")
        }
    }

    func testArray_Decoding() throws {
        let json = "[1, 2, 3]".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        if case .array(let arr) = value {
            XCTAssertEqual(arr.count, 3)
        } else {
            XCTFail("Expected array")
        }
    }

    func testArray_RoundTrip() throws {
        let original = JSONValue.array([.string("test"), .number(42), .bool(true)])
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(original, decoded)
    }

    // MARK: - Object Tests

    func testObject_EmptyEncoding() throws {
        let value = JSONValue.object([:])
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertEqual(json?.count, 0)
    }

    func testObject_SingleProperty() throws {
        let value = JSONValue.object(["key": .string("value")])
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertEqual(json?.count, 1)
        XCTAssertEqual(json?["key"] as? String, "value")
    }

    func testObject_MultipleProperties() throws {
        let value = JSONValue.object([
            "string": .string("test"),
            "number": .number(42),
            "bool": .bool(true),
            "null": .null
        ])
        let data = try encoder.encode(value)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertEqual(json?.count, 4)
    }

    func testObject_Nested() throws {
        let value = JSONValue.object([
            "outer": .object([
                "inner": .string("value")
            ])
        ])
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(value, decoded)
    }

    func testObject_ComplexNesting() throws {
        let value = JSONValue.object([
            "array": .array([.number(1), .number(2)]),
            "object": .object(["nested": .string("value")]),
            "primitives": .object([
                "string": .string("test"),
                "number": .number(3.14),
                "bool": .bool(false)
            ])
        ])
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(value, decoded)
    }

    func testObject_SpecialKeys() throws {
        let value = JSONValue.object([
            "key with spaces": .string("value"),
            "key-with-dashes": .string("value"),
            "key_with_underscores": .string("value"),
            "KeyWithCamelCase": .string("value")
        ])
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(value, decoded)
    }

    func testObject_Decoding() throws {
        let json = "{\"key\":\"value\"}".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        if case .object(let obj) = value {
            XCTAssertEqual(obj.count, 1)
            XCTAssertEqual(obj["key"], .string("value"))
        } else {
            XCTFail("Expected object")
        }
    }

    func testObject_RoundTrip() throws {
        let original = JSONValue.object([
            "test": .string("value"),
            "number": .number(123)
        ])
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(original, decoded)
    }

    // MARK: - Complex Nested Structures

    func testComplexStructure_DeepNesting() throws {
        let value = JSONValue.object([
            "level1": .object([
                "level2": .object([
                    "level3": .array([
                        .object(["level4": .string("deep value")])
                    ])
                ])
            ])
        ])
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(value, decoded)
    }

    func testComplexStructure_MixedTypes() throws {
        let value = JSONValue.object([
            "strings": .array([.string("a"), .string("b")]),
            "numbers": .array([.number(1), .number(2), .number(3)]),
            "bools": .array([.bool(true), .bool(false)]),
            "nulls": .array([.null, .null]),
            "mixed": .array([.string("x"), .number(1), .bool(true), .null])
        ])
        let data = try encoder.encode(value)
        let decoded = try decoder.decode(JSONValue.self, from: data)
        XCTAssertEqual(value, decoded)
    }

    // MARK: - Equality Tests

    func testEquality_SameStrings() {
        XCTAssertEqual(JSONValue.string("test"), JSONValue.string("test"))
    }

    func testEquality_DifferentStrings() {
        XCTAssertNotEqual(JSONValue.string("test1"), JSONValue.string("test2"))
    }

    func testEquality_SameNumbers() {
        XCTAssertEqual(JSONValue.number(42), JSONValue.number(42))
    }

    func testEquality_DifferentNumbers() {
        XCTAssertNotEqual(JSONValue.number(42), JSONValue.number(43))
    }

    func testEquality_SameBools() {
        XCTAssertEqual(JSONValue.bool(true), JSONValue.bool(true))
    }

    func testEquality_DifferentBools() {
        XCTAssertNotEqual(JSONValue.bool(true), JSONValue.bool(false))
    }

    func testEquality_BothNull() {
        XCTAssertEqual(JSONValue.null, JSONValue.null)
    }

    func testEquality_SameArrays() {
        let arr1 = JSONValue.array([.string("a"), .number(1)])
        let arr2 = JSONValue.array([.string("a"), .number(1)])
        XCTAssertEqual(arr1, arr2)
    }

    func testEquality_DifferentArrays() {
        let arr1 = JSONValue.array([.string("a")])
        let arr2 = JSONValue.array([.string("b")])
        XCTAssertNotEqual(arr1, arr2)
    }

    func testEquality_SameObjects() {
        let obj1 = JSONValue.object(["key": .string("value")])
        let obj2 = JSONValue.object(["key": .string("value")])
        XCTAssertEqual(obj1, obj2)
    }

    func testEquality_DifferentObjects() {
        let obj1 = JSONValue.object(["key1": .string("value")])
        let obj2 = JSONValue.object(["key2": .string("value")])
        XCTAssertNotEqual(obj1, obj2)
    }

    func testEquality_DifferentTypes() {
        XCTAssertNotEqual(JSONValue.string("42"), JSONValue.number(42))
        XCTAssertNotEqual(JSONValue.bool(false), JSONValue.null)
        XCTAssertNotEqual(JSONValue.array([]), JSONValue.object([:]))
    }

    // MARK: - Edge Cases

    func testInvalidJSON_ThrowsError() {
        let invalidJSON = "{ invalid }".data(using: .utf8)!
        XCTAssertThrowsError(try decoder.decode(JSONValue.self, from: invalidJSON))
    }

    func testEmptyData_ThrowsError() {
        let emptyData = Data()
        XCTAssertThrowsError(try decoder.decode(JSONValue.self, from: emptyData))
    }

    func testUnsupportedType_FallsToNull() throws {
        // JSONValue handles all JSON types, so this is hard to test
        // But we can verify null decoding works
        let json = "null".data(using: .utf8)!
        let value = try decoder.decode(JSONValue.self, from: json)
        XCTAssertEqual(value, .null)
    }

    func testLargeObject_Performance() throws {
        var largeObject: [String: JSONValue] = [:]
        for i in 0..<1000 {
            largeObject["key\(i)"] = .string("value\(i)")
        }
        let value = JSONValue.object(largeObject)

        measure {
            _ = try? encoder.encode(value)
        }
    }

    func testDeepNesting_Performance() throws {
        var nested = JSONValue.string("base")
        for _ in 0..<100 {
            nested = .array([nested])
        }

        measure {
            _ = try? encoder.encode(nested)
        }
    }
}
