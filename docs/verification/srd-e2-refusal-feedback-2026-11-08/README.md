# Everglade II cast-refusal feedback

Partial #13046 work on 2e9c805405, cached compiler 0.52.0.

The page now retains a fixed player-facing message for a matching refusal of
its most recent cast request. Cooldown, range, unavailable-slot and invalid-aim
reasons have concise messages; other reasons (including shared-rule `no`)
produce CAST NOT ALLOWED. No server-provided text is drawn raw. Version,
elsewhere and restart handling keeps its existing priority. Malformed/nonpositive
or older sequence refusals cannot replace cast feedback. The next locally
admitted cast clears the old message; otherwise it expires at 4000 ms, with
negative clock ages hidden.

The actual hotbar drawing path places the message above its class label in
warm red. The regression checks a matching real network record adds rendered
text geometry and exact expiry removes it. Presentation predicted before server
acceptance is not undone: already played sound/particles cannot be reclaimed
by this text feedback. This change makes refusal visible; it does not claim
complete prediction reconciliation or server-state display.

## Verification

The initial UI regression failed because the feedback API did not exist.
Final focused network parsing/reason/sequence/expiry/new-press checks pass, as
does the existing class hello/cast/broadcast scenario and the hotbar drawing
fixture. Tools-check and diff check pass. This is presentation handling, with
no core rule/admission change; no new mutation gate is imposed.

```
coax/bin/coax test apps/openagents-web/everglade2.coax --library --only 'cast refusal feedback'
coax/bin/coax test apps/openagents-web/everglade2.coax --library --only 'shows a matching cast refusal'
coax/bin/coax test apps/openagents-web/everglade2.coax --library --only 'your class goes with the hello'
```

No headless browser screenshot, actual socket play or rendered-page visual
qualification is inferred from geometry checks. Health/temp HP/feature display,
rejoin state, all level 1–6 effects and complete world acceptance remain open.

The ordinary checked Everglade II Wasm build exits 0 in 66.6 seconds:
632,395 bytes. Full log and artifact/source/compiler hashes retained. No
compiler or check-budget override is used. The temporary artifact name has no
shipping size-budget entry, as reported in the log.

```
coax/web/wasm.sh apps/openagents-web/everglade2.coax apps/openagents-web/everglade2-face.c /tmp/srd-e2-refusal.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```
