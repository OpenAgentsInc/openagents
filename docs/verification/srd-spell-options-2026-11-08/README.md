# Spell condition and ability choices

2026-11-08. Baseline `8415fee4f2`. Partial #13038/#13043 progress;
neither issue is complete. The containing commit and source hashes bind the
tested program, fixtures and cached compiler.

## Source and implementation

SRD 5.2.1 p.113 gives the caster the Blinded/Deafened choice, including additional
upcast recipients. Page 140 requires an ability choice for Hex, retains the curse
when moving it later, and gives concentration bands of one hour, four hours at
slot 2, eight hours at slots 3–4 and twenty-four hours at slots 5+. The PDF pin
remains `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

`rs_cast_creatures_with` takes one `RsCastOptions` per chosen recipient, or an
empty options view for defaults. An upcast cast can choose Blinded for one
recipient and Deafened for another. Hex accepts exactly one of the six ability
IDs, retains it in its owned record, and keeps it on curse transfer. Unrelated
or invalid option fields and mismatched list lengths refuse before any spending,
dice, events or concentration changes. Stable spell IDs and canonical records
are unchanged.

A request-local spell variant reaches both effect-capacity admission and the
shared program executor. The immunity branch therefore matches the selected
condition: immunity to Blinded cannot admit a full unimmune Deafened target,
while an immune Deafened target needs no record. Timed-condition execution no
longer emits a false condition event for an immune/no-applicable-condition
outcome. The original global record is not rewritten.

The two legacy executable page references were one page high. A new independent
comparison against the pinned canonical entries failed before correction;
the runtime records now cite pages 113 and 140. This is not a source-pin change.

Default and explicit Hex casts now use the pinned upcast concentration duration.
The retained effect and caster deadline agree; transfer preserves the original
deadline. Hex remains an ability-check penalty and does not become a saving
throw or attack-roll penalty. Its general check consumers are still incomplete;
recording all six choices is not a claim of a complete skill/check system.

## Evidence

- Core: 172 passed. Hotbar: 157 passed. Source band boundaries, all six ability
  choices, transfer, mixed recipients, immunity/storage boundaries, invalid
  options, default paths and identity preservation have focused fixtures.
- The full canonical/source entry after page correction passes 178 checks,
  including the core checks and the new page-provenance comparison.
- New option/variant/capacity authority: 83 mutants killed, zero survivors,
  no exemptions. Synthetic repeated programs exercise aggregate capacity and
  new-record ownership separately from current spell metadata combinations.
- Native strict witness: thirteen PASS, one FAIL (parked Polymorph). The new
  SPELL-OPTIONS witness checks malformed-option refusal, mixed retained flags,
  actual attack-edge differences and expiry, then Hex's chosen ability and
  eight-hour deadline boundary. Capacity and prior casting probes stay passing.
- Ordinary probes and Experimental selected checks are retained in their logs.
  These are scoped program checks, not whole-world or full-SRD qualification.

New checks preceded implementation; initially the option type/entry was absent.
Fixture construction errors were corrected before the first successful run.
The cached 0.52.0 compiler is hashed; no compiler upgrade is claimed. Tools-check
and any rebase evidence accompany the final commit.

## Remaining work

Deafened's complete hearing/check consequences and general Hex ability checks
remain #13039 work. Component/focus/cost/consumption, casting workflows, remaining
target geometry and player/wire option surfaces remain #13038/#13046 work.
Command still has its previous Halt program: Approach/Flee need movement/turn
control and Drop needs held-item ownership; those effects were not replaced by
incomplete state flags. Existing Halt timing/reaction limitations also remain
under #13040. This change closes none of those broader requirements.
