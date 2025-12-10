# Apple FoundationModels Framework: Complete API Surface

**Last Updated:** 2025-12-09

This document catalogs all capabilities exposed by Apple's FoundationModels framework and identifies what we currently wrap vs. potential future integrations.

## Framework Overview

The FoundationModels framework provides access to Apple's on-device Large Language Model that powers Apple Intelligence. Available on macOS 26+, iOS 26+, iPadOS 26+, and visionOS 26+.

## Core Classes & Protocols

### SystemLanguageModel

**Purpose:** Runtime capability checking and model selection.

**API Surface:**
```swift
class SystemLanguageModel {
    static let `default`: SystemLanguageModel

    init(useCase: UseCase)

    var availability: Availability
}

enum UseCase {
    case contentTagging
    // ... potentially others
}

enum Availability {
    case available
    case unavailable(Reason)
}

enum Reason {
    case appleIntelligenceNotEnabled
    case modelNotReady
    case deviceNotEligible
}
```

**What We Wrap:**
- ✅ `availability` check via `/health` endpoint
- ✅ `default` model via "apple-foundation-model" ID

**What We Don't Wrap:**
- ❌ `useCase` parameter (always use default)
- ❌ Granular unavailability reasons (just return available: true/false)

---

### LanguageModelSession

**Purpose:** Stateful session for interacting with the LLM.

**Initialization:**
```swift
class LanguageModelSession {
    init(
        model: SystemLanguageModel = .default,
        guardrails: Guardrails? = nil,  // Non-negotiable safety filters
        tools: [any Tool]? = nil,
        instructions: Instructions? = nil,
        transcript: Transcript? = nil
    )
}
```

**Properties:**
```swift
var isResponding: Bool  // Track active response state
```

**Methods:**
```swift
// Basic text generation
func respond(
    to prompt: String,
    options: GenerationOptions? = nil
) async throws -> Response<String>

// Guided generation (structured output)
func respond<T: Generable>(
    to prompt: String,
    generating type: T.Type,
    includeSchemaInPrompt: Bool = false,
    options: GenerationOptions? = nil
) async throws -> Response<T>

// Streaming text generation
func streamResponse(
    to prompt: String,
    options: GenerationOptions? = nil
) -> AsyncThrowingStream<Response<String>, Error>

// Streaming guided generation
func streamResponse<T: Generable>(
    generating type: T.Type,
    includeSchemaInPrompt: Bool = false,
    options: GenerationOptions? = nil,
    prompt: String
) -> AsyncThrowingStream<Response<T.PartiallyGenerated>, Error>

// Performance optimization
func prewarm(promptPrefix: String? = nil) async
```

**What We Wrap:**
- ✅ `respond(to:)` - Via `/v1/chat/completions`
- ✅ `respond(to:generating:)` - Via `response_format` with `schema_type`
- ✅ Session creation (each request gets fresh session to avoid context errors)

**What We Don't Wrap:**
- ❌ `streamResponse()` - Not implemented yet (TODO: SSE)
- ❌ `tools` - No tool calling support
- ❌ `instructions` - System messages not differentiated
- ❌ `transcript` - No multi-turn conversation state
- ❌ `guardrails` - Always use defaults
- ❌ `prewarm()` - No prewarming optimization
- ❌ `isResponding` - Stateless HTTP, can't track this

---

### GenerationOptions

**Purpose:** Control response generation behavior.

**API Surface:**
```swift
struct GenerationOptions {
    var temperature: Double?  // 0.0–2.0, controls randomness
    var maximumResponseTokens: Int?  // Limit response length
    var sampling: SamplingMode?
}

enum SamplingMode {
    case greedy  // Deterministic output
    case random(probabilityThreshold: Double, seed: Int)
    case random(top: Int, seed: Int)  // Top-k sampling
}
```

**What We Wrap:**
- ✅ `temperature` - Passed through (though FM may ignore it)
- ✅ `maximumResponseTokens` as `max_tokens`

**What We Don't Wrap:**
- ❌ `sampling` - No sampling mode control

---

## Structured Output (Guided Generation)

### @Generable Macro

**Purpose:** Mark Swift types for constrained generation.

