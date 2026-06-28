import Foundation
@testable import Khala
import XCTest

/// Coverage for the assistant-markdown parser used to render Khala coding
/// answers: fenced code blocks, headings, lists (incl. NESTED lists), and the
/// position-stable identity that keeps repeated/identical blocks from collapsing.
final class MarkdownParserTests: XCTestCase {
    // MARK: - Fenced code blocks

    func testExtractsFencedCodeBlockWithLanguage() {
        let blocks = MarkdownParser.parse("""
        Here is a function:

        ```swift
        func add(_ a: Int, _ b: Int) -> Int { a + b }
        ```
        """)
        guard case let .code(language, code) = blocks.last else {
            return XCTFail("Expected a trailing code block, got \(blocks)")
        }
        XCTAssertEqual(language, "swift")
        XCTAssertEqual(code, "func add(_ a: Int, _ b: Int) -> Int { a + b }")
    }

    func testKeepsTwoIdenticalCodeBlocksDistinct() {
        // Two identical fenced blocks previously collided on a content-hash id,
        // which dropped one of them and warned about a duplicate SwiftUI ID.
        let identified = MarkdownParser.parseIdentified("""
        ```
        same
        ```

        ```
        same
        ```
        """)
        let codeBlocks = identified.filter {
            if case .code = $0.block { return true }
            return false
        }
        XCTAssertEqual(codeBlocks.count, 2, "Both identical code blocks must survive")
        XCTAssertEqual(Set(identified.map(\.id)).count, identified.count, "IDs must be unique")
    }

    func testIdenticalParagraphsBothSurviveWithUniqueIDs() {
        let identified = MarkdownParser.parseIdentified("Repeat.\n\nRepeat.")
        XCTAssertEqual(identified.count, 2)
        XCTAssertEqual(Set(identified.map(\.id)).count, 2)
    }

    // MARK: - Nested lists

    func testParsesNestedBulletIndentLevels() {
        let blocks = MarkdownParser.parse("""
        - top
          - child
            - grandchild
        - top two
        """)
        guard case let .bullet(items) = blocks.first else {
            return XCTFail("Expected a bullet list, got \(blocks)")
        }
        XCTAssertEqual(items.map(\.indent), [0, 1, 2, 0])
        XCTAssertEqual(items.map(\.text), ["top", "child", "grandchild", "top two"])
    }

    func testParsesNestedNumberedIndentLevels() {
        let blocks = MarkdownParser.parse("""
        1. first
           1. nested first
           2. nested second
        2. second
        """)
        guard case let .numbered(items) = blocks.first else {
            return XCTFail("Expected a numbered list, got \(blocks)")
        }
        XCTAssertEqual(items.map(\.indent), [0, 1, 1, 0])
        XCTAssertEqual(items.first?.text, "first")
    }

    func testTabIndentCountsAsNesting() {
        let blocks = MarkdownParser.parse("- top\n\t- child")
        guard case let .bullet(items) = blocks.first else {
            return XCTFail("Expected a bullet list, got \(blocks)")
        }
        XCTAssertEqual(items.map(\.indent), [0, 1])
    }

    // MARK: - Headings + paragraphs

    func testParsesHeadingLevelAndText() {
        let blocks = MarkdownParser.parse("### A heading")
        guard case let .heading(level, text) = blocks.first else {
            return XCTFail("Expected a heading, got \(blocks)")
        }
        XCTAssertEqual(level, 3)
        XCTAssertEqual(text, "A heading")
    }

    func testHashWithoutSpaceIsNotAHeading() {
        let blocks = MarkdownParser.parse("#notaheading")
        guard case .paragraph = blocks.first else {
            return XCTFail("Expected a paragraph, got \(blocks)")
        }
    }

    func testInlineAttributedFallsBackToPlainTextForMalformedMarkdown() {
        // A partial stream can hand the parser an unbalanced `**`; it must not
        // throw or drop the text.
        let attributed = MarkdownParser.inlineAttributed("an **unfinished bold")
        XCTAssertTrue(String(attributed.characters).contains("unfinished bold"))
    }
}
