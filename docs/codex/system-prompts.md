# System Prompts

Authoritative, code‑linked documentation for how Codex assembles and sends system prompts to models.

## Sources of System Instructions

- Default base instructions: `codex-rs/core/prompt.md`
- GPT‑5 Codex base instructions: `codex-rs/core/gpt_5_codex_prompt.md`
- Review mode instructions: `codex-rs/core/review_prompt.md`
- Compaction templates (used when summarizing long histories):
  - `codex-rs/core/templates/compact/prompt.md`
  - `codex-rs/core/templates/compact/history_bridge.md`

## Selection Logic (by model)

- The active model’s family determines the base instructions string at runtime.
  - Include sites: `codex-rs/core/src/model_family.rs:6`, `codex-rs/core/src/model_family.rs:7`
  - For Codex/GPT‑5 Codex models, `base_instructions` is `gpt_5_codex_prompt.md`.
    - Mapping: `codex-rs/core/src/model_family.rs:102`–`codex-rs/core/src/model_family.rs:108`
  - For all other supported families, `base_instructions` is `prompt.md`.

- Some families require extra “apply_patch” guidance unless the tool is present:
  - Logic: `codex-rs/core/src/client_common.rs:43`–`codex-rs/core/src/client_common.rs:59`
  - Instructions source: `codex-rs/apply-patch/apply_patch_tool_instructions.md` (included via `APPLY_PATCH_TOOL_INSTRUCTIONS`).

## Where system instructions go on the wire

- OpenAI Responses API: placed in the top‑level `instructions` field.
  - Assignment: `codex-rs/core/src/client.rs:170`, `codex-rs/core/src/client.rs:208`–`codex-rs/core/src/client.rs:213`

- Chat Completions API: sent as the first chat message with `role: "system"`.
  - Construction: `codex-rs/core/src/chat_completions.rs:41`–`codex-rs/core/src/chat_completions.rs:42`

- Prompt cache key: responses include `prompt_cache_key` to enable server‑side prompt caching across turns.
  - Field: `codex-rs/core/src/client.rs:219`

## Overrides and Profiles

- Per‑session override: `Config.base_instructions` (optional).
  - Parsed and stored in config: `codex-rs/core/src/config.rs:984`–`codex-rs/core/src/config.rs:1011`
  - The file‑based override comes from `experimental_instructions_file` (path) and is loaded by `get_base_instructions(...)`.
    - Loader: `codex-rs/core/src/config.rs:1075`–`codex-rs/core/src/config.rs:1115`

- Per‑turn override in Review Mode: Codex uses the review prompt as base instructions.
  - Source constant: `codex-rs/core/src/client_common.rs:20`–`codex-rs/core/src/client_common.rs:21`
  - Applied for the review thread: `codex-rs/core/src/codex.rs:1574`, `codex-rs/core/src/codex.rs:1600`–`codex-rs/core/src/codex.rs:1608`
  - The initial review input includes the review instructions plus the task: `codex-rs/core/src/codex.rs:1612`–`codex-rs/core/src/codex.rs:1615`

## What else is prefixed with the conversation (not system instructions, but adjacent)

- User instructions (from config + project `AGENTS.md`) are wrapped and sent as a user message:
  - Wrapper type and serialization to `<user_instructions>…</user_instructions>`: `codex-rs/core/src/user_instructions.rs:17`–`codex-rs/core/src/user_instructions.rs:33`
  - Discovery/merge of `AGENTS.md` from repo root → cwd, plus size limits: `codex-rs/core/src/project_doc.rs:1`–`codex-rs/core/src/project_doc.rs:20`, `codex-rs/core/src/project_doc.rs:27`–`codex-rs/core/src/project_doc.rs:41`
  - Initial injection point alongside environment context: `codex-rs/core/src/codex.rs:715`–`codex-rs/core/src/codex.rs:726`

- Environment context is wrapped and sent as a user message after (or without) user instructions:
  - Wrapper and XML serialization: `codex-rs/core/src/environment_context.rs:126`–`codex-rs/core/src/environment_context.rs:166`
  - Injected in initial context: `codex-rs/core/src/codex.rs:720`–`codex-rs/core/src/codex.rs:725`

These two messages are part of the model input, but they are not “system prompts”. They are explicit user‑role messages that give the model the operating context.

## Context window accounting (baseline tokens)

- The UI estimates how much of the model’s context window is user‑controllable by subtracting an internal baseline that covers the system instructions and fixed tool schemas.
  - Baseline constant: `codex-rs/protocol/src/protocol.rs:609`
  - Explanation and computation: `codex-rs/protocol/src/protocol.rs:640`–`codex-rs/protocol/src/protocol.rs:668`

## Tests that exercise system prompts

- Base‑instructions override is propagated to API requests (Responses API):
  - `codex-rs/core/tests/suite/client.rs:382`–`codex-rs/core/tests/suite/client.rs:416`

- Default base instructions and conditional apply_patch guidance are stable across turns:
  - `codex-rs/core/tests/suite/prompt_caching.rs:134`–`codex-rs/core/tests/suite/prompt_caching.rs:149`
  - `codex-rs/core/tests/suite/prompt_caching.rs:216`–`codex-rs/core/tests/suite/prompt_caching.rs:231`

- Initial context prefixing (user instructions + environment context) and consistency across turns:
  - `codex-rs/core/tests/suite/prompt_caching.rs:236`–`codex-rs/core/tests/suite/prompt_caching.rs:304`

## Related docs

- Prompt system (broader, including custom prompts and caching): `docs/systems/prompts.md`
- Model family specifics: `docs/systems/core-model-family.md`
- Environment context envelope: `docs/systems/environment-context.md`

