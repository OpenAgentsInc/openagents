# Walking and jumping report actual rest interruption

This continues the stopped-agent handoff in #13040 at `9352b75109` without
reimplementing Short Rest benefits. Source is SRD 5.2.1 p.187, pinned PDF
SHA-256 `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`:
Short Rest downtime permits reading/talking/eating/standing watch, not strenuous
locomotion. Existing explicit Initiative/damage/leveled-casting interruptions
remain unchanged.

The shared `rs_rest_motion` closes either the in-progress rest or its sequential
completed choice window after accepted walking/jumping. It changes only phase;
spent Hit Dice, their seeded identity and daily recovery histories remain.
`rs_step_to` reports only after admission and payment. Invalid grid steps
cannot cancel rest by attempting movement.

The C face transports controller intent and the admitted body to Coax after
body admission. `eg_step_motion_intent` uses the actual analog dead zone and
busy/still/aboard/jump gates; the Lab pairs it with real horizontal displacement
or a grounded-to-airborne rise. Held keys against a wall and rejected jumps do
not interrupt. Passive displacement with no controller locomotion intent does
not invent strenuous movement. Both Experimental and ordinary player-grid body
paths use the same reporting hook. Accepted monster AI locomotion reports the
activity too; non-locomotion placement does not masquerade as walking.

## Verification

The accepted-grid-step regression failed before implementation. Full composed
rest/activity entry: 260 checks pass. The rest-state gate includes the original
Short Rest regressions, not only the new movement fixture: 95 killed, zero
survivors/no exemptions. Unchanged recovery gates remain at the handoff commit.
Focused real world checks pass for denied attempts, passive displacement,
accepted walking/jumping and AI locomotion versus placement. The controller
intent fixture checks analog equality/below/above threshold and denied world
input. Native and complete checked Wasm receipts are retained alongside these.

```
coax/bin/coax test aethyr/rules/srd/rest-activity-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/rest-activity-checks.coax --library --unit aethyr/rules/srd/rest-state.coax --jobs 4
coax/bin/coax test apps/openagents-web/everglade-step.coax --library --only 'controller motion intent'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental rest activity'
coax/web/tool.sh apps/openagents-web/experimental-rest-activity-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-activity-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
```

## Remaining scope

The authoritative world must begin rest only from restful downtime, including
waiting for the controller to settle. A rest-start control/admission flow is
still absent; this is not permission to begin rest while already coasting.
Network room movement, other strenuous activity/feature producers, crafting,
special movement and full world interruption qualification remain. Long Rest
admission/reset and rest controls are still separate active work. This does not
claim to complete #13040 or #13046 or to qualify world reconnect persistence.

Final native output is PASS REST-ACTIVITY: denied attempts and passive movement
retain rest, accepted own walking/jumping cancel it, and the old deadline cannot
complete an interrupted rest. Complete checked Lab Wasm build passes in 63.3
seconds, 2,538,964 bytes, compiling the real C face's intent/body reporting call.
No work budget was increased. Existing compiler-source and near-limit advisories
remain; the temporary artifact name has no registered shipping-size budget.
Tools-check/diff check pass. Hashes pin final sources/compiler/artifact; no
browser, audio playback or full world-control qualification is inferred.
