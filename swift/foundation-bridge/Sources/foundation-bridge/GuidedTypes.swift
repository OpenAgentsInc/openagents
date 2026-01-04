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

// MARK: - Environment-Aware Test Generation Schema

/// Language runtime information.
@Generable(description: "Programming language runtime info")
struct LanguageInfo: Codable {
    @Guide(description: "Language name (python, node, r, rust, go, java)")
    var name: String

    @Guide(description: "Version string (e.g., 3.11.4)")
    var version: String

    @Guide(description: "Installed packages with versions")
    var packages: [String]
}

/// File system entry information.
@Generable(description: "File system entry")
struct FileInfo: Codable {
    @Guide(description: "File name")
    var name: String

    @Guide(description: "Full path")
    var path: String

    @Guide(description: "Entry type", .anyOf(["file", "directory", "symlink"]))
    var fileType: String

    @Guide(description: "Size in bytes")
    var size: Int

    @Guide(description: "Permissions string (e.g., -rw-r--r--)")
    var permissions: String
}

/// File content preview with structure extraction.
@Generable(description: "File content preview")
struct FilePreviewInfo: Codable {
    @Guide(description: "File path")
    var path: String

    @Guide(description: "File extension")
    var fileExtension: String

    @Guide(description: "Total line count")
    var lineCount: Int

    @Guide(description: "Preview content (first 50 lines)")
    var preview: String

    @Guide(description: "Detected file type", .anyOf([
        "python_script", "r_script", "stan_model", "json", "csv",
        "yaml", "toml", "dockerfile", "makefile", "shell_script",
        "c_source", "cpp_source", "rust_source", "go_source", "unknown"
    ]))
    var detectedType: String

    @Guide(description: "Variable names found in file")
    var variables: [String]

    @Guide(description: "Function names found in file")
    var functions: [String]

    @Guide(description: "Parameter names found (for config/model files)")
    var parameters: [String]
}

/// Available tool information.
@Generable(description: "System tool info")
struct ToolInfoEntry: Codable {
    @Guide(description: "Tool name (e.g., git, curl)")
    var name: String

    @Guide(description: "Path to executable")
    var path: String

    @Guide(description: "Version if available")
    var version: String?
}

/// Prohibited tool (anti-cheat).
@Generable(description: "Tool that should NOT be present")
struct ProhibitedToolInfo: Codable {
    @Guide(description: "Tool name (e.g., R, Rscript)")
    var name: String

    @Guide(description: "Why this tool should be prohibited")
    var reason: String

    @Guide(description: "Whether the tool was found (should be false)")
    var found: Bool
}

/// Complete environment context for test generation.
@Generable(description: "Execution environment context")
struct EnvironmentContext: Codable {
    @Guide(description: "Platform type", .anyOf(["docker", "container", "local"]))
    var platform: String

    @Guide(description: "Container image name if applicable")
    var containerImage: String?

    @Guide(description: "OS distribution (e.g., ubuntu, debian)")
    var osDistro: String?

    @Guide(description: "Available programming languages")
    var languages: [LanguageInfo]

    @Guide(description: "Available system tools")
    var availableTools: [ToolInfoEntry]

    @Guide(description: "Tools that should NOT be present (anti-cheat)")
    var prohibitedTools: [ProhibitedToolInfo]

    @Guide(description: "Working directory")
    var workdir: String

    @Guide(description: "Files in workspace")
    var files: [FileInfo]

    @Guide(description: "Previews of key files")
    var filePreviews: [FilePreviewInfo]

    @Guide(description: "Memory limit in MB")
    var memoryLimitMB: Int?

    @Guide(description: "CPU count")
    var cpuCount: Int?
}

/// Environment-aware test generation result with categorized tests.
@Generable(description: "Environment-aware test suite")
struct EnvironmentAwareTestResult: Codable {
    @Guide(description: "Requirements extracted from task description")
    var descriptionRequirements: [String]

    @Guide(description: "Requirements inferred from environment")
    var environmentRequirements: [String]

    @Guide(description: "Anti-cheat tests verifying prohibited tools/patterns NOT used")
    var antiCheatTests: [GeneratedTest]

    @Guide(description: "File existence and structure tests")
    var existenceTests: [GeneratedTest]

    @Guide(description: "Correctness tests from description")
    var correctnessTests: [GeneratedTest]

