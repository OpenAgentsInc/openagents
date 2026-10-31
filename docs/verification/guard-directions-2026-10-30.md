# Requested guard directions: a bounded live candidate

The new `everglade-directions-v1` candidate chose the authored useful route in
**5/5** frozen contexts: **3/3 development and 2/2 held out**. Every response
passed exact model/provider/choice, normalization, confidence, deadline and
current-goal semantic checks. Browser API fetch time was **600–652 ms**, median
**625 ms**. These are five serial smoke observations, not a production latency
estimate, calibrated confidence or measured player benefit.

The deterministic goal-to-tip rule also matches **5/5**, immediately, with no
remote calls. Always-quiet matches **0/5** useful-tip labels. The result gives a
concrete purpose compared with the previous all-quiet courtesy cohort, but
establishes **no advantage over the strong local rule**. Directions is an
isolated candidate and runnable book example. No Directions UI, player-facing
remote selection, default model behavior or ambient rollout is enabled.

## What was fixed before calls

Independent labels, split and exact product request bytes were frozen before the
first candidate call. The label manifest is
[`guard-guide-labels-v1.jsonl`](../../psionic/fixtures/guard-guide-labels-v1.jsonl),
SHA-256 `fb63e7861e32e6c42178f9405ac7bcf96d845df306cb27fbc44dfea0a426efee`.
The [request inventory](../../psionic/fixtures/guard-guide-requests-v1/SHA256SUMS)
contains five unique contexts; unrelated weather/day combinations were not
counted as additional evidence. The source writer was freshly emitted and all
five files byte-compared before calls. No prompt, policy, labels or split were
changed between development and held-out calls.

The observation requires explicit authored Directions intent, an eligible calm
interaction, a current unmet goal and a known available route. It contains no
speech, saved quest history, account identity or precise coordinates. A separate
explicit find-work request is required for case 0; an empty/all-complete quest
tracker cannot invent it. Eight additional local cases refuse requests; they
were not sent to the model or counted as remote failures.

The original pinned tokenizer and the actual backend decoder/renderer/preparer
accepted every frozen request at **237/237/237/236/238 tokens**, prefixes
107/107/107/106/108 and suffix 130. All fit the measured resident capacity 512;
no model was opened for this offline preflight. Its
[output](data/guard-directions-2026-10-30/tokenizer-preflight.txt) retains hashes
and token counts.

## Actual API observations

Calls ran development 0/1/4, then held-out 2/3, on 2026-10-31 UTC
(2026-10-30 Chicago), after the [Elephant warm-fleet rollout](fleet-warm-rollout-2026-10-30.md).
These measurements used `oa-mesh-elephant-m0warm-20261031a`, the first warm
revision. The rollout record separately names its subsequent Nix packaging
correction; no candidate prompt or model arithmetic was changed.
They used the same single owned demo identity as the rollout, normal quotas,
one active caller and no retries or identity rotation. Each call used a fresh
headless, muted browser profile at the real TLS origin, normal NIP-98 login and
an HttpOnly/Secure session. The decision request explicitly supplied no
Authorization header. Sessions were logged out and browser profiles removed.

| Case | Split | Raw choice | Confidence | Browser fetch ms | Reported backend ms |
| --- | --- | --- | --- | --- | --- |
| 0 | Development | quest_board | 0.815147824 | 600 | 225 |
| 1 | Development | east_gate | 0.97789745 | 625 | 224 |
| 4 | Development | quest_board | 0.833107167 | 652 | 225 |
| 2 | Held out | north_trail | 0.985995037 | 625 | 224 |
| 3 | Held out | west_gate | 0.963086845 | 611 | 223 |

All five were HTTP 200 with no transport fault and the same exact Elephant
checkpoint/provider as the qualified deployment. The returned `latency_ms` is a
backend-reported field; browser timing uses `performance.now()` from fetch start
through bounded body completion. Login, browser startup and simulation-frame
queue time are outside that browser measurement. It must not be substituted for
CLI attempt time or actual frame-admission latency.

[Individual captures and ordered JSONL](data/guard-directions-2026-10-30/) retain
synthetic state results and public service metadata, never keys, signatures,
auth events, headers, cookies or raw DevTools traffic. The
[Coax summary](data/guard-directions-2026-10-30/summary.json) checks each record's
request SHA against that exact frozen case. It reports raw correctness, quiet,
wrong nonquiet choice, contract validity, timeliness, semantic legality,
admission, effective quiet fallback, missing cases and each split separately.
All five were useful without fallback; the perfect local rule remains separate.

The [separate actual-page measurement](data/guard-browser-warm-2026-10-30/README.md)
ran the existing ambient-v1 shadow consumer against that first warm fleet image.
Its byte-identical request took 671 ms on the network and 762 ms through actual
shadow admission, versus 5714/5724 ms in the earlier pilot. Pending blur canceled
fetch at 150 ms with no callback or admission through 7.5 seconds. The record
names both Wasm artifacts and does not claim an identical-binary experiment or
upstream GPU cancellation. It qualifies the existing consumer's changed service
environment, not a new Directions page integration.

