# OpenClaw Policy Parity Report

Date: 2026-02-19
Status: Active

## Scope

This report tracks fixture parity for the imported OpenClaw tool-policy baseline helpers ported into OpenAgents runtime.

- OpenClaw source: `src/agents/tool-policy.ts`
- OpenClaw commit: `8e1f25631b220f139e79003caecabd11b7e1e748`
- Runtime port: `lib/openagents_runtime/tools/policy/openclaw_tool_policy.ex`
- Fixture set: `test/fixtures/openclaw/tool_policy_parity_cases.json`
- Parity runner: `test/openagents_runtime/parity/openclaw_tool_policy_parity_test.exs`

## Upstream Capture Mechanism

OpenClaw outputs are captured directly from the OpenClaw TypeScript implementation via:

```bash
bun apps/openagents-runtime/scripts/capture_openclaw_tool_policy_parity.mjs
```

The script imports OpenClaw functions from the local checkout, evaluates all fixture cases, and writes `expected_openclaw` results back into the fixture file.

## Current Fixture Coverage

- `normalize_tool_name`
- `expand_tool_groups`
- `build_plugin_tool_groups`
- `expand_policy_with_plugin_groups`
- `strip_plugin_only_allowlist`

Current case count: `7`

## Verification Command

```bash
cd apps/openagents-runtime
mix test test/openagents_runtime/parity/openclaw_tool_policy_parity_test.exs
```

This test is part of the standard `mix test` run and therefore executes in runtime CI.
