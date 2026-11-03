# Parakeet dictation: repository vocabulary and code-aware text, 2026-11-03

GitHub #12670 (part of #12614, proposal phase D3). This record measures two
opt-in additions to local Parakeet dictation:

- A bounded, prioritized vocabulary that Parakeet's own transducer rescores.
- Fixed text rules for spoken symbols, casing and filler words.

The baseline is the #12665 record
([dictation-eval-2026-11-02](../dictation-eval-2026-11-02/README.md)). All
runs used recorded audio on the RTX 4080 through the real `openagents
dictate` path. No microphone was opened, no sound was played and no one's
voice was recorded. The new test speech is synthetic (espeak-ng) and is scored
apart from human speech.

## Summary

- **Gate first.** Term-free word error rate may rise by at most 0.3
  percentage points with the vocabulary. Measured: 2.79% without and 2.79%
  with, on 73 LibriSpeech test-clean and test-other speakers (1,543 words).
  CER is 0.99% both ways. No term-free clip changed.
- **Term recall, held-out synthetic sentences** (written for this record, not
  seen while tuning sounds_like): 13 of 42 terms before, **23 of 42** after.
  WER went from 20.16% to 15.32% and CER from 7.95% to 6.43%.
- **Term recall, #12665 code-term clips:** 59 of 80 before, **63 of 80**
  after. WER went from 27.64% to 26.09%. The sounds_like spellings were
  written after the #12665 transcripts were public, so this row is optimistic.
- **False replacements: 0.** The 40 held-out distractor clips use look-alike
  ordinary words: "cooks", "ether", "snore", "nostril", "Jason", "breeze",
  "pylon", "tap root", "psychic", "parakeets". None changed, and none of the
  73 LibriSpeech clips changed.
- **Raw text kept.** For all 193 dictations, the text before rescoring is
  byte-identical to the run without a vocabulary.
- **Chapter 153 parity with rescoring off:** 29/29 NeMo clips pass on the
  GPU path with the changed library
  ([raw/parity-comparison.json](raw/parity-comparison.json)).
- **Text rules** (`--format code`): `apps/shared/dictation-text.coax`,
  `#authority`, 582/582 mutants killed. Pushed in 644b09e486.
- **Vocabulary rules:** `psionic/lib/parakeet/vocab.coax`, `#authority`,
  675/675 mutants killed.

## What runs

```text
greedy TDT decode (unchanged) ──► tokens + emission frames ──► raw text (kept)
        │
        ▼ only with a vocabulary
vocab.coax: words of the tokens ─► spans of 1..3 adjacent words, no digit
            sound key of each span vs each term's spelling and sounds_like
            (edit distance <= a third of the term's key); not if the span is
            already the term's letters in any case; best 32 by priority
        │
        ▼
rescore.coax: for each span, the transducer's forward score (every
            alignment of tokens, blanks and durations) of the recognized
            tokens and of the term's tokens, over the same frames, from the
            span's first emission frame to the next token's, plus that next
            token's probability; on the GPU (the decoder's own kernels and a
            read of the joint's outputs) or the exact CPU rows
        │
        ▼
vocab.coax: replace when term score >= recognized score - PKV_BIAS (4 nats);
            chosen spans never overlap, largest gain first
        │
        ▼
rescored text ──► (optional) dictation-text.coax rules ──► the draft
```

- **Limits.** 64 terms (the first in the file win, later ones are counted as
  dropped), 4 sounds_like spellings per term, 48 bytes per term, 24 tokens
  per term, 32 candidates per model call, spans of at most 48 encoded frames
  (3.84 s) and 6,000 joint evaluations per call.
- **Terms.** A term never holds a digit, and a span with a digit is never a
  candidate, so numbers are never made or replaced.
- **Spelling.** A term is spelled with the pinned tokenizer's byte-pair
  merges, whose rank equals the merged piece's id (all 13,476 merges are in
  id order). A term needs pieces for every character, so `cd_submit` (no `_`
  piece) is refused; the text rules' "snake case" covers identifiers.
- **Refusals.** A malformed vocabulary is refused whole, with the offending
  line named. If the device fails or rescoring is refused, the call keeps
  the recognized text.

The vocabulary file format is one term per line, highest priority first:
`TERM<TAB>SOUNDS|LIKE`. This repository's list is
[apps/shared/dictation-terms.tsv](../../../apps/shared/dictation-terms.tsv)
(22 product and code names). User terms go first in the same file, or in
their own file.

