import XCTest
@testable import OpenAgentsCore

final class ACPPlanStateTests: XCTestCase {

    // MARK: - ACPPlanStatus Tests

    func testPlanStatus_AllCases() {
        let allCases = ACPPlanStatus.allCases
        XCTAssertEqual(allCases.count, 4)
        XCTAssertTrue(allCases.contains(.idle))
        XCTAssertTrue(allCases.contains(.running))
        XCTAssertTrue(allCases.contains(.completed))
        XCTAssertTrue(allCases.contains(.failed))
    }

    func testPlanStatus_RawValues() {
        XCTAssertEqual(ACPPlanStatus.idle.rawValue, "idle")
        XCTAssertEqual(ACPPlanStatus.running.rawValue, "running")
        XCTAssertEqual(ACPPlanStatus.completed.rawValue, "completed")
        XCTAssertEqual(ACPPlanStatus.failed.rawValue, "failed")
    }

    func testPlanStatus_InitFromRawValue() {
        XCTAssertEqual(ACPPlanStatus(rawValue: "idle"), .idle)
        XCTAssertEqual(ACPPlanStatus(rawValue: "running"), .running)
        XCTAssertEqual(ACPPlanStatus(rawValue: "completed"), .completed)
        XCTAssertEqual(ACPPlanStatus(rawValue: "failed"), .failed)
        XCTAssertNil(ACPPlanStatus(rawValue: "invalid"))
    }

    func testPlanStatus_Encoding() throws {
        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        for status in ACPPlanStatus.allCases {
            let data = try encoder.encode(status)
            let decoded = try decoder.decode(ACPPlanStatus.self, from: data)
            XCTAssertEqual(decoded, status)
        }
    }

    func testPlanStatus_Equatable() {
        XCTAssertEqual(ACPPlanStatus.idle, ACPPlanStatus.idle)
        XCTAssertNotEqual(ACPPlanStatus.idle, ACPPlanStatus.running)
        XCTAssertNotEqual(ACPPlanStatus.running, ACPPlanStatus.completed)
        XCTAssertNotEqual(ACPPlanStatus.completed, ACPPlanStatus.failed)
    }

    // MARK: - ACPPlanState Tests

    func testPlanState_TypeConstant() {
        let state = ACPPlanState(status: .idle)
        XCTAssertEqual(state.type, "plan_state")

        // Type should always be "plan_state" regardless of status
        let runningState = ACPPlanState(status: .running)
        XCTAssertEqual(runningState.type, "plan_state")
    }

    func testPlanState_InitWithStatusOnly() {
        let state = ACPPlanState(status: .idle)
        XCTAssertEqual(state.status, .idle)
        XCTAssertNil(state.summary)
        XCTAssertNil(state.steps)
        XCTAssertNil(state.ts)
    }

    func testPlanState_InitWithAllParameters() {
        let steps = ["Step 1", "Step 2", "Step 3"]
        let state = ACPPlanState(
            status: .running,
            summary: "Processing request",
            steps: steps,
            ts: 1234567890000
        )

        XCTAssertEqual(state.status, .running)
        XCTAssertEqual(state.summary, "Processing request")
        XCTAssertEqual(state.steps, steps)
        XCTAssertEqual(state.ts, 1234567890000)
    }

    func testPlanState_Encoding() throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let state = ACPPlanState(
            status: .running,
            summary: "Test summary",
            steps: ["Step A", "Step B"],
            ts: 1000
        )

        let data = try encoder.encode(state)
        let json = String(data: data, encoding: .utf8)!