```swift
@Generable
struct Recipe {
    var name: String
    var ingredients: [Ingredient]
    var steps: [String]
}

@Generable
struct Ingredient {
    @Guide(description: "Ingredient name")
    var name: String

    @Guide(description: "Amount in cups", .range(0.0...10.0))
    var amount: Double
}
```

**Features:**
- Auto-generates JSON schema
- Auto-generates `PartiallyGenerated` type for streaming
- Constrains model output to valid structure

**What We Wrap:**
- ✅ Pre-defined `@Generable` types:
  - `TestGenerationResult`
  - `EnvironmentAwareTestResult`
  - `ToolCallRequest`
- ✅ Schema type selection via `response_format.schema_type`

**What We Don't Wrap:**
- ❌ Dynamic schema generation from arbitrary JSON
- ❌ Custom user-defined `@Generable` types (would require Swift code changes)

### @Guide Macro

**Purpose:** Constrain individual properties.

```swift
@Guide(description: "Brief description")
var field: String

@Guide(description: "Numeric field", .range(0.0...1.0))
var confidence: Float

@Guide(description: "Category", .anyOf(["A", "B", "C"]))
var category: String
```

**Constraints:**
- `.range(min...max)` - Numeric bounds
- `.anyOf([...])` - Enum values
- `.count(min...max)` - Array/string length
- Regex patterns

**What We Wrap:**
- ✅ All constraints in pre-defined schemas

**What We Don't Wrap:**
- ❌ Dynamic constraint specification

---

## Tool Calling

### Tool Protocol

**Purpose:** Enable model to invoke custom Swift functions.

```swift
protocol Tool {
    associatedtype Arguments: Generable
    associatedtype Output

    var name: String { get }
    var description: String { get }

    func call(arguments: Arguments) async throws -> ToolOutput<Output>
}

struct ToolOutput<T> {
    var content: String  // Or structured data
}
```

**Example:**
```swift
struct WeatherTool: Tool {
    struct Arguments: Generable {
        @Guide(description: "City name")
        var city: String
    }

    let name = "get_weather"
    let description = "Get current weather for a city"

    func call(arguments: Arguments) async throws -> ToolOutput<String> {
        // Fetch weather
        return ToolOutput(content: "Sunny, 72°F")
    }
}

// Use in session
let session = LanguageModelSession(tools: [WeatherTool()])
```

**What We Wrap:**
- ❌ None - No tool calling support in our bridge

**What We Could Wrap:**
- [ ] POST /v1/tools/register - Register tool schemas
- [ ] POST /v1/chat/completions with `tools` parameter
- [ ] Tool invocation callback mechanism
- [ ] Multi-turn conversation with tool results

---

## Session Context & State

### Transcript

**Purpose:** Preserve multi-turn conversation history.

```swift
struct Transcript {
    // Contains all messages and responses
}

// Resume conversation
let session = LanguageModelSession(transcript: previousTranscript)
```

**What We Wrap:**
- ❌ None - Each HTTP request is stateless

**What We Could Wrap:**
- [ ] POST /v1/sessions/create - Create persistent session
- [ ] GET /v1/sessions/{id}/transcript - Retrieve conversation history
- [ ] POST /v1/sessions/{id}/continue - Continue with context
- [ ] DELETE /v1/sessions/{id} - Clean up session

---

### Instructions

**Purpose:** System-level behavioral guidance.

```swift
struct Instructions {
    // DSL for system prompts
}

let session = LanguageModelSession(
    instructions: Instructions {
        "You are a helpful coding assistant."
        "Always provide runnable code examples."
    }
)
```

**What We Wrap:**
- ⚠️  Partial - System messages in chat array (but model may not differentiate)

**What We Could Wrap:**
- [ ] Better system message handling
- [ ] Persistent instructions per session

---

## Response Types

### Response<T>

```swift
struct Response<T> {
    var content: T
    var finishReason: FinishReason?
}

enum FinishReason {
    case stop
    case length
    case toolCalls
}
```

**What We Wrap:**
- ✅ `content` - As `message.content` or structured JSON
- ✅ `finishReason` - As `finish_reason`

