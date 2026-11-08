# Strike features and sight through real Reaction frames

Partial #13039/#13040/#13044 progress on published opportunity execution and
movement from `48bac0297c`, in the existing isolated worktree. Incoming Reaction
frame, slot/timing, selected movement and Lab adapters are preserved. This
change adds no competing opportunity executor or automatic world selector.

## Result

- Berserker Frenzy requires `rs_turn_owner`, the actual turn rather than the
  reactor borrowed by `reaction-frame.coax`. An off-turn attack retains Rage
  damage without incorrectly adding Frenzy. Own-turn eligibility and use remain.
- Stunning Strike accepts represented Monk weapons as well as Unarmed Strike:
  Shortsword, Mace, Dagger, Spear, Ritual Sickle and Priest's Mace. The common
  readiness predicate requires the feature, chosen option, positive Focus and
  unused strike-local mark. It does not restrict the feature to the own turn.
- Execution and retained-rider preflight use the same weapon/readiness policy.
  Full target storage refuses before Reaction/Focus/dice/events; one available
  slot admits the strike. `rs_strikes_fit` is moved into a focused authority,
  preserving the remaining Vex/Sap/Cunning/retarget accounting and old checks.
- Shared attack edges apply both Blinded/Invisible sight directions in the
  existing flag model. Dodge's attack benefit requires seeing the attacker;
  Dexterity-save Advantage remains independent of sight.

`strike-reaction-checks.coax` exercises actual published opportunity attacks,
Reaction-frame restoration and selected movement. Seven real Monk-weapon hits
stun off-turn without consuming the mover's Stunning mark. A successful save
retains half-Speed through the reactor's actual upcoming turn, reprices movement
debt and stops departure without refunding the accepted Reaction/Focus. Existing
opportunity, movement and Dodge suites compose into the same entry. Fixtures
declare a coherent upcoming reactor turn rather than giving off-turn riders an
extra borrowed turn.

## Evidence

The authored [Frenzy](oa-strike-frenzy-red.log),
[Monk-weapon](oa-strike-monk-red.log) and [Dodge](oa-strike-dodge-red.log)
regressions all failed on published behavior before the fix. On the initial
composition, [core](oa-strike-core-final.log) passes **281/281** and
[hotbar](oa-strike-hotbar.log) passes **231/231**. The
[actual native harness](oa-strike-native.json) runs all six new root scenarios
under evaluator and emitted checked C at O0/O2: six passed, exit 0. It is ordinary
checked C, not a sanitizer or renderer receipt.

| Core authority scope | Killed | Survivors |
| --- | ---: | ---: |
| strike feature/weapon eligibility | 31 | 0 |
| strike rider admission | 55 | 0 |
| basic attack sight | 6 | 0 |
| Dodge | 47 | 0 |

All **139** killed, no equivalent-mutant exemptions. Feature and rider scopes
are freshly gated on the final source. The sight/Dodge units and compiler are
byte-identical to their earlier passing gates in this goal (local checkpoint
`e482b9ce04`); the retained scope receipts are reused rather than rerun. Their
current combined behavior includes the full Dodge suite and independent
16-pair sight/edge truth tables. Policy and compiler hashes are retained.
Published Reaction-frame/opportunity/movement policies are unchanged and retain
their own independent receipts; no blanket requalification is inferred.

The ordinary checked Lab Wasm build on `48bac0297c` plus this source passes,
exit 0, in 82.0 seconds, **2,536,345 bytes**, without compiler or check-budget
overrides. Its complete log and artifact hash are retained. The temporary
output name has no registered shipping size budget. This proves checked build
readiness, not shipping-size or actual browser/audio qualification.

Rebase onto `16da305c2c` preserves the incoming typed-monster opportunity API.
Only `battle.coax` changes among the initial hash set: its additional load;
the four gated units, authored fixture, compiler and existing Reaction/slot/
movement dependencies remain identical. The new composition passes **282/282
core** and **232/232 hotbar** checks. Its ordinary checked Lab Wasm build exits
0 in **80.7 seconds**, **2,536,475 bytes**; the rebased logs and artifact hash
are retained. Unchanged native/mutation receipts are reused.

The following rebase preserves `1f20120688`'s measured physical departure and
C body-admission changes. The four owned policy units and compiler remain
unchanged. Final composed core still passes **282/282**; all **four selected
physical opportunity** scenarios pass (of 774 listed). Because the incoming
query gains measured-distance fields, the six actual O0/O2 native scenarios
were freshly emitted and run on this composition; all pass. Final source hashes
include the physical Coax/C consumer and the measured/typed core dependencies.
Final hotbar passes **232/232**. The actual checked Lab Wasm build on that
composition exits 0 in **82.5 seconds**, **2,539,658 bytes**, with unchanged
budgets. Its final log/artifact hash are retained. No browser run is inferred.

The compiler is cached Coax 0.52.0, SHA-256
`3917d4c94c03cb731dc6a1742e78981288d63021d0783dfa67048c4a001122b9`.
No compiler/Nix/install change or non-Coax tool was written. Tools-check reuses
the passing run at `79a2ce92b3`, retained in
`../srd-stabilization-2026-11-08/tools.log`: no covered tracked
`.ts/.py/.js/.sh/.cjs` changed since that run. The empty path comparison is
retained as `tools-covered-paths.txt`. Commands use 180-second evaluator and
600-second mutation/native/Wasm bounds.

```sh
coax/bin/coax test aethyr/rules/srd/strike-reaction-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/strike-reaction-checks.coax --library --unit aethyr/rules/srd/strike-features.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/strike-reaction-checks.coax --library --unit aethyr/rules/srd/strike-admission.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
python3 coax/tools/native_checks.py aethyr/rules/srd/strike-reaction-checks.coax --out /tmp/oa-strike-native.json
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/oa-strike-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

## Limits and source

This is represented-weapon feature behavior, not a complete legal equipment or
class-grant model. An unspecified Pact Blade identity does not establish a
Simple/Light weapon; natural and spell proxies are excluded. Armor/feature grant
qualification, other class features, complete rests and legal loadouts remain
#13044 work. Basic sight flags do not implement See Invisibility, alternate
senses, cover/occlusion or complete visibility. Incoming physical player
departure is preserved; player reactions to monster movement, network movement
and remaining controls still need their authoritative producers and integration.
No wire/save-key change, owner action, browser or
complete world qualification is claimed, and no issue closes on these fixes.

Pinned SRD 5.2.1 PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`:
unseen attackers/targets p.16, Berserker Frenzy p.30, Monk weapons/Focus/Stunning
Strike pp.49–51, Blinded p.177 and Invisible p.184, plus Dodge's seen-attacker
requirement. This work includes material from the System Reference Document
5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at
https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative
Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.