        XCTAssertTrue(json.contains("\"type\":\"plan_state\""))
        XCTAssertTrue(json.contains("\"status\":\"running\""))
        XCTAssertTrue(json.contains("\"summary\":\"Test summary\""))
        XCTAssertTrue(json.contains("\"steps\""))
        XCTAssertTrue(json.contains("\"ts\":1000"))
    }

    func testPlanState_Decoding() throws {
        let json = """
        {
            "type": "plan_state",
            "status": "completed",
            "summary": "All steps finished",
            "steps": ["First", "Second", "Third"],
            "ts": 9999999999
        }
        """

        let decoder = JSONDecoder()
        let data = json.data(using: .utf8)!
        let state = try decoder.decode(ACPPlanState.self, from: data)

        XCTAssertEqual(state.type, "plan_state")
        XCTAssertEqual(state.status, .completed)
        XCTAssertEqual(state.summary, "All steps finished")
        XCTAssertEqual(state.steps, ["First", "Second", "Third"])
        XCTAssertEqual(state.ts, 9999999999)
    }

    func testPlanState_DecodingMinimal() throws {
        let json = """
        {
            "type": "plan_state",
            "status": "idle"
        }
        """

        let decoder = JSONDecoder()
        let data = json.data(using: .utf8)!
        let state = try decoder.decode(ACPPlanState.self, from: data)

        XCTAssertEqual(state.type, "plan_state")
        XCTAssertEqual(state.status, .idle)
        XCTAssertNil(state.summary)
        XCTAssertNil(state.steps)
        XCTAssertNil(state.ts)
    }

    func testPlanState_RoundTrip() throws {
        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        let original = ACPPlanState(
            status: .failed,
            summary: "Error occurred",
            steps: ["Initialize", "Process", "Failed at validation"],
            ts: 1234567890123
        )

        let data = try encoder.encode(original)
        let decoded = try decoder.decode(ACPPlanState.self, from: data)

        XCTAssertEqual(decoded, original)
    }

    func testPlanState_Equatable() {
        let state1 = ACPPlanState(status: .running, summary: "Test", steps: ["A", "B"])
        let state2 = ACPPlanState(status: .running, summary: "Test", steps: ["A", "B"])
        let state3 = ACPPlanState(status: .completed, summary: "Test", steps: ["A", "B"])
        let state4 = ACPPlanState(status: .running, summary: "Different", steps: ["A", "B"])
        let state5 = ACPPlanState(status: .running, summary: "Test", steps: ["A", "C"])

        XCTAssertEqual(state1, state2)
        XCTAssertNotEqual(state1, state3) // Different status
        XCTAssertNotEqual(state1, state4) // Different summary
        XCTAssertNotEqual(state1, state5) // Different steps
    }

    // MARK: - Edge Cases

    func testPlanState_EmptySteps() {
        let state = ACPPlanState(status: .idle, summary: "Test", steps: [])
        XCTAssertNotNil(state.steps)
        XCTAssertEqual(state.steps?.count, 0)
    }

    func testPlanState_EmptySummary() {
        let state = ACPPlanState(status: .running, summary: "", steps: ["Step"])
        XCTAssertEqual(state.summary, "")
    }

    func testPlanState_LargeStepCount() {
        let steps = (1...100).map { "Step \($0)" }
        let state = ACPPlanState(status: .running, steps: steps)
        XCTAssertEqual(state.steps?.count, 100)
    }

    func testPlanState_LongSummary() {
        let summary = String(repeating: "Long summary text. ", count: 100)
        let state = ACPPlanState(status: .completed, summary: summary)
        XCTAssertEqual(state.summary?.count, summary.count)
    }

    func testPlanState_UnicodeContent() {
        let state = ACPPlanState(
            status: .running,
            summary: "Processing 你好世界 🚀",
            steps: ["First step 🎯", "Second step ✅", "Third step 🔄"]
        )
        XCTAssertEqual(state.summary, "Processing 你好世界 🚀")
        XCTAssertEqual(state.steps?[0], "First step 🎯")
    }

    func testPlanState_SpecialCharacters() {
        let state = ACPPlanState(
            status: .completed,
            summary: "Test \"quotes\" and 'apostrophes'",
            steps: ["Step with \\ backslash", "Step with / slash"]
        )

        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        let data = try! encoder.encode(state)
        let decoded = try! decoder.decode(ACPPlanState.self, from: data)

        XCTAssertEqual(decoded.summary, state.summary)
        XCTAssertEqual(decoded.steps, state.steps)
    }

    // MARK: - ACP Protocol Compliance

    func testPlanState_ACPShapeCompliance() throws {
        // Verify the structure matches expected ACP protocol format
        let state = ACPPlanState(
            status: .running,
            summary: "Building application",
            steps: [
                "Compile sources",
                "Link libraries",
                "Generate executable"
            ],
            ts: 1699900000000
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .prettyPrinted]
        let data = try encoder.encode(state)
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]

        // Verify required fields
        XCTAssertEqual(json["type"] as? String, "plan_state")
        XCTAssertEqual(json["status"] as? String, "running")

        // Verify optional fields present when set
        XCTAssertNotNil(json["summary"])
        XCTAssertNotNil(json["steps"])
        XCTAssertNotNil(json["ts"])

        // Verify types
        XCTAssertTrue(json["summary"] is String)
        XCTAssertTrue(json["steps"] is [String])
        XCTAssertTrue(json["ts"] is Int64 || json["ts"] is Int)
    }

    func testPlanState_AllStatusTransitions() throws {
        // Test encoding/decoding for all status transitions
        let transitions: [(ACPPlanStatus, ACPPlanStatus)] = [
            (.idle, .running),
            (.running, .completed),
            (.running, .failed),
            (.idle, .completed),
            (.failed, .running)  // Retry scenario
        ]

        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        for (from, to) in transitions {
            let state1 = ACPPlanState(status: from, summary: "State 1")
            let state2 = ACPPlanState(status: to, summary: "State 2")

            let data1 = try encoder.encode(state1)
            let data2 = try encoder.encode(state2)

            let decoded1 = try decoder.decode(ACPPlanState.self, from: data1)
            let decoded2 = try decoder.decode(ACPPlanState.self, from: data2)

            XCTAssertEqual(decoded1.status, from)
            XCTAssertEqual(decoded2.status, to)
        }
    }

    func testPlanState_TimestampMilliseconds() {
        // Verify timestamp is in milliseconds (13 digits for dates after 2001)
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        let state = ACPPlanState(status: .running, ts: now)

        XCTAssertEqual(state.ts, now)
        XCTAssertGreaterThan(now, 1_000_000_000_000) // > year 2001 in ms
        XCTAssertLessThan(now, 10_000_000_000_000) // < year 2286 in ms
    }
}
