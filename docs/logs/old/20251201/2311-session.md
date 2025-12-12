## Session log — 2025-12-01

- Ported the edit tool to Effect with schema validation, abort handling, and diff output.
- Added a CLI harness for the edit tool and Bun tests covering success/non-unique/missing-text cases.
- Integrated the OpenRouter SDK:
  - Created typed client helpers (`loadOpenRouterEnv`, `createOpenRouterClient`, `toolToOpenRouterDefinition`).
  - Default model `x-ai/grok-4.1-fast`, env loading via `.env.local`.
  - Switched chat calls to SDK (`chat.send`) with proper tool definitions.
- Added an OpenRouter CLI (`src/llm/openrouter-cli.ts`) for manual prompts.
- Created demos:
  - `demo:edit` — local edit tool demo on `docs/scratchpad/demo.txt`.
  - `demo:agent-tools` — calls OpenRouter with the edit tool, executes the returned tool call, logs colored diff.
- Updated `AGENTS.md` to require using `bun add -E` for dependencies.
- Installed `@openrouter/sdk` dependency.
- Committed and pushed all changes to `main`.
