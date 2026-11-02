# Aethyr book handoff

Historical stopping point: the user explicitly resumed this work on
2026-11-02. The [implementation record](aethyr-book-implementation-2026-11-01.json)
tracks the active reconciliation, tooling and playable Grid work. The status
and outstanding work below describe the earlier stop and remain evidence of it.

The user requested a stopping point after the agents finished their current
issues. Work is stopped at chapter 11. Do not resume automatically. The entire
book is **not complete**; chapter 12 has not been started.

## Checkout and publication

- Continue from `/Users/christopherdavid/work/openagents-aethyr-book-main`.
  Its integration commit is `7c9289b4db25d2df527f6446debadaffe364e7ad`, based on
  `origin/main` at `39fa38023775f123173ba6dc3cab8173a311b597`. A subsequent local
  handoff commit contains the final records and this document; resolve
  `refs/codex/aethyr-book-handoff-2026-11-01` for its exact hash.
- This chapter's integration and handoff commits are **unpublished**. Do not
  describe chapter 11 as completed or push it as a completed chapter.
- The old worktree, `/Users/christopherdavid/work/openagents-aethyr-book`,
  remains intact at `08729561f9`, with its original dirty tree and index.
  Treat it as read-only historical evidence. The owner's original
  `/Users/christopherdavid/work/openagents` checkout was not changed.
- The complete old tree was captured atomically as commit
  `9df7d74b3638662b413b4a3d52e57de92ff53df1`, tree
  `a604a45ef0d33f723562d5cdeb3ba7095afa1ebb`, parent
  `4aa69fe6fa0c7619cf123f182558784cb29dee65`: 2,682 files,
  199,466,196 bytes, verified before and after staging through a private index.
  That common-base delta was merged into fresh main; no whole-tree copy or
  resurrection of upstream deletions was used. The manifest is retained in
  [main-integration-snapshot](data/aethyr-ch11-2026-11-01/main-integration-snapshot/).
- Already published dependencies include `8008dcc038` (authority compilation
  uses the existing check storage budgets) and `993dece6b4` (stable backing for
  the existing Beacon borrowed-string test fixture). Do not reapply them.

The merged book has **13 implemented, 5 partly implemented and 68 draft
chapters**. Chapter 11 is now honestly partly implemented, with its runnable
example registered across the four status surfaces. Upstream chapter 15,
book prerequisites, 512-row capacities, font aliases, optional supersampling,
and compiler/installer changes were preserved.

## Completed and retained

The central record is
[aethyr-book-implementation-2026-11-01.json](aethyr-book-implementation-2026-11-01.json).
Its older entries are historical epochs, not assertions about every current
source byte. The [authority index](data/aethyr-ch11-2026-11-01/current-authority-gates/index.json)
explicitly distinguishes exact inventories, historical dependency baselines and
log-only evidence. Do not upgrade those labels without evidence.

### Grid allocation

The original default Grid build succeeded, but the actual browser could not
allocate its arena inside 64 MiB. A call-graph analysis of its optimized Wasm
bounds linear stack use below 2 MiB. The Grid-only stack reservation changed
from 8 MiB to 4 MiB, preserving 400,000 vertices, package capacities and the
64 MiB memory limit.

The 320-row producer `ec39a47f...` then passed actual memory growth and both
full numerical comparisons at 320×180: 230,400 RGBA channels each, zero
failures, exclusions or nonfinite values; maximum normalized errors
0.0009746588694 (WebGPU) and 0.00146484375 (Vulkan). See the
[stack/allocation record](data/aethyr-ch11-2026-11-01/grid-memory-stack/qualification.json)
and its `link-fix`, `current-browser`, and linked Sans comparison archives.
The newer merged 512-row qualification is recorded separately below.

### Ice Cream Man doorway geometry

The physical repair is frozen at mesh SHA256
`bf84af4711650e3f8db1a72e2177f4cad222165d1244703ce3de2b56df386d46`.
It clips/culls only wall caps certified as covered by the actual emitted doorway
post solid. Exposed overhangs, incomplete donors, counts, packed normals,
materials and the other models are preserved. The earlier wall-band repair and
its failing runtime epoch remain separately retained.

All 31 Coax checks pass. Independent C++ and generated-C debug/O2/ASan+UBSan
profiles each compare 68 houses, 38 models and 134,729,811 floats; all 13 named
loss controls are caught. Short-view checks cover 11,088 cases and 15,611,904
vertex words per profile. O2/sanitized results are bit-exact; debug retains the
existing 1e-4 bound, with maximum error about 1.67e-5.

