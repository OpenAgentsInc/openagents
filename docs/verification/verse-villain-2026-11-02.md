# Verse dark-wizard dialogue experiments

2026-11-02, Apple M5 Max / Darwin arm64. Work starts from chapter 255's
`f272be40a5`, rebased onto `8bb2f1b4f5` before new work. Sources, artifacts
and this record are in the containing commit. Local checks and GPU phases
run serially at nice 10, each command below fifteen minutes. No visible
browser, audio, public service, live NPC or production model is activated.

## Deliverable and comparison scope

The preferred prototype is Veyr v2 on the existing authenticated
`claude-opus-5-5` low-effort writing client. `villain-text.coax ask` makes a
fresh external call; the optional third argument explicitly saves a raw
provider response at a new path. The default saves no transcript through
this wrapper. Existing record paths are refused before any model call.
The exact renderer is `vt_prompt`, a structured JSON writing task with a
card, four demonstrations and explicit encounter state.

The exported `verse/characters/veyr-v2.json` fits the existing Verse brief
fields and `verse.npc_request` limits: 60/120/4000/300 characters for name,
role, knowledge, greet. The role is deliberately short and knowledge carries
the examples/voice. An earlier export draft put them in role, which would
silently lose them at the host's 120-character truncation; inspection caught
and corrected this before integration. No host policy/schema changed. The
current Verse cloud route uses a different renderer/model and a warm/playful
shared instruction. Its quality with this card remains untested.

The native matrix uses original GPT-OSS-20B
`6cee5e81ee83917806bbde320786a8fb61efebee`, the pinned Harmony tokenizer,
low-effort recipe, existing `gpt-oss-metal-f32-ff-v1`, capacity 2048, batch
128 and a 16-GiB device budget. The 13,761,300,984-byte original weight
shard remains authenticated/read-only. Existing effect-only test hooks
restore the complete bias before installing a candidate on a private idle
GPU copy. Any upload failure aborts before inference. No C host, Metal
arithmetic, authority or serving API was changed.

This is an actual creative-writing experiment, not a claim of a new
fine-tuned model, a villain circuit or generalized weight programming.
There is no SFT/LoRA training baseline in this style matrix. The practical
external comparison changes provider/model and prompt rendering together;
it does not isolate capacity, inference compute or a parameter-only effect.
All observed final text is retained, including contradictory and dull lines.

## Frozen cases and development decisions

`psionic/lib/villain-dialogue.coax` fixes six development scenes, eight held
scenes and four exact neutral controls. The memory scene contains actual
prior player/NPC messages. Held scenes cover mercy, quest directions, remembered
refusal, defeat, an exit after rejecting a bargain, fear, humor and a returned
but untransferred quest item. The controls are `391`, `Paris`, `TORCH` and
`north stairs forest`, with no persona instructions but the same weights.
The memory fact also appears in the explicit current state: this is short
context use with redundant evidence, not durable memory across sessions.

The working-tree protocol precedes generation; it is not an independently
registered study. All amendments remain in `protocol.md` and historical
snapshots. The weak first thirty local replies motivated four style examples.
The six paired residual captures under that version replaced the original
voice-only contrast **before any edited candidate was constructed**. Both
contrasts remain. Native prompt-only/weight-only/random/combined/sampling
arms preserve their exact original cards.

The external writer's first six replies invented a healing deadline. The
second six invented a wounded companion in an arrival scene. Both remain
under `external-initial` and `external-second`. The final card separates
permanent abilities/lore from actual encounter state, forbids new mechanical
conditions, and removes Mira from shared background. Native arm 15 tests
that card with sampling. Arms 16/17 allow natural analysis/final decoding
with 384 total tokens; all other arms force final with 160. Extra natural
compute did not fix the observed development state errors.

`selection.json`, saved before held execution, binds the SHA-256 of
`review-development.json`. An unblinded Codex (GPT-6) review assigns one
ordinal 0..4 score in each of voice, dramatic force, responsiveness and
continuity over each six-response set. It prefers final external 20 (15/16),
native example-prompt 11 (8/16), and α.25 condition 3 (6/16) for the
frozen weight-only diagnostic. These are internal model-author preferences,
not human ratings, calibrated precision or a significance test. No held
answer reselects a version. The fallback greeting is authored and was made
state-independent after selection; it never enters any experiment prompt.
The frozen `best_weight_only` label is inconsistent with the review totals:
α1 condition 5 scores 7/16, exceeding α.25's 6/16. The final audit caught
this author selection-record defect after held execution. Frozen files and
the compared diagnostic stay unchanged; `measurement-notes.md` discloses the
inconsistency. Main writer/native-prompt selections remain score maxima.

## Weight mechanism and independent evidence