---

### T.PartiallyGenerated

**Purpose:** Incremental updates during streaming.

```swift
@Generable
struct Recipe {
    var name: String
    var steps: [String]
}

// Generated automatically:
extension Recipe {
    struct PartiallyGenerated {
        var name: String?  // Optional until received
        var steps: [String]?
    }
}
```

**What We Wrap:**
- ❌ None - Streaming not implemented

**What We Could Wrap:**
- [ ] Server-Sent Events (SSE) streaming
- [ ] Incremental JSON parsing
- [ ] Partial object updates

---

## Guardrails

**Purpose:** Enforce Apple's content safety policies.

```swift
struct Guardrails {
    // Non-negotiable safety filters
    // Cannot be disabled
}
```

**What We Wrap:**
- ✅ Implicitly - Always active (can't disable)

---

## Performance Optimization

### Prewarm

```swift
func prewarm(promptPrefix: String? = nil) async
```

**Purpose:** Load model into memory before first request.

**What We Wrap:**
- ❌ None - No prewarming API

**What We Could Wrap:**
- [ ] POST /v1/sessions/prewarm - Explicit model loading
- [ ] Automatic prewarming on server start

---

## Error Handling

### Potential Errors

```swift
enum LanguageModelError: Error {
    case contextWindowExceeded
    case modelUnavailable
    case requestFailed(String)
    case invalidArguments
    // ... more
}
```

**What We Wrap:**
- ✅ Model unavailability (503)
- ✅ Request failures (500)

**What We Don't Wrap:**
- ❌ Context window errors (currently just create fresh sessions)
- ❌ Granular error types

---

## Summary: What We Could Add

### High Priority (Valuable Features)

1. **Server-Sent Events Streaming**
   - Endpoint: `POST /v1/chat/completions?stream=true`
   - Returns: SSE stream of delta chunks
   - Impact: Faster perceived latency, better UX

2. **Tool Calling Support**
   - Endpoint: `POST /v1/chat/completions` with `tools` array
   - Callback mechanism for tool execution
   - Impact: Enable agent workflows

3. **Multi-turn Conversations**
   - Session management endpoints
   - Transcript storage
   - Impact: More natural conversations

### Medium Priority (Nice to Have)

4. **Sampling Mode Control**
   - Add `sampling` to CompletionOptions
   - Support greedy/top-k modes
   - Impact: More deterministic outputs

5. **Dynamic Schema Support**
   - Accept arbitrary JSON schemas
   - Use DynamicGenerationSchema from FoundationModels
   - Impact: More flexible structured generation

6. **Prewarm API**
   - Explicit model loading endpoint
   - Impact: Faster first response

### Low Priority (Less Impactful)

7. **Instructions DSL**
   - Better system message handling
   - Impact: Marginal (system messages already work somewhat)

8. **Use Case Selection**
   - Support `SystemLanguageModel(useCase: .contentTagging)`
   - Impact: Unknown benefits

9. **Granular Error Types**
   - More specific error codes
   - Impact: Better debugging

---

## Current Coverage

**Features We Wrap:**
- ✅ Basic text generation
- ✅ Guided generation with pre-defined schemas
- ✅ Health/availability checking
- ✅ Model listing
- ✅ Temperature control
- ✅ Token limit control
- ✅ Response content & finish reasons

**Feature Coverage:** ~40% of framework capabilities

**Most Notable Gaps:**
1. Streaming (SSE)
2. Tool calling
3. Multi-turn conversations with context
4. Dynamic schema generation

---

## References

- [Apple FoundationModels Documentation](https://developer.apple.com/documentation/foundationmodels)
- [WWDC 2025: Meet the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2025/286/)
- [WWDC 2025: Deep dive into the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2025/301/)
- [The Ultimate Guide To The Foundation Models Framework](https://azamsharp.com/2025/06/18/the-ultimate-guide-to-the-foundation-models-framework.html)
- [Exploring the Foundation Models framework](https://www.createwithswift.com/exploring-the-foundation-models-framework/)
- [Building AI features using Foundation Models](https://swiftwithmajid.com/2025/08/19/building-ai-features-using-foundation-models/)
