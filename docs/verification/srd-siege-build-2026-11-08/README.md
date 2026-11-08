# Siege tactics and checked Lab Wasm build

Partial #13046 work on be2e25fb77. The Siege leader's declared world kit now
supplies Circle of Death's reusable crushed black pearl powder worth 500+ GP
(SRD 5.2.1 p.115). This is explicit equipment, not a class benefit or component
waiver. The new component-availability regression failed before the kit change.
This preserves existing high-level content without qualifying it for the
levels 1–6 slice.

The old whole-battle tactics fixture assumed the swarm always engaged. A
controlled trace instead proves that the seeded default 60-foot approach ends
with the watch defeated and the leader never observing engagement. A separate
regression now preserves that default ranged-win outcome. The tactics fixture
starts its swarm six squares closer so its engagement-dependent assertions
actually run. It retains every original assertion: contact, Mage/Fanatic/leader
spell spending, leader spacing and at least 80% of standing front-line samples
within 15 feet of their posts. No production AI, default map, dice, rules or
check budgets change. An independent direct-turn fixture verifies actual
Cultist walking plus Dash closes the default distance within two action turns.

The checked build passes using cached compiler 0.52.0:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-siege-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

Exit 0 in 31.4 seconds: 2,509,126 bytes. SHA-256:
`ce42a5510f4481efa3813ff684154d2330d7fd43ed386e6254eb429d075f1f3d`.
Compiler SHA-256:
`52402aa1520ff7f267ce7c44b3d3dfa8e9dacc868bf7d84d95cdf95a0586d7f1`.
The temporary output name has no shipping size-budget entry; the build reports
that fact. Existing capacity/headroom notes remain in the log. No compiler or
budget override was used. Compilation executes the app's checks; unchanged
library cache entries were reused. Focused tactics, movement and component
checks also pass. Tools-check passes. No core admission/policy changed, so this
world-kit/fixture change uses focused verification rather than a new mutation
gate. Prior core mutation receipts remain in srd-lab-build-2026-11-08.

This proves a checked Wasm build, not browser rendering, audio, actual play,
complete class/spell/monster execution or all of #13046. Those remain open.