A fresh merged-source ordinary producer built in 35.307 seconds using the
explicit qualified compiler below. Wasm SHA256 is
`24f1d888c22b4d5275caeb64c2360e58865b8848b0f250fd9c4c0358c9030b2f`.
Actual browser and native frames, then both CPU raster profiles, pass all
230,400 RGBA channels each with **zero exclusions and nonfinite values** and
maximum normalized error **0.00146484375** under the unchanged 0.002 rule.
The first-frame HOWTO overlay covers much of the scene; the visual review does
not claim a clear full-room/camera survey.

The CPU executor is the explicitly historical 308/RF633 artifact. Its only
source-closure delta is the separately proven Everglade-only Geometry change,
unreachable for the Ice profile; no current-source reemission is claimed.
See [repair.json](data/aethyr-ch11-2026-11-01/icecream-doorway-caps/repair.json)
and [native runtime](data/aethyr-ch11-2026-11-01/native-renderer-progress/icecream-doorway-main-320/).

### Everglade admission

The full-size dark page was an actual admission refusal at water draw 189:
576 vertices and 120 instances were requested against a derived 118. The
producer reserves three spray slots; the desktop shader clips the two empty
slots before particle reads. Geometry now retains `max(3, ceil(64/per))` slots.
No shader, pass, tolerance or global quota changed.

The new regression checks failed before the fix. Final Geometry has 98 passing
checks and **710/710 mutants killed**, with no equivalent marks. Generated C
admits the actual 190-draw payload in debug, O2 and ASan/UBSan; excess instance,
excess index and short water payload controls refuse without partial output.
See [qualification.json](data/aethyr-ch11-2026-11-01/everglade-water-spray/qualification.json).

**A fresh full-size Wasm/runtime frame is still missing.** Both ordinary changed-
source build attempts (308 and explicitly pinned 1fac) reached their owned
810-second deadlines under heavy CPU contention. Both process groups stopped;
no successful new artifact was claimed. Logs and drivers are retained in
`ordinary-308-deadline/` and `ordinary-1fac-deadline/` under that record.
Do not repeat an unchanged passing gate or simply wait beyond the command limit.

### Dead Gears geometry and open precision issue

Canonical eight-direction rings are qualified by 8/8 mutation and 728 independent
native/WebGPU/Vulkan cases. The independent box-winding repair has 27 passing
source checks, three native profiles and five loss controls per profile; it
preserves every per-face vertex packet multiset and leaves culling disabled.
See [canonical-box36](data/aethyr-ch11-2026-11-01/canonical-box36/).

These are valid geometry repairs, **not numerical closure**. The fresh default
frame still fails three RGB channels at pixel (42,76), maximum normalized error
0.005706787109375, on both GPU comparisons. Older unobscured inputs retain their
12-channel/four-pixel failures. The different visible error counts come from
additional UI plates; camera/fade/object values did not change.

The surfaces actually intersect or share perpendicular edges. A universal
CPU-centred fixed-function depth-error band has not been justified. The
[precision research handoff](data/aethyr-ch11-2026-11-01/precision-contract-research/HANDOFF.md)
records primary sources and a possible input-only positive ambiguity certificate.
Its exploratory ideal-coverage family finds two pixels/eight channels, but model
conformance, clipping/coverage and VS premises remain unproved. **No mask,
comparator exception, candidate image policy or wider tolerance was implemented
or applied.** The exploratory count is not a result for the proposed snapped
family. Follow its independent proof/control requirements before product code.

### Images, teaching and static checks

Twenty reviewed screenshot scenarios were migrated with retained before/after
images and causal shader controls. The original screenshot allowances remain
unchanged. Both executors pass those reviewed baselines. Records live under
[image-goldens](data/aethyr-ch11-2026-11-01/image-goldens/), notably `fixture`,
`fairy`, `fixture-water-default`, `three-realms`, `belt-stair`, and `last-three`.
The last-three record separates pre-chapter-11 Hecate differences and existing
Haunted House UI text from noise/filter changes. These are recorded producer
and tool epochs, not blanket latest-main image qualification.

Orbit League separately passes its original screenshot at six changed blocks
under the unchanged six-block allowance; no PNG changed. See
[orbit-current](data/aethyr-ch11-2026-11-01/image-goldens/orbit-current/qualification.json).

