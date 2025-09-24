# Core: Model Families

Location: `codex-rs/core/src/model_family.rs`

Determines capabilities and base instructions for a model slug, and influences
prompt assembly and tool selection.

## Key fields on ModelFamily

- `slug`/`family` — used for lookups and telemetry.
- `needs_special_apply_patch_instructions` — when true, append
  `APPLY_PATCH_TOOL_INSTRUCTIONS` to the base instructions unless an
  apply_patch tool is explicitly provided.
- `supports_reasoning_summaries` — enables `reasoning` param in Responses
  payload.
- `reasoning_summary_format` — whether to emit experimental structured
  summaries.
- `uses_local_shell_tool` — models that expect a native `local_shell` tool.
- `apply_patch_tool_type` — prefer freeform vs function for apply_patch.
- `base_instructions` — resolves to `prompt.md` or GPT‑5 Codex variant.

## Family detection

`find_family_for_model(slug)` uses prefix matching to set options for:

- `o3`, `o4-mini` — reasoning summaries enabled.
- `codex-mini-latest` — reasoning summaries + local_shell.
- `gpt-4.1`, `gpt-4o`, `gpt-3.5` — extra apply_patch instructions.
- `gpt-oss*` — prefer function apply_patch tool.
- `gpt-5*` and `gpt-5-codex*` — reasoning summaries and special prompts.

## Defaulting

`derive_default_model_family(model)` provides a permissive family with base
instructions when an unknown slug is used.

