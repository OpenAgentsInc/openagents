# Long Rest completion work for the active SRD slice

Source: pinned SRD 5.2.1 p.185; Temporary Hit Points p.18; Font of Magic p.66.
PDF SHA-256: `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
This is remaining #13040/#13044/#13045/#13046 work, not an issue-completion claim.

## Source contract to implement

| Requirement | Evidence needed before completion |
| --- | --- |
| Start with at least 1 HP; no new start within 16 hours after a completed Long Rest | Actor/dead/HP/clock boundaries, refusal without mutation, actual completion timestamp retained across restart/rejoin |
| At least 8 hours, including at least 6 hours of sleep and no more than 2 hours of light activity | Retained authoritative activity segments; idle clock passage cannot manufacture sleep |
| Sleep imposes Unconscious | Owned condition retention, capacity preflight, concentration/action/attack consequences, waking preserves independent Unconscious sources |
| Initiative, non-cantrip casting, any damage, or an hour of walking/physical exertion interrupts | Actual accepted producers and exact event-time ordering; cantrips/refusals stay distinct; elapsed exertion is accounted, not every footstep treated as a Long Rest interruption |
| An interruption after at least an hour of rest grants Short Rest benefits | Grant once at the causal interruption, preserve optional choices/daily history, no healing without chosen Hit Dice |
| Resume immediately with an additional hour required per interruption | Retained resume/extension state and activity accounting; no fresh start silently discards the previous interruption or evades the 16-hour admission rule |
| Restore HP, spent Hit Dice, reduced HP maximum and reduced ability scores | Normal-stat provenance plus independent modifier ownership; preserve legitimate lasting increases and specific restoration restrictions |
| Reduce Exhaustion by one and recharge source features | Cause-specific restrictions where represented; all twelve level 1–6 class banks; no revival, turn grant or Reaction renewal |

## Implemented bank primitive

`long-rest-resources.coax` preflights represented counters and applies only the
resource portion after a caller has legitimately completed rest. It restores
slots/uses/Focus/Rage/Hit Dice, resets daily recovery histories and clears
Temporary Hit Points and pending cooldown bookkeeping. It neither admits nor
finishes rest and is not called by elapsed-time polling.

`created_slots` records **remaining** Font of Magic-created slots. Recovery
removes those before restoring ordinary slots; it does not infer provenance
from class or delete unrelated over-capacity banks. Font of Magic creation and
slot-payment producers must maintain this provenance before that feature is
exposed. The bank helper is not creation eligibility validation.

## Next implementation sequence

1. Add retained Long Rest progress and activity modes, with one authoritative
   time source and finite/monotonic boundary checks. Cover eight-hour, six-hour,
   two-hour and sixteen-hour edges and successive interruptions before code.
   Keep the existing Short Rest state/choices separate from Long Rest progress.
2. Introduce owned sleeping state and explicit sleep/wake transitions. Preflight
   condition storage before changing rest state. Account activity before each
   mode transition; clock ticks only account the mode the world actually owns.
3. Integrate interruptions with before-event synchronization. If completion or
   an interruption boundary precedes damage/casting/Initiative in the same
   update, apply events in chronological order. Never heal after damage merely
   because a late poll noticed an earlier completion. Do not let a later tick
   turn a sub-hour interrupted segment into an eligible Short Rest.
4. Add normal-stat/reduction provenance and the remaining completion benefits,
   then invoke the verified bank primitive once from admitted completion.
   Reset only histories whose source says Long Rest; preserve independent
   conditions, grants and current turn/Reaction budgets.
5. Add settled-downtime controls and actual world/network activity producers.
   Preserve rest/dice/daily/cooldown history through the real rejoin path. Treat
   a new preset/character replacement as an explicit profile decision.
6. Retain source fixtures, zero-survivor changed core policy gates, native
   execution and focused adapter evidence. Audit the full acceptance above
   before closing issues. Existing passing unchanged receipts are reused.

## Timing policy boundary

`long-rest-timing.coax` now checks the source duration and start boundaries in
integer seconds: eight hours plus one hour per interruption, six hours of
sleep, no more than two hours of light activity, nonoverlapping activity totals,
and sixteen hours since actual completion with positive starting HP. Arithmetic
avoids overflow at maximum integer inputs. This is a policy predicate, not
retained activity ownership or a world clock adapter. It has no completion
caller. The six integration steps above remain required.

Verification: two source boundary checks pass; all 41 generated mutants are
killed with zero survivors or exemptions. `coax/web/tool.sh tools-check` passes
(1897 tracked tools, 35 vendored, zero missing reasons). No product/world ABI
changed, so the previous unchanged world build receipt is not rerun.

## Retained activity journal

`long-rest-state.coax` now retains explicit activity segments and interruption
extensions, pauses without crediting elapsed interruption time, and reports
Short Rest eligibility at the interruption. It stops at the first completion,
one-hour exertion or two-hour light-activity boundary. Completion remains
pending until the owner applies benefits and acknowledges its exact timestamp;
that timestamp controls subsequent sixteen-hour admission. Actual world sleep,
clock conversion, causal producer hooks, benefits and persistence remain the
next integration work. [State/native evidence](../verification/srd-long-rest-state-2026-11-08/README.md).

## Owned sleep condition

`rest-sleep.coax` now provides capacity-safe sleep/wake primitives through the
shared battle engine. Natural sleep owns a nonexpiring Unconscious effect;
waking preserves independent raw, recorded and zero-HP sources, and leaves
Prone. Admission uses the existing incapacitation lifecycle and updates the
represented held-focus state. The retained journal and actual world producers
still need to call these in causal order. Full inventory dropping remains
separate from the existing focus representation. [Sleep/native evidence](../verification/srd-rest-sleep-2026-11-08/README.md).

## Body benefits and pending-completion consumer

`long-rest-benefits.coax` now restores explicitly recorded, source-authorized
HP-maximum and ability-score losses while preserving subsequent increases,
uses shared healing restrictions, recharges represented banks, and removes
one unlocked Exhaustion level. Separate food/water counts retain their source
restrictions. `long-rest-completion.coax` consumes an owned pending boundary
once, wakes natural sleep and commits its actual timestamp without granting a
turn or Reaction. The journal now lives on the creature. Source-specific loss
and nutrition producers, derived-stat consequences, world clock/activity
synchronization, controls and rejoin persistence remain required. [Completion evidence](../verification/srd-long-rest-completion-2026-11-08/README.md).

## Frame clock and microsecond boundaries

`rest-clock.coax` now advances an owned microsecond timeline from actual battle
ticks, retaining fractional carry and refusing malformed time/overflow before
ordinary tick effects. The journal supports explicit seconds or microseconds;
its unit cannot change under completed history. Native execution runs all
1,843,200 frames of a rest starting between seconds, then verifies exact
completion and sixteen-hour readmission. World admission/mode transitions and
automatic journal synchronization are still next; legacy f32 assignments do
not migrate this timeline. [Clock evidence](../verification/srd-rest-clock-2026-11-08/README.md).

## Clock-bound actor operations

`long-rest-owned.coax` now admits settled sleep/watch, observes progress on the
battle's validated microsecond timeline, and changes/resumes actual activity
with sleep-capacity checks. A late light-activity report abandons the old period
instead of retroactively crediting sleep; an earlier valid completion wins.
Results preserve observed boundaries separately from requested activity
admission. Automatic world scheduling, causal interruption producers and
consumption of Short Rest fallback/choices remain next, followed by controls
and rejoin persistence. [Owned-operation evidence](../verification/srd-long-rest-owned-2026-11-08/README.md).

## Ordered automatic scheduling

`rest-scheduler.coax` now supplies opt-in battle-owned scheduling. Actual ticks
split at actor rest boundaries, expire preceding timed effects, and consume
completion or one Short Rest fallback before advancing further. Fractional
completion/timer ordering and same-quantum light-limit progress are retained.
Explicit Short Rest completion consumes its quiet segment; exertion closes old
choices and cannot begin a new Short Rest. World controls have not enabled the
mode yet. Ordered damage/casting/Initiative hooks and optional-choice producers
are next, then actual world observations, controls and rejoin binding.
[Scheduler evidence](../verification/srd-rest-scheduler-2026-11-08/README.md).

## Accepted damage cause

The production `rs_damage` path now interrupts synchronized Long Rest and wakes
natural sleep before HP/protection snapshots, including damage absorbed by
Temporary HP. Earned Short Rest banks precede the ensuing Reaction without
renewing its budget. This producer declines unconfigured optional choices;
queued choices and controller-owned simultaneous ordering remain required.
Undead Fortitude now protects HP without erasing damage-taken triggers.
Casting must interrupt after complete admission but before slot payment; adding
it to the existing after-payment hook would incorrectly refill paid Pact slots.
Initiative, non-damage death, real activity/control producers and rejoin binding
remain next. [Damage evidence](../verification/srd-rest-damage-2026-11-08/README.md).
