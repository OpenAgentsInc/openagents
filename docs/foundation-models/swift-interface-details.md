Read(/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/System/Library/Frameworks/FoundationModels.frame
      work/Modules/FoundationModels.swiftmodule/arm64e-apple-macos.swiftinterface)
  ⎿  Read 100 lines

⏺ The .swiftinterface file is the actual Swift framework binary interface - it's Apple's compiled API surface for the FoundationModels framework.
  Here's what it specifically tells me:

  Exact Details From The File:

  1. Two Main Entry Points:

  SystemLanguageModel (line 572):
  final public class SystemLanguageModel : Swift.Sendable {
    // Availability checking
    final public var availability: Availability  // .available or .unavailable(Reason)
    final public var isAvailable: Swift.Bool

    // Two use cases
    public struct UseCase {
      public static let general: UseCase
      public static let contentTagging: UseCase
    }

    // Two guardrail modes
    public struct Guardrails {
      public static let `default`: Guardrails
      public static let permissiveContentTransformations: Guardrails
    }

    // Create model
    public init(useCase: UseCase = .general, guardrails: Guardrails = .default)

    // Or load custom adapter (LoRA/fine-tuned models)
    public init(adapter: Adapter, guardrails: Guardrails = .default)

    // Language support
    final public var supportedLanguages: Swift.Set<Foundation.Locale.Language>
    final public func supportsLocale(_ locale: Foundation.Locale) -> Swift.Bool
  }

  LanguageModelSession (line 331) - the workhorse class:
  final public class LanguageModelSession {
    // State tracking
    final public var transcript: Transcript  // Full conversation history
    final public var isResponding: Swift.Bool  // Currently generating?

    // Initialize with model, tools, instructions
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
  }

  2. Nine Different respond() Methods (lines 359-392):

  The file shows 9 overloads for generating responses:

  1. Basic text - respond(to: Prompt) -> Response<String>
  2. Text from string - respond(to: String) -> Response<String>
  3. Text with builder - respond(@PromptBuilder prompt: () -> Prompt) -> Response<String>
  4. Guided with schema - respond(to: Prompt, schema: GenerationSchema) -> Response<GeneratedContent>
  5. Guided from string - respond(to: String, schema: GenerationSchema) -> Response<GeneratedContent>
  6. Guided with builder - respond(schema:, @PromptBuilder prompt:) -> Response<GeneratedContent>
  7. Type-safe guided - respond<Content>(to: Prompt, generating: Content.Type) -> Response<Content>
  8. Type-safe from string - respond<Content>(to: String, generating: Content.Type) -> Response<Content>
  9. Type-safe with builder - respond<Content>(generating: Content.Type, @PromptBuilder prompt:) -> Response<Content>

  3. Nine Matching streamResponse() Methods (lines 397-567):

  Every respond() has a streaming equivalent that returns ResponseStream<Content>:
  - streamResponse(to:) → ResponseStream<String>
  - streamResponse(to:schema:) → ResponseStream<GeneratedContent>
  - streamResponse(to:generating:) → ResponseStream<Content>
  - etc.

  The stream is an AsyncSequence (line 493):
  extension ResponseStream : AsyncSequence {
    public struct Snapshot {
      public var content: Content.PartiallyGenerated  // <-- Partial results!
      public var rawContent: GeneratedContent
    }

    public func collect() async throws -> Response<Content>  // Stream → full response
  }

  4. Error Types Are Exhaustive (lines 420-464):

  public enum GenerationError : Swift.Error {
    case exceededContextWindowSize(Context)
    case assetsUnavailable(Context)  // Model not downloaded
    case guardrailViolation(Context)  // Safety filter triggered
    case unsupportedGuide(Context)  // Invalid constraint
    case unsupportedLanguageOrLocale(Context)  // Language not supported
    case decodingFailure(Context)  // Schema decode failed
    case rateLimited(Context)  // Too many requests
    case concurrentRequests(Context)  // Multiple simultaneous requests
    case refusal(Refusal, Context)  // Model refused to answer
  }

  public struct Refusal {
    public var explanation: Response<String> { get async throws }
    public var explanationStream: ResponseStream<String> { get }
  }

  Notice: Refusals have explanations - the model will tell you WHY it refused!

  5. Tool Calling Protocol (lines 1196-1204):

  public protocol Tool<Arguments, Output> : Swift.Sendable {
    associatedtype Output : PromptRepresentable
    associatedtype Arguments : ConvertibleFromGeneratedContent

    var name: Swift.String { get }
    var description: Swift.String { get }
    var parameters: GenerationSchema { get }  // Auto-generated from @Generable
    var includesSchemaInInstructions: Swift.Bool { get }

    func call(arguments: Self.Arguments) async throws -> Self.Output
  }

  Important constraints (lines 1227-1276):
  - Arguments CANNOT be String, Int, Double, Float, Decimal, or Bool directly
  - Must use @Generable struct with typed fields

  6. GenerationOptions (lines 1319-1342):

  public struct GenerationOptions {
    public struct SamplingMode {
      public static var greedy: SamplingMode  // Deterministic
      public static func random(top k: Int, seed: UInt64? = nil) -> SamplingMode  // Top-k
      public static func random(probabilityThreshold: Double, seed: UInt64? = nil) -> SamplingMode  // Nucleus
    }

    public var sampling: SamplingMode?
    public var temperature: Swift.Double?
    public var maximumResponseTokens: Swift.Int?
  }

  7. Transcript Structure (lines 708-807):

  public struct Transcript : RandomAccessCollection {
    public enum Entry {
      case instructions(Instructions)  // System prompt
      case prompt(Prompt)  // User message
      case toolCalls(ToolCalls)  // Model requested tools
      case toolOutput(ToolOutput)  // Tool results
      case response(Response)  // Model response
    }

    public init(entries: some Sequence<Entry> = [])
    public subscript(index: Int) -> Entry { get set }
  }

  This means you can:
  - Build conversation history manually
  - Inspect what the model said
  - Resume from any point

  8. Adapter System for Custom Models (lines 655-704):

  public struct Adapter {
    public init(fileURL: Foundation.URL) throws  // Load from .mlpackage
    public init(name: Swift.String) throws  // Load by name

    public func compile() async throws  // Optimize for device

    public var creatorDefinedMetadata: [Swift.String : Any] { get }

    public static func compatibleAdapterIdentifiers(name: String) -> [String]
    public static func removeObsoleteAdapters() throws
    public static func isCompatible(_ assetPack: BackgroundAssets.AssetPack) -> Bool
  }

  You can load LoRA adapters or custom fine-tuned models!

  9. Prewarm API (line 347):

  final public func prewarm(promptPrefix: Prompt? = nil)

  Load the model into memory before first request - reduces first-token latency.

  ---
  What This Means:

  The .swiftinterface file tells me:

  1. Exact method signatures - I can see every parameter, return type, default value
  2. What's actually implemented - 9 respond variants, 9 stream variants, not "maybe" but "definitely exists"
  3. Error cases - All 9 specific error types the model can throw
  4. Language support checking - supportedLanguages and supportsLocale() exist
  5. Multi-turn conversations - Transcript has 5 entry types including tool calls
  6. Custom models - Adapter API for loading fine-tuned models
  7. Two use cases - .general and .contentTagging (not just one model)
  8. Two guardrail modes - .default and .permissiveContentTransformations
  9. Three sampling modes - greedy, top-k, nucleus (not just temperature!)
  10. Refusal explanations - Model will tell you why it refused via async

  This is way more than web docs show - it's the actual binary contract Apple ships.
