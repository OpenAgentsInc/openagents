import XCTest
@testable import OpenAgentsCore

/// Tests for conversational question detection logic
/// Used to route simple questions to OpenAgents orchestrator vs coding tasks to specialized agents
final class ConversationalDetectionTests: XCTestCase {

    // MARK: - Conversational (should return true)

    func testGreetings_AreConversational() {
        XCTAssertTrue(ConversationalDetection.isConversational("hi"))
        XCTAssertTrue(ConversationalDetection.isConversational("hello"))
        XCTAssertTrue(ConversationalDetection.isConversational("hey"))
        XCTAssertTrue(ConversationalDetection.isConversational("greetings"))
        XCTAssertTrue(ConversationalDetection.isConversational("Hi"))
        XCTAssertTrue(ConversationalDetection.isConversational("HELLO"))
    }

    func testIdentityQuestions_AreConversational() {
        XCTAssertTrue(ConversationalDetection.isConversational("who are you"))
        XCTAssertTrue(ConversationalDetection.isConversational("Who are you?"))
        XCTAssertTrue(ConversationalDetection.isConversational("what are you"))
        XCTAssertTrue(ConversationalDetection.isConversational("tell me about yourself"))
        XCTAssertTrue(ConversationalDetection.isConversational("what can you do"))
        XCTAssertTrue(ConversationalDetection.isConversational("what do you do"))
        XCTAssertTrue(ConversationalDetection.isConversational("what are your capabilities"))
        XCTAssertTrue(ConversationalDetection.isConversational("how do you work"))
        XCTAssertTrue(ConversationalDetection.isConversational("how can you help"))
        XCTAssertTrue(ConversationalDetection.isConversational("can you help me"))
        XCTAssertTrue(ConversationalDetection.isConversational("what is openagents"))
        XCTAssertTrue(ConversationalDetection.isConversational("what's openagents"))
    }

    func testIdentityQuestionsWithContext_AreConversational() {
        // The original failing case from the user
        XCTAssertTrue(ConversationalDetection.isConversational("Now tell me, who are you?"))
        XCTAssertTrue(ConversationalDetection.isConversational("So, what can you do?"))
        XCTAssertTrue(ConversationalDetection.isConversational("I'm curious, who are you?"))
    }

    func testShortQuestions_AreConversational() {
        XCTAssertTrue(ConversationalDetection.isConversational("What?"))
        XCTAssertTrue(ConversationalDetection.isConversational("Why?"))
        XCTAssertTrue(ConversationalDetection.isConversational("How?"))
        XCTAssertTrue(ConversationalDetection.isConversational("What is this?"))
    }

    // MARK: - Coding Tasks (should return false)

    func testFileOperations_AreNotConversational() {
        XCTAssertFalse(ConversationalDetection.isConversational("Read the file"))
        XCTAssertFalse(ConversationalDetection.isConversational("List all files"))
        XCTAssertFalse(ConversationalDetection.isConversational("Create a new file"))
        XCTAssertFalse(ConversationalDetection.isConversational("Show me the file contents"))
    }

    func testCodeOperations_AreNotConversational() {
        XCTAssertFalse(ConversationalDetection.isConversational("Write a function"))
        XCTAssertFalse(ConversationalDetection.isConversational("Implement the class"))
        XCTAssertFalse(ConversationalDetection.isConversational("Refactor this code"))
        XCTAssertFalse(ConversationalDetection.isConversational("Debug the error"))
        XCTAssertFalse(ConversationalDetection.isConversational("Fix the bug"))
        XCTAssertFalse(ConversationalDetection.isConversational("Run the tests"))
        XCTAssertFalse(ConversationalDetection.isConversational("Build the project"))
        XCTAssertFalse(ConversationalDetection.isConversational("Compile this"))
    }

    func testGitOperations_AreNotConversational() {
        XCTAssertFalse(ConversationalDetection.isConversational("git status"))
        XCTAssertFalse(ConversationalDetection.isConversational("Commit these changes"))
        XCTAssertFalse(ConversationalDetection.isConversational("Create a git branch"))
    }

    func testFileExtensions_AreNotConversational() {
        XCTAssertFalse(ConversationalDetection.isConversational("Open main.swift"))
        XCTAssertFalse(ConversationalDetection.isConversational("Edit script.py"))
        XCTAssertFalse(ConversationalDetection.isConversational("Check app.js"))
        XCTAssertFalse(ConversationalDetection.isConversational("Read types.ts"))
    }

    func testDependencyOperations_AreNotConversational() {
        XCTAssertFalse(ConversationalDetection.isConversational("Install the package"))
        XCTAssertFalse(ConversationalDetection.isConversational("Add a dependency"))
        XCTAssertFalse(ConversationalDetection.isConversational("Update packages"))
    }

    // MARK: - Edge Cases

    func testLongQuestions_AreNotConversational() {
        // Questions over 100 chars are likely specific queries, not simple conversation
        let longQuestion = String(repeating: "What is this thing and why does it exist and what should I know about it? ", count: 2)
        XCTAssertFalse(ConversationalDetection.isConversational(longQuestion))
    }

    func testEmptyString_IsNotConversational() {
        XCTAssertFalse(ConversationalDetection.isConversational(""))
        XCTAssertFalse(ConversationalDetection.isConversational("   "))
    }

    func testMixedCodingAndConversational_PrefersCoding() {
        // If a prompt mentions both conversational and coding keywords, coding takes precedence
        XCTAssertFalse(ConversationalDetection.isConversational("who are you and can you fix this bug?"))
        XCTAssertFalse(ConversationalDetection.isConversational("tell me about yourself while you refactor this code"))
    }

    // MARK: - Whitespace Handling

    func testWhitespaceVariations_AreHandled() {
        XCTAssertTrue(ConversationalDetection.isConversational("  who are you  "))
        XCTAssertTrue(ConversationalDetection.isConversational("\nwho are you\n"))
        XCTAssertTrue(ConversationalDetection.isConversational("\t\thello\t\t"))
    }
}
