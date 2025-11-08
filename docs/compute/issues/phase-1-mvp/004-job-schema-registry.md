# Job Schema Registry (NIP-90 Extension)

**Phase:** 1 - MVP
**Component:** OpenAgentsCore (Shared) + Documentation
**Priority:** P1 (High - Enables marketplace standardization)
**Estimated Effort:** 1-2 weeks

## Summary

Define and publish an open job schema registry that maps common agent tasks to NIP-90 event kinds with standardized parameter and result schemas. This registry enables interoperability between OpenAgents clients and third-party clients/providers on the compute marketplace.

## Motivation

NIP-90 (Data Vending Machine) defines a framework for job requests but doesn't standardize specific job types or parameter schemas. Without a shared registry:

- **Fragmentation**: Each client invents its own job kinds and parameter names
- **No interoperability**: OpenAgents providers can't fulfill jobs from other clients
- **Poor UX**: Buyers don't know what parameters a provider supports
- **No versioning**: Schema changes break compatibility

A centralized (but open) job schema registry solves this by:

- **Standardizing common tasks**: Text summarization, code generation, image analysis, etc.
- **Defining parameter schemas**: Required/optional params, types, validation
- **Enabling discovery**: Providers advertise supported job kinds; clients browse available services
- **Versioning**: Schema evolution without breaking changes

## Acceptance Criteria

### Job Kind Definitions

#### Official DVM Kinds (NIP-90 Standard)

OpenAgents will support the following **official DVM kinds** (see `docs/nostr/dvm-kinds/` for full specifications):

**Text Processing (50xx range):**
- [ ] `kind:5000` - Text Extraction (extract text from media/documents)
- [ ] `kind:5001` - Text Summarization (summarize text inputs)
- [ ] `kind:5002` - Text Translation (translate to target language)
- [ ] `kind:5050` - Text Generation (AI text generation with prompts)

**Media Generation (51xx range):**
- [ ] `kind:5100` - Image Generation (AI image generation from prompts)

**Video/Audio (52xx range):**
- [ ] `kind:5200` - Video Conversion
- [ ] `kind:5201` - Video Translation
- [ ] `kind:5250` - Text-to-Speech

**Discovery (53xx range):**
- [ ] `kind:5300` - Nostr Content Discovery
- [ ] `kind:5301` - Nostr People Discovery

#### OpenAgents Custom Kinds (65xx range)

For agent-specific tasks not covered by official DVM kinds, we define **custom OpenAgents kinds** in the `65xx` range to avoid conflicts:

- [ ] `kind:6500` - Code Generation (generate code from natural language)
- [ ] `kind:6501` - Code Review (analyze and review code)
- [ ] `kind:6502` - Code Refactoring (refactor code with improvements)
- [ ] `kind:6503` - Q&A / RAG (question answering with retrieval augmentation)
- [ ] `kind:6504` - Codebase Search (hybrid search - future SearchKit integration)
- [ ] `kind:6505` - Agent Execution (autonomous agent task execution)
- [ ] `kind:6506` - Code Explanation (explain code functionality)
- [ ] `kind:6507` - Test Generation (generate unit/integration tests)

**Rationale for custom range:**
- Official DVM kinds (5000-5999) are reserved for standardized, cross-platform tasks
- OpenAgents custom kinds (6500-6599) are for agent/coding-specific workflows
- Using 65xx avoids conflicts with official result kinds (6000-6999 per NIP-90)
- See `docs/nostr/dvm-ranges/` for official kind allocations

- [ ] Each kind has:
  - Human-readable name and description
  - Input schema (param tags)
  - Output schema (result format)
  - Example request/result
  - Version number
  - Reference to official spec (if applicable)

### Parameter Schema Format
- [ ] Use JSON Schema for validation
- [ ] Support common types: string, number, boolean, array, object
- [ ] Required vs optional params
- [ ] Default values
- [ ] Validation rules (min/max, regex, enum)
- [ ] Param encoding in NIP-90 `param` tags