The current 320-row Grid screenshot still differs from its old baseline. Raw
controls isolate both portable noise and owned Sans filtering: current shader
in old executor equals the shared renderer; restoring old noise plus old Sans
sampling reproduces the exact old raw image. No Grid PNG was changed. The
[Grid visual-control archive](data/aethyr-ch11-2026-11-01/grid-visual-controls/qualification.json)
retains the actual images, producer, shader variants, receipts and before/after
review. Its stock helper returned zero outside a failing child; the retained
child exit and execution record correctly report failure.

The merged book structure passes, including status agreement, links, listings
and example inventory. The new worked example's debug/release transcript was
qualified separately; unchanged examples were not rerun. Static UI checks pass
for 82 HTML files. `check-visuals.sh` now scans the new `render.js`; an isolated
planted visible style is rejected, while the old omitted-module scanner misses
it. These records are under
[main-integration](data/aethyr-ch11-2026-11-01/main-integration/).

## Compiler identity and operational constraints

The explicitly pinned qualified compiler is SHA256
`1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa`.
Its qualified source digest is
`b3f0e5a69265dbbbe2f7dc07ebff868b415e465658bf68c2436202260f951dc9`,
with source/provenance under
`/Users/christopherdavid/work/openagents-aethyr-check-budgets-current`.
The frozen executable used by current builds is
`/tmp/aethyr-authority-budget-qualified-1fac/coax`.

It is **not** a matching-main compiler. The merged checkout's full fingerprint
is `4478591fbbd7b6a4ea86ce4581addde5b4977ecd76ab7a9f53dbc77d71c4c614`;
its published artifact was unavailable (404). An unrelated external build of
that fingerprint was already running. It was neither signalled nor modified,
and no duplicate compiler gate was started. Supported product builds may set
`COAX` explicitly, but must record the pinned compiler's older qualified source
separately and must not relabel it as current-source provenance.

Read AGENTS.md and the relevant INVARIANTS.md before continuing. Keep Coax product
logic and zero-survivor authority checks; never add equivalent waivers. Keep
checks below 15 minutes per command (owned wrappers use 810–840 seconds). Preserve
passing evidence on unchanged inputs. All browsers must remain muted/headless;
no visible windows or sound. Do not stash/reset another agent's work, kill
unrelated compiler jobs, or overwrite upstream changes. Rebase on latest
`origin/main` before any eventual push.

## Remaining work on explicit resume

1. Reuse the final merged-512 Grid proof below on unchanged inputs; its remaining
   screenshot migration is separate from numerical acceptance.
2. Obtain a timely qualified Everglade changed-source build and actual full-size
   frame, then complete its current image/numerical gates.
3. Prove and qualify the Dead Gears precision contract, or leave the strict
   failure open. Never turn exploratory witnesses into an admitted mask by label.
4. Finish the Grid's explicit visual migration and the remaining latest-source
   host qualifications. Retained native rendering of supplied decoded media
   does not establish native HTTP/video acquisition or live reload/watch support.
5. Check every final acceptance claim and source epoch before promoting chapter
   11 to implemented. Only then push the finished chapter to main and proceed
   to chapter 12, in order, if the user resumes the original book objective.

## Final merged-512 Grid result

The merged 512-row Grid qualification **passed**. The unchanged network boundary
check measured 77,453,931 steps; only its authored limit changed from 70M to
100M, with 25% headroom rounded up. The temporary 400M calibration header was
removed. The final ordinary build passed in 80.61 seconds without source or
compiler drift. Wasm SHA256 is
`9f229a3395a989ba693826303acdaa5d36e2244f3d6d460406292023dfa9c9ba`.

Actual memory has minimum 695 and maximum 1,024 pages, heap base 45,497,264,
and a 65,415,792-byte arena requiring 999 pages. Memory growth succeeds with
400,000 vertices retained. The final 500-function/170-export call graph has a
conservative 1,956,816-byte linear stack bound below 4 MiB, no unknown stack
expressions, and matching semantic sections.

The actual browser capture passes in 1.63 seconds with clean fences and no
shader messages. Every normalized RFI row/payload and every GPU readback byte
matches the previously strict-qualified 320-row case. That exact equality
supports transparent reuse of the prior CPU/native numerical observations;
there was no new numerical/golden run or current-source RF-backend claim.
See [merged-grid-512/qualification.json](data/aethyr-ch11-2026-11-01/merged-grid-512/qualification.json)
and [check-budget/index.json](data/aethyr-ch11-2026-11-01/merged-grid-512-check-budget/index.json).

All three agents are stopped, with no owned commands or writers remaining.
External build jobs were untouched.
