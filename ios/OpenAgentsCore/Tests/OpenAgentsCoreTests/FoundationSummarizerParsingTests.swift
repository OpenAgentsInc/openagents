import XCTest
@testable import OpenAgentsCore

final class FoundationSummarizerParsingTests: XCTestCase {
    func testExtractsContentFromResponseDescription() throws {
        let sample = "LanguageModelSession.Response<String>(userPrompt: \"...\", duration: 1.0, feedbackAttachment: nil, content: 'Short sample title', rawContent: 'Short sample title', transcriptEntries: ArraySlice([]))"
        let s = invokeExtract(sample)
        XCTAssertEqual(s, "Short sample title")
    }

    func testReturnsNilForGuardrailMessage() throws {
        let sample = "LanguageModelSession.Response<String>(..., content: 'Error during development', rawContent: 'Error during development')"
        XCTAssertNil(invokeExtract(sample))
    }

    private func invokeExtract(_ desc: String) -> String? {
        // Call the private helper via a thin shim
        return FoundationModelSummarizer_extractFromDescription(desc)
    }
}

