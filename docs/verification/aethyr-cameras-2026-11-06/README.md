# Aethyr 78: cinematics and close-ups

Recorded November 6, 2026, Chicago time, for [#12610](https://github.com/OpenAgentsInc/openagents/issues/12610).

The [chapter](../../../aethyr/book/ch078-cinematics-and-close-ups.md) remains partly implemented. Typed flight and close-up producers are adopted by Hecate and the Garden. A bounded native camera take records and replays five accepted commands over 256 ticks. A staged capture wrapper delegates to the existing fixed-step recorder and writes a checked sidecar. Saved realm replay to browser pixels remains unqualified.

## Camera checks

The complete camera-take suite passed **116/116 checks**, including imported cinematic, arranging, shot, input, random and replay checks. The actual Hecate arrival keys are shared with the page. The rate oracle checks 30, 60 and 144 FPS against a declared stationary live camera, ordinary path speed below 128 m/s, exactly one intentional cut, all 2,013 displayed source frames as skip points, each hand-back frame against the quintic derivative bound, exact endpoints and reduced motion. Thirty-two independent slices stay under the normal 20-million-step evaluator budget. Initial unsliced checks exceeded that budget; the budget was not raised.

The native example's replay marks include all six camera coordinate bit patterns, the close-up share and flower count. It does not digest the full realm. Omitting the recorded click causes the first divergence at tick 176.

New root mutation scopes retain all imported checks: blend **4/4**, flight **8/8**, close-up **19/19**, staged-label admission **95/95** mutants killed, **zero survivors**, no waivers. The label gate first retained a colon-at-the-end-of-the-digit-range survivor; the final test refuses that malformed commit. Conversion adapters copy fields without mutation sites. These are scoped scores, not a full legacy mutation claim. Earlier exploratory full runs on unchanged legacy cinematic/arranging files had 70/59 survivors; those files remain byte-identical to the starting commit and no equivalent-mutant exemptions were added.

Valid assertion-failure receipts are retained for the initial blend stub, refused-projection behavior and label stub. The native refusal example exits 2 for a zero horizontal projection scale and proves stale selection is cleared.

## Realm checks

The complete Villa suite passes **229/229** and Garden **191/191**. The Villa page result precedes the pure relocation of its authored key function; the subsequent normal Wasm build checks the relocated function. No authored key changed. Garden independently projects an opening spot and a stem tip with its real view matrix, clicks them, verifies addition/removal, and closes with the drawn return control.

Normal Wasm outputs are Villa **501,221 bytes** (budget 550,000), Garden **282,655 bytes** (budget 300,000). No budget or traversal limit changed. The headless Metal captures show the real Hecate arrival with golden mode off and the Garden close-up. The Garden development setup holds W, advances 3.2 seconds, presses E, then advances 1 second; golden mode freezes effects. These are staged test flows, not saved session replays.

The shared room native/worker and SRD page outputs are reused from the preceding qualified worktree at the same base commit `88fc9c67ea5397d0813573db7142ca862ef24895`; their tracked inputs are unchanged. Native room SHA-256: `83161c8267d24647cc2d9e9015fadf8183ce456becd43ded7a31113dd267bec2`; worker: `1189ca302eef79e6bca10b0dbb3280944b0747043bf8cc35d55cff4e0df5974c`; SRD page: `3692c5303c1431ebfa39b1fbb2da0b0884793bfb301ac89e6549b7a2396a085b`. The required drift rehearsal runs locally against those outputs; its final receipt is recorded alongside this file.

## Reproduction

The chapter lists the unit and mutation commands. Native debug/release fixtures, compiler/source identities, capture and drift receipts, Wasm budget checks and book gate results are stored in this directory. All local heavy builds used the shared four-slot gate. Browser runs were headless and muted on permitted localhost ports. No production deploy, database, bucket, credential or owner action was needed.

The staged recorder accepts only declared staged metadata and refuses `--replay`. Its labels describe caller-supplied game, commit, stage and seed; they do not prove a page applied that seed. Repeated staged frames, if identical, qualify only that specific setup on the measured adapter.
