# T3UI-02 transcript navigation and scale receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@97a95f2af9d1d55db1187acbd9d225b5983dbcc8`
- Scope: turn folding/duration, measured transcript virtualization, minimap
  navigation, reader modes, and long-history recovery

## Implemented

- Settled user turns retain the authored prompt and terminal assistant answer
  while intermediate commentary, reasoning, tools, and agent activity fold
  behind an elapsed-duration disclosure. Actionable plans, approvals, questions,
  errors, and notices remain visible rather than disappearing into a fold.
- The newest working turn remains expanded. A newly-authored turn enters an
  anchored mode with measured end space; wheel, pointer, selection, or minimap
  navigation yields to free-scroll; completion and Jump to latest return to
  ordinary following.
- More than 80 presentation rows activate variable-height measured windowing.
  Stable row keys, bounded overscan, top/bottom spacers, ResizeObserver updates,
  and correction for height changes above a free-scrolling viewport keep the
  admitted 500-row page bounded without flattening semantic rows.
- The turn minimap marks mounted turns intersecting the viewport. A turn outside
  the mounted window is reached through its measured/estimated offset, after
  which the virtual window mounts that stable turn row.
- Existing prepend retention, typed edge paging, same-key streaming replacement,
  session-key reset, focus retention, scroll-stable disclosure, and jump-to-end
  behavior remain in the mounted contract.

## Proof

- Transcript suite: **53 tests**, including pure fold/duration/window contracts,
  mounted disclosure, anchored-to-free reader transition, prepend retention,
  edge paging, same-key streaming, session replacement, minimap, and 500-row
  bounded mounting.
- Rebased Desktop sweep before final acceptance: **1,999 passed**, **39 skipped**
  across **205 passing files**.
- Desktop TypeScript: passed.
- Desktop production renderer: passed with **2,420 modules transformed** before
  the final documentation-only reconciliation.
- Desktop visual lane captured 16 states: 10 stayed within threshold and the
  same six transcript/composer baselines already recorded by T3UI-01 remained
  stale. Inspection confirmed actionable plan/approval rows remain visible;
  checked-in baselines were not rewritten.
- Final Electron classic/React fixture smoke, Sol guards, and pre-push acceptance
  are recorded by the publishing gate for the landed commit.

## Boundaries

This packet changes presentation and local reading state only. It adds no
provider, filesystem, process, credential, Sync, or remote-control authority.
It does not claim rich user contexts/images/revert, composer parity, side-panel
parity, installed signed-build evidence, the complete pinned component census,
or T3 parity.
