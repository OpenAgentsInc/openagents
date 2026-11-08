# Reaction casting and turn slot history

2026-11-08. Baseline `acee72340d`. Partial progress on #13038/#13040;
neither issue is complete. The containing commit and `source-hashes.txt`
identify the tested code and cached compiler.

Pinned SRD 5.2.1 p.104 permits only one spell slot for casting on a turn.
Quickened Spell, p.68, separately forbids a level 1+ cast after a Quickened
spell and forbids Quickening after a level 1+ cast on the same turn. The
source pin remains SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

Previously Shield and Hellish Rebuke could spend another slot on their
caster's own turn, bypassing ordinary casting's slot/Quickened restrictions.
They also failed to record slot and level history before subsequent casting.
The two regression checks in `red.log` fail on those boundaries before the fix.

Both reaction paths now use one authority after their existing readiness,
resource selection and trigger/capability checks. It validates the logical
creature span, bank index, remaining resource and unspent Reaction, then checks
the current turn's restrictions only when that turn belongs to the caster.
Refusal changes no state. Acceptance debits one resource and spends the
Reaction. On the caster's turn it records a level 1+ spell and, for an actual
slot, the slot expenditure. Ordinary casting and Quickened admission read
that same history. Another actor's turn flags remain unchanged.

Mage's Protective Magic (p.305) casts Shield 3/Day without a spell slot. The
legacy NPC representation stores these uses in the slots array; a typed
`costs_slot` choice prevents counting that bank as a real slot. It still counts
as a level 1+ cast for Quickened Spell. A concrete Mage regression covers use
after a prior slot, use before an ordinary spell and Quickened refusal.
This does not qualify all legacy NPC N/Day banks; that migration remains #13034.

## Verification

- Core integration: 152 passed. Hotbar: 147 passed.
- Reaction authority mutation through `cast-target-checks.coax`: 45 killed,
  zero survivors, no exemptions. An initial surviving logical-span boundary
  led to a retained-value fixture beyond the battle's live creature count.
- Native strict report: eleven PASS, one FAIL. New CAST-REACTION-SLOT passes;
  the remaining failure is parked Polymorph. Exit 1 remains intentional and
  unsuppressed. Capacity probes remain passing.
- Experimental selected checks and ordinary probes are retained alongside
  the core/native receipts. These are scoped checks, not whole-world or
  whole-SRD qualification.

Ordinary probes pass 140 checks. After rebasing onto `fd64e6c563`, all six
owned source/compiler hashes match the passing core receipts, so those gates
were reused. Upstream changed multiplayer consumers; a fresh Experimental
entry run passes 17 of 674 checks, with the same 32,653,111/33,554,432 value-slot
peak (97%). Tools-check passes for 1,896 tracked tool files, none missing a
reason. The new helper is Coax.

Core resource mutation was not imposed on catalogs or presentation. No new
non-Coax tool was added. Tools-check, source hashes and any post-rebase consumer
checks are retained with this evidence. The cached 0.52.0 compiler's unrelated
source-version advisory is not a claim of a new compiler qualification.

Remaining #13038 acceptance includes components/focus/cost/consumption,
required ritual/long-cast workflows, explicit upcast/multiple-target choices,
remaining grant provenance and range/visibility treatment. #13040 still needs
the complete action/reaction/visibility/recovery slice.
