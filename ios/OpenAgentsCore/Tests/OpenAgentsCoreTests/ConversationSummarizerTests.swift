import XCTest
@testable import OpenAgentsCore

final class ConversationSummarizerTests: XCTestCase {
    func testHeuristicSummaryProducesShortTitle() async throws {
        let msgs = demoConversation()
        let title = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: false)
        XCTAssertFalse(title.isEmpty)
        XCTAssertLessThanOrEqual(title.split(separator: " ").count, 5)
    }

    func testFoundationModelsPathDoesNotCrash() async throws {
        let msgs = demoConversation()
        let title = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: true)
        XCTAssertFalse(title.isEmpty)
    }

    private func demoConversation() -> [ACPMessage] {
        var ts: Int64 = 1_700_000_000_000
        func make(_ role: ACPRole, _ text: String) -> ACPMessage {
            defer { ts += 1000 }
            return ACPMessage(id: UUID().uuidString, thread_id: "t-demo", role: role, parts: [.text(ACPText(text: text))], ts: ts)
        }
        return [
            make(.user, "Build a Swift utility to parse JSONL."),
            make(.assistant, "You can stream lines with FileHandle.readLine."),
            make(.user, "We also need a fast tail reader for large files."),
            make(.assistant, "Read last 1MB and split by newlines; drop partial head."),
            make(.user, "Great, add tests and ensure stability."),
            make(.assistant, "Add unit tests and guard for empty lines."),
            make(.user, "Consider performance on macOS and iOS."),
            make(.assistant, "Use background QoS and avoid full-file reads."),
            make(.user, "Summarize the approach as a short title."),
            make(.assistant, "Provide a concise, 3–5 word title.")
        ]
    }
}

