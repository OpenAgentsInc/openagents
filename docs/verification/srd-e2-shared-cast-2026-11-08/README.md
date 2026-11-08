# Everglade II casts enter shared execution

Partial #13038/#13045/#13046 work on 7cb49eb7ea; cached compiler 0.52.0.

`eg_e2_cast` now commits shared execution before advancing the seat's cast
cooldown/counter and broadcasting its existing cast event. Slot, aim, range
and network cooldown admission remain unchanged. Rule refusal uses the existing
`no` reason and does not publish an accepted cast. No wire schema changes.

The authority `everglade2-rules.coax` uses the existing server-owned creature
and saved seat. A transaction wraps explicit unlimited-press preparation and
shared execution. Refusal restores resources, seat openness/turn and dice.
Accepted presses preserve spent Reactions and movement history and restore the
surrounding actor's turn. Logical indices are checked against battle n even
when the backing span contains an unused live record.

## Declared social-world profile

Everglade II still has no monsters or harmful player targeting. Self and ally
entries resolve on the caster through `fb_use`: e.g. Second Wind, Lay On Hands,
Rage, Healing Word and eligible self spells now change actual shared state.
Offensive spells use the existing shared `fb_cast_at_nothing` path: legal
access, speech/hands/material requirements, costs and concentration lifecycle
apply, but no creature receives damage/conditions and no harmful area/zone is
created. This is the world's explicit harmless/empty-target house policy,
not strict SRD target semantics or qualification of every spell. Empty melee
swings spend an Action without inventing a target, hit roll or on-hit benefit.
The world still selects its existing level-one class kit; this does not add a
level selector or automatically legalize every choice on those bars.

## Evidence

- Missing-module and protocol Second-Wind-without-healing regressions retained.
- 197 complete library checks pass: real self healing/Rage, component
  refusal rollback, cantrip/leveled spending, higher-slot selection, logical
  actor bounds, empty swings, unknown features, saved-turn isolation and
  movement/Reaction preservation. (Capacity policy itself retains earlier
  receipts; this entry does not add an effect-storage capacity fixture.)
- Policy mutation: 51 mutants, all killed, zero survivors, no exemptions.
- Both `everglade II room` protocol scenarios pass. The existing test now
  gives its Fighter actual damage for Second Wind to heal, retaining every
  broadcast, slot/range/cooldown and peer-isolation assertion. The new actual
  `rm_join`/hello/`rm_heard` scenario verifies HP changes and speech refusal
  before publishing/cooldown mutation.
- Actual native tool prints `PASS E2-SHARED-CAST`: shared self healing, speech
  refusal rollback and a harmless offensive cast execute after C compilation.
  This is a native component, not a TCP/browser/full-room-server run.
- Tools-check and diff check pass; source/compiler hashes retained.

```
coax/bin/coax test apps/openagents-web/everglade2-rules-checks.coax --library
coax/bin/coax mutate apps/openagents-web/everglade2-rules-checks.coax --library --unit apps/openagents-web/everglade2-rules.coax --jobs 4
coax/bin/coax test coax/web/room/room.coax --library --only 'everglade II room'
coax/web/tool.sh apps/openagents-web/everglade2-rules-native.coax
```

## Still open

The page predicts presentation before acknowledgement. Rejection display,
visible health/temp HP/features, complete state projection/room rejoin, actual
host/network/headless play, all class/loadout prerequisites through level 6,
non-self healing/targets, harmful combat, spatial zones and timing/profile
lifecycle equivalence remain separate work. The two legacy room baseline
failures retained in srd-room-press-2026-11-08 are not fixed or hidden here.
No full server build/runtime or complete world qualification is claimed.
