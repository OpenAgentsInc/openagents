# LanguageModelSession and Stateful Sessions (Apple Foundation Models)

This document summarizes the Foundation Models guidance on `LanguageModelSession`, stateful multi‑turn conversations, transcripts, snapshots/streaming, and how we should apply these practices in OpenAgents.

## Core Concepts

- `LanguageModelSession` represents a conversation context with the on‑device model.
- Sessions can be single‑turn or multi‑turn; multi‑turn sessions maintain memory via a transcript.
- Responses can be produced in one shot or streamed as snapshots (partial objects that fill in over time).
- Sessions can register tools and use typed instructions (`Instructions`) alongside user prompts.

## Creating Sessions

Common initializers (exact shape varies by SDK version):
- `LanguageModelSession()` — defaults.
- `LanguageModelSession(model: SystemLanguageModel.default)` — explicit model.
- `LanguageModelSession(instructions: Instructions)` — attach system instructions.
- `LanguageModelSession(tools: [Tool], instructions: Instructions)` — register tools.
- `LanguageModelSession(transcript: Transcript)` — restore stateful context.

Typical use:

```swift
let session = LanguageModelSession(
    model: SystemLanguageModel.default,
    tools: [],
    instructions: Instructions("You are a helpful assistant.")
)
let response = try await session.respond(to: "Write a haiku about the ocean")
```

## Single‑Turn vs Multi‑Turn

- Single‑turn: create a session, call `respond`, discard.
- Multi‑turn: keep the same session instance and call `respond` repeatedly; prior turns influence future outputs.
- For long conversations, manage token budget by summarizing and/or sliding windows (see Transcript below).

## Instructions vs Prompts

- `Instructions` are “system messages” that establish behavior/persona and safety constraints.
- `prompt` (user input) is the per‑turn request.
- Keep instructions concise; place examples and formatting rules in instructions when they apply across turns.

## Prewarming

- `session.prewarm(promptPrefix:)` improves perceived latency by loading model resources.
- Call prewarm when the user is about to interact (typing, screen appears, app foregrounded) to avoid cold starts.
- You can pass a common prefix (instructions/context) to warm the tokenizer/cache.

## Streaming with Snapshots

- Foundation Models stream snapshots (complete partial objects) rather than raw token deltas.
- Benefits: bind snapshot content directly to SwiftUI views as fields become non‑nil.
- Supports both plain text and structured `@Generable` models.
- Pattern: iterate `for try await snapshot in stream` and update bound state from `snapshot.content`.

## Error Handling (selected)

`LanguageModelSession.GenerationError` includes cases like:
- `exceededContextWindowSize` — shrink transcript/window.
- `guardrailViolation` — content blocked by safety layers; show a friendly message.
- `assetsUnavailable` — model not available locally.
- `concurrentRequests` — serialize access; avoid multiple simultaneous `respond` calls on the same session.
- `rateLimited`, `decodingFailure`, `unsupportedGuide`, `refusal(_,_)` — handle gracefully and surface actionable info.

## Transcript & Memory

- A `Transcript` models the conversation: entries/segments representing user messages, assistant replies, tool results, etc.
- Token budgeting:
  - Estimate tokens on the transcript and trim or summarize when near limits.
  - Use a sliding‑window approach for long sessions (keep the last N turns + a summary of earlier context).
- Persistence & restoration:
  - Save transcript to disk; recreate sessions with `LanguageModelSession(transcript:)` on app relaunch.

## Best Practices from the Guide

- Keep sessions alive across related turns; don’t recreate unless switching tasks.
- Prewarm before expected use (typing, navigation, foregrounding).
- Stream snapshots for long outputs; update UI incrementally.
- Strictly serialize `respond` calls per session; avoid `concurrentRequests`.
- Summarize older history to stay under context limits; apply sliding windows.
- Register tools on the session to enable model‑initiated retrieval/actions.

## Applying to OpenAgents

We currently use Foundation Models for:
- Plan generation (`ExploreOrchestrator.generateInitialPlan`) — one‑shot
- Final analysis synthesis (`generateFMAnalysis`) — one‑shot

To fully leverage stateful sessions:

1) Maintain a persistent session in ExploreOrchestrator
- Keep a `private var fmSession: LanguageModelSession?` inside the actor.
- Initialize once with `model`, `instructions`, and registered tools (see tool‑calling.md).
- Reuse the same session for plan + analysis so the model carries context forward (recent operations, high‑level goals).

2) Serialize access
- ExploreOrchestrator is an `actor`; keep all `respond` calls inside the actor to avoid `concurrentRequests`.
- If future UI threads need access, expose `await` functions on the actor and queue requests.

3) Prewarm strategically
- Call `session.prewarm(promptPrefix:)` during `startExploration()` and right before analysis.
- For predictable prompts (e.g., goals + workspace name), pass a short prefix.

4) Streaming (optional now, recommended next)
- For analysis or longer generations, switch to `stream` and forward partials to the bridge as ACP `agent_message_chunk` updates.
- Use snapshot content fields directly; compute stable IDs to prevent UI jitter.

5) Transcript management
- Append turns: goals/instructions (system), tool outcomes (assistant/tool), user nudges.
- When token estimates are high, summarize earlier transcript segments into a short “context note” and rebuild a windowed `Transcript`.
- Persist transcript alongside orchestration session ID for resilience across app relaunches.

6) Tool registration
- Register `FMTool_SessionSearch` and `FMTool_SessionRead` on the session to enable model‑driven traversals.
- Keep our deterministic plan execution path in parallel; the model can still call tools opportunistically.

## Minimal Skeleton for a Persistent Session

```swift
#if canImport(FoundationModels)
@available(iOS 26.0, macOS 26.0, *)
final class FMRuntime {
    private(set) var session: LanguageModelSession

    init(tools: [any Tool], baseInstructions: Instructions) {
        self.session = LanguageModelSession(
            model: SystemLanguageModel.default,
            tools: tools,
            instructions: baseInstructions
        )
        try? session.prewarm(promptPrefix: nil)
    }

    func respond(_ prompt: String) async throws -> String {
        // serialize inside owning actor if shared
        let res = try await session.respond(to: prompt)
        return res.content
    }
}
#endif
```

## Refactor Plan (OpenAgents)

- ExploreOrchestrator
  - Add `fmSession` and register FM tools; reuse across plan + analysis.
  - Prewarm at start; prewarm again right before analysis.
  - Optionally adopt streaming for analysis (forward partials via ACP).
  - Handle session errors explicitly; map to user‑visible messages.

- DesktopWebSocketServer
  - If we stream analysis, forward partials to iOS with stable IDs.
  - Continue composing a final message on completion.

- Tests
  - Add a smoke test that initializes a persistent session, calls `respond` twice, and verifies multi‑turn influence.
  - Add a sliding‑window test that rebuilds a Transcript when token estimates exceed limits.

## Takeaway

Using a persistent `LanguageModelSession` with prewarming, transcripts, and (optionally) streaming snapshots gives better latency, richer multi‑turn reasoning, and stable tool‑calling behavior. We’ll incrementally layer this into ExploreOrchestrator while preserving our deterministic, ACP‑first execution path.

