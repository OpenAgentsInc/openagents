import Foundation
import FoundationModels

// MARK: - Test Generation Schema

/// A single test case generated from task description.
@Generable(description: "A test case for verifying code correctness")
struct GeneratedTest: Codable {
    var id: String

    @Guide(description: "The test input (e.g., log line for regex-log)")
    var input: String

    @Guide(description: "Expected output or null if no match expected")
    var expectedOutput: String?

    @Guide(description: "Why this test is important")
    var reasoning: String

    @Guide(description: "Test category", .anyOf([
        "existence",      // File/output created
        "format",         // Structure valid
        "happy_path",     // Basic correct behavior
        "boundary",       // Min/max limits
        "edge_case",      // Tricky scenarios
        "invalid_input",  // Should fail/reject
        "integration"     // System-level
    ]))
    var category: String

    @Guide(description: "Confidence score 0-1", .range(0.0...1.0))
    var confidence: Float
}

/// Complete test generation result.
@Generable(description: "Generated test suite from task description")
struct TestGenerationResult: Codable {
    @Guide(description: "Requirements extracted from task description")
    var requirements: [String]

    @Guide(description: "Assumptions made during generation")
    var assumptions: [String]

    @Guide(description: "Areas of uncertainty")
    var uncertainties: [String]

    @Guide(description: "Generated test cases")
    var tests: [GeneratedTest]
}

// Note: Dynamic JSON schema types removed to avoid recursive struct issues.
// Using pre-defined Generable types (TestGenerationResult) for guided generation.
// For custom schemas, use DynamicGenerationSchema from FoundationModels framework.