### Result Schema Format
- [ ] Define result structure (JSON or plain text)
- [ ] Success/error response format
- [ ] Optional metadata (model used, tokens, latency)
- [ ] Streaming result format (chunked responses)

### Registry Format
- [ ] Machine-readable JSON file (job-schema-registry.json)
- [ ] Human-readable markdown docs (docs/compute/job-schemas/)
- [ ] Versioning scheme (semver: major.minor.patch)
- [ ] Changelog for schema updates

### Provider Capability Advertising
- [ ] Extend NIP-89 (application-specific data) for job kinds
- [ ] Providers advertise supported kinds, version, pricing
- [ ] Capability filters (model, max_tokens, languages, etc.)

### Client Integration
- [ ] Swift types for all job schemas
- [ ] Validation helpers (param validation, result parsing)
- [ ] Builder pattern for job creation
- [ ] Error handling for schema mismatches

## Technical Design

### Registry Structure

```json
// job-schema-registry.json

{
  "version": "0.1.0",
  "updated": "2025-11-08T00:00:00Z",
  "official_dvm_spec": "docs/nostr/dvm-kinds/",
  "kinds": {
    "5001": {
      "name": "text-summarization",
      "displayName": "Text Summarization",
      "description": "Summarize text content to a specified length (Official DVM kind)",
      "official_spec": "docs/nostr/dvm-kinds/5001.md",
      "input": {
        "types": ["text", "event", "job"],
        "params": {
          "length": {
            "type": "string",
            "description": "Desired length in words or paragraphs (e.g., '10 paragraphs', '100 words')",
            "default": "100 words"
          }
        },
        "required": []
      },
      "output": {
        "format": "text/plain",
        "schema": {
          "type": "object",
          "properties": {
            "summary": { "type": "string" },
            "original_length": { "type": "integer" },
            "summary_length": { "type": "integer" },
            "compression_ratio": { "type": "number" }
          },
          "required": ["summary"]
        }
      },
      "pricing": {
        "typical_range_msats": [100, 10000],
        "factors": ["input_length", "max_length", "model"]
      },
      "examples": [
        {
          "request": {
            "kind": 5001,
            "content": "",
            "tags": [
              ["i", "<event_id>", "event"],
              ["param", "length", "10 paragraphs"]
            ]
          },
          "result": {
            "kind": 6001,
            "content": "Summary of the content in 10 paragraphs...",
            "tags": [
              ["request", "<event_id>"],
              ["e", "<request_event_id>", "wss://relay.example.com"]
            ]
          }
        }
      ],
      "version": "0.1.0"
    },
    "6500": {
      "name": "code-generation",
      "displayName": "Code Generation (OpenAgents)",
      "description": "Generate code from natural language description (OpenAgents custom kind)",
      "custom_kind": true,
      "input": {
        "types": ["text"],
        "params": {
          "language": {
            "type": "string",
            "description": "Programming language",
            "enum": ["swift", "python", "javascript", "typescript", "rust", "go"],
            "required": true
          },
          "context": {
            "type": "string",
            "description": "Additional context or requirements"
          },
          "max_tokens": {
            "type": "integer",
            "default": 1000,
            "min": 100,
            "max": 8000
          },
          "include_tests": {
            "type": "boolean",
            "default": false
          }
        },
        "required": ["language"]
      },
      "output": {
        "format": "text/plain",
        "schema": {
          "type": "object",
          "properties": {
            "code": { "type": "string" },
            "language": { "type": "string" },
            "explanation": { "type": "string" },
            "tests": { "type": "string" }
          },
          "required": ["code", "language"]
        }
      },
      "examples": [
        {
          "request": {
            "kind": 6500,
            "content": "",
            "tags": [
              ["i", "Write a function to check if a number is prime", "text"],
              ["param", "language", "swift"],
              ["param", "include_tests", "true"]
            ]
          },
          "result": {
            "kind": 7500,
            "content": "{\"code\": \"func isPrime(_ n: Int) -> Bool { ... }\", \"language\": \"swift\", \"tests\": \"func testIsPrime() { ... }\"}",
            "tags": [
              ["request", "<event_id>"],
              ["e", "<request_event_id>", "wss://relay.example.com"]
            ]
          }
        }
      ],
      "version": "0.1.0"
    }
  }
}

// Note: Result kinds follow NIP-90 convention:
// - Official DVM results: request_kind + 1000 (e.g., 5001 -> 6001)
// - OpenAgents custom results: request_kind + 1000 (e.g., 6500 -> 7500)
```

