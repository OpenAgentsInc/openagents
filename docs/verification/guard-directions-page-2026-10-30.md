# Everglade Directions page, 2026-10-30

Chapter 21f connects requested directions to the actual Everglade game. The
visible line is always the deterministic local rule. Elephant remains an
explicitly enabled shadow. This record is separate from chapter 21e's five
real-model contexts and the older ambient-v1 page qualification.

## Implemented boundary

Near calm, healthy Bram, the current followed quest offers Directions. With no
followed quest, a separate Find Work gesture supplies that explicit goal. G,
mouse and touch use a dedicated control without invoking conversation or quest
acceptance. The local line lasts four seconds with a six-second cooldown.

Projection uses current gameplay after the world update. Both the strict world
GET envelope and the existing edit parser must succeed. Edits, hidden
buildings, destruction and spell-built geometry refuse the tip only when they
touch the route it describes (guide-route.coax, 2026-10-31); missing targets,
arrival and unsafe gameplay refuse it. Player IDs cannot authorize a model
request: a well-formed normal viewer hint and the server's ordinary cookie are
required. Only literal `guard-ai=directions-shadow` enables that request.

The page advances gameplay, Coax policy consumes its facts, and the HUD draws
the resulting local view in the same frame. C passes values, clock and HTTP
effects. Callbacks stage only. Epoch, actor/generation, world, followed
quest/stage, goal and intent bind each shadow; invalidation cancels its HTTP
slot. A queued input retains its original epoch and cannot become a fresh
request after map/world invalidation. Nonboundary lifecycle calls cannot consume
queued input or start/admit work.

The shadow has an eight-second lease, ten-second cadence and six-attempt visit
budget. No refusal queues a later retry. Quiet, wrong, malformed, failed or
late answers cannot replace the local line. Existing ambient policy, frozen
labels/requests, provider/checkpoint pins and production traffic are unchanged.

## Changed-source qualification

| Slice | Result |
| --- | --- |
| Session authority | 81 checks, including 27 new checks; 595/595 root mutants killed |
| Session independent C oracle | 105 cases each in debug, release and ASan/UBSan |
| Projection/input authority | 21 checks; 323/323 root mutants killed |
| Projection independent C oracle | 33,187 cases each in debug, release and ASan/UBSan, including all 32,768 eligibility masks |
| Runnable chapter example | Exact independently authored 15-line transcript in debug and release |
| Book inventory | 96 programs, 378 exact source-linked listings and valid local links |

The projection's first 320-mutant/33,168-case result preceded the added queued
epoch gate; the final changed source has the result above. Unchanged Gg, Gd,
Gm and ambient authority mutation suites were reused from their prior records.
No full Psionic mutation gate or whole-model CPU inference was run.

Qualified policy SHA-256 values:

