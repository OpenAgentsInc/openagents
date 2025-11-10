# Issue #10: Testing, Validation, and Documentation

**Phase:** 5 (Advanced) & 6 (Documentation)
**Priority:** P1
**Estimated Effort:** 2 days
**Dependencies:** All previous issues (#1-9)
**Related Issues:** None (final polishing phase)

---

## Summary

Comprehensive testing suite, Harmony compliance validation, performance benchmarking, user documentation, and ADR creation for GPTOSS 20B integration.

## Context

From `docs/gptoss/next-steps-20251110.md`:
- Must ensure tokenizer's chat template is always used (Harmony compliance)
- Add golden tests for chat template
- Persist manifest with checksums
- Log routing decisions for tuning
- Add explicit license acknowledgement

## Acceptance Criteria

### Testing
- [ ] Unit tests for all core components (≥80% coverage)
- [ ] Integration tests for delegation flow (FM → GPTOSS)
- [ ] Harmony compliance tests (chat template validation)
- [ ] Golden test corpus (50+ prompts with expected characteristics)
- [ ] Performance benchmarks (latency, throughput, memory)
- [ ] Regression tests (prevent quality degradation)
- [ ] License acknowledgement flow tested

### Documentation
- [ ] User guide (when to use GPTOSS, how it works)
- [ ] Settings documentation
- [ ] Troubleshooting guide
- [ ] ADR-0010 (GPTOSS Integration Architecture)
- [ ] API documentation (DocC comments)
- [ ] Harmony compliance documentation

### Validation
- [ ] Model manifest with checksums persisted
- [ ] Download integrity verification
- [ ] All tests pass on macOS (M1, M2, M3)
- [ ] Ready for beta release

## Technical Implementation

### 1. Harmony Compliance Tests

**File:** `ios/OpenAgentsCoreTests/GPTOSS/HarmonyComplianceTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

/// Tests to ensure Harmony chat template is always used correctly
///
/// GPT-OSS 20B requires Harmony format via tokenizer's chat template.
/// These tests verify we never bypass the template.
final class HarmonyComplianceTests: XCTestCase {

    func testTokenizerChatTemplateUsed() async throws {
        #if os(macOS)
        // Load tokenizer
        let manager = GPTOSSModelManager()
        try await manager.loadModel()

        // Verify tokenizer has chat template
        // (Implementation depends on MLX Tokenizers API)

        // Golden test: known input → expected token IDs prefix
        let messages = [
            ChatMessage(role: "system", content: "You are a helpful assistant."),
            ChatMessage(role: "user", content: "Hello!")
        ]

        let tokenizedPrompt = try await manager.applyChatTemplate(messages)

        // Expected prefix (from Harmony spec)
        let expectedPrefix = [/* token IDs from Harmony template */]

        XCTAssertTrue(
            tokenizedPrompt.starts(with: expectedPrefix),
            "Chat template must produce Harmony format"
        )
        #endif
    }

    func testNeverBypassChatTemplate() {
        // Code review test: grep for hand-rolled prompts
        // In actual implementation, enforce via API design:
        // - Only expose applyChatTemplate() method
        // - Make raw tokenizer.encode() private
    }

    func testMultiTurnConversation() async throws {
        #if os(macOS)
        let manager = GPTOSSModelManager()
        try await manager.loadModel()

        let messages = [
            ChatMessage(role: "system", content: "You are a coding assistant."),
            ChatMessage(role: "user", content: "Write a function"),
            ChatMessage(role: "assistant", content: "func hello() { print(\"Hello\") }"),
            ChatMessage(role: "user", content: "Now add error handling")
        ]

        let prompt = try await manager.applyChatTemplate(messages)

        // Verify Harmony format for multi-turn
        // (Check for proper role markers, separators)
        XCTAssertTrue(prompt.contains("user"), "Should have user role marker")
        XCTAssertTrue(prompt.contains("assistant"), "Should have assistant role marker")
        #endif
    }
}
```

### 2. Golden Test Corpus

**File:** `ios/OpenAgentsCoreTests/GPTOSS/GoldenTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

/// Golden tests with expected output characteristics
///
/// Tests quality and consistency of GPTOSS generation.
final class GPTOSSGoldenTests: XCTestCase {

    struct GoldenTest {
        var name: String
        var prompt: String
        var expectedCharacteristics: [Characteristic]

        enum Characteristic {
            case containsKeyword(String)
            case lineCountRange(ClosedRange<Int>)
            case validSwiftSyntax
            case includesDocumentation
            case handlesEdgeCases
        }
    }

    let goldenTests: [GoldenTest] = [
        GoldenTest(
            name: "Simple function generation",
            prompt: "Write a Swift function that calculates factorial",
            expectedCharacteristics: [
                .containsKeyword("func"),
                .containsKeyword("factorial"),
                .lineCountRange(5...30),
                .validSwiftSyntax,
                .handlesEdgeCases
            ]
        ),
        GoldenTest(
            name: "Actor with state management",
            prompt: "Create a Swift actor for managing user sessions with thread-safe access",
            expectedCharacteristics: [
                .containsKeyword("actor"),
                .containsKeyword("Session"),
                .lineCountRange(20...80),
                .validSwiftSyntax
            ]
        ),
        GoldenTest(
            name: "Documentation generation",
            prompt: "Write comprehensive API documentation for a WebSocket client class",
            expectedCharacteristics: [
                .includesDocumentation,
                .containsKeyword("WebSocket"),
                .lineCountRange(30...200)
            ]
        ),
        // Add 47 more golden tests...
    ]

    func testGoldenCorpus() async throws {
        #if os(macOS)
        let provider = GPTOSSAgentProvider()
        guard await provider.isAvailable() else {
            throw XCTSkip("GPTOSS not available on this system")
        }

        for test in goldenTests {
            print("[Golden Test] \(test.name)")

            let updateHub = MockSessionUpdateHub()
            let sessionId = ACPSessionId(value: "golden-\(test.name)")

            _ = try await provider.start(
                sessionId: sessionId,
                prompt: test.prompt,
                context: AgentContext(...),
                updateHub: updateHub
            )

            let output = await collectOutput(updateHub, sessionId)

            // Verify characteristics
            for characteristic in test.expectedCharacteristics {
                try verifyCharacteristic(output, characteristic)
            }
        }
        #endif
    }

    private func verifyCharacteristic(_ output: String, _ characteristic: GoldenTest.Characteristic) throws {
        switch characteristic {
        case .containsKeyword(let keyword):
            XCTAssertTrue(output.contains(keyword), "Output should contain '\(keyword)'")

        case .lineCountRange(let range):
            let lines = output.components(separatedBy: .newlines)
            XCTAssertTrue(range.contains(lines.count), "Line count \(lines.count) should be in range \(range)")

        case .validSwiftSyntax:
            // Run through Swift parser (swift-syntax library)
            // Or write to temp file and run `swiftc -parse`
            break

        case .includesDocumentation:
            XCTAssertTrue(output.contains("///") || output.contains("/**"), "Should include documentation comments")

        case .handlesEdgeCases:
            // Check for conditional handling, nil checks, etc.
            XCTAssertTrue(
                output.contains("guard") || output.contains("if") || output.contains("?"),
                "Should handle edge cases"
            )
        }
    }
}
```

### 3. Model Manifest with Checksums

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSManifest.swift`

```swift
import Foundation
import CryptoKit

/// Manifest for GPTOSS model files with integrity verification
public struct GPTOSSManifest: Codable {
    public var modelID: String
    public var version: String
    public var files: [FileEntry]
    public var createdAt: Date

    public struct FileEntry: Codable {
        public var name: String
        public var sizeBytes: Int64
        public var sha256: String
    }

    /// Verify all files match expected checksums
    public func verify(at modelDir: URL) throws -> VerificationResult {
        var results: [FileVerification] = []

        for file in files {
            let fileURL = modelDir.appendingPathComponent(file.name)

            guard FileManager.default.fileExists(atPath: fileURL.path) else {
                results.append(FileVerification(
                    name: file.name,
                    status: .missing
                ))
                continue
            }

            // Check size
            let attributes = try FileManager.default.attributesOfItem(atPath: fileURL.path)
            let actualSize = attributes[.size] as? Int64 ?? 0

            guard actualSize == file.sizeBytes else {
                results.append(FileVerification(
                    name: file.name,
                    status: .sizeMismatch(expected: file.sizeBytes, actual: actualSize)
                ))
                continue
            }

            // Check SHA256
            let data = try Data(contentsOf: fileURL)
            let hash = SHA256.hash(data: data)
            let hashString = hash.compactMap { String(format: "%02x", $0) }.joined()

            guard hashString == file.sha256 else {
                results.append(FileVerification(
                    name: file.name,
                    status: .checksumMismatch(expected: file.sha256, actual: hashString)
                ))
                continue
            }

            results.append(FileVerification(name: file.name, status: .valid))
        }

        return VerificationResult(files: results)
    }

    public struct FileVerification {
        public var name: String
        public var status: Status

        public enum Status {
            case valid
            case missing
            case sizeMismatch(expected: Int64, actual: Int64)
            case checksumMismatch(expected: String, actual: String)
        }
    }

    public struct VerificationResult {
        public var files: [FileVerification]

        public var isValid: Bool {
            files.allSatisfy { $0.status == .valid }
        }

        public var errors: [String] {
            files.compactMap { verification in
                switch verification.status {
                case .valid:
                    return nil
                case .missing:
                    return "\(verification.name): Missing"
                case .sizeMismatch(let exp, let act):
                    return "\(verification.name): Size mismatch (expected \(exp), got \(act))"
                case .checksumMismatch:
                    return "\(verification.name): Checksum mismatch"
                }
            }
        }
    }

    /// Create manifest from directory
    public static func create(modelID: String, modelDir: URL) throws -> GPTOSSManifest {
        let fileManager = FileManager.default
        let files = try fileManager.contentsOfDirectory(at: modelDir, includingPropertiesForKeys: [.fileSizeKey])

        let entries: [FileEntry] = try files.map { fileURL in
            let attributes = try fileManager.attributesOfItem(atPath: fileURL.path)
            let size = attributes[.size] as? Int64 ?? 0

            let data = try Data(contentsOf: fileURL)
            let hash = SHA256.hash(data: data)
            let hashString = hash.compactMap { String(format: "%02x", $0) }.joined()

            return FileEntry(
                name: fileURL.lastPathComponent,
                sizeBytes: size,
                sha256: hashString
            )
        }

        return GPTOSSManifest(
            modelID: modelID,
            version: "1.0",
            files: entries,
            createdAt: Date()
        )
    }
}
```

### 4. License Acknowledgement

**File:** `ios/OpenAgents/Views/macOS/Settings/GPTOSSLicenseView.swift`

```swift
import SwiftUI

struct GPTOSSLicenseView: View {
    @Binding var isPresented: Bool
    @Binding var accepted: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("GPTOSS 20B License Agreement")
                .font(.title)

            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Model: GPT-OSS 20B")
                        .font(.headline)

                    Text("License: Apache 2.0")
                        .font(.subheadline)

                    Text("""
                    By downloading and using this model, you agree to:

                    1. The model is provided "as is" without warranty
                    2. The model may generate incorrect or inappropriate content
                    3. You are responsible for reviewing and validating generated code
                    4. The model is for local use only (not for redistribution)
                    5. Usage is subject to the Apache 2.0 license terms

                    Full license: https://huggingface.co/openai/gpt-oss-20b
                    """)
                    .font(.body)
                }
                .padding()
            }

            HStack {
                Button("Decline") {
                    accepted = false
                    isPresented = false
                }
                .buttonStyle(.bordered)

                Spacer()

                Button("Accept and Download") {
                    accepted = true
                    isPresented = false
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding()
        .frame(width: 600, height: 500)
    }
}
```

### 5. User Documentation

**File:** `docs/gptoss/user-guide.md`

```markdown
# GPTOSS 20B User Guide

