# Apple FoundationModels Framework: .swiftinterface Analysis

**Source:** `/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/System/Library/Frameworks/FoundationModels.framework/Modules/FoundationModels.swiftmodule/arm64e-apple-macos.swiftinterface`

**Last Analyzed:** 2025-12-09
**Framework Version:** 1.1.7
**Swift Version:** 6.2.1
**Platform:** arm64e-apple-macos26.1

---

## What Is This File?

The `.swiftinterface` file is Apple's **compiled binary interface** for the FoundationModels framework. Unlike web documentation (which can be incomplete or requires JavaScript), this file shows the **exact, guaranteed API surface** that ships with macOS 26+.

**Why this matters:**
- Shows every method signature with exact types and defaults
- Reveals features not documented on the web
- Is the source of truth for what the framework can do
- Shows all error cases, not just "it might fail"

**What we found:** The framework is **much more capable** than web docs suggest. We currently wrap ~40% of its functionality.

---

## 1. Entry Points: Two Main Classes

### SystemLanguageModel (line 572)

**Purpose:** Model selection and runtime capability checking.

```swift
final public class SystemLanguageModel : Swift.Sendable {
  // Availability checking
  final public var availability: Availability
  // Returns: .available or .unavailable(Reason)

  final public var isAvailable: Swift.Bool
  // Quick boolean check

  // Two use cases (different model behaviors)
  public struct UseCase {
    public static let general: UseCase
    public static let contentTagging: UseCase
  }

  // Two guardrail modes
  public struct Guardrails {
    public static let `default`: Guardrails
    public static let permissiveContentTransformations: Guardrails
  }

  // Initialize with system model
  public init(
    useCase: UseCase = .general,
    guardrails: Guardrails = .default
  )

  // Or load custom adapter (LoRA/fine-tuned models)
  public init(
    adapter: Adapter,
    guardrails: Guardrails = .default
  )

  // Language support
  final public var supportedLanguages: Swift.Set<Foundation.Locale.Language>
  final public func supportsLocale(_ locale: Foundation.Locale) -> Swift.Bool

  // Singleton
  public static let `default`: SystemLanguageModel
}
```

**Availability enum** (lines 608-625):
```swift
public enum Availability {
  case available
  case unavailable(UnavailableReason)

  public enum UnavailableReason {
    case deviceNotEligible       // Not Apple Silicon or wrong macOS version
    case appleIntelligenceNotEnabled  // Disabled in System Settings
    case modelNotReady          // Still downloading
  }
}
```

**What we wrap:**
- ✅ `.default` model via "apple-foundation-model" ID
- ✅ `availability` check via `/health` endpoint (`model_available` field)

**What we don't wrap:**
- ❌ `useCase` parameter (always use `.general`)
- ❌ Granular `UnavailableReason` (just return true/false)
- ❌ Language/locale checking
- ❌ Custom `Adapter` loading

---

### LanguageModelSession (line 331)

**Purpose:** Stateful session for interacting with the LLM. This is the workhorse class.

```swift
final public class LanguageModelSession {
  // State tracking
  final public var transcript: Transcript
  // Full conversation history (editable!)

  final public var isResponding: Swift.Bool
  // True while model is generating

  // Initialize with fresh session
  convenience public init(
    model: SystemLanguageModel = .default,
    tools: [any Tool] = [],
    instructions: Instructions? = nil
  )

  // Resume from previous conversation
  convenience public init(
    model: SystemLanguageModel = .default,
    tools: [any Tool] = [],
    transcript: Transcript  // <-- Continue from history
  )

  // Preload model into memory
  final public func prewarm(promptPrefix: Prompt? = nil)
}
```

**What we wrap:**
- ✅ Session creation (but create fresh session per request to avoid context errors)
- ✅ Basic `respond(to:)` method

**What we don't wrap:**
- ❌ `tools` - No tool calling support in our bridge
- ❌ `instructions` - System messages not differentiated from user messages
- ❌ `transcript` - Each HTTP request is stateless
- ❌ `prewarm()` - No prewarming optimization
- ❌ `isResponding` - Can't track state across HTTP requests

---

## 2. Response Generation: 9 Variants of `respond()`

**Lines 359-392** define **9 overloaded methods** for different use cases:

