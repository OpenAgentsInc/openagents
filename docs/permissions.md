Permissions, Sandbox, and Approvals
===================================

This doc explains how our bridge and mobile app configure Codex CLI permissions, why you may sometimes see confusing "sandbox" lines in the logs, and what the minimal/recommended flag sets are.

TL;DR
-----

- The bridge starts Codex CLI with full access by default (no sandbox, no approvals).
- The app also prepends a one‑line JSON config to stdin as a belt‑and‑suspenders override.
- If you prefer safer defaults, switch to `workspace-write` (see “Recommended safer setup”).
- Log lines that say `sandbox: read-only` can be misleading; rely on the bridge’s spawn args and whether writes succeed.

What the bridge passes to Codex CLI
-----------------------------------

At spawn, the bridge adds these (unless already provided via `CODEX_ARGS`/CLI):

- `--dangerously-bypass-approvals-and-sandbox`
- `--sandbox danger-full-access`
- `-c sandbox_permissions=["disk-full-access"]`
- `-c sandbox_mode="danger-full-access"`
- `-c approval_policy="never"`

It also adds a model + quality setting:

- `-m gpt-5`
- `-c model_reasoning_effort="high"`

You can see these in the spawn log (example):

```
args=[..., "--dangerously-bypass-approvals-and-sandbox", "-s", "danger-full-access", "-c", "sandbox_permissions=[\"disk-full-access\"]", "-c", "sandbox_mode=\"danger-full-access\"", "-c", "approval_policy=\"never\"", "exec", "--json", "resume", "--last"]
```

What the app prepends to stdin
------------------------------

Before the prompt, we write an explicit JSON line:

```
{"sandbox":"danger-full-access","approval":"never"}
```

Some Codex builds read an initial JSON object as runtime configuration. This line ensures any legacy or prompt‑based default (e.g. a stray `"sandbox":"read-only"`) is superseded by our desired mode.

Why you might still see `sandbox: read-only`
-------------------------------------------

Codex prints a configuration blob early on. In some versions, that header reflects a conservative default before flags/config override are fully applied.

We log both:

- a preview of what the app wrote to stdin (look for `writing to child stdin … preview=`), and
- any `sandbox` lines from Codex stdout/stderr (look for `observed sandbox string …`).

Trust the spawn args and whether writes succeed (e.g., creating `docs/scratchpad.md`). Those reflect the effective policy.

Minimal flag sets (from Codex docs)
-----------------------------------

- “YOLO” (no sandbox, no prompts):
  - `--dangerously-bypass-approvals-and-sandbox`
  - or `--sandbox danger-full-access --ask-for-approval never`

- “Full‑auto” (safe default for trusted repos):
  - `--full-auto` (equivalent to `--sandbox workspace-write --ask-for-approval on-request`)
  - Network is disabled by default in `workspace-write` unless enabled in config:
    - `-c '[sandbox_workspace_write].network_access=true'`

Recommended safer setup (if you don’t want full bypass)
------------------------------------------------------

Edit bridge flags (or provide `CODEX_ARGS`) to prefer:

```
--sandbox workspace-write \
--ask-for-approval on-request \
-c '[sandbox_workspace_write].network_access=true'   # optional
```

This lets Codex edit files in the repo and only asks when it needs to leave the workspace or do riskier things.

Overriding via env
------------------

Append raw flags with `CODEX_ARGS`, e.g.:

```
CODEX_ARGS='--sandbox workspace-write --ask-for-approval on-request' \
  cargo run -p codex-bridge -- --bind 0.0.0.0:8787
```

If you pass `CODEX_ARGS`, the bridge keeps your values and only adds missing essentials.

App toggles vs. bridge flags
----------------------------

The Settings toggles (read‑only/write, network, approvals, attach preface) control the preface text and the JSON config we prepend. The bridge flags are authoritative for the actual sandbox process. If you want to reduce noise, you can turn off “Attach environment preface to prompts.”

Verifying effective policy
--------------------------

- Check spawn args in logs for `-s danger-full-access` (or your chosen mode) and any `-c …` overrides.
- Look for `writing to child stdin … preview={"sandbox":"danger-full-access"…}`.
- Test writes with a simple change: create `docs/scratchpad.md` via the app.

Are all current flags strictly necessary?
----------------------------------------

Strictly speaking, **no** — any one of the following is sufficient:

- `--dangerously-bypass-approvals-and-sandbox` (alone), or
- `--sandbox danger-full-access --ask-for-approval never`.

We keep `-c sandbox_mode="danger-full-access"` and `-c sandbox_permissions=["disk-full-access"]` in place to prevent edge cases across Codex versions and ensure full disk access. If you prefer a slimmer set, you can safely remove the extra `-c …` lines and rely on either:

- the bypass flag alone; or
- `-s danger-full-access -a never`.

For safety, consider the “Recommended safer setup” instead of full bypass.