## What is GPTOSS?

GPTOSS 20B is a powerful open-source language model that runs locally on your Mac. It provides production-quality code generation and complex reasoning without API costs or network dependency.

## When to Use GPTOSS

### Use GPTOSS for:
- **Code Generation**: Functions, classes, modules
- **Documentation**: README files, API docs, tutorials
- **Refactoring**: Multi-file code improvements
- **Analysis**: Codebase understanding, architecture review
- **Long-form Content**: Guides, explanations (>200 tokens)

### Use OpenAgents (default) for:
- Quick questions ("who are you?", "what can you do?")
- Conversation titles and summaries
- Simple explanations (<140 tokens)

### Use External Agents for:
- **Codex**: When explicitly requested or GPTOSS unavailable
- **Claude Code**: Advanced analysis requiring Claude-specific features

## System Requirements

### Minimum:
- macOS 13.0+ (Ventura)
- Apple Silicon (M1 or later)
- 16 GB unified memory
- 25 GB free disk space

### Recommended:
- M2 Pro/Max, M3, or M4
- 24 GB+ memory
- 50 GB+ free disk space

## Installation

1. Open Settings → Agents → GPTOSS
2. Review license agreement
3. Click "Download Model" (12.1 GB download)
4. Wait for download and verification (~10-30 minutes)
5. Click "Load Model" when ready

