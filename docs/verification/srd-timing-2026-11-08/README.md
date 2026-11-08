# Declared strict/action timing boundary

The implemented policy and named deviations are documented in
[timing-profiles.md](../../dnd/timing-profiles.md). This is partial #13045
progress, not full profile or world qualification.

The existing consumers independently wrote the same 1.5-second beat and
nine-second recharge. Those mutable fields have been removed from RsBattle.
The default typed profile is strict; known action consumers select the action
preset through a validated setter. Unknown identities and live changes refuse
without changing state. Begin, start-turn and tick seal selection; reset returns
to strict. Strict movement grants now retain all current Speed in feet rather
than discarding partial cells. The regression first failed at Speed 7 (red.log).

Action standing deliberately costs half its beat's movement grant; strict
standing still pays half actual Speed. The saved-seat fixture was updated to
test this named deviation using its actual grant, without artificially supplying
extra movement. The authority Prone payment functions are unchanged from
e58f409197's 62-killed/zero-survivor gate, which is reused rather than repeated.

## Verification

All checks run locally with the hashed cached 0.52.0 compiler, each command
below fifteen minutes. Timing checks cover default/reset isolation, invalid
and sealed refusal, no event/die/schedule changes on rejection, exact movement,
standing prices, paired cooldown behavior, Reaction refresh and death/repeat
saves across five short turns spanning the next six-second boundary.
The fixture initially included the actor's already-rolled initiative in its
expected per-operation die count; it now measures from that baseline. The old
hotbar fixture manually cleared only some battle fields; it now uses rs_reset
before selecting timing. Neither correction changes production admission.

- Timing entry: 157 checks passed.
- New timing authority: 43 mutants killed, zero survivors, no exemptions,
  including selection, sealing and reset state transitions.
- Saved-seat/hotbar: 172 checks passed. Prone entry: 157 passed.
- Experimental: 17 selected checks passed; mode/reset switch, MMO action clock,
  strict one-shot turn and room Cult Priest checks each passed.
- Strict native TIMING-PROFILE witness passed: 16 PASS and the one parked
  Polymorph FAIL, with exit 1 retained.
- Casting: 186 passed. Probes: 166 passed. Tools-check is retained in its log.

Selection/seal/reset bodies were subsequently moved intact into the authority
file so their transitions join the mutation gate. The final 43-mutant/core/native
results include that extraction. Earlier saved-seat/world results exercise the
same bodies before relocation; they were not repeated merely to confirm them.

Selected world checks do not claim whole-entry authority coverage, full-world
qualification, Wasm/phone ABI testing or visual/browser verification. World
integration changes are focused preset selections with rule tests; no visible
browser was opened. Older failed attempts were fixture issues and are not
counted as successful checks.

Commands:

```sh
coax/bin/coax test aethyr/rules/srd/timing-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/timing-checks.coax --library --unit aethyr/rules/srd/timing-profile.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test aethyr/rules/srd/prone-checks.coax --library
coax/bin/coax test aethyr/rules/srd/cast-target-checks.coax --library
coax/bin/coax test aethyr/rules/srd/compliance-probes.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'Escape pauses'
coax/bin/coax test apps/openagents-web/fivemmo-battle.coax --library --only 'on the action clock'
coax/bin/coax test apps/openagents-web/oneshot-table.coax --library --only 'a turn runs on the engine'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Cult Priests'
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax
coax/web/tool.sh tools-check
```

## Remaining acceptance

#13045 stays open: free-casting/resource resets, grants/material waivers and
house content are not yet centralized as active-world profiles. Frame stalls,
pending trigger windows, full lifecycle equivalence, world input/physics and
other action/movement rules need their remaining implementation/qualification.
The clock seal prevents live retiming; it is not a general turn-based adapter
or profile editor. Epic/world acceptance remains unproven. No owner action is
required.
