# Chapter 5 tokenizer data and owned references

`tokenizer.json.gz` contains the byte-exact original tokenizer shared by the
pinned Base Labs Elephant and Mammoth revisions. Its uncompressed SHA-256 is
`a5cd9732badce41de57e6efce8302930ded1c1188c5f81feb2bd6c24c4a1941f`.
The two originals were read separately and had identical bytes. No weights or
executable reference runtime are included.

[manifest.json](manifest.json) pins deterministic table identities and counts.
[provenance.json](provenance.json) records the model/runtime revisions and hashes
of the small source texts read to derive the contract. The default template is
OneJev `81ce62f1597c91e46767d4d02d6ac2e18534fe94` plus the model's `serve.patch`.
The retained model card, tokenizer config and chat template under `notices/`
are inert reference text and are never executed.

| File | Purpose |
| --- | --- |
| `qwen35.pst` | Original vocabulary/merge/added-token data plus Unicode 17 NFC/property lookup indexes. |
| `tokenizer.json.gz` | Original tokenizer data, retained compressed; unpacking reproduces the pinned bytes. |
| `pins.coax` | Original and packed-table SHA-256 pins used by the Coax library. |
| `slots.coax` | The pinned renderer's 256 single-token answer labels; choice requests admit 1–255 options. |
| `toy.pst`, `toy.coax` | Small owned teaching vocabulary with overlapping merges, added tokens and a Unicode subset. Its IDs are unrelated to Qwen IDs. |
| `goldens.json.gz` | Owned independent token, rendered-request, prefix-candidate and slot-ID vectors. |
| `goldens.coax`, `render-goldens.coax` | Evaluator fixtures generated from the independent reference, without candidate output. |
| `normalization.bin` | 26,506 standard-library NFC vectors; length-prefixed UTF-8 input/output pairs. |

The `PST5` little-endian lookup format begins with seven u32 counts, following
its four-byte magic. A 256-u32 byte-to-token-ID map follows. Sections contain
vocabulary `(offset,length)` rows, merge `(left,right,rank,result)` rows,
property `(first,last,flags,ccc)` intervals, fully decomposed
`(codepoint,length,a,b,c,d)` rows, canonical `(left,right,result)` compositions,
added-token IDs sorted by decreasing literal length, and a byte pool.
Row widths are 8/16/16/24/12/4/1 bytes. Counts and source identities are checked
before use. The packed index is reproducible tooling output; a runtime host
loads it as immutable data and admits its exact SHA-256 once.

```shell
python3 -I psionic/tests/text_fixtures.py
python3 -I psionic/tests/text_oracle.py
```

Authoring requires Python's Unicode database version 17.0.0 (the tested
Python is 3.15.1). Both commands verify committed inputs by default. `--write`
is intentional fixture authoring; it requires reviewing changed pins, contracts
and expected values. The oracle uses standard-library `re`, `unicodedata` and
priority-queue BPE over linked symbols; it never reads candidate results or runs downloaded
scripts. Its source and evidence scope are described in the chapter.

The tokenizer is from Base Labs' MIT-labeled model repository, based on
Qwen3.5 (Apache-2.0). Attribution: Base Labs / Baseten and the Qwen team.
[The retained model card](notices/README.md.txt) supplies that metadata;
[Apache-2.0](notices/APACHE-2.0.txt) and the
[Unicode data license](notices/UNICODE-LICENSE.txt) are retained. Source links
and original hashes preserve provenance. This directory redistributes tokenizer
data and owned fixtures, without model weights or converted checkpoints.
