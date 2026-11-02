# Helper-write composition candidate — not merged

Issue [#12625](https://github.com/OpenAgentsInc/openagents/issues/12625).
The request polling fix is delivered in `31944a2a34`; this B2 compiler
candidate is **not delivered**. Main's checker, generated seed, manual fixture
expectations and book capability claims remain unchanged by this record.

The proposed implementation tracks selected helper writes into caller-owned
carriers and substitutes returned fields through actual arguments. It retains
incoming/default and all may-write origins, rejects known callee-local backing
at its source, and keeps recursive/foreign/unsupported cases incomplete. Its
last scope applies precise composition to raw return-origin queries. Retained-
store-only misses keep the existing manual helper boundary; complete raw facts
may still serve retained-class queries. Paths8, origin sets64, depth32 and the
shared 1,048,576-work bound were not increased.

## Why it stopped

Small fixtures and native checks passed, but the actual compiler input did not
fit the unchanged work budget when checked by the new checker. Three candidate
images hit `CK_resource` at work 1,048,577; the final inline-substitution
experiment still failed. No further optimization was started after that one
bounded experiment and the owner's stop instruction.

A paired debug comparator uses byte-identical retained main/qualified checker
C/header, the same native source/object tuple and the **same final compiler
input**. It completes at 1,047,492 work, leaving only 1,084. The existing qualified
normal compiler also completes the preceding same-input comparison, but its
stripped debug variables prevent observing counters; that receipt explicitly
labels the instrumentation incomplete. These are separate observations. The
new images are development stage0 links, not normal release or bootstrap
qualifications. The debugger stops at the first resource failure; the final
candidate's total required work is unknown beyond the bound.

See [record.json](record.json) for producer/input hashes and exact receipt
locations. No compiler-cap increase, caller catalogue exception, unknown-to-
empty shortcut or survivor waiver was used to make the candidate pass.

## Retained checks and their scope

- The raw-helper candidate has eight new check groups/54 graph cases plus seven
  affected existing groups. Helper-only physical-source projection mutation
  killed 220/220 mutants. The later `c206` raw/retained dispatch candidate reran
  this focused scope on its changed dependencies:220/220 killed again.
- Twenty-one independent native source/internal-query cases ran at O0/O2 and
  ASan+UBSan:63 observations. The recorded b751→6ce continuity covers426
  runtime declarations and eight load sets. It does **not** establish whole-
  runtime continuity to `c206` or the final `1757` inline candidate.
- `c206`'s new raw/class boundary adds four cases at O0/O2/ASan+UBSan:12
  observations. Two isolated forced dispatch controls both fail their oracle.
  Unsupported Apple `detect_leaks=1` and ABI2 `E0413` setup attempts remain
  visible; the sanitizer-only continuation uses `detect_leaks=0`.
- Actual generated `so_post` code passes46 buffer/default scenarios across
  three native profiles:138 observations. Its stubbed process oracle does not
  qualify real child-process lifecycle; the separate polling record does.
- A linked `c206` development compiler passes21 physical source fixtures and
  the two affected existing helper-carrier fixtures. Internal completeness
  comes from the separate origin oracle, not source acceptance alone.
- The final `1757` inline candidate passes six focused substitution/boundary
  cases and canonical single-unit C/header generation. It has no complete
  normal/compiler-release/bootstrap qualification and still fails the actual-
  compiler cost gate. Earlier `c206` successes are not relabeled as final `1757`.

The actual `so_post` return oracle distinguishes output parameter2, stderr
parameter3, static problem messages and default-empty values; foreign retained
lifetime remains incomplete. Full settings/Lab still have their independently
recorded eight-step path exhaustion. B3 backing-buffer lifetime, asynchronous
handled outcomes and the external R2 integration remain open.

## Portable handoff

[source-and-tests.patch.gz](source-and-tests.patch.gz) holds the final unmerged
source/tests/fixture proposal against its recorded base.
[proposed-current-docs.patch.gz](proposed-current-docs.patch.gz) is a **proposal for
when the feature qualifies**, not a statement about main. It must not be
applied as delivered capability documentation while the cost gate fails.
Decompress the gzip files before applying; exact original patch hashes and
roundtrip checks are in `record.json`. Generated seeds are intentionally excluded from both patches; regenerate and
qualify them from the final rebased source after resolving the cost blocker.
[patch-manifest.json](patch-manifest.json) binds all38 proposed files and the base revision.
Merge both sides of shared compiler changes with the existing R2 owner.

[candidate-manifest.json](candidate-manifest.json) maps637 first-candidate raw
text files to148 deduplicated members in `candidate-proof.tar.xz`.
[final-manifest.json](final-manifest.json) maps subsequent source snapshots,
commands, generated C/headers, failed attempts and cost comparisons into
`final-proof.tar.xz`. Each member is named `sha256/HASH`; both archives were
read back and every member hash verified. Executables/objects are hash-only
identities. Original absolute paths are observation identities, not portable
reproduction commands. [candidate-archive.json](candidate-archive.json) and
[record.json](record.json) bind archive and manifest bytes.

Resume by reducing analysis work soundly, preferably coordinating the existing
R2 analysis-cost workstream; then apply/rebase the candidate, test changed
paths, regenerate the seed and qualify the real compiler input before claiming
B2 delivery. Keep all retained failures and the original bound.
