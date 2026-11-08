# Dash uses current walking Speed and exact feet

Partial #13040 progress. Baseline `032762d4ee`, compiler 0.52.0. Primary source:
pinned SRD 5.2.1 p.180 Dash, p.51 Step of the Wind, p.62 Cunning Action,
p.182 Grappled and p.186 Restrained. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Change

The three Dash producers previously added base Speed divided by five, ignoring
tracked reductions and truncating residual feet. `dash.coax` now owns admission,
Action/Bonus Action payment, current walking Speed credit and feature events.
It applies the existing shared Slow, halving and Spirit Guardians Speed resolver.
Grappled, Restrained, Steady Aim and the existing zero-Speed Dazed carrier grant
no extra movement. A creature able to act can still spend Dash at Speed zero;
Dash does not override the zero. Invalid logical actors (even backed by allocated
storage), wrong turns, spent budgets, missing features and incapacity refuse
without movement, dice, resource or event changes. Cunning Action and Step of
the Wind retain separate stable feature identities. The free Wind option does
not spend Focus. An Action Dash can combine with one Bonus Action Dash.

Exact feet are added to whole-square plus residual storage. This does not change
the action profile's existing full-current-Speed Dash grant: only its ordinary
turn movement grant is proportional to the short beat. No special movement mode
or new timing profile is introduced.

## Checks and evidence

Checks precede implementation. The retained original red demonstrates modified
Speed, zero-Speed and odd-Speed failures. The later fixtures also verify
unrelated conditions, logical actor bounds, one-point feature capacity, event
identity and a native-callable witness. Initial mutation survivors prompted
specific checks; none were exempted.

- `coax/bin/coax mutate aethyr/rules/srd/dash-checks.coax --library --unit aethyr/rules/srd/dash.coax --jobs 4`: every baseline check passes; 78 killed, zero survivors.
- `coax/web/tool.sh aethyr/rules/srd/dash-native.coax`: actual native `PASS DASH-MOVEMENT`, exit 0. Tests and native use the same source-derived scenario through real wrappers; this is not a second independent oracle.
- `coax/bin/coax test apps/openagents-web/fivebar-test.coax --library`: 198 checks pass, including saved-seat exact credit and preservation of another actor's turn.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`: all 21 selected checks pass; no authority coverage claim for the filtered run.
- `coax/web/tool.sh tools-check`: zero new tool files without a reason.

Every command finished below fifteen minutes. Source/compiler hashes are retained.
The unchanged broad native suite's deferred Polymorph failure remains as recorded
in the previous shared-damage receipt; it was not rerun or suppressed.

## Remaining active requirements

#13040 stays open. Special speeds, complete mid-turn Speed-change reconciliation
of previously credited movement, Disengage/opportunity windows, hiding/visibility,
and the remaining action/reaction/rest requirements are not qualified here.
The current shared walking-Speed resolver's preexisting effect ordering is reused,
not a complete audit of all Speed modifiers. General class identity/grant legality
and the Focus-paid Wind option remain #13044 work. This is not whole-world or
full-SRD qualification.
