# Claude packaged-binary repeat evidence — 0.3.0-rc2 (issue #4859)

Date: 2026-06-12. Promise: `pylon.local_claude_agent_bridge.v1` (yellow).
Sole blocker:
`blocker.product_promises.pylon_claude_agent_packaged_binary_repeat_missing`.
Retained proof:
`apps/pylon/docs/proofs/2026-06-12-claude-agent-packaged-rc-proof.json`.

## What ran

The bounded `claude_agent_task` (the #4755-class sum-repair fixture run,
exactly the scope the claude-agent-task smoke defines) was repeated from the
**published packaged artifact** instead of the source checkout:

- `@openagentsinc/pylon@0.3.0-rc2` installed under the `rc` dist-tag into a
  fresh isolated bun prefix (registry `dist.shasum`
  `9c2511287536cc437f260c78cdb3f3a85614b858`); the `pylon` bin and every
  executed module resolved to the installed `node_modules` package.
  `pylon bootstrap --json` ran from that install.
- Harness shape mirrors `smoke:claude-agent-task` (#4718/#4719/#4720) with
  the CI leg's two mocks replaced by the real lane: the **real** readiness
  probe (state `ready`, credential source
  `credential.source.claude_agent.local_claude_session`) and the **real**
  Claude Agent SDK runner. The lease came from the smoke's own local
  assignment-API harness (`claudeAgentSmokeLease`), not a live dispatch.
- Execution was `local_bounded` only (never danger mode): bounded fixture
  workspace under a temporary `PYLON_HOME` cache, PreToolUse workspace-escape
  guard, user settings excluded, `maxTurns` 12, 300 s wall clock,
  `paymentMode: no-spend`.
- Outcome: one real SDK session (hashed ref
  `session.pylon.claude_agent.294b4ccba7b9be010ffbb2b8`, hash verified
  against the operator-local session store), 1 file edit, the packaged
  executor's real `bun test sum.test.ts` verification passed, closeout
  `accepted` with `settlementState: not_applicable` and
  `payoutClaimAllowed: false`. The smoke-level scan over all 6 retained
  harness requests plus the closeout, and the 8-pattern scan over the
  retained proof artifact, both came back clean.

Deviations (recorded in the proof): the rc2 artifact self-reports
`0.3.0-rc1` via hardcoded constants in its bootstrap/state modules (npm
metadata and package.json correctly say `0.3.0-rc2`); and bun blocked the
`nostr-effect` prepare lifecycle script during install without affecting the
executed lane.

## The rc-vs-stable wording boundary, stated plainly

The promise verification text says green requires the repeat from a
**published stable packaged binary**. This run is from the `rc` dist-tag
(`latest` remains `0.2.5`; no stable 0.3.0 exists). Therefore this evidence
is **banked, not claimed**: it does not clear the blocker by itself, and no
yellow→green transition is proposed as cleared here. Whether an rc dist-tag
artifact satisfies the registry's "stable" wording is the owner's call,
recorded on #4858/#4859.

## Owner options

1. Publish stable `0.3.0` (promote rc2 or cut 0.3.0) and either accept this
   identical-bits evidence or re-run the same repeat against the stable tag,
   then flip the promise green receipt-first with a registry bump.
2. Decide the rc dist-tag satisfies the "stable packaged binary" wording (a
   registry-wording call), record that decision, and flip green on this
   banked evidence.
3. Leave the promise yellow until a stable publish; this evidence stands as
   the packaged-binary repeat precedent (mirroring #4661's Codex shape) for
   that moment. Bumping the self-reported version constants before a stable
   publish would also remove the rc1/rc2 self-report mismatch noted above.