## Usage

### Automatic Mode (Recommended)

OpenAgents automatically routes tasks to the best agent:

```
You: Generate a Swift actor for managing WebSocket connections

OpenAgents will automatically use GPTOSS for this code generation task.
```

### Manual Selection

Select GPTOSS from the agent dropdown:

1. Click agent selector (top of chat)
2. Choose "GPTOSS 20B"
3. Type your prompt
4. Watch tokens stream in real-time

## Performance

### Expected Performance:
- **First Token**: 1-3 seconds
- **Throughput**: 10-30 tokens/second (varies by chip)
- **Memory Usage**: 14-17 GB while loaded

### Tips for Best Performance:
- Close memory-intensive apps
- Use 24 GB+ Mac for long contexts
- Enable auto-unload in Settings (frees memory when idle)

## Troubleshooting

See [troubleshooting.md](./troubleshooting.md) for common issues.

## License

GPTOSS 20B is licensed under Apache 2.0. See model card for details.
```

### 6. ADR-0010

**File:** `docs/adr/ADR-0010-gptoss-integration.md`

```markdown
# ADR-0010: GPTOSS 20B Integration Architecture

**Date:** 2025-11-10
**Status:** Accepted
**Deciders:** OpenAgents Core Team

## Context

OpenAgents needs a local LLM capability for code generation and complex reasoning that doesn't require external APIs or network connectivity. We evaluated several options for bringing production-quality on-device intelligence to macOS.

