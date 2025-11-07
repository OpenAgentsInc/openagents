#+ Advanced Chat Patterns

Designing durable conversations requires managing transcripts, token budgets, and user feedback.

## Transcript & Memory

- Use a persistent `LanguageModelSession` so the transcript carries context across turns
- Persist transcripts to disk; recreate sessions with `LanguageModelSession(transcript:)`
- Append tool results into the transcript to ground future turns

## Token Budgeting

- Estimate usage and apply a sliding window: keep the last N turns plus a brief summary of earlier context
- Summarize older segments into a short “context note” when approaching limits

## Streaming Responses

- For chat UIs, observe `session.transcript` rather than manually appending; snapshot streaming updates entries in place

## UI Construction

- Build from transcript entries (user, assistant, tool). Use stable IDs to avoid list jitter

## Learning from Users (Feedback)

- Capture lightweight feedback signals (thumbs up/down, category) per assistant message
- Feed into evaluation, prompt tuning, and future instruction updates (out‑of‑band)

## OpenAgents Guidance

- Keep ExploreOrchestrator as the owner of the session (actor). All `respond/stream` calls flow through it, ensuring serialization
- Persist transcript keyed by orchestration session; restore on relaunch to continue chat context
- Add a simple feedback API on message bubbles to collect outcome signals; store with timestamps and categories

