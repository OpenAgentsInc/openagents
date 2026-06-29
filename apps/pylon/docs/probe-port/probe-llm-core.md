# Probe LLM Core

Probe's provider-neutral LLM core lives under `packages/runtime/src/llm/`.
It is intentionally smaller than Opencode's `@opencode-ai/llm` package, but it
keeps the same implementation boundary needed for Gemini and future
OpenAI-compatible backends.

The core owns:

- messages and content parts;
- normalized provider events;
- token usage accounting;
- model request envelopes;
- named tool definitions;
- local tool dispatch and tool-result projection.

Backend-specific code should lower this core contract into provider wire
formats. For example, Gemini should lower tool definitions into native function
declarations and tool results into `functionResponse` turns. Apple FM may keep
its callback bridge, but new API backends should not add backend-specific tool
contracts when the provider-neutral layer is sufficient.

The core must not store raw provider credentials, raw provider payloads, private
repository contents, or raw prompts in public evidence. Backend receipts should
record redacted summaries and provider metadata only where the existing Probe
projection rules allow it.
