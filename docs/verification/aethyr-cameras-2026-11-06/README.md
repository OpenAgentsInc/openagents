# Aethyr 78: cinematics and close-ups

Recorded November 6, 2026, Chicago time, for [#12610](https://github.com/OpenAgentsInc/openagents/issues/12610).

The [chapter](../../../aethyr/book/ch078-cinematics-and-close-ups.md) remains partly implemented. Typed flight and close-up producers are adopted by Hecate and the Garden. A bounded native camera take records and replays five accepted commands over 256 ticks. A staged capture wrapper delegates to the existing fixed-step recorder and writes a checked sidecar. Saved realm replay to browser pixels remains unqualified.

## Camera checks

The complete camera-take suite passed **116/116 checks**, including imported cinematic, arranging, shot, input, random and replay checks. The actual Hecate arrival keys are shared with the page. The rate oracle checks 30, 60 and 144 FPS against a declared stationary live camera, ordinary path speed below 128 m/s, exactly one intentional cut, all 2,013 displayed source frames as skip points, each hand-back frame against the quintic derivative bound, exact endpoints and reduced motion. Thirty-two independent slices stay under the normal 20-million-step evaluator budget. Initial unsliced checks exceeded that budget; the budget was not raised.

The native example's replay marks include all six camera coordinate bit patterns, the close-up share and flower count. It does not digest the full realm. Omitting the recorded click causes the first divergence at tick 176.

New root mutation scopes retain all imported checks: blend **4/4**, flight **8/8**, close-up **19/19**, staged-label admission **95/95** mutants killed, **zero survivors**, no waivers. The label gate first retained a colon-at-the-end-of-the-digit-range survivor; the final test refuses that malformed commit. Conversion adapters copy fields without mutation sites. These are scoped scores, not a full legacy mutation claim. Earlier exploratory edits to legacy cinematic/arranging files exposed 70/59 surviving root mutants. Those edits were abandoned; the files now remain byte-identical to the starting commit and no equivalent-mutant exemptions were added. The exploratory [flight](legacy-flight-exploration.log) and [arranging](legacy-arrange-exploration.log) reports remain separate from the new producer gates.

Valid assertion-failure receipts are retained for the initial blend stub, refused-projection behavior and label stub. The native refusal example exits 2 for a zero horizontal projection scale and proves stale selection is cleared.

## Realm checks

The complete Villa suite passes **229/229** and Garden **191/191**. The Villa page result precedes the pure relocation of its authored key function; the subsequent normal Wasm build checks the relocated function. No authored key changed. Garden independently projects an opening spot and a stem tip with its real view matrix, clicks them, verifies addition/removal, and closes with the drawn return control.

Normal Wasm outputs are Villa **501,221 bytes** (budget 550,000), Garden **282,655 bytes** (budget 300,000). No budget or traversal limit changed. The headless Metal captures show the real Hecate arrival with golden mode off and the Garden close-up. The Garden development setup holds W, advances 3.2 seconds, presses E, then advances 1 second; golden mode freezes effects. These are staged test flows, not saved session replays.

The shared room native/worker and SRD page outputs are reused from the preceding qualified worktree at the same base commit `88fc9c67ea5397d0813573db7142ca862ef24895`; their tracked inputs are unchanged. Native room SHA-256: `83161c8267d24647cc2d9e9015fadf8183ce456becd43ded7a31113dd267bec2`; worker: `1189ca302eef79e6bca10b0dbb3280944b0747043bf8cc35d55cff4e0df5974c`; SRD page: `3692c5303c1431ebfa39b1fbb2da0b0884793bfb301ac89e6549b7a2396a085b`. The required worker-backed drift rehearsal passed local, 100 ms and 250 ms profiles, including gameplay and raids, within the 15-minute command limit. See [drift.log](drift.log).

## Reproduction

The chapter lists the unit and mutation commands. Native debug/release fixtures, compiler/source identities, capture and drift receipts, Wasm budget checks and book gate results are stored in this directory. All local heavy builds used the shared four-slot gate. Browser runs were headless and muted on permitted localhost ports. No production deploy, database, bucket, credential or owner action was needed.

The staged recorder accepts only declared staged metadata and refuses `--replay`. Its labels describe caller-supplied game, commit, stage and seed; they do not prove a page applied that seed. Repeated staged frames, if identical, qualify only that specific setup on the measured adapter.

## Receipts

- [Native and artifact identities](qualification.json), [camera checks](take-tests.log), [Villa checks](villa-tests.log), [Garden checks](garden-tests.log).
- [Blend](blend-mutate.log), [flight](flight-mutate.log), [close-up](closeup-mutate.log) and [label](label-mutate.log) root mutation scopes.
- [Blend red](blend-red.log), [pointing red](pointing-red.log), [label red](label-red.log).
- [Book structure](book-structure.log), [tools check](tools-check.log), [Villa build](villa-wasm.log), [Garden build](garden-wasm.log). The separate budget commands exit 0 silently.
- [Hecate arrival](hecate-arrival.png) and [Garden close-up](garden-closeup.png), captured with headless hardware Metal.

The first wrapper used the basename `clip`, colliding with the shared recorder cache and recursively launching itself. The owned process tree was stopped and killed; no process from it remained. `a5543ba23e` renames the wrapper to `staged-clip`, clears the ambiguous local cache and keeps its command distinct. The staged capture integration check uses that corrected source.

## Staged capture repetition

The corrected wrapper exits 0, writes an MP4 and the staged sidecar, and ffprobe reports 480 × 300, 30 FPS and 30 frames. Two fresh headless captures of the normal-motion Garden close-up transition match **30/30 decoded RGBA frames**, with **22 distinct frames**. [The comparison](clip-motion-comparison.json) records every frame hash; [take A](garden-motion-a.mp4), [take B](garden-motion-b.mp4) and their `.json` sidecars retain the actual outputs. The URL uses `at=0.1` to keep normal motion. A probe without `at` requests reduced motion; the earlier repeated endpoint setup also matches 30/30 but has only two distinct frames, retained in [its separate comparison](clip-frame-comparison.json). Neither is a saved realm replay.

[Hecate after skip](hecate-skip.png) also renders through the actual page's key and hand-back path. Screenshots were inspected; the native/page suites provide the numeric assertions. The capture sidecars retain their original pre-rebase source commit; the qualification record maps it to the landed commit without rewriting captured metadata.

Rebase preserves every camera/label source hash recorded in `qualification.json`: pre-rebase `a5543ba23e` maps to landed `b7c843788e`. Concurrent main adds a storage-buffer limit request only for pages exporting `coaxial_storage_limit_v1` (the Valley page). Villa, Garden and SRD do not export it; their request keeps the default limits. No tested Coax closure changed. A merged-host input rehearsal records the remaining browser bootstrap check separately from the already passing full gameplay/raid result.

The [merged-host input rehearsal](merged-host-input.log) passes, and the [merged tools check](merged-tools-check.log) passes for all 1,837 tracked tool files. The full earlier local/100/250 rehearsal reports worker p95 tick costs of 600/765/1,210 µs, each below 11,000 µs.
