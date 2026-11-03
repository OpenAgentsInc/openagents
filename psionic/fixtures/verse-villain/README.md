# Veyr: original-model villain-dialogue experiments

Real experiments, 2026-11-02, following chapter 255. These files contain
generated fictional dialogue and model reasoning as inert observations.
They are not game instructions or changes to a running realm.

The practical prototype is **Veyr v2 on `claude-opus-5-5`, low effort**,
with a grounded character card, four authored style examples and explicit
encounter state. The selection was made on six development scenes before
opening eight held scenes. It is an internal writing preference, not an
independent human playtest, calibrated quality score or fine-tuned model.
The stronger provider also receives a different prompt rendering, so this
is not a controlled estimate of model size alone.

Use [the character instructions](../../../verse/characters/README.md) for a
fresh dialogue command and the exported NPC brief. The
[offline example](../../examples/veyr-dialogue.coax) prints saved real held
replies without calling a model. The
[book](../../book/ch255-edit-an-original-language-model.md) explains the
result and failures; the
[verification record](../../../docs/verification/verse-villain-2026-11-02.md)
records execution and limits.

## Conditions and artifacts

All native trials use the admitted original GPT-OSS-20B checkpoint at
`6cee5e81ee83917806bbde320786a8fb61efebee`, Psionic's existing
`gpt-oss-metal-f32-ff-v1` profile. Original shard files stay read-only;
edits replace only a private idle GPU copy. No model download, upstream
script, full-original CPU inference, service or live NPC activation occurs.

| ID | Native condition |
| --- | --- |
| 0 | Plain NPC brief, greedy |
| 1 | Generic dark-wizard instruction, greedy |
| 2 | Authored voice card, greedy |
| 3 / 4 / 5 | Plain brief + final-MLP expert bias edit, α .25 / .5 / 1 |
| 6 | Constrained encoded random control, count/norm matched to 5 |
| 7 | Voice card + α .5 edit |
| 8 | Voice card, sampling .8 / top-k 40 / top-p .95 |
| 11 / 12 | Original card + four examples, greedy / same sampler |
| 15 | Release v2 card + examples, same sampler |
| 16 / 17 | Release v2 card + examples, natural analysis then final, greedy / same sampler |

Sampling seeds are `255+case`. Ordinary conditions force the final channel
and permit 160 emitted tokens; 16/17 permit 384 total analysis/final tokens.
These are decoding/compute comparisons, not weight-only gains. Four neutral
controls omit all persona instructions but still receive each candidate's
weights. IDs 9 (two physical no-ops) and 10 (strong edit then exact restore)
are explicit additional controls, not complete matrix arms.

* [protocol.md](protocol.md) preserves the initial plan and all development
  amendments. The first local pilot was weak; stronger demonstrations and
  two reference-prompt revisions were tried only on development scenes.
* [selection.json](selection.json) binds the frozen
  [development review](review-development.json). `held_opened:false` means
  the state **when selection was saved**, not that held results remain absent.
  [Measurement notes](measurement-notes.md) disclose its inconsistent
  `best_weight_only` label without revising the frozen data or choice.
* [matrix.csv](matrix.csv) gives every split's completion, word limits,
  tokens, repetition detector, exact neutral controls and generation time.
  Native emitted IDs and provider output including thinking are distinct
  token accounting bases. A finished reply is not automatically good prose.
* `runs/`: all 255 native sequences: 14 conditions × 18 cases + 3 controls.
  Prefix/generated IDs, raw text, decoder status, parsed final reply, budget,
  overlay identity, sampler condition and timing are retained. Early records
  predate explicit `budget`/`raw_status`: defaults are 160/0 respectively.
* `external-initial/`, `external-second/`, `external/`: six initial, six
  revised, then eighteen final writer calls. Exact JSON text prompts, raw
  provider responses/usage and per-call wall times preserve the failures.
* `judge-local.*`, `judge-weight.*`: two additional text-only critic calls.
  Each compares eight unseen native pairs, alternating A/B position, with
  identity hidden. The critic never judges its own writing. One model critic
  is corroborating subjective evidence, not independent human validation.
* `fresh-question.txt`, `fresh-response.json`, `fresh.out`: one new question
  outside the matrix demonstrates the opt-in fresh-dialogue CLI.
* `original-bias.bf16`, `original-norm.bf16`, `capture/`, `direction.f64`,
  `candidates/`, `equivalence/`, `parity-*.f32`: original slices, calibration,
  complete replacement bytes/digests and independent numerical observations.
  This contains small checkpoint excerpts, not a distributable full model.
* `builds/`: compiler/native/generated-code hashes and exact historical Coax
  snapshots. Native binary/C hashes are observations; old binaries are not
  committed. Historical source preimages are retained and checked. The final
  natural-channel build is shared by every held phase. The authored fallback
  greeting changed afterward; it is never passed to experimental model prompts.
* `live/`, `checks/`, `*.command.json`: bounded phase logs, failure logs,
  offline verification receipts and successful command timers. No passing
  unchanged original-window mutation gate was rerun.

## Reproduce without a model

```sh
nice -n 10 python3 psionic/tests/verse_villain_oracle.py --complete
nice -n 10 coax/bin/coax run psionic/examples/veyr-dialogue.coax
```

The independent oracle reconstructs the mean direction, exact BF16 candidate
bytes/identities/counts/norms, fixed-prefix routed residual and RMS change,
retained reference parity, tokenizer/framing, diagnostics, no-op/restoration,
provider usage, blind-pair identities and CSV. Subjective quality remains a
review judgment.

For a **new live capture**, use a disposable checkout, move this committed
fixture directory aside and make an empty directory at the same path with
the protocol preserved. A new checkout already has saved observations and
refuses to overwrite them. Build `check-villain.coax build`; run `pilot`,
`calibrate`, then `villain-prepare.coax` once, then `dev3`, `dev6`, `dev16`.
Preserve/review development replies, write a new explicit selection, then
`held0`, `held3`, `held6`, `held11`, `held16`. Each run takes the admitted
original checkpoint directory as its third argument. Keep every phase serial,
at nice 10, below fifteen minutes; the wrapper bounds it at 840 seconds and
the driver checks 790 seconds between sequences. Writing-model `dev`,
`controls`, `held` commands make new external calls and save new observations;
earlier writer revisions require their historical snapshots. Offline checks
do not trigger any external model call.