### Swift Types

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/JobSchemas/

JobSchema.swift              // Schema types
JobKind.swift                // Job kind enum
JobSchemaRegistry.swift      // Registry loader and validator
JobBuilder.swift             // Job creation helpers
```

```swift
// JobKind.swift

/// Job kinds combining official DVM kinds and OpenAgents custom kinds
///
/// Official DVM kinds (5000-5999) follow the standard from docs/nostr/dvm-kinds/
/// Custom OpenAgents kinds (6500-6599) are for agent/coding-specific workflows
public enum JobKind: Int, Codable, CaseIterable {
    // MARK: - Official DVM Kinds (Text Processing - 50xx)

    /// Text Extraction - Extract text from media/documents (Official DVM)
    case textExtraction = 5000

    /// Text Summarization - Summarize text inputs (Official DVM)
    case textSummarization = 5001

    /// Text Translation - Translate to target language (Official DVM)
    case textTranslation = 5002

    /// Text Generation - AI text generation with prompts (Official DVM)
    case textGeneration = 5050

    // MARK: - Official DVM Kinds (Media Generation - 51xx)

    /// Image Generation - AI image generation from prompts (Official DVM)
    case imageGeneration = 5100

    // MARK: - Official DVM Kinds (Video/Audio - 52xx)

    /// Video Conversion (Official DVM)
    case videoConversion = 5200

    /// Video Translation (Official DVM)
    case videoTranslation = 5201

    /// Text-to-Speech (Official DVM)
    case textToSpeech = 5250

    // MARK: - Official DVM Kinds (Discovery - 53xx)

    /// Nostr Content Discovery (Official DVM)
    case nostrContentDiscovery = 5300

    /// Nostr People Discovery (Official DVM)
    case nostrPeopleDiscovery = 5301

    // MARK: - OpenAgents Custom Kinds (65xx)

    /// Code Generation - Generate code from natural language (OpenAgents custom)
    case codeGeneration = 6500

    /// Code Review - Analyze and review code (OpenAgents custom)
    case codeReview = 6501

    /// Code Refactoring - Refactor code with improvements (OpenAgents custom)
    case codeRefactoring = 6502

    /// Q&A / RAG - Question answering with retrieval augmentation (OpenAgents custom)
    case qaRag = 6503

    /// Codebase Search - Hybrid search (OpenAgents custom, future SearchKit)
    case codebaseSearch = 6504

    /// Agent Execution - Autonomous agent task execution (OpenAgents custom)
    case agentExecution = 6505

    /// Code Explanation - Explain code functionality (OpenAgents custom)
    case codeExplanation = 6506

    /// Test Generation - Generate unit/integration tests (OpenAgents custom)
    case testGeneration = 6507

    // MARK: - Computed Properties

    /// Result kind (add 1000 to request kind per NIP-90)
    public var resultKind: Int {
        rawValue + 1000
    }

    /// Whether this is an official DVM kind
    public var isOfficialDVMKind: Bool {
        rawValue >= 5000 && rawValue < 6000
    }

    /// Whether this is a custom OpenAgents kind
    public var isCustomKind: Bool {
        rawValue >= 6500 && rawValue < 6600
    }

