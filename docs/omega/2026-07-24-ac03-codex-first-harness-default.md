# AC-03: Codex-first managed-cloud harness default

- Date: 2026-07-24
- Packet: `AC-03`
- OpenAgents issue: [#9207](https://github.com/OpenAgentsInc/openagents/issues/9207)
- Depends on: [#9205](https://github.com/OpenAgentsInc/openagents/issues/9205) for live
  Codex readiness only
- Plan: [2026-07-24-agent-computer-omega-completion-plan.md](./2026-07-24-agent-computer-omega-completion-plan.md)
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: complete (selection policy)

## Result

No-preference managed-cloud turns already prefer Codex first when runtime-
qualified and the ChatGPT/Codex provider is ready.
They fall back through the admitted order on typed unreadiness.
Explicit `harnessId` stays authoritative.

## Authority

| Symbol | Path |
| --- | --- |
| Preference order | `MANAGED_AGENT_COMPUTER_DEFAULT_HARNESS_PREFERENCE_ORDER` |
| Readiness mirror | `DEFAULT_MANAGED_AGENT_COMPUTER_HARNESS_RUNTIME_READINESS` |
| Selector | `selectManagedAgentComputerDefaultHarnessId` |
| Apply before claim | `applyManagedCloudHarnessDefaultSelection` |
| Source | `apps/openagents.com/workers/api/src/khala-cloud-runtime-dispatch.ts` |

Order: `codex`, `claude-code`, `opencode`, `pi`, `goose`, `cursor`, `grok`.

AC-01 has now qualified Codex with a real native-auth Firecracker turn. The
runtime mirror can therefore admit Codex as the first no-preference choice.
Typed unreadiness still advances to the next ready harness.

## Tests

`apps/openagents.com/workers/api/src/khala-cloud-runtime-dispatch.test.ts`:

- Selects Codex when runtime-qualified and the Codex provider is ready
- Falls back to OpenCode when Codex runtime is blocked but Gemini is ready
- Falls back to Claude Code before Gemini harnesses when Codex is blocked

## Scope

- The selection-policy packet did not itself qualify Codex. AC-01 supplied the
  separate real-turn receipt.
- Seven-harness and epic closeout remain separate issue reconciliations.
- Does not change live capacity probes
