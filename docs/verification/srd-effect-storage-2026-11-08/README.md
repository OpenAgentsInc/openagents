# Effect capacity and ownership

2026-11-08. Baseline `25b0c8cb59`. Implements the bounded-storage part of
[#13041](https://github.com/OpenAgentsInc/openagents/issues/13041).
The containing commit and `source-hashes.txt` identify the tested sources.

Insertion now returns a Boolean outcome. Full storage refuses without changing
any old record or condition; it never replaces slot zero. `rs_put_effect` also
returns an explicit outcome, distinguishing storage refusal from a processed
immune target. Admitted internal executors consume prepaid capacity.

New casting preflight checks all selected targets before costs, rolls, events,
affinity reset or concentration changes. It uses actual target selection, area
geometry and Sculpt exemptions, accounts for repeated programs and credits only
records that replacing the caster's old concentration actually releases.
Invalid creature types retain the p.106 spending rule; known immunity needs no
record. Synthetic programs exercise repeat and area boundaries even where the
current executable spell subset does not combine those instructions.

Attack/Extra Attack, ranged attacks, Martial Arts and Flurry preflight potential
Sap/Vex, Stunning Strike and planned poison records. Automatic retargeting
reserves the worst case at each eligible adjacent foe. This deliberately can
refuse before discovering that a miss, save, death or consumed effect would
have reduced demand. Turn Undead checks its whole batch; moving Hex/Mark checks
the destination before releasing the old owner. Direct strike adapters in the
room check before spending their action. The lower effect-program executor
checks capacity before any instruction; current direct world uses are healing,
damage and push programs, not unreserved record-producing batches.

Both development Hold Person adapters propagate insertion refusal rather than
claiming that a full target was held: nearest-person returns `-1`, named person
returns `false`. A room integration compile caught their old void-API calls;
full-capacity regressions cover both adapters.

`EF_ZONE_RESTRAIN` retains the zone slot, caster and spell per independent
restraint, using the existing variant payload. Reapplication of the same live
lease is idempotent. Zone release removes its leases synchronously before slot
reuse; escape removes only the selected binding. The legacy `restrained_by`
field is a derived escape-selection cache plus compatibility for older raw
bindings, rather than the sole owner. Aggregate condition retirement clears
only bits no remaining owner imposes, including compound conditions.

Authoritative records remain in `RsCreature.eff` and `RsBattle.zone`; neither
is stored in the presentation queue. Experimental's capped-number queue check
still passes. No presentation capacity or event-ring eviction releases those
gameplay records. This is a bounded implementation contract, not an SRD rule
limiting concurrency to six effects or eight zones.

## Verification

- `coax/bin/coax test --library aethyr/rules/srd/cast-target-checks.coax`:
  144 passed. The initial ordinary/Quickened full-storage regression failed
  because the cast was accepted before preflight was added.
- Mutation through that entry, `--unit aethyr/rules/srd/effect-storage.coax`:
  113 killed, zero survivors. Admission: 224 killed, zero survivors. Zone
  lifetime: 58 killed, zero survivors. No equivalent exemptions. Earlier
  surviving runs drove stronger boundaries and removal of redundant guards.
- Hotbar: 143 passed; probes: 135 passed; Experimental: 17 selected passed;
  watch melee/throw consumer: one selected passed; scripted hold adapters:
  five selected passed. Retained logs distinguish complete and selected runs.
  Native strict reporting intentionally exits 1: ten PASS,
  only parked `SPELL-POLYMORPH` FAIL. Both capacity witnesses pass.
- Scripted hold root mutation gate: 39 killed, zero survivors, no exemptions.
- `coax/web/tool.sh tools-check` passed: no new tools missing a reason.

Rebased onto `c64b5ae9a5`. All owned core hashes and the compiler matched their
earlier passing evidence, so those gates were reused. Upstream had changed
Experimental's scene/consumer; its fresh selected run passes 17 of 670 checks,
using 32,653,111 of 33,554,432 compile-time value slots (97%). Room and hold
consumer checks ran after the adapter changes. No complete browser, whole-room
or full repository qualification is claimed.

The cached compiler is the existing 0.52.0 binary (hash retained). Main's
unrelated compiler sources have advanced; its stale-source advisory accompanies
the intentionally failing native diagnostic. No compiler qualification claim
is made by these rules checks.

Full same-spell stacking, every condition's consequences, spell duration
qualification, source catalogs, timing profiles and world migration remain
separate active issues. Polymorph remains parked. Passing these selected
witnesses does not qualify the whole SRD.
