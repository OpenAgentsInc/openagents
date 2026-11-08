# Long Rest resource-bank operation

Source: SRD 5.2.1 Long Rest p.185, Temporary Hit Points p.18 and Font of Magic
p.66, pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

The new benefit primitive preflights the entire represented bank before writing
or seeding. It restores Spellcasting/Pact slots, represented limited uses
(including Lay On Hands), Focus, Rage and all spent Hit Dice. It clears daily
Arcane/Natural Recovery, Sorcerous Restoration and Wild Resurgence histories,
Temporary Hit Points and old cooldown bookkeeping. HP and actual turn/Reaction
state remain untouched by this bank operation.

Remaining created slots have explicit provenance. Only those slots vanish;
unrelated over-capacity resources are preserved. Malformed counters/provenance
or seeded dice refuse before any partial reset. New unseeded Hit Dice use the
existing one-time seed; an existing seed is not replaced with a fresh preset.

## Evidence

Missing-API fixtures preceded implementation. Full composed entry: 264 checks
pass, including all twelve class presets at levels 1–6, all represented use/slot
indices, zero/exact Hit Dice boundaries, invalid counter/capacity/provenance
refusal, daily flags, cooldowns and unrelated state isolation. Policy gate:
104 killed, zero survivors/no exemptions. Native and checked build receipts are
retained alongside this report. No compiler or work-budget change was made.

```
coax/bin/coax test aethyr/rules/srd/long-rest-resources-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/long-rest-resources.coax --jobs 4
coax/web/tool.sh aethyr/rules/srd/long-rest-resources-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-long-rest-resources-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
```

## Remaining admission and completion

This is a trusted completed-rest benefit operation, like the existing Short
Rest resource layer. No world timer or user control invokes it. It is not a
Long Rest admission/completion API and grants no sleep credit. Eight-hour/sleep/
light-activity/16-hour timing, interruption/resume/Short Rest fallback, owned
sleep, normal HP/ability restoration, Exhaustion eligibility, rest controls and
world persistence still need implementation. Font creation/payment must maintain
remaining created-slot provenance before being exposed; this is not that
feature's execution. Monster Hit Die catalog data and unrepresented features
are not invented. [Full remaining contract and sequence](../../dnd/long-rest-implementation.md).
No issue closes on this bank layer.

Final native output is PASS LONG-REST-RESOURCES, including the 72 class/level
combinations, uses/slots/Focus/Rage/Hit Dice, temporary HP removal, Reaction
preservation and explicit created-slot/refusal cases. Complete checked Lab
Wasm build passes in 38.6 seconds, 2,539,147 bytes, with no new work-budget
overrides. Existing compiler-source/near-limit advisories remain; the temporary
output name has no registered shipping-size budget. Tools-check/diff check
pass. No browser, elapsed-rest completion or full world qualification is inferred.