### Basic Text Generation

```swift
// 1. With Prompt type
nonisolated(nonsending) final public func respond(
  to prompt: Prompt,
  options: GenerationOptions = GenerationOptions()
) async throws -> Response<String>

// 2. With String (convenience)
@_disfavoredOverload nonisolated(nonsending) final public func respond(
  to prompt: String,
  options: GenerationOptions = GenerationOptions()
) async throws -> Response<String>

// 3. With builder pattern
nonisolated(nonsending) final public func respond(
  options: GenerationOptions = GenerationOptions(),
  @PromptBuilder prompt: () throws -> Prompt
) async throws -> Response<String>
```

**Use case:** Simple text generation, no structured output.

---

### Guided Generation (Schema-based)

```swift
// 4. With schema (Prompt)
nonisolated(nonsending) final public func respond(
  to prompt: Prompt,
  schema: GenerationSchema,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions()
) async throws -> Response<GeneratedContent>

// 5. With schema (String)
@_disfavoredOverload nonisolated(nonsending) final public func respond(
  to prompt: String,
  schema: GenerationSchema,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions()
) async throws -> Response<GeneratedContent>

// 6. With schema (builder)
nonisolated(nonsending) final public func respond(
  schema: GenerationSchema,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions(),
  @PromptBuilder prompt: () throws -> Prompt
) async throws -> Response<GeneratedContent>
```

**Use case:** Generate JSON that conforms to a dynamic schema. Returns `GeneratedContent` (untyped).

---

### Type-Safe Guided Generation

```swift
// 7. Type-safe (Prompt)
nonisolated(nonsending) final public func respond<Content>(
  to prompt: Prompt,
  generating type: Content.Type = Content.self,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions()
) async throws -> Response<Content> where Content : Generable

// 8. Type-safe (String)
@_disfavoredOverload nonisolated(nonsending) final public func respond<Content>(
  to prompt: String,
  generating type: Content.Type = Content.self,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions()
) async throws -> Response<Content> where Content : Generable

// 9. Type-safe (builder)
nonisolated(nonsending) final public func respond<Content>(
  generating type: Content.Type = Content.self,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions(),
  @PromptBuilder prompt: () throws -> Prompt
) async throws -> Response<Content> where Content : Generable
```

**Use case:** Generate Swift types marked with `@Generable`. The schema is auto-derived from the type.

**Example:**
```swift
@Generable
struct Recipe {
  var name: String
  var ingredients: [String]
}

let response = try await session.respond(
  to: "Give me a recipe for pancakes",
  generating: Recipe.self
)
print(response.content.name)  // Type-safe!
```