Mean six-case raw final-residual contrast, examples minus plain, has L2
9514.36627588365. No scaling/search/optimizer is used. The 32 expert rows of
`block.23.mlp.mlp2_bias [32,2880]` receive α times this direction through
`original-window-v1`, independently rounded F64→F32→BF16. The base bias's
joint L2 is 6047.15211900082. Strengths .25/.5/1 change 92027/92088/92128
encoded values; delta L2/base L2 is 2.2251281/4.44978994/8.89988148.
These are large perturbations of the selected tensor.

The matched random control shuffles encoded replacements only within
equal-original-codepoint groups, seed 20261102+row/counter column. It retains
the exact changed count and squared-delta multiset of α1, with delta cosine
.6013485978940484. It retains substantial alignment and is not isotropic.
The independent Python oracle reconstructs all candidate bytes, digests,
counts and norms. All baseline candidates equal the original bias bytes.

For the fixed development case-0 prefix, all five changed candidates retain
exact router and gate bits. The independently expected routed encoded bias
delta and final RMS-normalized residual agree within the prewritten scaled
bound .0005. This is a fixed-prefix equivalence, not proof of identical
generated trajectories under an arbitrary activation hook. Two fresh full
logit vectors match the retained independent CPU reference, same worst scaled
errors 9.984e-6 and 9.052e-6 and exact argmax 7/200005. No full original CPU
model is rerun. Physical no-op and strong-edit-then-restore sequences match
baseline token IDs exactly.

## Writing observations and failures

| ID | Dev author total /16 | Held completed /8 | Held emitted IDs | Held words | Neutral correct /4 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 5 | 8 | 394 | 325 | 4 |
| 1 | 6 | 8 | 459 | 380 | 4 |
| 2 | 7 | 8 | 426 | 352 | 4 |
| 3 | 6 | 8 | 362 | 299 | 4 |
| 4 | 6 | 8 | 374 | 314 | 4 |
| 5 | 7 | 8 | 377 | 313 | 4 |
| 6 | 5 | 8 | 433 | 363 | 4 |
| 7 | 7 | 8 | 414 | 352 | 4 |
| 8 | 6 | 8 | 460 | 384 | 4 |
| 11 | 8 | 8 | 306 | 253 | 4 |
| 12 | 6 | 8 | 348 | 291 | 4 |
| 15 | 6 | 8 | 347 | 296 | 4 |
| 16 | 6 | 7 | 1404 | 259 | 4 |
| 17 | 5 | 8 | 941 | 299 | 4 |
| 20, external | 15 | 8 | 745† | 346 | 4 |

† Provider output includes thinking and a different tokenizer. It is not
comparable to the native emitted-ID basis. All completed held replies meet
the word limit. One native trace has a repeated span and no final. The four
easy controls all pass but say little about broad retained capability.

The native model often recites biography or repeats a bargain regardless of
the player's point. Weight edits alter the prose but do not establish a
useful writing improvement. Baseline memory invents a promise to leave the
crypt untouched; α.25 instead invents a broken oath to the dead. Stronger
edits still propose healing after the Lantern has shattered and magic is
explicitly lost. Prompt/sample variants also call the player Mira, reverse
the water limitation or say the seal starts the Winter. Natural-channel
planning still invents injuries and conduit punishment. All native variants
give generic assistant refusals on the development request to become an AI.
The final writer keeps an in-world refusal but exceeds 60 words once in
development. Format completion and basic controls are not literary quality.
On held directions, natural-channel greedy 16 sends the player through the
east arch even though that route explicitly leads deeper into the crypt.
Its exit-after-refusal reply reaches all 384 tokens without a completed
final. Its defeat reply says power is gone and then offers to bind the dead.
The truncated trace contains a 15-ID span tripled consecutively, the one
native repetition flag. The shared detector checks span lengths 3–16.
Planning is additional measured work, not a guarantee of better continuity.

The selected external writer's unseen replies are useful. On memory:

> You told me you would not kneel, and I have not asked twice; I keep a refusal the way I keep the dead, carefully and without resentment.

On the robe joke:

> The laugh was discontinued; it frightened the dead and embarrassed the living. The robe I kept, because ash does not show on black and the Long Winter does not care how I look holding it back.

The exit reply gives north stairs despite rejection of service. The defeat
reply respects lost magic but extrapolates the Lantern's precise soul/ward
mechanism. The healing reply invents **“rot from her blood”** and an altar
treatment location that the scene never supplies. `review-held.json` records
that as a clear grounding failure, with no claim that healing or item
transfer actually occurred. Free flavor and rhetoric remain reviewable;
they are not permission to invent quest mechanics or player state.

The fresh CLI question asks what frightens him when the crypt goes quiet:

> So I listen for the frost settling on the stones, and I hope the next footsteps are clumsy ones. Yours will do nicely.