    /// Machine-readable name
    public var name: String {
        switch self {
        // Official DVM kinds
        case .textExtraction: return "text-extraction"
        case .textSummarization: return "text-summarization"
        case .textTranslation: return "text-translation"
        case .textGeneration: return "text-generation"
        case .imageGeneration: return "image-generation"
        case .videoConversion: return "video-conversion"
        case .videoTranslation: return "video-translation"
        case .textToSpeech: return "text-to-speech"
        case .nostrContentDiscovery: return "nostr-content-discovery"
        case .nostrPeopleDiscovery: return "nostr-people-discovery"
        // OpenAgents custom kinds
        case .codeGeneration: return "code-generation"
        case .codeReview: return "code-review"
        case .codeRefactoring: return "code-refactoring"
        case .qaRag: return "qa-rag"
        case .codebaseSearch: return "codebase-search"
        case .agentExecution: return "agent-execution"
        case .codeExplanation: return "code-explanation"
        case .testGeneration: return "test-generation"
        }
    }

    /// Human-readable display name
    public var displayName: String {
        switch self {
        // Official DVM kinds
        case .textExtraction: return "Text Extraction"
        case .textSummarization: return "Text Summarization"
        case .textTranslation: return "Text Translation"
        case .textGeneration: return "Text Generation"
        case .imageGeneration: return "Image Generation"
        case .videoConversion: return "Video Conversion"
        case .videoTranslation: return "Video Translation"
        case .textToSpeech: return "Text-to-Speech"
        case .nostrContentDiscovery: return "Nostr Content Discovery"
        case .nostrPeopleDiscovery: return "Nostr People Discovery"
        // OpenAgents custom kinds
        case .codeGeneration: return "Code Generation"
        case .codeReview: return "Code Review"
        case .codeRefactoring: return "Code Refactoring"
        case .qaRag: return "Q&A / RAG"
        case .codebaseSearch: return "Codebase Search"
        case .agentExecution: return "Agent Execution"
        case .codeExplanation: return "Code Explanation"
        case .testGeneration: return "Test Generation"
        }
    }

    /// Reference to official spec (if applicable)
    public var officialSpec: String? {
        guard isOfficialDVMKind else { return nil }
        return "docs/nostr/dvm-kinds/\(rawValue).md"
    }
}
```

```swift
// JobSchema.swift

import Foundation

/// Job schema definition
public struct JobSchema: Codable {
    public let kind: JobKind
    public let name: String
    public let displayName: String
    public let description: String
    public let input: InputSchema
    public let output: OutputSchema
    public let pricing: PricingGuidance?
    public let examples: [JobExample]
    public let version: String

    public struct InputSchema: Codable {
        public let types: [InputType]  // text, url, event, job
        public let params: [String: ParamSchema]
        public let required: [String]

        public enum InputType: String, Codable {
            case text, url, event, job
        }
    }

    public struct ParamSchema: Codable {
        public let type: ParamType
        public let description: String
        public let defaultValue: AnyCodable?
        public let required: Bool
        public let validation: ValidationRules?

        public enum ParamType: String, Codable {
            case string, integer, number, boolean, array, object
        }

        public struct ValidationRules: Codable {
            public let min: Double?
            public let max: Double?
            public let regex: String?
            public let enum: [String]?
        }
    }

    public struct OutputSchema: Codable {
        public let format: String  // MIME type
        public let schema: [String: Any]?  // JSON Schema
    }

    public struct PricingGuidance: Codable {
        public let typicalRangeMsats: [Int64]  // [min, max]
        public let factors: [String]  // ["input_length", "model"]
    }

    public struct JobExample: Codable {
        public let request: ExampleRequest
        public let result: ExampleResult

        public struct ExampleRequest: Codable {
            public let kind: Int
            public let tags: [[String]]
        }

        public struct ExampleResult: Codable {
            public let kind: Int
            public let tags: [[String]]
            public let content: String
        }
    }

    /// Validate parameters against schema
    public func validate(params: [String: Any]) throws

    /// Get required parameters
    public func requiredParams() -> [String]

    /// Get parameter schema
    public func param(_ name: String) -> ParamSchema?
}

// Type-erased Codable wrapper for default values
public struct AnyCodable: Codable {
    public let value: Any

    public init(_ value: Any) {
        self.value = value
    }