## Decision

We will integrate GPT-OSS 20B via MLX Swift as a native agent provider, following these principles:

1. **Native Swift Provider**: Implement as `GPTOSSAgentProvider` (actor-based, not CLI)
2. **Task-Appropriate Allocation**: Foundation Models for routing, GPTOSS for generation
3. **MLX Framework**: Use Apple's MLX for optimized Apple Silicon inference
4. **Hugging Face Hub**: Download via Hub.snapshot for resumable, verifiable installs
5. **Harmony Compliance**: Always use tokenizer's chat template (never bypass)
6. **Memory Management**: Proactive checks, idle unload, pressure monitoring
7. **macOS-Only**: Too large for iOS (16 GB+ requirement)

## Alternatives Considered

### Alternative 1: CoreML Conversion
**Rejected**: CoreML conversion for 20B model is complex, lacks streaming, and has limited MLX precedent.

### Alternative 2: Cloud-Only (OpenAI/Anthropic APIs)
**Rejected**: Requires network, costs money, compromises privacy, doesn't meet "local-first" goal.

### Alternative 3: Smaller Model (7B)
**Considered**: Could support iOS, but 20B provides significantly better code quality. We may add 7B later for iPad Pro.

### Alternative 4: CLI Wrapper (like Codex)
**Rejected**: Native Swift provider is faster, more integrated, easier to manage lifecycle.

## Consequences

### Positive:
- No API costs or network dependency
- Full privacy (code stays on device)
- Fast inference (~20 tok/sec on M2+)
- Integrated with OpenAgents architecture
- Easy to swap models later

### Negative:
- macOS-only (no iOS support)
- Large download (12.1 GB)
- Requires 16 GB+ memory
- Complexity of managing model lifecycle

### Risks:
- Memory pressure on 16 GB Macs → Mitigated by preflight checks, auto-unload
- Download failures → Mitigated by Hub.snapshot resume support
- Quality issues → Mitigated by golden tests, user feedback

## Implementation

See `docs/gptoss/gptoss-integration-spec.md` for full specification.

Key files:
- `GPTOSSAgentProvider.swift`: Main provider
- `GPTOSSModelManager.swift`: Lifecycle management
- `GPTOSSMemoryManager.swift`: Memory monitoring
- Integration with `OpenAgentsLocalProvider` via `gptoss.generate` tool

## References

- [GPTOSS Research](../gptoss/research.md)
- [Integration Spec](../gptoss/gptoss-integration-spec.md)
- [Next Steps](../gptoss/next-steps-20251110.md)
- MLX Swift Examples: https://github.com/ml-explore/mlx-swift-examples
- GPT-OSS Model Card: https://huggingface.co/openai/gpt-oss-20b
```

## Testing Checklist

- [ ] Unit tests (≥80% coverage)
- [ ] Integration tests (delegation flow)
- [ ] Harmony compliance tests
- [ ] Golden test corpus (50+ prompts)
- [ ] Performance benchmarks
- [ ] Memory profiling
- [ ] License acknowledgement flow
- [ ] Manifest verification
- [ ] All tests pass on M1, M2, M3 Macs

## Documentation Checklist

- [ ] User guide written
- [ ] Settings documentation
- [ ] Troubleshooting guide
- [ ] ADR-0010 created
- [ ] API documentation (DocC comments)
- [ ] Harmony compliance documented
- [ ] Release notes prepared

## References

- docs/gptoss/next-steps-20251110.md
- Integration Spec Section 8
- ADR guidelines: docs/adr/AGENTS.md

## Definition of Done

- [ ] All tests pass
- [ ] Documentation complete
- [ ] ADR-0010 merged
- [ ] License flow tested
- [ ] Manifest verification works
- [ ] Ready for beta release
- [ ] Committed with message: "Add comprehensive testing and documentation for GPTOSS"
