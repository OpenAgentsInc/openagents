# Everglade II displays authoritative health

Partial #13046 work on 3641952d80, cached compiler 0.52.0.

The hotbar now displays the local player's HP/class maximum and positive TEMP
from the existing authoritative WireMirror row. It does not fabricate health
from a fresh class preset or predict healing. Zero HP displays UNCONSCIOUS
unless the server's WG_DEAD bit says DEAD. The label is separate from cast
refusal text and the class name.

`e2n_vitals` reads only the current seat's WK_PLAYER row while connected and
seated. Missing own rows, another player's row, non-player identities and
negative HP/TEMP, zero/nonpositive maximum or HP above maximum yield unknown
state (no label). Existing snapshot removal, seat replacement/mirror reset and
disconnection therefore do not leave a fabricated/stale health display. The
lookup does not depend on movement-input ACK advancing, nor the remote entity
render pool having room. No wire, server health ownership or core rule changes.

## Evidence

The first fixture was authored before the API and failed on the missing health
projection. Final checks feed actual serialized D/s snapshot and delta rows
through the page network decoder, verify exact HP/TEMP text, confirm the hotbar
adds/removes text geometry and verify full snapshot omission removes the label.
The second covers peer/wrong-kind/invalid values, unconscious versus dead and
disconnection. Both pass. An extended actual room join/hello/cast regression
captures the server's outgoing row after Second Wind and verifies its HP/max/
TEMP match the authoritative creature. It passes. Tools-check/diff check pass.
This is presentation projection; unchanged core mutation receipts are reused.

```
coax/bin/coax test apps/openagents-web/everglade2.coax --library --only 'Everglade II health'
coax/bin/coax test coax/web/room/room.coax --library --only 'everglade II room executes'
```

These are real codec/server and drawing-function checks, not actual TCP,
headless browser screenshots or complete rendered gameplay. Feature/condition
presentation beyond zero HP, player prediction reconciliation, all-level class
and spell execution, rejoin/runtime qualification and the remaining #13046
requirements still need work. No issue closes on this change.

The ordinary checked Everglade II Wasm build also exits 0 in 23.7 seconds,
producing 633,653 bytes. The full build log and artifact/source/compiler hashes
are retained. No compiler or check-budget override was used. The temporary
artifact name has no shipping size-budget entry, as reported in the log.

```
coax/web/wasm.sh apps/openagents-web/everglade2.coax apps/openagents-web/everglade2-face.c /tmp/srd-e2-vitals.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```