    // Codable implementation (encode/decode Any)
}
```

```swift
// JobSchemaRegistry.swift

import Foundation

/// Job schema registry
public class JobSchemaRegistry {
    public static let shared = JobSchemaRegistry()

    private var schemas: [JobKind: JobSchema] = [:]
    private let version: String

    private init() {
        // Load schemas from embedded JSON
        guard let url = Bundle.module.url(forResource: "job-schema-registry", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let registry = try? JSONDecoder().decode(RegistryJSON.self, from: data) else {
            fatalError("Failed to load job schema registry")
        }

        self.version = registry.version
        self.schemas = registry.kinds.mapValues { JobSchema(from: $0) }
    }

    private struct RegistryJSON: Codable {
        let version: String
        let updated: String
        let kinds: [String: JobSchemaJSON]
    }

    private struct JobSchemaJSON: Codable {
        // Mirror JobSchema structure for JSON
    }

    /// Get schema for job kind
    public func schema(for kind: JobKind) -> JobSchema? {
        schemas[kind]
    }

    /// Get all schemas
    public func allSchemas() -> [JobSchema] {
        Array(schemas.values)
    }

    /// Validate job request against schema
    public func validate(kind: JobKind, params: [String: Any]) throws {
        guard let schema = schemas[kind] else {
            throw JobSchemaError.unknownKind(kind)
        }
        try schema.validate(params: params)
    }

    /// Registry version
    public var registryVersion: String {
        version
    }
}

public enum JobSchemaError: Error, LocalizedError {
    case unknownKind(JobKind)
    case validationFailed(param: String, reason: String)
    case missingRequiredParam(String)
    case invalidParamType(param: String, expected: String, actual: String)

    public var errorDescription: String? {
        switch self {
        case .unknownKind(let kind):
            return "Unknown job kind: \(kind.rawValue)"
        case .validationFailed(let param, let reason):
            return "Validation failed for param '\(param)': \(reason)"
        case .missingRequiredParam(let param):
            return "Missing required parameter: \(param)"
        case .invalidParamType(let param, let expected, let actual):
            return "Invalid type for param '\(param)': expected \(expected), got \(actual)"
        }
    }
}
```

```swift
// JobBuilder.swift

import Foundation

/// Job request builder
public struct JobBuilder {
    private let kind: JobKind
    private var inputs: [Input] = []
    private var params: [String: String] = [:]
    private var bid: Int64?
    private var output: String?
    private var relays: [String] = []
    private var encrypt: Bool = false

    public enum Input {
        case text(String)
        case url(String)
        case event(String)
        case job(String)
    }

    public init(kind: JobKind) {
        self.kind = kind
    }

    public mutating func input(_ input: Input) -> Self {
        inputs.append(input)
        return self
    }

    public mutating func param(_ name: String, _ value: String) -> Self {
        params[name] = value
        return self
    }

    public mutating func bid(_ msats: Int64) -> Self {
        self.bid = msats
        return self
    }

    public mutating func output(_ mimeType: String) -> Self {
        self.output = mimeType
        return self
    }

    public mutating func relays(_ urls: [String]) -> Self {
        self.relays = urls
        return self
    }

    public mutating func encrypted(_ encrypt: Bool = true) -> Self {
        self.encrypt = encrypt
        return self
    }

