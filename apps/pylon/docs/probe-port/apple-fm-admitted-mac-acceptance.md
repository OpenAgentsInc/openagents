# Apple FM Admitted-Mac Acceptance Runbook

Date: 2026-06-07

Status: local admitted-hardware runbook for Probe issue #170.

## Purpose

The Apple FM backend is useful for bounded local Probe work, but it is not a
global Codex replacement claim. Live acceptance is admitted-Mac only and is not
part of default CI.

Default CI uses fake bridge/runtime tests so developers without eligible Apple
Silicon and Apple Intelligence do not see false failures.

## Retained Cases

The retained comparison cases are:

- `read_file_answer`
- `list_then_read`
- `search_then_read`
- `shell_then_summarize`
- `patch_then_verify`
- `approval_pause_or_refusal`

CI coverage for those cases lives in
`packages/runtime/tests/apple-fm-acceptance.test.ts`. That test exercises the
Probe runtime boundary and tool-callback receipts with fake local tools.

## Live Local Preconditions

Run live Apple FM checks only on a Mac that is admitted for Apple Foundation
Models:

- Apple Silicon host
- Apple Intelligence available and enabled for the current OS/user context
- local Apple FM bridge running and reachable
- no ChatGPT account auth, OpenAI API key, or OpenAgents product surface provider grant required

If the host is not admitted, record the result as `unsupported` or
`unavailable`, not `failed`.

## Live Smoke Commands

Check readiness:

```sh
bun run --cwd packages/runtime probe apple-fm status
```

Run a plain-text smoke:

```sh
bun run --cwd packages/runtime probe apple-fm smoke --prompt "Summarize this repository in one sentence."
```

Run a live tool-use stream through the local Swift Foundation bridge. The demo
now uses the static Blueprint registry fixture, typed signature lookup,
backend-independent Probe tool menu planner, and Apple FM projector before
creating the Foundation Models session:

```sh
bun run --cwd packages/runtime probe apple-fm tool-stream-demo --path README.md --prompt "Use the Blueprint-selected read_file tool to inspect README.md and report the first heading."
```

Use a non-default bridge URL only from trusted local configuration:

```sh
PROBE_APPLE_FM_BASE_URL=http://127.0.0.1:11435 bun run --cwd packages/runtime probe apple-fm status
```

## Interpreting Results

Acceptance receipts must preserve:

- backend kind `apple_fm_bridge`
- model id `apple-foundation-model`
- Blueprint lookup id
- Blueprint menu id
- Blueprint registry version ref
- selected Program Signature refs
- selected tool refs
- availability facts
- usage truth as `exact`, `estimated`, or `unknown`
- relevant tool/refusal facts

Apple FM can pass a retained case and still be weaker than Codex or a stronger
Qwen route for larger coding-agent work. Do not publish a global parity claim
from these checks. Treat the suite as evidence for bounded local offload and
Pylon/SHC capability routing.

## Default CI Boundary

Default CI should run:

```sh
bun run test
```

That command intentionally runs fake bridge/runtime coverage only. Live
admitted-Mac checks should remain an operator/local runbook until the project
adds an explicit hardware-gated live test lane.
