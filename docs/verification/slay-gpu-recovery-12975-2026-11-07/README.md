# Slay full-groom GPU recovery — #12975

Source baseline: `6efb630712`. Machine: Apple M5 Max, hardware WebGPU/Metal,
headless muted Chrome. Current full-groom private pack: 1,023,642,576 bytes,
SHA256 `a2d937dbe5deed4ec3e1085ddae039160cadae2f9d585cefd3fba14c422b4a56`.
It contains 35 image layers, 187,564,308 vertex bytes and 73,674,948 index bytes.
This is the current rect-light/lens revision; it is 8,000 bytes larger than the
historical issue's pack. The full groom remains present and uploaded. Existing
draw-detail selection is unchanged. No source data or captures are committed.

## Change

The baseline already has same-engine retained-source recovery (`fd9d908c30`)
and explicit decoder lifetime management. On this machine the old historical
11.8–14.2 s failure did not reproduce: baseline shot recovery was 8,022 ms.
It still redundantly staged another 261,239,256 geometry bytes and serialized
35 atlas decodes/uploads plus 455 bitmap/mip operations.

Aligned immutable entry views now upload directly; empty and unaligned spans
retain their padded staging. Two bounded upload workers overlap atlas decoding
with mip construction/upload. Mip dimensions, high-quality resize filter,
sRGB format and uploaded content are unchanged. At most two source bitmaps and
one transient mip per worker are live; this trades one additional active atlas
for less serialization, without retaining the full decoded array. Both workers
settle and close their images before a refused texture is destroyed. Canonical
renderer-owned layer construction and broker authorization remain unchanged.

The host's 10,000 ms fence remains unchanged. Face and shot checks now explicitly
require reported recovery <=10,000 ms (the pre-existing labeled diagnostic mode
is still separate and was not used for these results).

## Evidence

| Observation | Recovery | Result |
| --- | ---: | --- |
| Baseline source-shot, source-once | 8,022 ms | passed |
| Final source-shot, source-once | 6,124 ms | passed |
| Before-pipeline facial performance | 8,148 ms | passed |
| Final facial performance | 7,264 ms | passed |

These are observed runs, not a claim of a timing guarantee under arbitrary host
load. Machine load was about 2.6–3.1 at the end; the historical report named 6–8.
The source-shot test blocks a second private source request, requires one fetch,
checks exact source-frame round trips, state digest and recovery pixel equality,
then checks ambient locomotion and the phone viewport. All six source-shot
pixel hashes also match baseline vs final. The face test checks moving facial
poses, exact pause/neutral pixels and matching state/pixels after recovery.
Both final runs report no unexpected errors.

Independent effect oracles execute the actual host functions. New assertions
failed before the code change, then passed: aligned upload buffers borrow the
retained storage; empty/unaligned bytes pad with zeros; two-source scheduling
uploads every layer/mip once and joins/cleans up on decoder refusal. Existing
corruption, generation isolation, fresh-device resources, broker reauthorization,
incomplete installation and v1 delegation checks pass. Character Create is also
checked as another live legacy texture consumer. No deadline or realm budget
was increased, and source admission remains in Coax.

Commands:

```sh
node coax/web/tests/legacy-pack-recovery.cjs
node coax/web/tests/legacy-layer-upload.cjs
coax/web/tool.sh coax/web/valley-slay-shot-check.coax 'http://127.0.0.1:8856/worlds/valley-canyon?pack=valley-slay' /tmp/issue12975-pipelined --source-once
coax/web/tool.sh coax/web/valley-face-check.coax 'http://127.0.0.1:8856/worlds/valley-canyon?pack=valley-slay' /tmp/issue12975-face-final
```
