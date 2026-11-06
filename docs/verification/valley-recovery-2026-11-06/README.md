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

These results do not qualify mobile hardware memory or performance. The
extended shot still has its documented lighting, skin/eye, strand, camera
shake, source orb/flare and scenery differences. The earlier source export
and mutation receipts retain their scope; this work does not replace them
or claim an exact cinematic match. No private source pack, SDK payload,
production deployment or owner sign-off is included.