Interfaces:

- `openagents dictate --terms FILE [--format code]`. JSON adds `raw_text`
  (the model's words before rescoring and rules) and `terms`.
- `psionic_parakeet_vocabulary`, `psionic_parakeet_raw_text` and
  `psionic_parakeet_vocabulary_info` in `psionic-parakeet.h`. The
  `psionic_parakeet_info` struct is unchanged, so existing callers (Coder for
  Mac) keep their ABI.

## Results

`dictate-score.coax vocab OFF ON`. The same `gen` step (each clip held from
300 ms before speech to 200 ms after) ran once without a vocabulary and once
with `apps/shared/dictation-terms.tsv`. The manifest is
[corpus/corpus.tsv](corpus/corpus.tsv): the #12665 rows unchanged, plus 80
held-out synthetic clips ([corpus/vocab-script.tsv](corpus/vocab-script.tsv),
20 term sentences and 20 distractor sentences, in two voices each).

| Set | Clips | Words | WER % off | WER % on | CER % off | CER % on | Terms | Right off | Right on | Clips changed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LibriSpeech test-clean + test-other speakers (no terms) | 73 | 1543 | 2.79 | 2.79 | 0.99 | 0.99 | - | - | - | 0 |
| held-out distractors (synthetic, no terms) | 40 | 280 | 7.50 | 7.50 | 3.76 | 3.76 | - | - | - | 0 |
| #12665 code-term clips (synthetic) | 40 | 322 | 27.64 | 26.09 | 20.36 | 19.78 | 80 | 59 | 63 | 5 |
| held-out term clips (synthetic) | 40 | 248 | 20.16 | 15.32 | 7.95 | 6.43 | 42 | 13 | 23 | 10 |

What changed, including words the model got wrong in both runs
([raw/results.md](raw/results.md)):

```text
OpenAgen's dictate prints ...          -> OpenAgents dictate prints ...
Noster events are signed ...           -> Nostr events are signed ...
Kubernetes, JSON and WebDPU ...        -> Kubernetes, JSON and WebGPU ...
... asked the eron to revill ...       -> ... asked Zeron to revill ...
... and replace on origin main.        -> ... and rebase on origin main.
The Cooks book explains ...            -> The Coax book explains ...
Arakeet listens on the laptop.         -> Parakeet listens on the laptop.
Xiron opened a pull request ...        -> Zeron opened a pull request ...
Pass the gase on before you store it.  -> Pass the JSON before you store it.
Draw it with Web DPU in the browser.   -> Draw it with WebGPU in the browser.
Open Agents is the name ...            -> OpenAgents is the name ...
Sionic and Cloak's ship together.      -> Sionic and Coax ship together.
```

These were still wrong with the vocabulary:

- "Sidonic" and "Sionic" for Psionic. The transducer scores Psionic's tokens
  more than 4 nats lower.
- "CUBA", "QDACAR's" and "Etherist".
- "corks"/"coats"/"torques" for Coax in "run coax mutate". The span covers
  "Run coats" or "coats mutate" only partly.

### Tuning (after the gate was in place)

| Bias | Case-only changes | Held-out terms | #12665 terms | Term-free WER | False replacements |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2 nats | allowed | 18 / 42 | 62 / 80 | 2.79 (+0.00) | 0 |
| 4 nats | allowed | 24 / 42 | 63 / 80 | 2.79 (+0.00) | 3 clips: "pylon" to "Pylon" twice, "parakeet" to "Parakeet" |
| **4 nats (shipped)** | **refused** | **23 / 42** | **63 / 80** | **2.79 (+0.00)** | **0** |

A span that already has the term's letters in another case is never a
candidate. That keeps "the parakeet" (the bird) and "the pylon" (the tower)
as ordinary words. The cost: "parakeet" in "runs parakeet on the GPU" is no
longer capitalized. The raw records for every row are in [raw/](raw/).

### Time

| Stop to final text (ms) | n | p50 | p99 |
| --- | ---: | ---: | ---: |
| without a vocabulary | 193 | 114 | 533 |
| with the vocabulary, 4 nats, no case rule (load about 50) | 193 | 113 | 533 |
| with the vocabulary, shipped (load 55-65) | 193 | 148 | 746 |

The machine was shared, with a load average of 50 to 90. The two 4-nat runs
differ only in the case rule, which removes candidates, so most of the
difference in time is load.

Each joint evaluation launches the joint and head kernels, then reads 8,198
logits back. A call spends at most 6,000 evaluations. Per-evaluation time was
not measured separately.

The #12665 gate on one-call stop-to-text p99 (600 ms) was not run with a
vocabulary. Batching each span's reads into one transfer is the next cost to
cut.

## Checks

- `coax test psionic/lib/parakeet/vocab.coax --library`: 13/13 checks.
- `coax mutate ... --library --root-only`: **675/675 killed**. The check
  helpers are in `psionic/tests/parakeet-vocab-checks.coax`.
- `coax test psionic/lib/parakeet/rescore.coax --library`: 166/166 checks,
  including the files it loads. The forward sum equals an explicit
  enumeration of every alignment, with and without a next token, with no
  labels, and on a one-frame span.
- `apps/shared/dictation-text.coax`: 14/14 checks; 582/582 mutants killed.
- `apps/openagents-cli/dictate-rules.coax`: 260/260 mutants killed.
- `dictate-check`: the fixture cases, plus `terms-gpu` and `terms-bad` with
  the model: 29 passed; the three GPU cases first hit CUDA out of memory while
  another process held the GPU, then passed alone (3/3). See
  [raw/dictate-check.txt](raw/dictate-check.txt).
- Chapter 153: `parakeet-runtime-gpu` and `parakeet_reference_compare.py`
  report 29/29 passed with the rebuilt library and no vocabulary.

## Text rules (`--format code`)

`apps/shared/dictation-text.coax` (`#authority`) runs only when asked.

**What it does:**

- **Spoken symbols:** "dot", "underscore", "slash", "dash" and "hyphen" join
  the words on both sides. "colon" attaches to the word before it. "open
  paren", "open bracket" and "open brace" attach to the word after them;
  the "close" forms attach to the word before. "new line" becomes a line
  feed.
- **Casing:** "snake case", "camel case", "pascal case", "kebab case" and
  "all caps" turn the next one to six plain words into one identifier. The
  identifier stops at a stop word, at another rule, or after a word with
  punctuation.
- **Fillers:** "um", "uh", "erm", "hmm" and their variants are dropped. A
  dropped capital passes to the next word, and a dropped sentence end stays.

**What it never does:**

- Change, join or case a word with a digit.
- Treat "send", "enter", "submit", "run" or "execute" as anything but words.
- Output a control byte other than the line feed of "new line". The CLI
  prints it inside the text; Coder's composer inserts it as text, never as
  Enter.

**When it refuses** (invalid UTF-8, control bytes, more than 8,000 bytes, or
nothing left after fillers), the raw transcript is the draft. The JSON
result always carries `raw_text`.

**Languages.** The rule words and the sound keys are English. Other
languages pass through the text rules unchanged, since no rule word matches
and UTF-8 is kept. The vocabulary's sound keys use ASCII letters only. The
model detects its language and rescoring runs on whatever it produced, but
it was measured only on English.

## Not done here

- **Coder for Mac.** It does not call the vocabulary or text-rule entry
  points yet. Rescoring on the Mac's CPU decoder path works but is slow and
  unmeasured. See NEEDS_OWNER.md.
- **Representative human speech with code terms.** The term rows are
  synthetic. The owner's own dictations are the missing measurement.
- **A repository miner** (identifiers near the caret, recent commits). The
  repository list here is curated by hand.

## Reproduce

```shell
apps/openagents-cli/build.sh
# held-out audio: for each line of corpus/vocab-script.tsv and voice en-us, en-gb-scotland
#   espeak-ng -v VOICE -w /tmp/x.wav "TEXT"          (writes a file, plays nothing)
#   ffmpeg -i /tmp/x.wav -ar 16000 -ac 1 -c:a pcm_s16le ~/.cache/openagents/corpus/dictation-12665/vocab-12670/ID-VOICE.wav
# (corpus.tsv pins every file's SHA-256)
export OPENAGENTS_DICTATION_MANIFEST=docs/verification/dictation-vocab-2026-11-03/corpus/corpus.tsv
coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax run /tmp/voff gen
OPENAGENTS_DICTATION_TERMS=apps/shared/dictation-terms.tsv \
  coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax run /tmp/von gen
coax/web/tool.sh apps/openagents-cli/tests/dictate-score.coax vocab /tmp/voff /tmp/von
```

Machine and model: coderos-4080, as in the #12665 record. Coax compiler:
`~/.cache/coax-release/727901f6…/coax` (coax0 0.48.0), a current
development build.