    /// Build Nostr event (NIP-90 job request)
    public func build(
        privateKey: String,
        recipientPubkey: String? = nil
    ) throws -> NostrEvent {
        // Validate params against schema
        if let schema = JobSchemaRegistry.shared.schema(for: kind) {
            let paramDict = params.mapValues { $0 as Any }
            try schema.validate(params: paramDict)
        }

        // Build tags
        var tags: [[String]] = []

        // Input tags
        for input in inputs {
            switch input {
            case .text(let content):
                tags.append(["i", content, "text"])
            case .url(let url):
                tags.append(["i", url, "url"])
            case .event(let id):
                tags.append(["i", id, "event"])
            case .job(let id):
                tags.append(["i", id, "job"])
            }
        }

        // Param tags
        for (name, value) in params {
            tags.append(["param", name, value])
        }

        // Bid tag
        if let bid = bid {
            tags.append(["bid", "\(bid)"])
        }

        // Output tag
        if let output = output {
            tags.append(["output", output])
        }

        // Relays tag
        if !relays.isEmpty {
            tags.append(["relays"] + relays)
        }

        // Encrypt params if requested
        let content: String
        if encrypt, let recipientPubkey = recipientPubkey {
            // Serialize params as JSON and encrypt (NIP-04)
            let paramsJSON = try JSONSerialization.data(withJSONObject: params)
            content = try NostrEncryption.encrypt(
                content: String(data: paramsJSON, encoding: .utf8)!,
                privateKey: privateKey,
                recipientPubkey: recipientPubkey
            )
        } else {
            content = ""
        }

        // Create and sign event
        return try NostrEvent.sign(
            privateKey: privateKey,
            created_at: Int64(Date().timeIntervalSince1970),
            kind: kind.rawValue,
            tags: tags,
            content: content
        )
    }
}
```

### Capability Advertising (NIP-89 Extension)

```swift
// ProviderCapability.swift

/// Provider capability advertisement (NIP-89 extension)
public struct ProviderCapability: Codable {
    public let jobKind: JobKind
    public let version: String
    public let pricing: PricingModel
    public let limits: Limits?
    public let features: [String: String]?  // Model, max_tokens, etc.

    public struct PricingModel: Codable {
        public let basePrice: Int64  // msats
        public let perUnitPrice: Int64?  // e.g., per token
        public let unit: String?  // e.g., "token", "word", "image"
    }

    public struct Limits: Codable {
        public let maxInputSize: Int?  // bytes
        public let maxOutputSize: Int?
        public let maxTokens: Int?
        public let timeout: Int?  // seconds
    }

