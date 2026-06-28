# Forge SU-7 Dogfood Lane Runbook

> Status: public-safe operator runbook for issue #6797, 2026-06-28. This names
> the first bounded OpenAgents Codex/Pylon lane and the rollback path. It does
> not include secrets, local paths, raw prompts, private logs, or provider data.

## Lane

- Lane ref: `lane.forge.su7.openagents-codex-low-risk`
- Issue: `#6797`
- Repository: `OpenAgentsInc/openagents`
- Intake ref: `refs/forge/intake/openagents/codex-low-risk`
- Change ref: `change.forge.su7.openagents-codex-low-risk`
- Verification receipt: `receipt.forge.su7.su5-check-deploy`
- Queue ref: `queue.forge.su7.nextActualPromotion`
- Promotion receipt: `promotion.forge.su7.su4-blueprint-gated`
- Mirror ref: `mirror.github.openagents.main.su7`

## Command Sequence

1. Select one low-risk OpenAgents Codex/Pylon change and route it through Forge
   smart-Git intake. The lane must not open a competing GitHub PR while Forge is
   coordinating it.
2. Record the work record and change record in Forge, with base head
   `refs/heads/main` and patch head
   `refs/forge/changes/openagents/codex-low-risk`.
3. Require the SU-5 verification receipt before promotion:

   ```sh
   bun run --cwd apps/openagents.com check:deploy
   ```

4. Queue the change behind `queue.forge.su7.nextActualPromotion`.
5. Promote only after SU-4 Blueprint gates approve the promotion receipt. The
   promotion is a Forge-owned ref fast-forward, not a GitHub merge.
6. Mirror the promoted commit through SU-6 so GitHub receives downstream
   visibility after Forge has already accepted the lane.

## Escape Hatch

If Forge intake, verification, promotion, or mirror work blocks the lane, pause
the Forge lane and reopen the GitHub PR path for that one change. Keep the Forge
rows and refs as audit evidence, mark the lane escaped, and do not delete the
verification or queue records. GitHub then becomes the temporary coordination
path for the escaped change only.

## First Workbench Lessons

- Triage needs one lane owner, one issue ref, and one visible blocked reason.
- The change inspector must show base head, patch head, verification receipt,
  promotion receipt, and mirror ref together.
- The attention queue should sort by the next operator action: needs
  verification, needs gate, needs mirror, or escaped.
- Cycle and velocity metrics should measure Forge intake-to-mirror time, not
  GitHub PR time.

## UI Evidence

`apps/forge` exposes the public-safe lane in `/dogfood` and `/shell.json`. The
route intentionally renders only refs, commands, states, and lessons that are
safe for public operator visibility. Control-plane writes still belong to the
`/api/forge/*` Worker surface.
