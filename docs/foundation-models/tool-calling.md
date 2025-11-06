# Tool Calling with Apple Foundation Models (v1.0.0)

This document distills the Foundation Models chapters on Basic Tool Use and Integrating External JSON APIs and maps them onto OpenAgents. It outlines how to implement and register tools, how to adapt our existing session.search/read operations into model-callable tools, and how to refactor for best practices.

## What “Tool Calling” Means

Foundation Models let the model call developer-defined functions (“tools”) at runtime to fetch data or perform actions. A tool is a small, focused capability the model can invoke with typed arguments and receive structured results from.

At a glance:
- Define a `Tool` type with a clear `name` and `description`.
- Define `Arguments` with `@Generable` and `@Guide` to constrain input.
- Implement `call(arguments:)` to do the work and return a structured `Output` (also `PromptRepresentable`, often another `@Generable`).
- Register tools when creating the `LanguageModelSession`. The model decides when to call them.

## The Tool Protocol (from the guide)

```swift
struct MyTool: Tool {
    let name = "getCurrentWeather"
    let description = "Gets current weather for a city including temperature, humidity, and conditions"

    @Generable
    struct Arguments {
        @Guide(description: "City name to fetch weather for")
        var city: String

        @Guide(description: "Units of measure", .oneOf(["metric", "imperial"]))
        var units: String?
    }

    struct Output: PromptRepresentable { /* …or @Generable */ }

    func call(arguments: Arguments) async throws -> Output {
        // Fetch data, return structured Output
    }
}
```

Key points (Basic Tool Use → Tool Building Best Practices):
- Keep tools focused: a single action with a narrow purpose.
- Write clear names/descriptions; avoid abbreviations.
- Use `@Generable` + `@Guide` to constrain and document arguments (e.g., ranges, oneOf).
- Return structured, compact output; keep it immediately useful to the model.
- Handle errors explicitly with meaningful messages; don’t silently swallow failures.
- Bound runtime (timeouts, result caps) to guarantee responsiveness.

## Reusing @Generable with External Providers

From “Integrating External JSON APIs”: the same `@Generable` types you use for Apple’s on-device models also produce a JSON Schema that external providers (OpenAI, Anthropic, Google, etc.) accept for tool/function calling or strict JSON outputs. You can export `GenerationSchema` from your types and send it to providers as a single source of truth. This enables a clean fallback path when Apple Intelligence isn’t available.

## Where OpenAgents Is Today

We already have internal, bounded operations (“tools” in the OpenAgents sense) that the orchestrator calls directly:
- `session.list`, `session.search`, `session.read`, `session.analyze` implemented in `OpenAgentsCore/Orchestration/SessionTools.swift` and dispatched via `ToolExecutor`.
- FM is used for planning (typed plan/schemas) and final analysis synthesis, but tool calls are executed by our runtime rather than the FM’s built-in tool-calling loop.

This document proposes adding model-callable tools that wrap these existing operations so the FM can opportunistically decide to call them when it needs data. We’ll keep our deterministic plan path and add a tool-calling path for more autonomous, multi-step exploration.

## Implementing session.search and session.read as FM Tools

Below are skeletons that adapt our existing `SessionSearchTool` and `SessionReadTool` into FM `Tool`s (names/descriptions tuned for the model). They delegate to the current implementations in `SessionTools.swift` and preserve all safety caps/timeouts.

```swift
import Foundation
#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionSearch: Tool {
    let name = "session.search"
    let description = "Search conversation history for a regex pattern with limited context (bounded results)."

    @Generable
    struct Arguments {
        @Guide(description: "Regex pattern to match (case-insensitive)")
        var pattern: String

        @Guide(description: "Provider filter: claude-code, codex, or omit for both", .oneOf(["claude-code","codex"]))
        var provider: String?

        @Guide(description: "Optional session IDs to scope the search (max 50)", .count(0...50))
        var sessionIds: [String]?

        @Guide(description: "Maximum matches to return (1-200)", .range(1...200))
        var maxResults: Int?

        @Guide(description: "Context lines before/after each match (0-5)", .range(0...5))
        var contextLines: Int?
    }

    // Reuse existing result type, or create a compact @Generable Output mirroring SessionSearchResult
    struct Output: PromptRepresentable, Codable {
        let totalMatches: Int
        let truncated: Bool
        let matches: [Match]

        struct Match: Codable {
            let sessionId: String
            let provider: String
            let lineNumber: Int
            let line: String
            let contextBefore: [String]?
            let contextAfter: [String]?
        }

        func promptRepresentation() throws -> String { // minimal stringification for the model
            return "matches: \(matches.count), truncated: \(truncated)"
        }
    }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionSearchTool()
        let res = try await tool.search(
            pattern: a.pattern,
            provider: a.provider,
            sessionIds: a.sessionIds,
            maxResults: a.maxResults,
            contextLines: a.contextLines
        )
        let matches = res.matches.map { m in
            Output.Match(
                sessionId: m.sessionId,
                provider: m.provider,
                lineNumber: m.lineNumber,
                line: m.line,
                contextBefore: m.contextBefore,
                contextAfter: m.contextAfter
            )
        }
        return Output(totalMatches: res.totalMatches, truncated: res.truncated, matches: matches)
    }
}

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionRead: Tool {
    let name = "session.read"
    let description = "Read a bounded slice of a conversation session and extract event summaries + file references."

    @Generable
    struct Arguments {
        @Guide(description: "Session ID to read")
        var sessionId: String

        @Guide(description: "Provider for the session", .oneOf(["claude-code","codex"]))
        var provider: String

        @Guide(description: "Start line (1-based)", .range(1...10_000_000))
        var startLine: Int?

        @Guide(description: "End line (inclusive)", .range(1...10_000_000))
        var endLine: Int?

        @Guide(description: "Max events (1-200)", .range(1...200))
        var maxEvents: Int?
    }

    struct Output: PromptRepresentable, Codable {
        let sessionId: String
        let truncated: Bool
        let totalEvents: Int
        let fileReferences: [String]

        func promptRepresentation() throws -> String {
            return "events: \(totalEvents), files: \(fileReferences.prefix(5).joined(separator: ", "))"
        }
    }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionReadTool()
        let res = try await tool.read(
            sessionId: a.sessionId,
            provider: a.provider,
            startLine: a.startLine,
            endLine: a.endLine,
            maxEvents: a.maxEvents
        )
        return Output(
            sessionId: res.sessionId,
            truncated: res.truncated,
            totalEvents: res.events.count,
            fileReferences: res.fileReferences
        )
    }
}
#endif
```