- Session: `9db88ebed40e896ae7c8b5ce19dd371bae5a59210a1fa3902b8e1320a74c1ee4`
- Projection/input: `4d1a4ff977c9fc6f7c392c6ee50fb8ed90e1c469829613691c9972e58787ae97`
- Page guide adapter: `74acbd591100bc667872e56e0eb6de407c7ea929c6db443a39a9b09a9dd9274e`
- C effect bridge: `967e55b12335a65dd55a835b57d10368491d93d1aef61ef394973c0b6f640b03`
  (after merging main's guard-captain request beside it; `f90bc6b3…` before)

## Compiler and actual Wasm

The local installed compiler predates main's composable public-header emitter.
A temporary, explicit compiler was built from source snapshot
`f762c4f7ae66eae27ec5664734e3f37025f050e9` and
fresh authoritative compiler aggregate, rather than linking an old aggregate
against a newer header. It re-emitted that aggregate byte-for-byte, passed the
three actual proto numeric-range checks and a fresh projection/header native
oracle. No compiler source, seed, installed binary or provenance control changed;
this is a focused build qualification, not a bootstrap release gate.

- Driver: `/tmp/guard-ch21f-full-compiler/coax`
- Driver SHA-256: `33f2c23ad3923c40383fb422ac5c8811b65da45129d077dc939d83e6cace66ef`
- Public-header emitter source SHA-256: `f91fc098c7f512f4ea697655333b5d707824f12fbdc889561b0b21fe565ea0bf`
- Temporary qualification receipt: `/tmp/guard-ch21f-full-compiler/QUALIFICATION.txt`

The change was rebased onto `38fd4927cc`, retaining main's repository-relative
compilation and gzip/preload build changes. The public-header emitter is
unchanged by that rebase. The Wasm wrapper accepts an explicit `COAX` path and
retains the normal installed-driver default. The game links a single aggregate policy library so
shared ambient/directions exports and data types have one owner. Release build:

```shell
COAX=/tmp/guard-ch21f-full-compiler/coax COAXIAL_MODE=release \
  coax/web/wasm.sh \
  apps/openagents-web/everglade.coax \
  apps/openagents-web/everglade-face.c \
  apps/openagents-web/build/everglade.wasm \
  verse/player.coax verse/grid.coax verse/everglade/river.coax \
  verse/everglade/guard-guide-policy.coax
```

Release module SHA-256 after the final rebase onto `2d3d741173` (which adds
main's guard-captain request, reply and fork controls):
`5cec37586ce8ee32a1f61a0e0ebf4da46c6745ce2cb1c591262ae781755a45ee`.
The pre-rebase module `c0b8a55f…` had passed the same 26 cases.

## Actual browser scope

The Coax CDP driver ([psionic/tests/guard-directions](../../psionic/tests/guard-directions/browser.coax))
uses headless muted WebGPU Chrome, the real emitted game Wasm/host, ordinary
game input and controlled loopback API responses. It denies external network
traffic. No live model, key or production deployment is part of this client
qualification. Build it as a Coax tool and run every group in one browser:

```shell
coax/web/tool.sh psionic/tests/guard-directions/browser.coax \
  http://127.0.0.1:18883 NEW_RESULT.json all
```

Final receipt (2026-10-31, module `5cec3758…`, stable for the whole run):
**26/26 cases passed** in 64 seconds; 20 intercepted credential-free
Directions requests, 0 live model calls, 0 external requests, 8 stale replies
refused (each a fulfilment of an already-cancelled request, which Chrome
rejects as a CDP error; no other CDP error).

The decision route is shared with main's guard captain, which asks while the
player is wanted. The driver counts a request as Directions only when its
`state.policy` is exactly `everglade-directions-v1`; any other is answered 503
(the captain's own refusal path) and counted apart. The combat case saw one
such captain request. Before that split, the rebased build failed both combat
cases on the second request; the Directions behavior itself was unchanged.

| Group | Cases | Result |
| --- | --- | --- |
| Local | default URL, unqualified mode, guest shadow, button + East Gate tip (screenshot reviewed), mouse Find Work, touch Find Work | 6/6; 0 starts, local line drawn |
| Replies | reply staged until the frame boundary, delayed accept, quiet stays local | 3/3; local line unchanged |
| Service failures | wrong route, malformed, oversized, HTTP 401 (disables), HTTP 429 (disables), HTTP 503 | 6/6; refused with reasons 8, 6, 9, 2, 3, 9; local line kept |
| Timeout | eight-second lease expires | 1/1; reason 5, no reply admitted |
| Leaving/edits | goal change, area leave, pagehide, world edit, blur | 5/5; cancelled (reason 7) before any reply |
| Reuse | new intent, then a late reply on the reused ticket | 1/1; late reply ignored, new one admitted |
| Combat | authored wanted-combat revokes, natural respawn keeps the revocation | 2/2; cancelled (reason 7) |
| Queued input | G then map open/close yields no tip or request; a fresh G works | 2/2 |

The same 26 cases had also passed on the pre-rebase module `c0b8a55f…`,
both per group and in one run.

## Production

First released by `coax/web/release.sh` as `bcb6099592` (stamp
`20261031T052547Z`): Directions was never offered there, because the published
world (version 21) has 11 owner edits and the projection then refused any
edited world.

Edit-aware routes and the empty-demolition-world guard shipped as `b713ceb8a6`
(stamp `20261031T054424Z`, staged, live in 288 s; module
`everglade.71884e3c1d05.wasm`, local build
`1842022e7b4be7a071d04c38b9625959175b8440428455e8a885124e76833c3c`). That build
passed 28/28 browser cases, including the `edited` group: the published 11-edit
world keeps Find Work and its local tip with no request, and the same list plus
one crate in Inn Street refuses the offer while Bram is eligible.

Headless muted Chrome on `https://openagents.com/realms/everglade` as a guest,
against the live 11-edit world: walking north from the default spawn, Bram
offered **Find Work** after 0.8 s; G drew "The quest board is beside the
commons fountain." with 0 decision requests. A headless tab never has focus,
and a page without focus suspends the guard, so the check set
`document.hasFocus` true and resent the page facts, as a focused tab reports them.

## Remaining gates

This makes local help usable and exercises the real optional shadow consumer.
It does not demonstrate model advantage over the instant perfect local rule,
player benefit, reachability around obstacles that do not touch the street strip, independent-provider
recovery or production fault tolerance. Remote-selected dialogue, ambient
default activation, combat/crime decisions and Belt remain separate work.
