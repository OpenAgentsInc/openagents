# Valley facial recovery and visible captures

This record qualifies the legacy pack loader's source reuse and corrects the
Slay verifier's screenshot path. It does not claim completion of #12962's
extended cinematic rendering work.

## Recovery

The old host downloaded the same private pack again during GPU replacement,
although the catalogue's entry views still retained its complete encoded
storage. Recovery now reuses that storage only for the same live engine and
source generation, with a weak owner reference. It does not retain an old
Wasm instance. It rechecks the complete SHA through the existing Coax gate,
parses the pack again, and creates fresh buffers and textures on the new
device. Broker packs still authorize and download each time. Another engine,
failed installation, unavailable private pack or missing WeakRef retains its
ordinary path. The v1 transport delegates before this legacy code.

The [independent SDK oracle](../../../coax/web/tests/legacy-pack-recovery.cjs)
executes the actual loader with effect doubles. Its [red result](loader-red.log)
observes two downloads for same-session replacement. All nine
[delivered cases pass](loader-green.log), including source corruption before
upload and separate engine/generation ownership fences. No recovery deadline,
shader, source pose, material or mip-generation arithmetic was changed.

On November 6, 2026, approximately **05:41–05:49 Chicago time (CST)**, two
muted headless Chromium runs used the same 794,477,568-byte private Slay pack.
The baseline host came from `a06449b17d`; only the loader and verifier changed.
The source-once probe makes the pack unavailable after its initial download.

| Observation | Result |
| --- | --- |
| Baseline, source unavailable after first load | Recovery fails: assets cannot reload |
| Changed loader, source-once probe | Recovery in 8,272 ms, exact state and GPU crop retained |
| TF0100 with corrected visible capture | Recovery in 8,387 ms; exact pause, recovered and neutral crops |
| Source shot with corrected visible capture and source-once probe | Recovery in 8,352 ms; exact state and recovered crop |
| Source shot after recovery | Body/face clock, ambient locomotion and phone viewport pass |

Every delivered observation uses the unchanged 10,000 ms default. None uses
`--diagnostic-recovery`. The source-once assertion proves one pack request,
including recovery. The [original failure](source-unavailable-red.log),
[first recovery](recovery-first-green.log), [face receipt](face-green.log)
and [shot receipt](shot-green.log) are retained.

## Visible verification

The original screenshot helper captured the page canvas while the numerical
renderer drew to an offscreen RGBA target. Those screenshots were black even
though GPU readback hashes were valid. The new [visibility regression](visible-capture-red.log)
refuses that output. The helper presents the actual recorded GPU bytes on
an owned temporary 2D surface, captures it, removes the surface, decodes the
PNG in Coax and refuses empty, flat or status-notice-only images. It does not
advance the source clock or alter the producer's pixels.

The neutral, frame 12 and frame 28 TF0100 captures were visually inspected.
They show the eyes, brows, jaw and lips changing. Pause and GPU replacement
retain the exact crop, and zero morph weights restore the original neutral
crop. The fixed source-shot and phone captures were inspected too. These are
QA observations, not source payloads or training data.

- [Neutral](neutral.png)
- [TF0100 frame 12](tf0100-frame12.png)
- [TF0100 frame 28](tf0100-frame28.png)
- [Source shot](shot1125.png)
- [Phone viewport](shot-phone.png)

## Artifact identity and scope

The observations reuse the qualified `valleycanyon.wasm` from the source port:
91,877 bytes, below its unchanged 96,000-byte budget. Its
[SHA-256](wasm.sha256) is
`c1c833463b2de270a3d87d00053a7dcf4aa6b3f4bbd2593b007e1970495291ae`.
All 59 Coax dependencies and the C face are unchanged from `5d4baa60c1`.
The intervening `wasm.sh` changes apply only to opt-in fast editing; the
ordinary compilation/link profile is unchanged. The measured compiler is
Coax 0.51.2. The visibility helper and its PNG checks compile with the actual
native observer. [Tools-check](tools-check.log) reports zero new files
without a firm reason.

## Integration with Coax 0.52

After integrating `b2e866b1c3`, a fresh ordinary Valley build with the
checked Coax 0.52 compiler produces the exact same 91,877-byte Wasm and SHA
above. The unchanged 96,000-byte gate passes. The fixed-frame facial and
source-shot probes then pass again on November 6, approximately **06:11–06:13
Chicago time (CST)**. Face recovery takes 9,032 ms; source-shot recovery takes
8,289 ms. Both use the ordinary 10,000 ms deadline. The shot probe denies any
second source download and verifies that exactly one request occurred.

The [face receipt](face-052.log) retains distinct neutral/frame-12/frame-28
hashes, unchanged pause/replacement hashes, exact neutral restoration and
next-clip selection. The [shot receipt](shot-052.log) retains frame stepping,
body/face clock agreement, replacement state/pixel identity, ambient
locomotion and the phone viewport. The new [source-shot image](shot-052.png)
was visually inspected. The original baseline failure receipt has only its
trailing whitespace normalized for repository checks; its failure values
are unchanged.

The [shared-host regression](host-drift-052.log) passes local, 100 ms and
250 ms profiles, including the gameplay and raid comparisons. It uses the
new host and independently rebuilt [native room](room-build-052.log) and
[worker](worker-build-052.log). Their SHA-256 identities exactly match the
Coax 0.52 final delivery: native
`83161c8267d24647cc2d9e9015fadf8183ce456becd43ded7a31113dd267bec2`, worker
`1189ca302eef79e6bca10b0dbb3280944b0747043bf8cc35d55cff4e0df5974c`.
The reused SRD page also has that record's exact artifact identity. Valley,
SRD and worker byte-budget commands exit zero. Builds use
`COAX_NEAR_LIMIT=100`, the final delivery's informational-diagnostic setting;
their checks, authority coverage and resource limits remain enabled.
[Tools-check](tools-check-052.log) reports no new tool without a reason.
All owned browsers and servers close after these observations.

After rebasing onto `440c9101c0` and the concurrent host's separate shadow-list
support, the [actual loader oracle](loader-rebased.log) passes all nine cases.
The [rebased drift run](host-drift-rebased.log) also passes local, 100 ms and
250 ms gameplay and raid profiles. The native room, worker and SRD page inputs
are unchanged, so their passing build and budget results above are retained.
This run completed November 6 at approximately 07:22 Chicago time. Its own
room and proxy processes exited; no production service was used.

Reproduce from a fresh worktree with the private pack installed:

```sh
mkdir -p /tmp/slay-face /tmp/slay-shot
# With an own dev server on an allowed port:
coax/web/tool.sh coax/web/valley-face-check.coax 'http://127.0.0.1:9027/worlds/valley-canyon?pack=valley-slay' /tmp/slay-face
coax/web/tool.sh coax/web/valley-slay-shot-check.coax 'http://127.0.0.1:9027/worlds/valley-canyon?pack=valley-slay' /tmp/slay-shot --source-once
```

These results do not qualify mobile hardware memory or performance. The
extended shot still has its documented lighting, skin/eye, strand, camera
shake, source orb/flare and scenery differences. The earlier source export
and mutation receipts retain their scope; this work does not replace them
or claim an exact cinematic match. No private source pack, SDK payload,
production deployment or owner sign-off is included.
