# Environment Context Envelope

File: `codex-rs/core/src/environment_context.rs`

Encodes the user’s execution environment as a small XML snippet that can be
inserted into the conversation for the model to consume.

## Fields

- `cwd` — working directory for relative path resolution.
- `approval_policy` — on-request / unless-trusted / never.
- `sandbox_mode` — read-only / workspace-write / danger-full-access.
- `network_access` — restricted / enabled.
- `writable_roots` — explicit write access list when using workspace‑write.
- `shell` — human‑readable shell name (initial context only).

## Serialization

- `EnvironmentContext::serialize_to_xml()` renders the envelope between the
  special tags defined in protocol constants.
- `impl From<TurnContext>` provides convenient conversion from live session
  state.

