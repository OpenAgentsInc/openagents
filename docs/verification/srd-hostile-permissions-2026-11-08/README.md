# Experimental hostile movement and action permissions

Partial #13046 migration on baseline `33669a9ece`. Compiler 0.52.0.
Source clauses: pinned SRD 5.2.1 p.183 Incapacitated, p.185 Stunned,
p.186 Restrained. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Adapter change

The physical hostile had only `held`, which stops pursuit but permits strikes.
Experimental projected inability to take an action into that movement-only flag:
Stunned froze pursuit but could still attack a nearby player. The body now has
an independent `inactive` flag. Shared `rs_can_act` supplies action permission;
`rs_can_move` and current walking Speed supply movement permission. No duplicate
condition mask is introduced in the world adapter.

Accepted casts and each shared expiry/frame reconciliation update both flags.
Losing action permission immediately cancels a pending swing; recovery cannot
resume it. The physical tick also checks inactivity before hit-stop handling,
cancels an old windup and refuses a new one. It can still pursue when movement
is legal. Restrained creatures stay in place but keep their action permission.
The existing hit-stop contract still freezes an otherwise legal swing.

The return-home path after player death now respects dead, movement-held and
hit-stopped bodies too. It no longer makes a corpse or movement-held body walk
home merely because the player died. Inactive alone does not prevent walking.
These are internal body-state fields, not published protocol changes.

## Verification

Checks precede code; red records the missing independent carrier. New physical
checks exercise pursuit while inactive, cancellation just before landing and
during hit stop, repeated attack refusal, recovery starting a fresh swing,
held-but-active attacks, and movement blockers on the return-home path. The
Experimental fixture retains Stunned through the real shared effect API,
projects both permissions, advances the physical pursuit, then changes to
Restrained and removes it. It checks immediate cancellation and recovery.

- `coax/bin/coax test verse/experimental-hostiles.coax --library`: 198 checks pass, including hostile behavior, legacy combat, terrain/collision and geometry. The near-limit step note is advisory; exit 0, no budget increase.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`: 22 selected checks pass, including the final shared-to-body permission fixture. No filtered authority coverage claim.
- `coax/web/tool.sh aethyr/rules/srd/hostile-permission-native.coax`: actual native `PASS HOSTILE-PERMISSIONS`, exit 0. The native body scenario is shared with its evaluator probe; it is not an independent rule oracle.
- `coax/web/tool.sh tools-check`: zero new tools without a reason.

All commands finished locally below fifteen minutes. This is world integration,
so lean interaction/native checks apply. Shared condition/action policies are
unchanged; their zero-survivor receipts are reused from `33669a9ece` and earlier
condition/lifecycle work. No blanket world/geometry mutation gate is imposed.

## Remaining

#13046 stays open. The physical hostile still uses `verse/rules/srd5-combat.coax`
for damage/attack resolution and its own player HP. This change does not apply
shared attack edges, defenses, features or turn/resource budgets to those rolls.
Full used-monster records, class/loadout spell coverage, authoritative shared HP,
Everglade II integration and the remaining world controls/replay flows still
need work. Inactive is an action gate, not a generic ban on all source-specific
passive effects or Reactions. No full Everglade/SRD qualification is claimed.
