import XCTest
@testable import OpenAgentsCore

final class HistoryLoaderTests: XCTestCase {
    func testDeduplicatesBySourceAndIdKeepingNewestUpdate() {
        let options = HistoryLoader.Options()
        let codex = [
            makeSummary(source: "codex", id: "abc", updated: 100, lastMessage: 90),
            makeSummary(source: "codex", id: "abc", updated: 150, lastMessage: 140)
        ]

        let result = HistoryLoader.mergeSummaries(
            opts: options,
            codexSummaries: codex,
            claudeSummaries: []
        )

        XCTAssertEqual(result.count, 1)
        XCTAssertEqual(result.first?.updated_at, 150)
        XCTAssertEqual(result.first?.last_message_ts, 140)
    }

    func testSortsByUpdatedAtAndLastMessageTimestamp() {
        let options = HistoryLoader.Options()
        let codex = [
            makeSummary(source: "codex", id: "c-old-last", updated: 200, lastMessage: 50),
            makeSummary(source: "codex", id: "c-young", updated: 100, lastMessage: 20)
        ]
        let claude = [
            makeSummary(source: "claude_code", id: "c-new", updated: 250, lastMessage: 240),
            makeSummary(source: "claude_code", id: "c-tie-wins", updated: 200, lastMessage: 180)
        ]

        let result = HistoryLoader.mergeSummaries(
            opts: options,
            codexSummaries: codex,
            claudeSummaries: claude
        )

        XCTAssertEqual(result.map(summaryKey), [
            "claude_code::c-new",
            "claude_code::c-tie-wins",
            "codex::c-old-last",
            "codex::c-young"
        ])
    }

    func testRespectsProviderFlagsAndMaxResults() {
        var options = HistoryLoader.Options(
            includeCodex: false,
            includeClaude: true,
            maxFilesPerProvider: 10,
            maxResults: 2
        )

        let codex = [
            makeSummary(source: "codex", id: "ignored", updated: 300, lastMessage: 300)
        ]
        let claude = [
            makeSummary(source: "claude_code", id: "first", updated: 500, lastMessage: 400),
            makeSummary(source: "claude_code", id: "second", updated: 400, lastMessage: 300),
            makeSummary(source: "claude_code", id: "trimmed", updated: 300, lastMessage: 200)
        ]

        var result = HistoryLoader.mergeSummaries(
            opts: options,
            codexSummaries: codex,
            claudeSummaries: claude
        )

        XCTAssertEqual(result.count, 2)
        XCTAssertTrue(result.allSatisfy { $0.source == "claude_code" })
        XCTAssertEqual(result.map(\.id), ["first", "second"])

        options.includeCodex = true
        options.includeClaude = false
        result = HistoryLoader.mergeSummaries(
            opts: options,
            codexSummaries: codex,
            claudeSummaries: claude
        )
        XCTAssertEqual(result.count, 1)
        XCTAssertEqual(result.first?.source, "codex")
        XCTAssertEqual(result.first?.id, "ignored")
    }

    // MARK: - Helpers

    private func makeSummary(
        source: String,
        id: String,
        updated: Int64,
        lastMessage: Int64?,
        title: String? = nil,
        messageCount: Int? = nil
    ) -> ThreadSummary {
        ThreadSummary(
            id: id,
            title: title,
            source: source,
            created_at: updated - 60,
            updated_at: updated,
            last_message_ts: lastMessage,
            message_count: messageCount
        )
    }

    private func summaryKey(_ summary: ThreadSummary) -> String {
        "\(summary.source)::\(summary.id)"
    }
}
