# Room presses retain Reaction ownership

Partial #13045/#13046 work on 1e0992123a using cached compiler 0.52.0.

The legacy Everglade 5E room now calls the same declared `RS_UNLIMITED_PRESS`
cost policy as Experimental instead of maintaining a separate reset loop.
The public `e5rr_use_atomic` transaction continues to restore the complete
battle/seat on refusal. A press is not a logical turn and no longer clears
the actor's spent Reaction. Other players' slots, turn budgets and Reactions
remain untouched. The profile restores slots, ordinary uses, Focus and Rage;
this explicitly extends the room's former slots/ordinary-uses-only refill to
the same unlimited class-feature policy as Experimental. The new empty-Rage
regression verifies this behavior. It does not waive speech, components,
access, concentration, target requirements or storage admission.

The initial two-seat Bard regression failed on the Reaction reset before the
change. It now verifies successful casting retains spent Reactions and another
seat's spent budgets; a speech-blocked cast restores the caller's empty slot
and spent Action/Bonus Action through the actual public transaction. The
existing refused-cast regression passes, as does the explicit Rage regression.
Core policy is unchanged: reuse the 51-killed/zero-survivor receipt from
srd-press-budget-2026-11-08 rather than rerun it. Tools-check and diff check pass.

## Full room evidence and remaining failures

The full room library run exits 1: 323 checks, 321 passed, two failed. Both
failures were independently reproduced with this worktree's room-rules file
replaced by its unmodified HEAD version (1e0992123a), then restored:

- Conjure Animals' caster-leaving fixture expects the pack to persist; its
  `left == E5RR_PACK` assertion fails.
- The horde-asynchronous-actions fixture exceeds its existing 134217728
  aggregate traversal budget in the shared condition/capability path.

No failing check was skipped, rewritten or given a larger budget. These are
remaining room qualification work, not evidence of a clean full suite. The
additional Rage fixture passes separately after the full run. Exact logs
retain both the combined failure and unchanged-main reproductions.

```
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Everglade room unlimited'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'a refused cast restores'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Everglade room declares'
```

## Everglade II is still separate

Inspection confirms `everglade2-cast.coax:e2k_press` predicts presentation and
sends a cast request. `coax/web/room/room-everglade.coax:eg_e2_cast` currently
validates class slot, finite aim/range and cooldown, then broadcasts the cast;
it does not call the shared spell/feature executor. This room reset migration
is not evidence of Everglade II execution. Server-owned creature/turn state,
shared admission/execution and resulting state/event projection still need
integration. NPC raid-hero slot refills, custom summons and other special
paths also remain outside this change. No issue is closed by this slice.