    @Guide(description: "Boundary tests from environment constraints")
    var boundaryTests: [GeneratedTest]

    @Guide(description: "Integration tests for system-level behavior")
    var integrationTests: [GeneratedTest]

    @Guide(description: "Uncertainties and assumptions made")
    var uncertainties: [String]
}

// MARK: - Tool Call Schema

/// Tool call request from MAP orchestrator.
/// Constrains tool name to valid values only.
@Generable(description: "A tool call from the agent")
struct ToolCallRequest: Codable {
    @Guide(description: "Tool to call", .anyOf([
        "read_file",
        "write_file",
        "verify_progress"
    ]))
    var name: String

    @Guide(description: "Tool arguments as JSON object")
    var arguments: ToolArguments
}

/// Tool arguments for MAP orchestrator tools.
@Generable(description: "Tool arguments")
struct ToolArguments: Codable {
    @Guide(description: "File path (for read_file, write_file)")
    var path: String?

    @Guide(description: "File content (for write_file)")
    var content: String?
}

// Note: Dynamic JSON schema types removed to avoid recursive struct issues.
// Using pre-defined Generable types (TestGenerationResult) for guided generation.
// For custom schemas, use DynamicGenerationSchema from FoundationModels framework.

// MARK: - FRLM Tool Schemas

/// FRLM tool names for guided generation tool selection.
@Generable(description: "An FRLM tool for recursive LLM execution")
enum FrlmTool: String, Codable, CaseIterable {
    case llmQueryRecursive = "llm_query_recursive"
    case loadEnvironment = "load_environment"
    case selectFragments = "select_fragments"
    case executeParallel = "execute_parallel"
    case verifyResults = "verify_results"
    case checkBudget = "check_budget"
    case getTraceEvents = "get_trace_events"

    var description: String {
        switch self {
        case .llmQueryRecursive:
            return "Make a recursive sub-LM call"
        case .loadEnvironment:
            return "Load fragments into execution context"
        case .selectFragments:
            return "Select relevant fragments from loaded environment"
        case .executeParallel:
            return "Execute multiple sub-queries in parallel"
        case .verifyResults:
            return "Verify sub-query results using specified verification tier"
        case .checkBudget:
            return "Check remaining token budget or update allocation"
        case .getTraceEvents:
            return "Get execution trace events for debugging"
        }
    }
}

/// FRLM tool call request for guided generation.
@Generable(description: "A tool call request from FM to FRLM")
struct FrlmToolCall: Codable {
    @Guide(description: "The FRLM tool to call", .anyOf([
        "llm_query_recursive",
        "load_environment",
        "select_fragments",
        "execute_parallel",
        "verify_results",
        "check_budget",
        "get_trace_events"
    ]))
    var tool: String

    @Guide(description: "Tool arguments as JSON string")
    var arguments: String
}

/// Arguments for llm_query_recursive tool.
@Generable(description: "Arguments for recursive LLM query")
struct LlmQueryArgs: Codable {
    @Guide(description: "The prompt to send to the sub-LM")
    var prompt: String

    @Guide(description: "Optional context to include")
    var context: String?

    @Guide(description: "Maximum tokens for this sub-query")
    var budget: Int?

    @Guide(description: "Verification tier", .anyOf([
        "none", "redundancy", "objective", "validated"
    ]))
    var verification: String?
}

/// Arguments for load_environment tool.
@Generable(description: "Arguments for loading environment")
struct LoadEnvironmentArgs: Codable {
    @Guide(description: "Fragment ID to load")
    var fragmentId: String

    @Guide(description: "Fragment content")
    var content: String
}

/// Arguments for check_budget tool.
@Generable(description: "Arguments for budget management")
struct CheckBudgetArgs: Codable {
    @Guide(description: "Budget action", .anyOf([
        "check", "reserve", "release"
    ]))
    var action: String

    @Guide(description: "Number of tokens (for reserve/release)")
    var tokens: Int?
}

/// Result from tool selection indicating which tool to call and with what arguments.
@Generable(description: "Tool selection result from guided generation")
struct FrlmToolSelection: Codable {
    @Guide(description: "Selected tool name")
    var selectedTool: String

    @Guide(description: "Reasoning for tool selection")
    var reasoning: String

    @Guide(description: "Should we execute this tool?")
    var shouldExecute: Bool
}
