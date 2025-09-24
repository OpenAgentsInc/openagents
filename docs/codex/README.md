# Systems Documentation Index

This directory documents the internal architecture of Codex. Start with
`architecture.md`, then dive into specific subsystems as needed.

## Overview

- architecture.md — high‑level architecture and data flow
- prompts.md — prompt system and caching (already present)
- system-prompts.md — how system instructions are selected and sent
- sandbox.md — platform sandboxing and bypass options (already present)

## Core runtime

- core-codex.md — session orchestration and tool dispatch
- core-client.md — Responses/Chat client streaming behavior
- core-client-common.md — `Prompt`, `ResponseEvent`, and helpers
- core-sse.md — SSE processing details
- chat-completions.md — chat adapter and mapping
- core-model-family.md — model families and capabilities
- core-openai-model-info.md — context windows and token limits
- openai-wire-providers.md — provider definitions and wire API
- core-openai-tools.md — tool definitions and JSON schema handling
- core-config-internals.md — config structure and precedence
- core-error-handling.md — error types and UI mapping
- rate-limits.md — header parsing and UI hints

## Exec and safety

- core-exec.md — exec, sandbox selection, truncation, timeouts
- core-exec-streaming.md — delta streaming and aggregation
- core-exec-command.md — interactive sessions and stdin writes
- core-approval.md — approval modes and trusted commands
- core-safety.md — safety policy and platform sandbox choice
- azure-responses-compat.md — Azure Responses API quirks
- notifications.md — external/TUI notifications
- environment-context.md — environment envelope serialization

## Files and history

- apply-patch.md — safe patch grammar and integration
- turn-diff-tracker.md — unified diff across a turn
- core-rollout.md — rollout persistence and resume
- core-conversation-history.md — conversation state and compaction
- core-compaction.md — summarization strategy and triggers
- file-search.md — search and ranking
- protocol-overview.md — shared events and data structures
