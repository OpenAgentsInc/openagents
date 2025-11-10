# Harmony Guidelines for GPT‑OSS in OpenAgents

Last updated: 2025‑11‑10

These notes summarize the key points from OpenAI’s Harmony response format as they apply to our GPT‑OSS integration.

## Always Use the Chat Template
- Do not hand‑roll prompts. Use `ChatSession` (MLX) or `tokenizer.applyChatTemplate(messages:)` to render Harmony‑compliant prompts.
- Construct messages with roles: `system`, `developer`, `user`, `assistant` (and `tool` as needed for tool output).

## Channels
- Assistant messages may specify channels:
  - `final`: user‑visible output
  - `analysis`: chain‑of‑thought; never show to users
  - `commentary`: preambles/tool calls; may be user‑visible when it’s a plan/preamble

UI policy:
- Stream only `final` channel to the chat bubble. Suppress `analysis` channel. Render `commentary` preambles as a lightweight plan if desired.

## Stop Tokens and History
- `<|return|>` indicates “done generating”. Normalize persisted assistant replies to end with `<|end|>` when appending to history.
- Drop prior `analysis` on the next turn unless the previous step involved tool calls; in that case, include the CoT snippet for continuity per harmony guidance.

## Function/Tool Calls
- Define tools in the `developer` message (`functions` namespace) or use built‑ins in the `system` message.
- The model will emit tool calls with recipient `to=functions.name` and `<|constrain|>json` followed by JSON arguments.
- After execution, supply a tool message from `{toolname}` role with the output, channel `commentary`.

## Reasoning Effort
- Set `Reasoning: low|medium|high` in the `system` message. Default is medium; we can expose this in Settings.

## Tests
- Add a unit test that validates the chat template rendering (first token ID prefix) for a trivial conversation.
- Add an integration test that verifies channel filtering and `<|return|>` normalization in history.

