# Declared state qualification — #12991

Source baseline: `536bcf7d45f162b1cb4190ab45ad65450c9213cd`, with the changes in
this delivery commit, rebased onto `42507b7d57`. Neither native nor browser
driver dependency closures changed during that rebase; their passing evidence
is reused. [Source/compiler/Wasm byte identities](receipts/source-identity.json)
pin the measured implementation. Recorded on macOS arm64, headless muted Chrome,
2026-11-08 UTC. No GPU performance or distributed transaction claim is made.

## Delivered boundary and actual adoption

[`aethyr/state`](../../../aethyr/state/README.md) supplies admitted field identities,
values/defaults, owner/scope, reset/save/retry masks, lifecycle fences, typed
predicate format 1, six-root atomic variants, FST1 migrations and temporary retry.
Full names are retained; names are never hashed into addresses. Duplicate names
or IDs refuse. SHA-256 pins the complete schema and logical content catalogue.

State Lab is a playable engine-owned consumer using the existing walker,
semantic fluid/overlap queries, navigation flow, RCM1 resource owners/decoders,
input edges and single 64 Hz clock. C/JS only expose the SDK. Its six variant
members are placements, collision, navigation, fluid, lighting and rendering.
All twelve gate/NPC/water combinations use actual decoded resources; the
fixture never forges resource-ready flags. It has a direct `/worlds/state-lab`
page, a release build entry and a 170,000-byte Wasm budget.

The existing Haunted House save/editor path adopts shared quest/fired-zone fact
admission in `verse/haunt-state.coax`. The duplicated range decisions were
removed from `verse_haunt_save_valid`; writing also checks the same declarations.
HH_Save v1 bytes, source revision and authored document rules remain unchanged.

Residual adapters are explicit: other HH_Save fields, full entity/physics saves,
forge document/undo encoding, private realm flags and account/server CAS retain
their previous implementations. The new fixture's player pose is transient;
cold loading restores the declared world, with the player at its new session's
spawn. CPU cell-body records represent nav/fluid/NPC descriptors and are charged
conservatively as actors. This profile qualifies selection/publication of a real
PNG lighting root and Canvas presentation, not a general GPU lighting pipeline.

## Acceptance evidence

- Every reset event has an independently declared field and an unrelated survivor.
  A runtime contract probe runs all seven events, repeated-operation refusals,
  natural 16-tick midnights and a recorded jump at paid step 33. Its per-step FST1 hash chain
  is independently constructed by Python and identical on native and Wasm at
  30/60/144 FPS and uneven frames:
  `0f5902dc17a0f51a7f04d150d3743494bd22718a0fb1c7850172eb633d34d54c`.
- Eight playable replays each pay 640 steps in ten simulated seconds, with 19
  identical observed cuts and final digest
  `ef30283611cc952233a0e079c8f6579378c65f1f755bbae93aa597882f84f48d`.
  Frames are 300/600/1440/557; no time is dropped. The uneven schedule repeats
  1/23/7/48/11 ms. The same entry point drives interactive RAF and the probes.
- Before gate opening, the actual shared walker stops at x=7.2. Opening changes
  collision, navigation and rendering together; the walker crosses x=8.5.
  Water selection uses the semantic fluid query and affects walking pace.
  NPC presence derives from admitted schedule and canonical phase.
- Cold Wasm loads start with empty derived caches. Gate, nav, fluid and NPC
  selection rebuild to variant 9 from the AESV facts. A native-written save loads
  in Wasm with identical cold-world digest, and its bytes remain unchanged.
  Invalid imports refuse while retaining the previous good save.
- Each of six stale member receipts returns WAITING and preserves variant 0,
  gate/nav/water/credit. Clean retries publish variant 6 once. Core checks also
  cover missing/ambiguous combinations, wrong content, epoch/serial mismatches,
  duplicates/old-world aliases, partial-acquisition rollback and interrupted
  restore publication. No subset of the new six roots publishes.
- Migration checks cover exact/current and old/newer revisions, explicit bounded
  clamps/defaults/discards with receipts, unknown IDs, duplicate identities,
  owner/type mismatch, stale entity generations/content pins, every truncation,
  corrupt hashes and nonfinite bit patterns. Reference/economic identity is
  never repaired. The independent struct/hashlib oracle checks all six value
  kinds, signed integers, schema bytes, AESV CRC and actual FST1 trailer.
- Retry resets the retry-participating gate/NPC facts while preserving the
  nonparticipating fluid field and separate credit domain. Repeated break,
  duplicate operations, load and editor reset cannot duplicate that credit.
  This is a checked external-domain **model**, not a wallet/server reward service.
  Stale owners cannot publish. Checkpoint/journal and owner fencing modules are
  reused; their protocol and revision rules are not replaced.
- A visual-only time change and disabled observation leave the authoritative
  final digest identical. Midnight and time-jump resets are exercised in the
  runtime probe; unit checks additionally cover mask ordering and day boundaries.