Notes:
- These wrappers keep our bounded behavior (time/byte caps, event caps) and reuse the existing parsing code.
- You can also expose `session.list` and `session.analyze` as tools, but prioritize search/read so the model can chain: search → read → analyze.

## Registering Tools in ExploreOrchestrator

When creating the `LanguageModelSession` in `ExploreOrchestrator`, register these tools so the model can call them directly when producing a plan or analysis.

```swift
#if canImport(FoundationModels)
let tools: [any Tool] = {
    var ts: [any Tool] = []
    if #available(iOS 26.0, macOS 26.0, *) {
        ts.append(FMTool_SessionSearch())
        ts.append(FMTool_SessionRead())
        // Optional: expose session.list/analyze if desired
    }
    return ts
}()

let session = LanguageModelSession(
    model: model,
    tools: tools,
    instructions: instructions
)
try? session.prewarm(promptPrefix: nil)
#endif
```

Then, in the orchestration loop, you have two modes:
- Deterministic plan (what we ship today) → execute via `ToolExecutor`.
- Model-driven tool calling → listen for tool requests from the FM session, dispatch to the registered tool implementation (Foundation Models handles this internally), then continue the conversation.

We can keep both: keep the deterministic typed plan for baseline reliability, and allow opportunistic autonomous calls for richer traversals when the model sees fit.

## Best Practices We Should Adopt

From the guide’s Basic Tool Use + Tool Building Best Practices, applied to OpenAgents:

- Clear names & descriptions
  - Use action-oriented names: `session.search`, `session.read`, `session.list`, `session.analyze`.
  - Description must include: what, when, and bounds (e.g., “bounded to 200 matches”).

- Argument constraints with `@Guide`
  - Ranges (`.range`), enumerations (`.oneOf`), and counts (`.count`) help the model call tools correctly.
  - Keep arguments minimal; infer defaults server-side.

- Structured outputs
  - Return compact, informative fields. Avoid very large blobs; prefer counts, paths, and short snippets.
  - Use `PromptRepresentable` for quick summarization inside the model loop.

- Strong bounding + timeouts
  - Preserve caps (time/bytes/events) and log when they trip.
  - Never block the tool loop; always make forward progress.

- Error handling
  - Throw/return meaningful errors the model can recover from (e.g., “pattern invalid”, “session not found”).
  - Surface recoverable hints (e.g., “try provider=codex”).

- Observability
  - Log “tool start/stop”, input sizes, output sizes, and elapsed time.
  - Correlate with ACP tool_call/tool_call_update via `call_id` for UI.

- Streaming considerations
  - Prefer small outputs. When large, return summaries in `Output` and let the model follow-up with more targeted reads.

## Refactor Plan for OpenAgents

Short, incremental steps to align with best practices without disrupting the working flow:

1) Introduce FM tool wrappers (as above)
   - New file: `OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift` (or `Tooling/`) guarded by `#if canImport(FoundationModels)`.
   - Wrap existing `SessionSearchTool` / `SessionReadTool` logic; no duplication.

2) Register tools in `ExploreOrchestrator`
   - Pass `tools:` to `LanguageModelSession` during plan/analysis.
   - Keep deterministic plan path; allow tool calling in analysis/follow-up prompts.

3) ACP streaming bridge for FM tool calls
   - Map FM tool invocation lifecycle to ACP `tool_call` / `tool_call_update` events so iOS shows consistent rows with progress.
   - Reuse `call_id` correlation and in-place row updates we’ve already implemented in `BridgeManager`.

4) Tighten argument guides
   - Add `.range`, `.oneOf`, `.count` to constrain model inputs and reduce invalid calls.

5) Validation and tests
   - Add unit tests for wrapper tools (argument validation, error surfaces, caps).
   - Add a smoke test that registers tools in a session and verifies a simple tool-call loop.

## Security & External Providers

If we enable external fallbacks, route keys through a server/proxy (see “Integrating External JSON APIs”). Only allow direct client calls when users provide their own keys, stored in Keychain. Reuse `@Generable.generationSchema` to define schemas once.

## Summary

- We will expose `session.search` and `session.read` as model-callable tools by wrapping the existing implementations with the FM `Tool` protocol.
- We’ll register these tools in `ExploreOrchestrator` to let the model invoke them autonomously while keeping our deterministic plan as a baseline.
- We’ll adopt guide-backed argument constraints, structured outputs, bounded execution, and strong logging/ACP streaming to align with the guide’s best practices.

