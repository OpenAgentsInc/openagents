# T3UI-01 transcript message composition receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@544bb2a2e00421984d415fc1d81d70e530bef05f`
- Scope: ordinary transcript message composition, scroll-stable disclosure,
  and basic turn minimap navigation

## Implemented

- Long user prompts collapse after eight lines or 600 characters, using the
  T3 `Show full message` / `Show less` interaction and a bounded fade.
- Only the terminal assistant message in each user-turn segment exposes the
  copy/timestamp/details metadata row. Commentary remains visually quiet;
  streaming text never exposes a stale copy action.
- Expanding/collapsing a long prompt or settled work group preserves the
  reader's visual position while free-scrolling. At the live edge the existing
  message-scroller remains the sole follow authority.
- Two or more user turns expose a compact keyboard-operable turn minimap. Each
  stop carries bounded user and terminal-assistant previews and releases live
  following before navigation.
- Existing typed plan, reasoning, command, file, tool, agent, approval, notice,
  refusal, redaction, working, and waiting rows remain distinct.

## Proof

- Focused transcript tests: **48 passed**.
- Related timeline/catalog tests: **51 passed** before the terminal-assistant
  refinement; the final focused run supersedes that result for changed code.
- Sol document tests: **19 passed**; manifest and offline Sol guards pass.
- Desktop production build: **passed** (`2,420` renderer modules transformed).
- Shared UI typecheck: **passed**.
- Desktop typecheck: **passed** after rebasing onto the landed conversation-
  catalog status reconciliation.
- Desktop full sweep: final clean rerun **1,989 passed**, **39 skipped** across
  **204 passing files**. A prior serial sweep's two unrelated timing/portal
  flakes passed focused rerun before the clean full sweep.
- Desktop visual lane: captured all 16 fixtures; 10 were within tolerance and
  six existing transcript/composer baselines drifted. Side-by-side inspection
  showed current user-bubble/empty-state/plan/message composition versus stale
  checked-in pixels. Baselines were not rewritten in this packet.

## Residual

This is not full transcript parity or T3 parity. T3UI-02 still owns exact
turn folds/duration, measured full virtualization, anchored new-turn space,
minimap viewport indication, rich user contexts/images/revert, and installed
long-thread visual evidence. Side panels remain deliberately later.
