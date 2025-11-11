import XCTest
@testable import OpenAgentsCore

final class ToolNameTests: XCTestCase {
    func testFromStringRecognizesCoreNames() {
        XCTAssertEqual(ToolName.fromString("bash"), .bash)
        XCTAssertEqual(ToolName.fromString("delegate.run"), .delegate)
        XCTAssertEqual(ToolName.fromString("session.list"), .sessionList)
        XCTAssertEqual(ToolName.fromString("content.get_span"), .contentGetSpan)
    }

    func testFromStringReturnsCustomForUnknownNames() {
        let custom = ToolName.fromString("codex.custom")
        guard case .custom(let raw) = custom else {
            return XCTFail("Expected custom value, got \(custom)")
        }
        XCTAssertEqual(raw, "codex.custom")
        XCTAssertEqual(custom.rawValue, "codex.custom")
    }

    func testBaseNameExtractsSuffixWhenNamespaced() {
        XCTAssertEqual(ToolName.custom("codex.bash").baseName, "bash")
        XCTAssertEqual(ToolName.delegate.baseName, "run")
        XCTAssertEqual(ToolName.read.baseName, "read")
    }

    func testMatchesDetectsCoreVariants() {
        XCTAssertTrue(ToolName.custom("codex.bash").matches(.bash))
        XCTAssertTrue(ToolName.custom("claude_code.read").matches(.read))
        XCTAssertFalse(ToolName.custom("codex.bash").matches(.read))
        XCTAssertTrue(ToolName.bash.matches(.bash))
        XCTAssertFalse(ToolName.bash.matches(.read))
    }

    func testIsCoreFlagsNamespacedValues() {
        XCTAssertTrue(ToolName.bash.isCore)
        XCTAssertFalse(ToolName.custom("codex.bash").isCore)
        XCTAssertFalse(ToolName.custom("delegate.run").isCore)
        XCTAssertTrue(ToolName.custom("grep").isCore) // custom but no namespace
    }

    func testCodableRoundTripPreservesRawValue() throws {
        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        let original: [ToolName] = [
            .bash,
            .delegate,
            .custom("codex.bash"),
            .custom("grep")
        ]

        let data = try encoder.encode(original)
        let decoded = try decoder.decode([ToolName].self, from: data)

        XCTAssertEqual(decoded, original)
        XCTAssertEqual(decoded.map(\.rawValue), ["bash", "delegate.run", "codex.bash", "grep"])
    }
}
