# Unicode 18.0.0 NFKC, initial evidence

Task `unicode-nfkc` (#e34f), an explicit prerequisite of the optional NIP-49
primitives. Product logic lives in `coax/lib/text/nfkc.coax`; Python is explicit
data-generation and test tooling only. Normal builds use the committed Coax
tables and need no Python, host Unicode normalizer or data download.

The exact official Unicode 18.0.0 UnicodeData, DerivedNormalizationProps,
CompositionExclusions, complete NormalizationTest, ReadMe, Unicode license and
UAX #15 revision 58 are pinned in
`coax/tests/unicode/upstream/unicode-18.0.0/SOURCES.json`. Larger payloads are
losslessly archived with deterministic gzip; both original bytes and archive
bytes have SHA-256 pins. No downloaded code or build script is executed.
The source integrity tests reject tampered, missing and unlisted data.

The deterministic generator parses only those inputs. It recursively expands
compatibility and canonical mappings, models Hangul, checks explicit exclusions
against Full_Composition_Exclusion, and generates 5,982 decompositions with
9,265 values, 1,002 nonzero combining classes and 961 composition pairs.
The largest recursive expansion is eighteen codepoints. Generated data SHA-256:
`ce5e30f7f0579d1153df5c52e209b454e921c098fa0aafb138065cff43d2b832`.
Runtime source SHA-256:
`5a157bf8d2b923a0fcf205dcd0fca2c963895170526bf3d65ad67eb4e7b22c99`.

The initial byte profile admits up to 1,024 input bytes, at most 18,432 caller
scratch codepoints and 73,728 destination bytes. Strict UTF-8 rejects malformed,
overlong, surrogate and out-of-range sequences. Recursive decomposition,
stable canonical ordering and blocked canonical/Hangul composition finish in
scratch; only after the complete encoded size is admitted are destination bytes
written. Refusals report zero written and preserve destination, while scratch
can contain intermediate data. Stable valid extents, disjointness, alignment,
exclusive access and input lifetime remain caller obligations. No pointer
authentication or concurrent publication is claimed.

The checked-in [JSON report](2026-10-29-unicode-nfkc.json) records the actual
compiler/tool/target/source/artifact identities and results:

- Seven Coax checks pass under the normal check budget; the complete reachable
  mutation set is **417/417 killed**, with no filtered mutants or waivers.
- All **20,171 official normalization rows × five NFKC columns** match c4:
  **100,855 invariants**, without using the host's Unicode normalization.
- **293,187 assigned scalar identities** omitted from Part 1 pass. Surrogates
  are excluded because they are not Unicode scalar values; private-use assigned
  ranges remain included. Three additional maximal/long-order sequences bring
  the actual total to **394,045 cases per profile**.
- Actual checked and release native and Wasm implementations each pass that
  full set, with read-only input and state/output guard checks. Native ASan/UBSan
  exercises malformed counts/UTF-8, reported alias, zero/null-empty input,
  maximum ASCII input and insufficient destination. Wasm separately tests
  malformed metadata, alias refusal and guards with a 1 MiB stack.
- Both modes compile for arm64 iOS Simulator. This is compilation evidence;
  there is no simulator execution or physical-phone claim.

The first complete mutation gate was 362/429, exposing missing encoding and
boundary cases as well as redundant initialization/arithmetic. The next was
414/417. The final three missing cases were odd supplementary-plane lead
decoding, U+E000 admission and exact decomposition capacity. Independent
byte/boundary checks were added; overwritten initializers and redundant
arithmetic were simplified. Both failed scores/source/log hashes are retained
in the report. No equivalence marker, test omission or budget change was used.

An independent read-only review traced scalar UTF-8, recursive generation,
composition exclusions, ordering/blocking, Hangul and publish-after-capacity
behavior and found no concrete correctness defect under the stated contract.
It identified performance/stack limits that remain explicit:

- Canonical insertion ordering is quadratic, and generated C materializes local
  copies of the 1,002/5,982/961-entry lookup arrays before native optimization.
- The tested native `-O2` compiler reported a largest individual frame of
  96 bytes after optimization. A separate `-O0` observation of identical emitted
  C reported 24,192 bytes. These are individual compiler frame reports, not
  complete stack bounds or promises for another compiler/target.
- A 1,023-byte reverse-class sequence measured 2.046 ms checked and 1.978 ms
  release natively, and 2.872/2.381 ms in the named Node/V8 Wasm run. The report
  retains all three boundary-case timings. These are single warm observations
  on the development host, not a latency, worst-case or phone guarantee.

Unicode-version stability is explicit: the pinned version must be shared by
consumers, and no host-version fallback is permitted. An older host may treat
newly assigned characters differently. A NIP-49 integration must separately
admit normalized output length, own secret cleanup and establish its device
workflow. This slice grants no constant-time, erasure, production-key or backup
integration approval. API and reproduction commands: `coax/lib/text/README.md`.