    /// Create NIP-89 capability event
    public func toNostrEvent(
        privateKey: String,
        relays: [String]
    ) throws -> NostrEvent {
        // kind:31990 (NIP-89 handler information)
        let content = try JSONEncoder().encode(self)
        return try NostrEvent.sign(
            privateKey: privateKey,
            created_at: Int64(Date().timeIntervalSince1970),
            kind: 31990,
            tags: [
                ["d", "\(jobKind.rawValue)"],  // Job kind as identifier
                ["k", "\(jobKind.rawValue)"],  // Kind it handles
                ["relays"] + relays
            ],
            content: String(data: content, encoding: .utf8)!
        )
    }
}
```

## Dependencies

### OpenAgents Dependencies
- **Issue #001**: Nostr Client Library (NostrEvent, event signing)

### System Frameworks
- **Foundation**: JSON encoding/decoding

## Testing Requirements

### Unit Tests
- [ ] Load job schema registry from JSON
- [ ] Validate params against schema (valid/invalid cases)
- [ ] Required param enforcement
- [ ] Type validation (string, integer, boolean, etc.)
- [ ] Range validation (min/max)
- [ ] Enum validation
- [ ] JobBuilder creates valid NIP-90 events
- [ ] Capability advertisement event creation

### Integration Tests
- [ ] All example requests in registry are valid
- [ ] Parse and validate real job requests
- [ ] Round-trip: build job → parse result

### Schema Validation
- [ ] JSON Schema validation for all schemas
- [ ] No duplicate job kinds
- [ ] All examples match their schemas
- [ ] Version format (semver)

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 2.5.2 (No Downloaded Code)**
- ✅ **Compliant**: Job schemas are **data** (JSON), not executable code
- ✅ Registry is embedded in app bundle (not downloaded at runtime)
- ⚠️  **Future**: If dynamic schema updates are added, ensure they're data-only

**Foundation Models AUP**
- ⚠️  **Job filtering required**: Some job kinds may violate AUP
- ❌ Prohibited job kinds (examples):
  - Medical diagnosis/treatment (regulated healthcare)
  - Legal advice (regulated legal services)
  - Financial advice (regulated financial services)
  - Academic textbook generation
- ✅ **Mitigation**: Policy module (issue #009) enforces AUP filters

### Compliance Mapping

Add AUP compliance metadata to each job schema:

```json
{
  "kind": 5100,
  "aup_compliance": {
    "foundation_models_allowed": true,
    "prohibited_use_cases": [],
    "content_warnings": ["Ensure input is not regulated content"]
  }
}
```

## Reference Links

### Official DVM Specifications (Local)
- **DVM Kinds Directory**: `docs/nostr/dvm-kinds/` - All official DVM kind specifications
- **DVM Ranges Directory**: `docs/nostr/dvm-ranges/` - Kind range allocations
- **Copied from**: https://github.com/nostr-protocol/data-vending-machines

### Specifications (Remote)
- **NIP-90 (Data Vending Machine)**: https://github.com/nostr-protocol/nips/blob/master/90.md
- **NIP-89 (Recommended Application Handlers)**: https://github.com/nostr-protocol/nips/blob/master/89.md
- **JSON Schema**: https://json-schema.org/
- **Data Vending Machines Website**: https://www.data-vending-machines.org/

### OpenAgents
- **Issue #001**: Nostr Client Library
- **Issue #009**: Policy & Safety Module (AUP enforcement)
- **Apple Terms Research**: docs/compute/apple-terms-research.md
- **Nostr Integration**: docs/nostr/README.md

## Success Metrics

- [ ] 10-15 job kinds defined for MVP
- [ ] All schemas have examples and validation rules
- [ ] Swift types compile and pass unit tests
- [ ] JobBuilder creates valid NIP-90 events
- [ ] Documentation published (machine + human readable)
- [ ] Schema versioning in place
- [ ] AUP compliance metadata for all job kinds

## Notes

### Kind Range Allocation

Per NIP-90 and official DVM specifications:

- **Request kinds**: 5000-5999
  - **5000-5099**: Text processing (official DVM)
  - **5100-5199**: Image manipulation (official DVM)
  - **5200-5299**: Video/audio manipulation (official DVM)
  - **5300-5399**: Discovery (official DVM)
  - **5400-5499**: Analytics (official DVM)
  - **5500-5599**: Software analysis (official DVM)
  - **5900-5999**: Other/miscellaneous (official DVM)
  - **6500-6599**: OpenAgents custom kinds (agent/coding workflows)
- **Result kinds**: 6000-6999 (request_kind + 1000)
  - **6000-6099**: Text processing results
  - **6100-6199**: Image manipulation results
  - **6200-6299**: Video/audio results
  - **6300-6399**: Discovery results
  - **6400-6499**: Analytics results
  - **6500-6599**: Software analysis results
  - **6900-6999**: Other results
  - **7500-7599**: OpenAgents custom results (custom_kind + 1000)

**Why 65xx for OpenAgents custom kinds?**
- Avoids collision with official DVM kinds (5000-5999)
- Avoids collision with official result kinds (6000-6999)
- 6500-6599 range is currently unused in official spec
- Reserves 7500-7599 for our custom result kinds

### Registry Management

- **Open Registry**: Publish registry on GitHub for community contributions
- **Versioning**: Follow semver (major.minor.patch)
  - Major: Breaking schema changes
  - Minor: New job kinds or backward-compatible changes
  - Patch: Bug fixes, clarifications
- **Community Input**: Accept PRs for new job schemas (custom kinds only)
- **Official DVM Sync**: Periodically sync `docs/nostr/dvm-kinds/` with upstream
- **Documentation**: All custom kinds must have full specs like official kinds

## Future Enhancements (Post-MVP)

- Dynamic schema updates (fetch from GitHub releases)
- Schema validation in provider (reject invalid requests)
- Job kind discovery UI (browse available services)
- Schema migration tools (upgrade old job requests)
- Multi-language schemas (i18n descriptions)
- Schema composition (job chains, pipelines)
- Integration with Foundation Models @Generable types (typed results)
