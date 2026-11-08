# Explicit creature recipients and rays

2026-11-08. Baseline `8126d0eae3`. Partial progress on #13038/#13043.
The containing commit and source hashes identify the tested inputs.

## Source and behavior

The pinned SRD 5.2.1 text specifies optional recipients for Bless and Bane,
additional upcast recipients for Command, Hold Person and Blindness/Deafness,
and separate same-or-different ray assignments for Eldritch Blast and
Scorching Ray. Source descriptions remain in the canonical catalog; source
SHA-256 is `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
The existing executable records now separately represent base recipient count,
per-slot growth and the optional-set rule. This does not qualify all mechanics
of those spells: Blindness/Deafness still has its old condition choice and
Command its old Halt program.

`rs_cast_creatures(b, actor, spell, slot, picks, quick)` supplies an explicit
borrowed creature list to the same admission/cost/effect dispatcher used by
ordinary casting. Distinct-target spells accept a legal subset, include the
caster where legal, reject duplicates and never fill unchosen creatures.
Optional Bless/Bane can select an empty set and still spend the cast. Rays
require one ordered assignment per ray, allow repeated recipients, and retain
separate attack rolls. Growth from a base of one now works in this API.
The implementation request bound is 16 picks, above the active slice's maximum;
it is not a new SRD target limit.

All indices, life/range/Self/count/shape checks and aggregate effect storage
precede costs, dice, events, affinity reset or concentration changes. Area and
destination-point requests use the existing point API instead. Unknown actors,
executor IDs, unavailable slots and unready spells still refuse through shared
admission. Invalid creature types spend under p.106 and report apparent save
success without invented dice; they do not cancel another chosen recipient's
effect. A successful individual save also cannot end the whole chosen spell.
Concentration that actually breaks, including a self Hold Person, stops later
lasting programs rather than leaving orphaned records. Ordinary lifecycle
release removes every retained owner.

The shared dispatch retains Blessed Healer, Spirit Guardians activation and
Produce Flame setup. Non-chosen `rs_cast`/`rs_cast_quick` calls retain the legacy
world convenience picker and same-target rays for compatibility during #13046.
They are not newly qualified as explicit source target selection. No wire,
save-format or player choice UI was changed in this engine slice.

## Verification

- 164 core checks and 152 hotbar checks pass. Source-derived counts, chosen
  caster/fewer recipients, empty sets, rays, malformed/dead/distant/duplicate/
  Self/point shapes, partial-capacity batches, ordinary/Quickened refusal,
  known release, immunity and concentration ownership have retained fixtures.
- Choice/count/capacity/continuation authority: 94 mutants killed, zero
  survivors. Invalid-target outcome authority: 26 killed, zero survivors.
  No equivalent exemptions. Red API checks were written before implementation;
  the initial entry was absent, so the initial failure was compilation rather
  than a successful call with incorrect behavior.
- Native strict witness report: twelve PASS, only parked Polymorph FAIL.
  CAST-CHOSEN-TARGETS verifies full-storage refusal and subsequent exact-capacity
  publication at the chosen caster and recipient, excluding an unchosen person.
- Ordinary probes and selected Experimental regressions are retained in logs;
  they are not whole-SRD, whole-room or browser qualification. A local formatting
  error in dispatch delimiters was corrected before final core/hotbar/native
  checks and before committing.

The cached 0.52.0 compiler is hashed with the inputs. No compiler upgrade or
qualification is inferred. Tools-check and post-rebase evidence accompany the
commit; unchanged passing core policies are not rerun solely for confirmation.

## Remaining active work

Components/costs, rituals/long casts, visibility, object and point targets,
adaptive ray workflows where needed, all remaining spell options and legal
grant/progression workflows remain open. World request/presentation adoption
belongs to #13046. The existing area/Sculpt/Spirit Guardians exclusion choices
are not qualified by this creature-recipient API. Neither #13038 nor #13043 is
closed, and parked work remains parked.

## Rebase and combined verification

Rebased onto `471f88df1e`, which independently fixed overlapping named roll
effects for #13039. Conflicts were the two new battle loads and adjacent ledger
paragraphs; both changes were retained. That upstream change also removed old
roll helpers from battle and installed its authority, so battle's hash changed.
All other owned source hashes and the compiler match the preceding evidence.
The new choice/outcome authorities are unchanged; their 94/26 zero-survivor
receipts are reused. The upstream roll authority's 39-mutant receipt remains
with its own commit and evidence.

Fresh combined checks pass: 164 core, 153 hotbar and 145 dedicated roll-effect
checks. Experimental passes 17 of 679 selected-entry checks, with the same
32,653,111/33,554,432 value-slot peak (97%). Native strict reporting remains
twelve PASS, only parked Polymorph FAIL. The earlier Experimental invocation
while rebase markers were present was invalid and is not counted as evidence;
`*-rebased.log` files are the resolved-tree runs. Final source hashes are in
`source-hashes-rebased.txt`; the original hashes remain retained separately.
