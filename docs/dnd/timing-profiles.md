# Shared combat timing profiles

Implemented subset of #13045, 2026-11-08. This is the engine's current typed
timing boundary, not the complete world-profile format proposed in README.md.
The policy lives in `aethyr/rules/srd/timing-profile.coax`; worlds select it with
`rs_set_timing` before play. No public wire format is introduced.

| Decision | `RS_STRICT_TIMING` (default) | `RS_ACTION_TIMING` (house rule) |
| --- | --- | --- |
| Logical turn/Reaction refresh | 6 seconds | 1.5 seconds |
| Movement grant | Exact current Speed in feet | Existing nearest whole-square quarter-round grant, at least one square for positive Speed |
| Standing cost | Half current Speed, round down in feet | Half that beat's movement grant, round down in feet |
| Slot, Rage, ordinary limited-use recovery | No timer recovery | One spent resource at a time after 9 seconds |
| Focus, Sorcery, Bardic Inspiration recovery | No timer recovery | One point/use after 4.5 seconds |
| Lay On Hands pool recovery | No timer recovery | Up to 5 points after 4.5 seconds |
| Repeat saves, death saves, Rage upkeep | Six-second round boundary | Same six-second round boundary |
| Timed durations | World-clock seconds | Same world-clock seconds |

Cooldowns retain the existing scheduling convention: the first tick observing
a missing resource schedules its recovery. They are a house rule, not a claim
that the SRD grants timed resource recovery. Strict recovery/rest workflows
still require their own implementation and verification.

The action grant preserves the previous adapter: Speed 30 gets two five-foot
squares per beat; Speed 25 gets one. The standing override makes its cost
explicit rather than requiring half a full round's Speed from a short beat.
It does not clear Prone automatically. Strict mode preserves every residual
foot, including Speed 7 granting seven feet and standing at Speed 25 costing
twelve feet. Grid steps still require five feet of actual displacement.

Only these two identities are supported. Unknown profiles reject without
changing state. Begin, turn start or tick seals selection; changing a live
clock would invalidate already scheduled turns and cooldowns. To remove a
profile, reset/recreate the battle: reset restores strict and reopens selection.
This also prevents a previous world's action timing from leaking into strict
tests. Copying a battle/saved seat must preserve its timing and movement fields.

Experimental and the existing Everglade single/room battle initializers select
the action preset. 5E Lab's action/round switch recreates the battle and selects
action/default strict respectively. 5E MMO selects action; the one-shot table
retains strict. These callers no longer own independent beat/recharge numbers.
Constants retained solely in old world assertions are independent expected
values, not core timing inputs.

This timing subset does not authorize or qualify world spell grants, free
casting, material waivers, unlimited-use preparation, monster conversions,
non-grid physics, damage scaling, cast bars or house spell records. The legacy
room's per-press resource reset still needs a declared shared cost profile.
Everglade II adoption and visible standing/input choices remain #13046 work.
No world-profile editor or general profile text parser ships here.

Evidence: [timing verification](../verification/srd-timing-2026-11-08/README.md).

Experimental now applies the separate shared `press-budget.coax` cost policy:
`RS_PAID_PRESS` preserves paid state; `RS_UNLIMITED_PRESS` explicitly restores
slots/ordinary uses/Focus/Rage and clears Action/Bonus Action/slot-spent budgets
for a press. Refused execution rolls those fields back. This does not advance
a turn, refresh Reactions, grant access or waive components. It is separate
from clock timing, and does not qualify the remaining room/world reset paths.
Evidence: [press-budget verification](../verification/srd-press-budget-2026-11-08/README.md).

The legacy Everglade 5E multiplayer room also declares `RS_UNLIMITED_PRESS`
through its existing public transaction. Presses preserve Reactions and other
seats; refusal restores the override with the rest of the transaction. Its
class-feature budget now includes the same Focus/Rage refill as Experimental.
This is not Everglade II's currently presentation-only cast broadcast path.
Evidence and full-room failures:
[room press verification](../verification/srd-room-press-2026-11-08/README.md).

Everglade II's server now commits shared execution under its explicit social
profile: self/ally entries affect the caster, offensive spells pay shared
empty-target costs without harming anyone or creating harmful areas, and empty
swings spend an Action. Unlimited press budgets do not renew Reactions or
movement; failed execution rolls back before cooldown/publication. This is a
named world deviation, not strict target semantics. Page prediction and complete
state/runtime qualification remain open. See
[Everglade II evidence](../verification/srd-e2-shared-cast-2026-11-08/README.md).
