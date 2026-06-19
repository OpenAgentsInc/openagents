# 2026-06-19 Coding-Agent Live Verification Receipt

This is a dereferenceable verification receipt for the three live coding-agent
execution lanes that the OpenAgents product surface exposes: the local **Claude
Agent** bridge, the local **Codex** bridge, and the **Tassadar** exact-execution
executor / replay package. It exists so product-promise records can cite a
single, independently re-run proof rather than prose.

## Honest scope (read first)

- This receipt proves **local single-task execution**: a real coding-agent
  session ran one bounded objective end to end, the stated verification command
  passed, and a public-safe closeout was produced.
- It does **NOT** prove production-scale, at-volume, multi-tenant, unattended,
  packaged-stable-binary, or public-settlement claims. Those gates remain on
  their own promise records.
- All values below are refs/digests and exit/verify outcomes only. No raw
  prompts, transcripts, file contents, provider payloads, credentials, wallet
  material, or raw local paths are reproduced here.

## Environment

- Repo worktree base commit: `b6e523a77` (clean `origin/main`).
- Runtime: `bun 1.3.11` on macOS arm64.
- Run timestamp: 2026-06-19 (UTC timestamps recorded per result below).
- Verifications were re-run independently for this receipt; the producing
  agent did not trust a prior claim of passing.

## 1. Claude Agent bridge — local single-task exec

- Command shape (`apps/pylon`):
  `bun src/index.ts sessions exec --adapter claude_agent
  --objective "Create verified.txt containing exactly: WORKING"
  --worktree <bounded temp git repo> --verify "test -f verified.txt"
  --on-approval auto`
- Auth path used: local `~/.claude` subscription credentials (contributor's own
  credentials, on-device). No OpenAgents-supplied Claude access.
- Result schema: `openagents.pylon.sessions_exec_result.v0.1`
- `ok`: `true`
- `outcome` / `state`: `completed`
- `verify.passed`: `true` (`verify.state: passed`)
- Verify command ref: `command.dev_check.98b58ec98d48bed11139fa07`, `exitCode: 0`,
  `status: passed`
- Changeset: `dirty`, one untracked `.txt` file
  (`file.local_change.c4659fbd0063181107e7c327`) — confirmed to contain exactly
  `WORKING`.
- Session ref: `session.pylon.control.1cf1bc818f4a3e631ba5b9b6`
- Result ref: `result.pylon.control_session.2b6af6bb4019fbf8af1775da`
- Artifact ref: `artifact.pylon.control_session.proof.4a899727f815465697140aa5`
- Started: `2026-06-19T14:11:12.181Z`; Completed: `2026-06-19T14:11:22.791Z`
- Elapsed: `10650 ms` (`polls: 43`, `timedOut: false`)
- Process exit code: `0`

## 2. Codex bridge — local single-task exec

- Command shape (`apps/pylon`):
  `bun src/index.ts sessions exec --adapter codex
  --objective "Create verified.txt containing exactly: WORKING"
  --worktree <bounded temp git repo> --verify "test -f verified.txt"
  --on-approval auto`
- Auth path used: local `~/.codex/auth.json` (owner-held Codex credentials,
  on-device).
- Result schema: `openagents.pylon.sessions_exec_result.v0.1`
- `ok`: `true`
- `outcome` / `state`: `completed`
- Sandbox / network: `sandbox: workspace-write`, `network: disabled`
  (bounded executor — read/edit/test only, no network).
- `verify.passed`: `true` (`verify.state: passed`)
- Verify command ref: `command.dev_check.11e86b70ba6e8a5b90eeb8a3`, `exitCode: 0`,
  `status: passed`
- Changeset: `dirty`, one untracked `.txt` file
  (`file.local_change.6ca25f4886a45a1de8e91b08`) — confirmed to contain exactly
  `WORKING`.
- Session ref: `session.pylon.control.f74c9c542a066e4f73061147`
- Result ref: `result.pylon.control_session.d63bdd7d5a9663fbbde0cdc3`
- Artifact ref: `artifact.pylon.control_session.proof.252ff6e5bfd62284915ea17e`
- Started: `2026-06-19T14:11:40.891Z`; Completed: `2026-06-19T14:11:53.957Z`
- Elapsed: `13166 ms` (`polls: 53`, `timedOut: false`)
- Process exit code: `0`

## 3. Tassadar executor — execute + exact trace replay

- Command (repo root): `bun test packages/tassadar-executor`
- Result: `23 pass`, `0 fail`, `193 expect() calls`, `23 tests across 5 files`,
  `86 ms`
- Process exit code: `0`
- Test files exercised:
  - `packages/tassadar-executor/src/capability-envelope.test.ts`
  - `packages/tassadar-executor/src/compiled-program-corpus.test.ts`
  - `packages/tassadar-executor/src/dense-weight-module.test.ts`
  - `packages/tassadar-executor/src/linked-dense-module.test.ts`
  - `packages/tassadar-executor/src/numeric-executor.test.ts`
- This covers deterministic execution and exact-trace replay of bounded
  compiled-program workloads (the same exact-replay primitive that
  `compute.tassadar_executor_poc.v1` is built on). It is package-level proof of
  the execute + replay mechanism; it is not a live paid network workload and
  grants no settlement, scale, or model-training claim.

## Summary

| Lane | Adapter / target | verify.passed | exit | elapsed | auth |
| --- | --- | --- | --- | --- | --- |
| Claude bridge | `claude_agent` | true | 0 | 10.65 s | `~/.claude` subscription |
| Codex bridge | `codex` | true | 0 | 13.17 s | `~/.codex/auth.json` |
| Tassadar replay | `bun test packages/tassadar-executor` | n/a (23/0) | 0 | 0.086 s | none |

All three lanes executed a real task / replay and passed independently on
2026-06-19 from clean `origin/main` (`b6e523a77`). This receipt is the
dereferenceable evidence cited by the coding-agent product-promise records; it
proves local single-task execution only and does not widen any production-scale,
at-volume, packaged-stable-binary, or public-settlement claim.
