# Conversation Titles & Sidebar Summaries

This app lists recent chats in the left sidebar using the rollouts that Codex writes to `${CODEX_HOME}/sessions/YYYY/MM/DD/rollout-*.jsonl`. Titles are derived from the content of each rollout with a few practical heuristics to keep them readable and relevant.

## What we scan
- The first ~120 JSONL lines of each rollout file (supports both flattened `{ type, payload }` and nested `{ item: { type, payload } }` formats).
- We look for:
  - `EventMsg::UserMessage` (preferred)
  - `ResponseItem::Message { role: "user" }`

## What we ignore for titles
- Instruction wrappers (`<user_instructions>...</user_instructions>`) and environment context (`<environment_context>...</environment_context>`).
- Markdown headings and boilerplate (e.g., a leading `#` line or the string `Repository Guidelines`).
- Empty/whitespace‑only lines.

## Title selection
- Take the first non‑instruction user message, sanitize it, and truncate to ~80 chars.
- Sanitization:
  - Strips wrappers and markdown `#` heading markers
  - Uses the first non‑blank line
- If no good candidate is found, fall back to the file name.

## Items hidden by default
- Instruction/context messages still exist in the transcript, but render collapsed as a small toggle (e.g., “Show instructions”, “Show context”). Click to expand.

## When the list refreshes
- On app launch.
- When a new session is created:
  - Clicking “New chat” restarts the Codex proto, which emits a `session_configured` event.
  - The UI listens for that event and refreshes the sidebar automatically.

## Notes & future improvements
- Manual renaming: If we introduce per‑chat metadata later, we can add an inline rename UI and persist custom titles.
- Additional fallbacks: If you have unique project‑level boilerplate, we can extend the ignore list here without touching core Codex.

