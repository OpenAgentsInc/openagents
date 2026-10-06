# V24 platform client evidence

Issue [#10742](https://github.com/OpenAgentsInc/openagents/issues/10742) adds
portable authoritative chamber sessions over proved REACH channels.

`checks.json` records the candidate revision, patch and log digests, exact
commands, test summaries, compiler, and linked Wasm digest. `source.patch` applies
to its recorded source base. The Wasm artifact stays in the agent's existing
warm target; its bytes are not a distributable browser page without matching
`wasm-bindgen` glue. The source and logs are retained here.

The final checks run after integration of current `main`'s control-acknowledgment
and prediction fixes. `v24-world-before-integration.log` preserves the earlier
592-test pass. Native REACH tests cover TCP/WebSocket, signer binding,
revocation, and reconnect. Mobile tests cover the mounted session, mapping,
suspend/resume, host failure, respawn, and logical/physical viewport conversion.
Session and content tests exercise shared prediction and verified prepared
content identity. The Wasm build checks target compilation and linking. `first-source.patch`
retains the earlier native component phase; `source.patch` retains the latest
integration phase. The latest integrated phase reruns mounted-session and
changed-prediction tests, browser linking, and the latest scratch host build; the earlier world and native
compile checks remain separately recorded. The standalone movement regression
also runs again in the final session suite.

Earlier failed logs retain the stale-metadata mobile failure and the browser
dependency/time adaptation failures. They are development failures, not accepted
checks. Each final command must exit 0 before this receipt is admitted.

The [platform matrix](../../../../docs/verse/platform-clients.md) and
[owner checks](../../../../NEEDS_OWNER.md) state the supported combat subset,
audio-output gaps, and unperformed hardware-browser and physical-device acceptance.
Neither native loopbacks nor Wasm linking prove those outcomes.

`browser-smoke.json` and the two images retain a headless Chromium/software
WebGL2 run with a temporary profile, temporary keys, and a scratch host. It checks
authenticated HUD/rendering and server-observed keyboard movement at a
1000-by-800 CSS viewport with adaptive graphics resolution, visible 44-pixel controls, narrow viewport scaling,
focus release/rejoin, shortcut preservation, and refused/restored grants.
`browser-cached-grant-failure.json` records the first run's stale-cache failure.
The corrected configuration fetch bypasses browser caches.
`browser-software-gpu-budget-failure.json` retains a failed 1000-by-800 movement
probe: software rendering ran at about four frames per second and retired
control. The earlier `browser-movement-control-failure.json` probe used KeyA,
which turns; its translation assertion was invalid. The retained budget failure
uses KeyQ to strafe, as the passing final probe does. Full-resolution software
rendering failed the performance budget. The final
probe now passes at a 1000-by-800 CSS viewport after graphics work adapts to a
237-by-190 backing canvas. DOM targets retain their CSS size.
`browser-low-resolution-before-adaptation.json` preserves the earlier smaller
browser-window run. Session and graphics still share one browser thread. The
driver removes old receipts before each
run and requires authority position, sequence, and stable-epoch progress. The
retained Rust
host and Python CDP driver are infrastructure fixtures; their absolute checkout
and browser paths identify this run and must be adapted when reproducing it.
No owner browser profile, resident service, or physical device is used.

The existing warm target was removed during verification. `v24-warm-recovery.json`
records the surviving temporary caches restored with file hash checks. Source,
logs, and runtime images remain retained independently of mutable build caches.

`integration-current/checks.json` retains the final integration of the renderer's
budget degradation, procedural character updates, and bounded movement tracing.
Its source patch, seven session tests, browser link, scratch-host build, formatting,
and browser receipt identify that candidate independently of the earlier phases.
The authority moves 5.28 meters with a stable epoch and zero stopped-input drift
at the same 1000-by-800 CSS viewport and 237-by-190 backing canvas.