**What we wrap:**
- ✅ `respond(to: String)` for basic text (variant #2)
- ✅ `respond(to: String, generating: Content.Type)` for guided generation with pre-defined types (variant #8)

**What we don't wrap:**
- ❌ Variants #1, #3, #4-7, #9 (different input types, schema variants)
- ❌ `@PromptBuilder` DSL
- ❌ `includeSchemaInPrompt` control

---

## 3. Streaming: 9 Variants of `streamResponse()`

**Lines 397-567** define streaming equivalents for all 9 `respond()` methods.

### Stream API

```swift
final public func streamResponse(
  to prompt: String,
  options: GenerationOptions = GenerationOptions()
) -> ResponseStream<String>

final public func streamResponse<Content>(
  to prompt: String,
  generating type: Content.Type = Content.self,
  includeSchemaInPrompt: Bool = true,
  options: GenerationOptions = GenerationOptions()
) -> ResponseStream<Content> where Content : Generable

// ... 7 more variants
```

### ResponseStream Type (lines 483-523)

```swift
public struct ResponseStream<Content> : AsyncSequence
  where Content : Generable
{
  public struct Snapshot {
    public var content: Content.PartiallyGenerated  // Partial results!
    public var rawContent: GeneratedContent
  }

  public typealias Element = Snapshot

  // Use in for-await loop
  public struct AsyncIterator : AsyncIteratorProtocol {
    public mutating func next() async throws -> Snapshot?
  }

  // Or collect all chunks
  public func collect() async throws -> Response<Content>
}
```

**Usage example:**
```swift
let stream = session.streamResponse(
  to: "Count to 10",
  generating: NumberList.self
)

for try await snapshot in stream {
  print(snapshot.content)  // Partial NumberList with increasing items
}
```

**What we wrap:**
- ❌ None - Streaming not implemented in Swift bridge yet

**Why it matters:**
- Reduces perceived latency (show partial results)
- Enable streaming UIs
- `PartiallyGenerated` types show incremental progress for structured output

---

## 4. Error Handling: 9 Specific Error Cases

**Lines 420-464** define comprehensive error types.

```swift
public enum GenerationError : Swift.Error, Foundation.LocalizedError {
  case exceededContextWindowSize(Context)
  // Prompt + history too long for model

  case assetsUnavailable(Context)
  // Model not downloaded or can't load

  case guardrailViolation(Context)
  // Safety filter blocked the request/response

  case unsupportedGuide(Context)
  // Invalid constraint in @Guide macro

  case unsupportedLanguageOrLocale(Context)
  // Model doesn't support the requested language

  case decodingFailure(Context)
  // Generated output doesn't match schema

  case rateLimited(Context)
  // Too many requests in time window

  case concurrentRequests(Context)
  // Multiple simultaneous requests on same session

  case refusal(Refusal, Context)
  // Model refused to answer

  // Each error has localized descriptions
  public var errorDescription: String? { get }
  public var recoverySuggestion: String? { get }
  public var failureReason: String? { get }
}
```

### Refusal Type (lines 431-439)

```swift
public struct Refusal : Swift.Sendable {
  // Get explanation for why model refused
  public var explanation: Response<String> { get async throws }
  public var explanationStream: ResponseStream<String> { get }
}
```

**This is remarkable:** The model will tell you **why** it refused your request!

**Example:**
```swift
do {
  let response = try await session.respond(to: "How do I hack a server?")
} catch let error as GenerationError {
  switch error {
    case .refusal(let refusal, _):
      let explanation = try await refusal.explanation
      print("Model refused: \(explanation.content)")
      // "I can't help with hacking servers as that could be used to harm others."
    default:
      print("Error: \(error.localizedDescription)")
  }
}
```

**What we wrap:**
- ✅ General error responses (HTTP 500, 503)
- ✅ Model unavailability (`assetsUnavailable` → 503)

**What we don't wrap:**
- ❌ Specific error cases (all mapped to generic errors)
- ❌ Refusal explanations
- ❌ Localized error messages
- ❌ Context window errors (we just create fresh sessions)

---

## 5. Tool Calling: Function Invocation

**Lines 1196-1204** define the `Tool` protocol.

```swift
public protocol Tool<Arguments, Output> : Swift.Sendable {
  associatedtype Output : PromptRepresentable
  associatedtype Arguments : ConvertibleFromGeneratedContent

  var name: Swift.String { get }
  var description: Swift.String { get }
  var parameters: GenerationSchema { get }
  var includesSchemaInInstructions: Swift.Bool { get }

  func call(arguments: Self.Arguments) async throws -> Self.Output
}
```

### Constraints (lines 1227-1276)

**Arguments CANNOT be primitive types directly:**
- ❌ `String`, `Int`, `Double`, `Float`, `Decimal`, `Bool`
- ✅ Must use `@Generable struct` with typed fields

**Example:**
```swift
// ❌ WRONG - Won't compile
struct WeatherTool: Tool {
  typealias Arguments = String  // Error!
  typealias Output = String

  func call(arguments: String) async throws -> String {
    return "Weather data"
  }
}

// ✅ CORRECT
struct WeatherTool: Tool {
  @Generable
  struct Arguments {
    @Guide(description: "City name")
    var city: String

    @Guide(description: "Temperature unit")
    var unit: String  // "celsius" or "fahrenheit"
  }

  typealias Output = String
  let name = "get_weather"
  let description = "Get current weather for a city"

  func call(arguments: Arguments) async throws -> String {
    return "Weather in \(arguments.city): 72°F"
  }
}

// Use in session
let session = LanguageModelSession(tools: [WeatherTool()])
let response = try await session.respond(to: "What's the weather in SF?")
// Model can invoke get_weather tool automatically
```

**What we wrap:**
- ❌ None - No tool calling support in our bridge

**What we could add:**
- [ ] Register tool schemas via POST `/v1/tools/register`
- [ ] Include tools in chat completions request
- [ ] Parse tool call requests from model
- [ ] Execute tool functions
- [ ] Send results back to model

---

## 6. GenerationOptions: Control Output Behavior

**Lines 1319-1342** define all generation parameters.

```swift
public struct GenerationOptions : Swift.Sendable, Swift.Equatable {
  public var sampling: SamplingMode?
  public var temperature: Swift.Double?
  public var maximumResponseTokens: Swift.Int?

  public struct SamplingMode : Swift.Sendable, Swift.Equatable {
    // Deterministic (always pick most likely token)
    public static var greedy: SamplingMode

    // Top-k sampling (pick from k most likely tokens)
    public static func random(
      top k: Int,
      seed: UInt64? = nil
    ) -> SamplingMode

    // Nucleus/top-p sampling (pick from tokens with cumulative probability > threshold)
    public static func random(
      probabilityThreshold: Double,
      seed: UInt64? = nil
    ) -> SamplingMode
  }
}
```

**Three sampling modes:**

1. **Greedy** - Always pick most likely token (deterministic)
   ```swift
   let options = GenerationOptions(sampling: .greedy)
   ```

2. **Top-k** - Sample from k most likely tokens
   ```swift
   let options = GenerationOptions(sampling: .random(top: 40, seed: 42))
   ```

3. **Nucleus (top-p)** - Sample from smallest set of tokens with cumulative probability > threshold
   ```swift
   let options = GenerationOptions(sampling: .random(probabilityThreshold: 0.9))
   ```

**What we wrap:**
- ✅ `temperature` (passed through, though FM may ignore it)
- ✅ `maximumResponseTokens` as `max_tokens`

**What we don't wrap:**
- ❌ `sampling` modes (no control over determinism/randomness)
- ❌ Random seeds (no reproducibility control)

---

## 7. Transcript: Conversation History

**Lines 708-807** define the conversation transcript structure.

```swift
public struct Transcript : Swift.Sendable,
                           Swift.Equatable,
                           Swift.RandomAccessCollection
{
  public typealias Index = Swift.Int

  public enum Entry {
    case instructions(Instructions)  // System prompt
    case prompt(Prompt)              // User message
    case toolCalls(ToolCalls)        // Model requested tools
    case toolOutput(ToolOutput)      // Tool execution results
    case response(Response)          // Model response
  }

  public init(entries: some Sequence<Entry> = [])

  // Access like an array
  public subscript(index: Int) -> Entry { get set }
  public var startIndex: Int { get }
  public var endIndex: Int { get }
}
```

### Entry Types Explained

**Instructions** (lines 787-797):
```swift
public struct Instructions {
  public var id: String
  public var segments: [Segment]  // Text or structured content
  public var toolDefinitions: [ToolDefinition]
}
```
System-level guidance, tool definitions.

**Prompt** (similar structure):
User input.

**ToolCalls** (lines 808+):
Model requested function invocations.

**ToolOutput** (lines 808+):
Results from executed tools.

**Response** (lines 808+):
Model's generated text.

### What This Enables

```swift
// Build conversation manually
var transcript = Transcript(entries: [
  .instructions(Instructions(segments: [.text("You are a helpful assistant")])),
  .prompt(Prompt(segments: [.text("What is 2+2?")])),
  .response(Response(content: "4")),
  .prompt(Prompt(segments: [.text("What about 3+3?")]))
])

// Resume conversation
let session = LanguageModelSession(transcript: transcript)
let response = try await session.respond(to: "And 4+4?")
// Model has full context of previous math questions
```

**What we wrap:**
- ❌ None - Each HTTP request is stateless

**What we could add:**
- [ ] POST `/v1/sessions/create` - Create persistent session with ID
- [ ] GET `/v1/sessions/{id}/transcript` - Retrieve history
- [ ] POST `/v1/sessions/{id}/continue` - Continue with context
- [ ] DELETE `/v1/sessions/{id}` - Clean up

---

## 8. Adapter System: Custom Models

**Lines 655-704** define the adapter API for loading custom models.

```swift
public struct Adapter {
  // Load from .mlpackage file
  public init(fileURL: Foundation.URL) throws

  // Load by name (e.g., "my-custom-model")
  public init(name: Swift.String) throws

  // Optimize adapter for device
  public func compile() async throws

  // Metadata from model creator
  public var creatorDefinedMetadata: [Swift.String : Any] { get }

  // Find compatible adapters
  public static func compatibleAdapterIdentifiers(name: String) -> [String]

  // Clean up old/unused adapters
  public static func removeObsoleteAdapters() throws

  // Check if asset pack contains compatible adapter
  public static func isCompatible(_ assetPack: BackgroundAssets.AssetPack) -> Bool
}
```

### Adapter Errors (lines 682-704)

```swift
public enum AssetError : Swift.Error {
  case invalidAsset(Context)           // Malformed .mlpackage
  case invalidAdapterName(Context)     // Name doesn't match any adapter
  case compatibleAdapterNotFound(Context)  // No adapter works with this device
}
```

### What This Means

**You can load LoRA adapters or fine-tuned models:**

```swift
// Load from file
let adapter = try Adapter(fileURL: URL(fileURLWithPath: "/path/to/model.mlpackage"))
try await adapter.compile()  // Optimize for M1/M2/M3/M4

// Use in session
let model = SystemLanguageModel(adapter: adapter)
let session = LanguageModelSession(model: model)
```

**Use cases:**
- Domain-specific models (medical, legal, coding)
- Fine-tuned on your data
- LoRA adapters (lightweight modifications)
- On-device personalization

**What we wrap:**
- ❌ None - Only use `SystemLanguageModel.default`

**What we could add:**
- [ ] POST `/v1/adapters/load` - Load custom adapter from path
- [ ] GET `/v1/adapters/list` - List available adapters
- [ ] POST `/v1/adapters/{id}/compile` - Optimize adapter
- [ ] Use adapter via `model` parameter in chat completions

---

## 9. Prewarm API: Reduce First-Token Latency

**Line 347** defines the prewarm method.

```swift
final public func prewarm(promptPrefix: Prompt? = nil)
```

**Purpose:** Load model into memory before first request.

**Benefits:**
- Reduces latency for first generation
- Optionally pre-processes prompt prefix (cached for all requests)
- Useful for long-running sessions

**Example:**
```swift
let session = LanguageModelSession()

// Prewarm with system prompt
session.prewarm(promptPrefix: Prompt(segments: [
  .text("You are a helpful coding assistant.")
]))

// First response is faster
let response = try await session.respond(to: "Write hello world in Python")
```

**What we wrap:**
- ❌ None - No prewarming API

**What we could add:**
- [ ] POST `/v1/sessions/prewarm` - Explicit model loading
- [ ] Automatic prewarming on server start (reduce first request latency)

---

## 10. @Generable and @Guide Macros

**Lines 16-28** define the macros for constrained generation.

### @Generable Macro (line 16)

```swift
@attached(extension, conformances: Generable)
@attached(member, names: arbitrary)
public macro Generable(description: String? = nil)
```

**Applies to:** Structs, enums
**Generates:**
- `Generable` conformance
- `generationSchema` property
- `PartiallyGenerated` type for streaming
- Initializer from `GeneratedContent`

**Example:**
```swift
@Generable(description: "A recipe with ingredients and steps")
struct Recipe {
  var name: String
  var ingredients: [Ingredient]
  var steps: [String]
}

// Compiler generates:
extension Recipe: Generable {
  static var generationSchema: GenerationSchema { ... }
  init(_ content: GeneratedContent) throws { ... }

  struct PartiallyGenerated {
    var name: String?
    var ingredients: [Ingredient]?  // Accumulates as items stream in
    var steps: [String]?
  }
}
```

### @Guide Macro (lines 20-28)

```swift
@attached(peer)
public macro Guide<T>(
  description: String? = nil,
  _ guides: GenerationGuide<T>...
) where T : Generable

@attached(peer)
public macro Guide<RegexOutput>(
  description: String? = nil,
  _ guides: Regex<RegexOutput>
)

@attached(peer)
public macro Guide(description: String)
```

**Applies to:** Properties within `@Generable` types
**Purpose:** Add constraints to generated output

**Example:**
```swift
@Generable
struct Product {
  @Guide(description: "Product name")
  var name: String

  @Guide(description: "Price in USD", .range(0.0...10000.0))
  var price: Double

  @Guide(description: "Category", .anyOf(["Electronics", "Clothing", "Food"]))
  var category: String

  @Guide(description: "SKU", /[A-Z]{3}-\d{4}/)  // Regex constraint
  var sku: String
}
```

**Constraint types:**
- `.range(min...max)` - Numeric bounds
- `.anyOf([...])` - Enum values
- `.count(min...max)` - String/array length
- Regex patterns

**What we wrap:**
- ✅ Pre-defined `@Generable` types:
  - `TestGenerationResult`
  - `EnvironmentAwareTestResult`
  - `ToolCallRequest`
- ✅ All constraints in those types

**What we don't wrap:**
- ❌ Dynamic schema generation from arbitrary JSON
- ❌ User-defined `@Generable` types (would require Swift code changes in bridge)

---

## Summary: What the .swiftinterface File Reveals

### Capabilities We Found

| Feature | Documented on Web | In .swiftinterface | We Wrap |
|---------|-------------------|-------------------|---------|
| Basic text generation | ✅ | ✅ (9 variants) | ✅ |
| Streaming | ⚠️  Minimal | ✅ (9 variants, AsyncSequence) | ❌ |
| Guided generation | ✅ | ✅ (3 types: schema/type-safe/dynamic) | ✅ Partial |
| Tool calling | ⚠️  Mentioned | ✅ (Full protocol + constraints) | ❌ |
| Multi-turn conversations | ❌ | ✅ (Transcript with 5 entry types) | ❌ |
| Custom models (adapters) | ❌ | ✅ (Load LoRA/fine-tuned models) | ❌ |
| Error types | ⚠️  Generic | ✅ (9 specific cases) | ⚠️  Partial |
| Refusal explanations | ❌ | ✅ (Async explanation getter) | ❌ |
| Sampling modes | ❌ | ✅ (greedy/top-k/nucleus) | ❌ |
| Language support | ❌ | ✅ (Locale checking) | ❌ |
| Prewarm | ⚠️  Mentioned | ✅ (With optional prefix) | ❌ |
| UseCase variants | ❌ | ✅ (.general, .contentTagging) | ❌ |
| Guardrail modes | ❌ | ✅ (.default, .permissiveContentTransformations) | ❌ |

### Key Discoveries

1. **9 respond() variants** - Different input types (String/Prompt/builder), different output types (String/schema/type-safe)
2. **9 streamResponse() variants** - All respond variants have streaming equivalents with incremental `PartiallyGenerated` updates
3. **9 error types** - Exhaustive, localized, with recovery suggestions
4. **Tool protocol** - Function calling with type-safe arguments via `@Generable`
5. **Transcript** - Full conversation history with 5 entry types (instructions, prompt, toolCalls, toolOutput, response)
6. **Adapter system** - Load custom LoRA/fine-tuned models from .mlpackage files
7. **3 sampling modes** - greedy (deterministic), top-k, nucleus/top-p
8. **2 use cases** - .general vs .contentTagging (different model behaviors)
9. **2 guardrail modes** - .default vs .permissiveContentTransformations
10. **Refusal explanations** - Model tells you why it refused via async getter

### What This Changes

**Before .swiftinterface analysis:**
- "Foundation Models has basic inference and maybe some structured output"
- "We'll use what the web docs show"

**After .swiftinterface analysis:**
- "Foundation Models is a complete LLM framework with streaming, tools, adapters, and multi-turn conversations"
- "We're wrapping ~40% of the actual capabilities"
- "High-priority additions: streaming, tool calling, session management"

---

## Files Reference

| Path | Description |
|------|-------------|
| `/Applications/Xcode.app/.../FoundationModels.framework/.../arm64e-apple-macos.swiftinterface` | Source of truth (1536 lines) |
| `docs/foundation-models/framework-capabilities.md` | What we wrap vs don't wrap |
| `docs/foundation-models/README.md` | Integration guide |
| `docs/foundation-models/QUICK-START.md` | 5-minute setup |
| `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift` | Our current `respond()` usage |