Here “admitted” is the evaluator's stateless contract/time/context result.
There is no new live Directions game-frame consumer. The existing ambient-v1 book/labels
and earlier dated verification records remain unchanged.

## Changed-code qualification

Checks were written before implementation. New policy roots were mutated with
`--root-only`; imported unchanged model, crypto, tokenizer and prior lifecycle
roots were not mutated again. No whole-model CPU inference or full release gate
was run.

| New root / path | Focused result |
| --- | --- |
| `guard-guide.coax` | 39 policy checks; 280/280 mutants killed; 45 native checks in debug/release with ASan/UBSan |
| `guard-guide-eval.coax` | 299/299 mutants killed |
| `guard-guide-eval-wire.coax` | 321/321 mutants killed; frozen request/hash, report and framing checks |
| Evaluation native oracle | 126 independent C cases each in debug, release and ASan/UBSan |
| Token-capacity policy | 5/5 mutants killed; five original-tokenizer preparations fit 512 |
| Cookie probe policy | 62/62 mutants killed; native tool refuses missing live opt-in and malformed request before signing/network |
| Book example | Debug/release transcripts pass; exact source listing and local links checked |

The Coax-only [cookie probe](../../psionic/tests/guard-live/cookie-probe.coax)
keeps capture/session/request bounds in its separately mutated policy. Browser
JavaScript only executes a supplied bounded HTTP descriptor. Its generic outer
request check is not frozen-case admission: the replay evaluator enforces the
case/hash binding. Independent review checked CDP node lifetime, session flags,
bounded UTF-8 body capture and credential exclusion before results were claimed.
The replay binary SHA-256 was
`956ada8cab42684d0007088fbe74acbff5b80f04a2658e9956abbfeb1792ff1a`.

Tool-cache inspection found that imports outside `coax/lib` could leave a stale
executable. `coax/web/tool.sh` now also watches Coax sources under coax, Verse,
Psionic, apps and proto while preserving native/compiler/external-entry checks.
An unchanged external entry observed transitive-only edits in proto, coax/web
and Verse as **3 → 4 → 13 → 112**. No model call was used for that smoke.
[Source hashes](data/guard-directions-2026-10-30/SOURCES.sha256) identify the new
candidate, evaluation, preflight, capture, example and cache code.

## What comes next

[Chapter 31](../../psionic/book/ch031-directions-on-request.md) composes semantic
admission with existing Gd lifecycle tickets using synthetic responses. A real
page integration still needs an explicit Directions interaction and a
current-frame observation. Every new intent, actor generation, objective,
world/route revision and guide-tip/cooldown change must fence pending work,
including cancel/re-request for the same goal. Edited-world reachability remains
unqualified; the first integration should refuse those routes conservatively.

The sensible player-facing reference for this finite task is immediate local
directions. A later opt-in remote shadow consumer can qualify real frame timing
and cancellation without delaying that reference. Showing remote-selected tips
needs a reason beyond reproducing the local rule: a richer independently labeled
task and comparison with a strong local baseline. Availability under faults,
independent providers, remote cancellation, calibration and player benefit
remain open. Belt remains outside this pilot.

The subsequent Nix-compliant fleet image was separately qualified and promoted
to 100%; its fresh public CLI median was 736 ms across three paired requests.
That deployment does not turn the earlier API-only directions captures into
new-image measurements. All task model calls are complete, the owned demo key
was deleted, and no local authenticated proxy or browser profile remains.

## Compatibility after rebasing main

The rebase retained main's native-header composition change `789fb4909a`.
A default compiler build began its bootstrap mutation gates; that attempt was
stopped before publication and its results are not counted here. For the narrow
header check, a temporary C++ host was built from the archived current Coax
sources and committed bootstrap seeds. The installed compiler was left intact.
These plain-Coax modules need no protobuf importer.

Fresh GG and evaluation-wire headers compiled as C11 in both include orders,
with a repeated include and no aliases. The
[probe](data/guard-directions-2026-10-30/header-compose.c) uses `-Werror` and
`-fsyntax-only`. The existing native evaluation driver then passed its 126
independent cases in debug, release and combined ASan/UBSan using fresh emitted
C/headers. This extra run was justified by the changed emitter dependency;
mutation suites were not repeated for it. The new example's complete release C
was byte-identical before/after, so its passing transcripts were retained.
[Compiler/emitter identities and results](data/guard-directions-2026-10-30/post-rebase-header-check.json)
and the [native output](data/guard-directions-2026-10-30/header-native-check.txt)
record that compatibility scope.

The staged tool catalog passed before rebase (894 files, no new tool missing a
reason). After rebase the complete catalog reports five unchanged upstream
files without reason annotations: `coax/tests/semantic_profiles.py`,
`coax/web/api/makers.sh`, `test_makers.py`, `test_makers_tips.py`, and
`coax/web/card.js`. Their diff against `origin/main` is empty. The
[post-rebase lint output](data/guard-directions-2026-10-30/post-rebase-tools-check.txt)
is retained as a failing broader check; this task adds no exception or port-list
entry for them. The changed policy/native/header checks passed as recorded above.