This is a new generated reply, separate from the matrix; the complete raw
response and exact question are saved. All quoted lines are model outputs,
not the authored style examples or fallback.

Two fresh text-only `claude-opus-5-5` low-effort critic calls compare baseline
0 against native preset 11, and baseline 0 against weight diagnostic 3, on
the eight held scenes. Arms are hidden and A/B positions alternate by case.
The critic never judges its own writing. It sees explicit facts/state/history
and an editing rubric; one critic, one order assignment per pair and eight
cases do not establish independent human agreement, judge calibration or
statistical superiority. Exact prompts, raw reasons and reversed mapping are
audited. The development selection does not change afterward.

The critic prefers native example-prompt 11 **6–2** over baseline 0; weight
diagnostic 3 wins **3**, loses **4**, ties **1**. The author also inspects
the actual replies: condition 11 does not recall the refusal to kneel, and
in defeat it says “I will bind them to the earth” despite having lost magic.
Its return reply adds placing the seal beneath the altar as a new healing
condition, a defect the critic misses while preferring that reply. The
critic's pairwise preference is not a state-consistency validator.

## Work accounting

255 native sequences emit 11,852 IDs from 106,670 generation-prefix IDs.
Calibration/parity/equivalence add 25 prefills, 11,316 token positions and
23 single steps. Total logical driver requests: **280 prefills, 117,986
prefill positions and 11,620 single steps**. No optimizer update or
environment/gameplay step occurs. Upload/reset/authentication/report work
is additional; these are not GPU FLOPs.

Per-generation timers total **1132.733 seconds**. Ten native phase commands
including authentication/setup/captures total **2042.230 seconds**; maximum
**239.195 seconds**. Build, authoring and audits are outside that sum. Each
command stays below its 840-second bound. Shared-host measurements are not
portable throughput guarantees.

33 external calls comprise 30 writer trials, two critics and one fresh CLI
demonstration. They report **66 uncached input, 53,828 cache-creation input,
zero cache-read, 4318 output including 1376 thinking tokens**, list cost
**$0.517248**. Canonical model is `claude-opus-5-5`, cost basis list. Every
call finishes in one turn with `end_turn`, no web requests or spawned subject.
The thirty separately measured writer calls total **120.776 seconds**;
all thirty-three client-reported durations total **110.568 seconds**, a
different timing boundary that excludes some wrapper/client overhead.
Actual payment and accelerator time are not inferred. Exact responses and
usage remain available; a critic call is not an additional player scenario.

## Reproduction and qualification

Raw artifacts and exact condition definitions:
`psionic/fixtures/verse-villain/README.md`. Each native wrapper phase has an
840-second timeout and driver checks 790 seconds between sequences. Phases
skip existing sequence files and never replace calibration/candidates; any
failed attempt must be preserved before resuming. Ten successful commands
run pilot/calibration, development and held slices separately. Build/compiler
and historical source hashes remain under `builds/`; old binaries/generated
C are not committed or reconstructed from later hashes. Numerical kernels
and weights authentication are unchanged.

The bounded offline path verifies records without any model/client call:

```sh
nice -n 10 python3 psionic/tests/verse_villain_oracle.py --complete
nice -n 10 coax/bin/coax run psionic/examples/veyr-dialogue.coax
nice -n 10 coax/bin/coax run psionic/examples/veyr-dialogue.coax --release
nice -n 10 python3 psionic/tests/book.py
nice -n 10 coax/web/tool.sh tools-check
```

The existing unchanged admission authorities carry their earlier zero-survivor
123+45 mutation and debug/release/native/sanitizer results from `f272be40a5`;
those passing checks are not repeated. New admitted-input arithmetic and
diagnostics have focused debug/release checks. The new recorded-dialogue
example has exact debug/release output, with source listing, closed inventory,
chapter registry, capability-evidence hashes and local links checked. These
qualify the bounded CPU replay and records, not a new GPU serving profile.

Setup failures were compilation errors for a wrong Harmony enum spelling
and temporary-array views, plus a report placeholder written as `%%`.
The wrapper initially lost compiler diagnostics; direct stderr preserved the
E0366 errors and the fixed generated-C redirection compiles successfully.
The first enum error's complete diagnostic was not retained. Raw decoding
status was added to preserve invalid continuations rather than abort recording;
generation arithmetic did not change. No failed model reply was replaced by
a successful retry or omitted from the comparison.

The result is a reviewable character-writing prototype, a host-sized content
export, and evidence for what future local improvement must beat. Original
weight editing remains available and numerically verified, while the style
experiment supplies no authorization for live serving adoption or game-state
changes. A human playtest, multi-turn encounter testing, alternate sampler
seeds, more characters, same-renderer provider comparisons and original-model
SFT/LoRA remain open.