The [independent oracle result](receipts/oracle.log) validates the actual receipt
files, not output generated from an imported engine model. Run it against copies
named by the driver prefixes described below. [Native](receipts/native-60.json),
[Wasm](receipts/wasm-60.json), [cold](receipts/cold.json),
[cross-host save](receipts/cross-save.json), [visual](receipts/visual.json) and all
six `faultN.json` records retain the numerical evidence.

## Work, allocation and timing bounds

The supported bounds are 32 fields, 63-byte names, 64 reference entries, eight
patches, 16 variants, 32 predicate terms and six roots; the owner pool has 64
reference slots. The fixture owns 12 pin claims plus six active claims (18),
allows four simultaneous resource loads, 32 admitted actors, seven category
caps of 262,144 bytes and 524,288 total bytes. Peak charged resource bytes were
3,045, peak admitted actors six, final outstanding loads zero. StateLab occupies
175,792 bytes on both measured ABIs. FST1 has at most 538 LE64 words; the actual
AESV save is 748 bytes. Wasm's measured memory is a fixed initial 16 MiB (64 MiB
maximum, no growth observed). Drawing has 128 records; observer storage is a
fixed 19×28 matrix plus hashes. Probe steps are bounded to 1,024 per SDK call.

The release Wasm is 146,029 bytes; the recorded build took 3.6 s. Browser execution
of the full ten-second replay plus lifecycle probe took about 37–40 ms per fresh
instance under the existing machine load. This is CPU fixture throughput, not a
rendered-frame benchmark. The timed cached native wrapper/run took 0.07 s wall;
its process reported 7,290,880 bytes maximum RSS and 2,834,840 peak footprint.
Those process values include the wrapper; they are not heap-only measurements.

Content catalogue SHA-256:
`6e52d0dffe045f997fe848cd5b54c0400221b6d15642d33b760c6d6d78c003c9`.
Schema SHA-256:
`8df9e67bee7e53ba42bf39e903d869cee4ec7a04071ec1c7b80fb8aecf459279`.
Actual AESV SHA-256:
`992efd3d8afe08cc4cdfa6c40981ab14bc20610ebcffe081b1b2cbdb127590c4`.
Logical content hashes exclude host-dependent allocation accounting.

## Gates and reproducibility

Seven new authority roots kill **1,186/1,186 mutants**, zero survivors:
declarations 201, lifecycle 206, codec 355, variants 327, restore 52, external
credit model 20 and Haunted House admission 25. Root-only mutation retains all
dependency checks. Unchanged passing roots were reused; the versioned predicate
change reran its changed root. The integrated fixture had 670 passing checks
before that additional predicate-version check; the final native/Wasm compiler
builds include the new check. No equivalent-mutant suppression was used.

The native 640-step fixture runs with ASan/UBSan. The existing complete Haunted
House sanitizer smoke passes gameplay, nine triggers, rendering packets,
teaching, editing and a 10,900-byte protobuf round trip; its unchanged buffer
boundary kills 96 mutants. The baseline smoke had stale literals (1,645/87)
for the existing 1,659 instances/88 meshes. An unchanged-source baseline run
reproduced it; only those expected inventory counts were corrected.

The Aethyr chapter registry passes, including the narrowly qualified day/phase
portion of chapter 71. The full book command currently fails its separate
reviewed-capability gate on unchanged `verse/everglade/gpu.bindings` evidence;
[the failure is retained](receipts/book-wide.log). It is not reported as a pass.
Other chapter status and renderer/weather ambitions remain scoped as before.

```
coax/bin/coax mutate aethyr/state/variants.coax --root-only
coax/bin/coax test aethyr/state/fixture.coax
COAXIAL_RUN_STEPS=250000000 coax/web/wasm.sh aethyr/state/fixture.coax apps/openagents-web/state-lab-face.c apps/openagents-web/build/state-lab.wasm
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:9379
# Native: run each FPS (0 means uneven), retaining the 60 FPS receipt as native60-current.json.
coax/web/tool.sh aethyr/state/tests/run.coax 60 /tmp/native60.aesv
coax/web/tool.sh aethyr/state/tests/browser.coax http://127.0.0.1:9379/worlds/state-lab /tmp/browser /tmp/native60.aesv
python3 aethyr/state/tests/oracle.py /tmp/browser /tmp/native
coax/web/tool.sh tools-check
# Native sanitizer harness (mkdir /tmp/state-asan first):
coax/bin/coax c aethyr/state/fixture.coax --library --header /tmp/state-asan/fixture.h --run-steps 250000000 > /tmp/state-asan/fixture.c
clang -std=c11 -O1 -g -fsanitize=address,undefined -ffp-contract=off -fno-fast-math -I/tmp/state-asan /tmp/state-asan/fixture.c aethyr/state/tests/sanitized.c -lm -o /tmp/state-asan/run
/tmp/state-asan/run
```

Desktop 960×640 and phone 390×844 were inspected headlessly; controls and status
fit after shortening the narrow view labels. The capture shows the cold loaded
world. See [desktop](desktop.png) and [phone](phone.png).
