# Coax changelog

The compiler's recorded releases, newest first, including patch releases.
Each commit identifies the version's introduction on `main`; explicitly labelled
follow-ups landed while that version was still current. Dates use the repository's
America/Chicago calendar.

Reconstructed from the version transitions in `src/main.cpp` (through 0.13.0)
and [`release.coax`](release.coax) (from 0.14.0), reviewed against their changes
and retained evidence. The initial reconstruction covers **0.37.0 to 0.1.0** through
[`99f0d403f2b335654a54e1a60611f8c8251b3776`](https://github.com/OpenAgentsInc/openagents/commit/99f0d403f2b335654a54e1a60611f8c8251b3776).
These are compiler versions, separate from inventory, wire and protocol schema
versions. [`STATUS.md`](STATUS.md) describes current support;
[`RELEASE.md`](RELEASE.md) describes the release workflow.

## 0.38.0 — 2026-10-31

Commit: [`0b82a1587a56a4eed8ead255dd32300efe49fc30`](https://github.com/OpenAgentsInc/openagents/commit/0b82a1587a56a4eed8ead255dd32300efe49fc30)

- `#foreign` parameters may be borrowed views of integers, floats and `bool`
  (`[] u8`, `[] s16`, `[] f32`, `[] bool`, ...), passed as the `{data, count}`
  struct `#export_c` uses and declared in the generated header. The host reads
  and writes the view during the call only: no retention, ownership transfer or
  asynchronous use; views may overlap and an empty view may have a null `data`.
  Views of strings, structs or views, fixed arrays and view results stay E0376.
- The entropy bridge gains `cxrand_draw(out, n)`: one call writes the caller's
  view; `relay_conn.coax` and `cdp.coax` use it instead of n + 1 per-byte calls.
- Same-version follow-up: `place op= value` stages the place's address when the
  place's own indexes are sequenced through temporaries
  (`c[a[i-1]*256+a[i]] += 1`). Spelling it twice assigned those temporaries
  twice in one C expression, which C leaves undefined and clang rejects with
  `-Wunsequenced`. `tests/pass/compound_view` covers it.

## 0.37.0 — 2026-10-31

Commit: [`6132cbaf0a0dddccbb1d3d9ae32a9659c5bf6644`](https://github.com/OpenAgentsInc/openagents/commit/6132cbaf0a0dddccbb1d3d9ae32a9659c5bf6644)

- Generic structs: `Pair :: struct($T)` and `Box :: struct($T, $N)`, explicit
  type applications, field-based literal deduction and generic-struct procedure
  parameters. Concrete instances receive their own layout, defaults, evaluation
  and C emission within the shared 256-instance budget.
- Bindings require fixed storage; views and aggregates containing views refuse.
- Same-version follow-ups:
  [`3ea326018c`](https://github.com/OpenAgentsInc/openagents/commit/3ea326018c652ec0b099607e94adccce6073830d)
  added selected authored Verse/Psionic plain `#test` comparisons between the
  evaluator and checked native `-O0`/`-O2` execution. This tooling subset does
  not qualify native `#forall` or replace authority gates.
  [`99f0d403f2`](https://github.com/OpenAgentsInc/openagents/commit/99f0d403f2b335654a54e1a60611f8c8251b3776)
  reconciled both books with delivered capabilities and added reviewed prose,
  feature and evidence drift checks.
- The [frozen four-task authoring corpus](https://github.com/OpenAgentsInc/openagents/commit/a56793efc76411986dc48d878208f2dbaec1f767)
  also landed under this version, with immutable oracle/start identities and
  recorded repair, elapsed-time and edit measurements.

## 0.36.0 — 2026-10-31

Commit: [`03d153e89a947797c50330c13f48c1984fa53711`](https://github.com/OpenAgentsInc/openagents/commit/03d153e89a947797c50330c13f48c1984fa53711)

- `$N` value parameters deduce fixed-array lengths in `[$N] T`; generic element
  types work in `[4] $T` and `[$N] $T`.
- Repeated bindings must agree on length; wrong argument kinds and names used
  as both type and length parameters refuse. Corrected generated error-reference
  headings for E06xx/E07xx.

## 0.35.0 — 2026-10-31

Commit: [`b75820e208a781453c41af45bf85ed95023e548a`](https://github.com/OpenAgentsInc/openagents/commit/b75820e208a781453c41af45bf85ed95023e548a)

- Callable generic procedures deduce `$T` from concrete argument types and
  borrowed-view element types. Each procedure/binding has one checked concrete
  instance, with a 256-instance cap.
- Instances participate in `#run`, authored checks, authority coverage, mutation
  and native emission; timing reports include instance counts and generated
  bytes. Constraints and value parameters were still pending.
- Same-version follow-up:
  [`ffdbded841`](https://github.com/OpenAgentsInc/openagents/commit/ffdbded84153007a41517dc50c5b6f0e0d5f221d)
  added `coax wgsl FILE --functions`, a Coax WGSL emitter using the checked tree.
  Web builds adopted it and removed the Python WGSL translator.

## 0.34.0 — 2026-10-31

Commit: [`1cc85cf74f0f85aad369aee59cd151950e0f2c84`](https://github.com/OpenAgentsInc/openagents/commit/1cc85cf74f0f85aad369aee59cd151950e0f2c84)

- `$T` procedure signatures gained lexing, parsing and placeholder type checking,
  with explicit refusals for unbound/result-only parameters, unsupported
  nesting, recursion, C exports and placeholder layout queries.
- Generic calls still refused with E0397 and templates were omitted from C
  emission. Callable instantiation arrived in 0.35.0.

## 0.33.0 — 2026-10-31

Commit: [`6da88715c00baaf7848942c1d074e953de6df01c`](https://github.com/OpenAgentsInc/openagents/commit/6da88715c00baaf7848942c1d074e953de6df01c)

- `coax test/mutate --evidence FILE` writes `coax.evidence.v1`, binding individual
  checks, quantified checks, authority coverage and mutation claims to sources,
  compiler identity, evaluator target, profile, selection and budgets.
- Outcomes distinguish passed, counterexample, inconclusive,
  infrastructure-error and not-checked; the worst outcome controls the verdict.
- Same-version follow-ups added
  [`#flat` GPU varyings](https://github.com/OpenAgentsInc/openagents/commit/e9ee646abffd0eb3dc1246967cd1c8b41ec8846b),
  [Coax DEFLATE/zlib/PNG decoding](https://github.com/OpenAgentsInc/openagents/commit/8f1456e4d96af7d858acacc3ee92006f8dc3fd9a)
  and [baseline JPEG decoding](https://github.com/OpenAgentsInc/openagents/commit/b5ce59b0bada23c1cea0ca704854745823541730).

## 0.32.0 — 2026-10-31

Commit: [`e132ce1f3465d74e97ec3304e280e9713fd67e6a`](https://github.com/OpenAgentsInc/openagents/commit/e132ce1f3465d74e97ec3304e280e9713fd67e6a)

- `coax deps --lowering` exposes `coax.lowering.v1`: the checked-effect ordering
  decisions used by C emission, with declaration identities and source spans.
- The differential reducer minimizes a seeded reversed-order lowering defect.
  Added E0705 documentation and grammar coverage for delivered module syntax.
- Same-version follow-up
  [`950b375e38`](https://github.com/OpenAgentsInc/openagents/commit/950b375e38e9886ddf6f5cc3f329c997f578d0e2)
  fixed SPIR-V local arrays borrowed through views to use Private storage.

## 0.31.0 — 2026-10-31

Commit: [`6d7af890ec90eed4b977b856e6de9a191670be9a`](https://github.com/OpenAgentsInc/openagents/commit/6d7af890ec90eed4b977b856e6de9a191670be9a)

- The checked graph classifies expression effects; native C stages two trapping
  operands and trapping destination/value evaluation in source order.
- Qualified first-trap order at `-O0` and `-O2`, caller-visible writes,
  exactly-once compound places and reverse cleanup for the supported subset.
  Lowering inspection followed in 0.32.0.

## 0.30.0 — 2026-10-31

Commit: [`cbfe5dbc50530d91d4a1dd0520411815aad1bc40`](https://github.com/OpenAgentsInc/openagents/commit/cbfe5dbc50530d91d4a1dd0520411815aad1bc40)

- `#export NAME, ...;` seals a logical module: other modules can access only its
  listed declarations. Member source units share access while keeping their own
  checks and authority coverage. Module inventories report exports.
- Export lists name owned declarations only; reexports refuse. Verse HUD became
  the first sealed production module.

## 0.29.0 — 2026-10-31

Commit: [`105acc70f0abbb17d220fe36ada8c2d8ea1c5309`](https://github.com/OpenAgentsInc/openagents/commit/105acc70f0abbb17d220fe36ada8c2d8ea1c5309)

- Dependency inventories report exact authored declaration byte ranges, lines
  and content hashes, or generated protobuf-directive origins.
- Added the typed `native-lp64` layout record, checked against independent C
  size/alignment observations. `coax deps` and `coax layout` reject other target
  names rather than implying unsupported layouts.

## 0.28.0 — 2026-10-31

Commit: [`e6437772c29b9554a47687fd3f78aaabf42b391d`](https://github.com/OpenAgentsInc/openagents/commit/e6437772c29b9554a47687fd3f78aaabf42b391d)

- `#load "FILE.coax" as NAME;` adds module-scoped lookup and qualified types,
  literals, procedures, constants and enum members. Ambiguity is diagnosed at
  use; C emission qualifies private collisions and refuses incompatible public
  symbol collisions.
- Same-version follow-up
  [`926cae2bfe`](https://github.com/OpenAgentsInc/openagents/commit/926cae2bfee9d25b6f8b8879813cef0e5cc02ea9)
  added checked-tree SPIR-V vertex/fragment lowering for the bounded graphics
  subset, including vectors, records, arrays, views, textures and samplers.

## 0.27.0 — 2026-10-31

Commit: [`427ce9d4bed6bb2a609db728f9d283becccd4ebc`](https://github.com/OpenAgentsInc/openagents/commit/427ce9d4bed6bb2a609db728f9d283becccd4ebc)

- `coax deps --modules` emits `coax.modules.v1`, with conservative module-content
  and dependency revisions separate from nominal identities and source hashes.
  These revisions do not constitute a complete cache key.
- Shared Everglade guard types became a production module consumer.

## 0.26.0 — 2026-10-31

Commit: [`9150d202ad932fc4a94254114b6d032468ba1e41`](https://github.com/OpenAgentsInc/openagents/commit/9150d202ad932fc4a94254114b6d032468ba1e41)

- `#module "a.b";` establishes logical ownership across loaded member units.
  Declarations gain stable nominal `coax.decl.v1:MODULE:KIND:NAME` identities.
- `coax deps --declarations` reports owners, identities and checked references.
  This release established identity; lookup remained flat until 0.28.0.

## 0.25.0 — 2026-10-30

Commit: [`48fa03aa887cf6c30b9e4d8a28e0e50f0289d1c9`](https://github.com/OpenAgentsInc/openagents/commit/48fa03aa887cf6c30b9e4d8a28e0e50f0289d1c9)

- Independent `--arithmetic checked|wrapping`, `--opt 0|1|2|3`,
  `--build-profile development|release` and debug-information selections, while
  preserving legacy defaults and `--release` behavior.
- `--profile-report FILE` records the resolved profile; mutation receipts bind
  it. Compile-time arithmetic and authority checks retain their existing rules.
- Same-version follow-ups added
  [`coax test --only`, `--list`, per-check progress and separate check-storage limits](https://github.com/OpenAgentsInc/openagents/commit/03ffce741a36bb5ba5996907e8043f22ac1a3742),
  [checked Coax scalar-to-SPIR-V emission](https://github.com/OpenAgentsInc/openagents/commit/69eff0b430aad4551d35af4a6980533ab4d58ba8)
  and [SPIR-V fill/stroke passes](https://github.com/OpenAgentsInc/openagents/commit/d9804e9510a273002ce2e60a457d6fec90923744).
- A same-version [provenance-verified compiler cache](https://github.com/OpenAgentsInc/openagents/commit/6e8520ac0062dbce7e43b15c8a69e08f23f5d95c)
  added reusable release builds, builder locking and compiler-input snapshots;
  `COAX_BUILD_FRESH=1` retains a fresh-bootstrap path.

## 0.24.0 — 2026-10-29

Commit: [`3dbc6292fc7dd34237d84b142b00920ba4629a4d`](https://github.com/OpenAgentsInc/openagents/commit/3dbc6292fc7dd34237d84b142b00920ba4629a4d)

- CUDA warp kernels: `for lane: 0..<32`, per-lane register arrays and
  `cu_mma_bf16_16x8x16`, with a Coax CPU meaning and CUDA tensor-core lowering.
- Same-version follow-ups added
  [`coax metal-compute`](https://github.com/OpenAgentsInc/openagents/commit/c98e206bb6e00a05a62f3a8be2d3441630c6195a),
  including the `APPLE-FF-v1` float-float profile, and
  [bounded group kernels](https://github.com/OpenAgentsInc/openagents/commit/e23facf54abddd36a4ca55bf389b6b2f76774f52)
  with owned shared-memory writes, barriers and Apple matrix primitives.
  General atomics remained unsupported.
- Other same-version additions:
  [`coax mutate --root-only`](https://github.com/OpenAgentsInc/openagents/commit/9322269a89db5ae31cfa2ed05e869bd0f4cd350c)
  retains all baseline checks while limiting mutation sites to the entry source;
  [composable native headers](https://github.com/OpenAgentsInc/openagents/commit/789fb4909a1a6c86cbe84b63e2d63361128f7bae)
  share compatible ABI declarations and reject incompatible public C spellings.
- Shared-library/tooling follow-ups added
  [native relay/WebSocket support](https://github.com/OpenAgentsInc/openagents/commit/d7373bcc825985a284f15c9d3805ee64ca00652d),
  [TLS socket effects](https://github.com/OpenAgentsInc/openagents/commit/604767080e86383ea64285ea0e83117a646f8982),
  [JSON node parsing/writing](https://github.com/OpenAgentsInc/openagents/commit/a3829b63315b090b875443422eb6b76022a48581),
  base64, filesystem/process/argv adapters and a
  [headless Chrome DevTools client](https://github.com/OpenAgentsInc/openagents/commit/62778126aa080ca980f0015c1a608b6fd5293645).
  The [source-unit ceiling rose from 64 to 128](https://github.com/OpenAgentsInc/openagents/commit/cfe213bcfa0892aa7a7bf90c947b47af2116a4a1).

## 0.23.0 — 2026-10-29

Commit: [`666bae043c9bd800d15ed0e7e60912fb8866fa7f`](https://github.com/OpenAgentsInc/openagents/commit/666bae043c9bd800d15ed0e7e60912fb8866fa7f)

- `coax cuda` emits the admitted compute subset as CUDA C for NVRTC: numeric
  views, fixed scalar arrays and reachable procedures rooted at exported
  item kernels. Admission policy is checked Coax; refusals use E0700/E0701.
- Psionic's BF16 projection exercised the emitted code on an RTX 4080. The
  catalogue also recorded tested Linux x86-64/GCC host support with explicit
  qualification limits.
- Same-version follow-up
  [`c5681dbb57`](https://github.com/OpenAgentsInc/openagents/commit/c5681dbb57a7fc2e3cd50df2c5bda1d008a7d1dc)
  added parallel mutation workers, `--jobs`, `--shard I/N` and `--merge`, preserving
  the serial report for the same scope.

## 0.22.0 — 2026-10-29

Commit: [`041464193d180e49cd0e71bf9a14ff1b77d2c38a`](https://github.com/OpenAgentsInc/openagents/commit/041464193d180e49cd0e71bf9a14ff1b77d2c38a)

- Compiler provenance records bind declared build inputs, generated artifacts,
  toolchain/environment identity and the compiler binary. Bootstrap builds from
  a private input snapshot; site releases verify consistency before staging.
- Unrelated repository changes can reuse the verified compiler. The record is
  a consistency check, not a signature or compiler-correctness proof.

## 0.21.0 — 2026-10-29

Commit: [`73c6b07d1bf8c926d51a89dd3447dc7ff63e648a`](https://github.com/OpenAgentsInc/openagents/commit/73c6b07d1bf8c926d51a89dd3447dc7ff63e648a)

- An independent aggregate-traversal meter bounds copying, assignment, view
  detection and initialization at 134,217,728 visits per frontend evaluation
  or individual check, refusing with E0400.
- Dependency/timing reports expose the limit and consumed work. Existing
  expression-step budgets remain separate; this does not bound total process
  memory or wall time.

## 0.20.0 — 2026-10-28

Commit: [`59df8f79a361a1dde9c7d7f8448b3594895d2e81`](https://github.com/OpenAgentsInc/openagents/commit/59df8f79a361a1dde9c7d7f8448b3594895d2e81)

- Interpreted byte-view-to-string casts use owned immutable snapshots, releasing
  temporary owners while retaining frozen values and shared slices correctly.
- Separate live-byte and materialization-work budgets refuse exhaustion with
  E0400; timing reports expose owned-string peaks. Allocation failures recover
  cleanly during freezing and default evaluation.

## 0.19.0 — 2026-10-28

Commit: [`17c2656677d168bb4a0d037608dc425b7e531aad`](https://github.com/OpenAgentsInc/openagents/commit/17c2656677d168bb4a0d037608dc425b7e531aad)

- Conservative mutation scheduling records baseline procedure execution and
  runs affected isolated checks per mutant, while retaining the complete mutant
  set and falling back to all checks for shared or unknown state.
- `--mutation-reference`, `--mutation-stats` and `--mutation-trace` expose the
  reference runner and scheduling evidence. Bounded evaluation failures restore
  constant state before later checks.

## 0.18.2 — 2026-10-28

Commit: [`0c7ce1d9c26aa0105e4a16b460e35cb00ad87ef0`](https://github.com/OpenAgentsInc/openagents/commit/0c7ce1d9c26aa0105e4a16b460e35cb00ad87ef0)

- Raised the compile-time aggregate-slot ceiling from 1,048,576 to 1,572,864 to
  admit existing Nostr JSON boundary checks. Kept the work and container limits
  unchanged and retained boundary/refusal checks for the revised ceiling.

## 0.18.1 — 2026-10-28

Commit: [`d5fc7e58328986e6e16a5b62609fc181d5c439b7`](https://github.com/OpenAgentsInc/openagents/commit/d5fc7e58328986e6e16a5b62609fc181d5c439b7)

- Iterative aggregate release prevents deep retained-view chains from exhausting
  the host stack during cleanup.
- Clarified that timing counters cover frontend evaluation before authority
  checks, and documented the traversal work still outside the initial meter.

## 0.18.0 — 2026-10-28

Commit: [`bb65b83271523eaec0b015a2b5241a3d211e675a`](https://github.com/OpenAgentsInc/openagents/commit/bb65b83271523eaec0b015a2b5241a3d211e675a)

- `--run-steps N` configures bounded shared compile-time work, with a 100,000-step
  default and 250,000,000 maximum. Invalid inputs refuse before evaluation.
- Independent live aggregate-slot/container limits, last-owner accounting,
  cycle cleanup, dependency inventory fields and peak-storage timing counters.
  These frontend limits are separate from `coax test --steps`.

## 0.17.0 — 2026-10-28

Commit: [`ed17d7cd26b591a4aa8cc2abcb50c2528ae72e37`](https://github.com/OpenAgentsInc/openagents/commit/ed17d7cd26b591a4aa8cc2abcb50c2528ae72e37)

- `#if BUILD_DEBUG { ... } else { ... }` selects struct fields and statement
  blocks. Both branches parse; only the selected nodes participate in checking,
  layout, reflection, compile-time execution, authority reachability and emission.
- The selector follows the build profile and can change native layout. General
  conditions, conditional imports and top-level conditional declarations remained
  unsupported.

## 0.16.0 — 2026-10-28

Commit: [`6acc1f76936b09775d83d172ef48ca968d2de72b`](https://github.com/OpenAgentsInc/openagents/commit/6acc1f76936b09775d83d172ef48ca968d2de72b)

- `mul_wide(u64, u64)` returns the exact two-word product as
  `CoaxU64Product.{hi, lo}`.
- Matching checked frontend rules and compile-time evaluation, native 128-bit
  lowering and portable wasm32 lowering, with independent arithmetic evidence.

## 0.15.0 — 2026-10-28

Commit: [`2c570ddb9718a3fcd51a75d1405d0e66c27e267d`](https://github.com/OpenAgentsInc/openagents/commit/2c570ddb9718a3fcd51a75d1405d0e66c27e267d)

- Explicit `u32`/`u64` modular arithmetic: `add_wrap`, `sub_wrap` and `mul_wrap`.
  `rotl` and `rotr` reduce the rotation count modulo the word width.
- Matching compile-time, checked native C and wasm32 semantics preserve argument
  order and refuse mismatched operand widths.

## 0.14.0 — 2026-10-28

Commit: [`3ea7287819c37b8a0c903f249f23f99a7ce831db`](https://github.com/OpenAgentsInc/openagents/commit/3ea7287819c37b8a0c903f249f23f99a7ce831db)

- A single checked Coax release catalogue generates CLI metadata, JSON inventory,
  feature tables and managed book/current-version regions.
- Compiler bootstrap, app generation and book publication refuse source,
  snapshot or binary drift. Release, inventory-schema and historical feature
  versions remain separate.

## 0.13.0 — 2026-10-28

Commit: [`4830ad9c30ef749d12c75cb57e35ea7ecff2ff2f`](https://github.com/OpenAgentsInc/openagents/commit/4830ad9c30ef749d12c75cb57e35ea7ecff2ff2f)

- Original-source native line mapping, `--debug-info`, hashed `--source-map`
  artifacts and source-aware LLDB presentation. `--timings` reports compiler
  phases; real engine/Metal harnesses measure native execution cost.
- Same-version follow-ups added
  [E0383 local-array borrow-return diagnostics](https://github.com/OpenAgentsInc/openagents/commit/9d0f246ba1b95ae0a491cce8e6cb15f4a1de3d21)
  through local aliases, assignments, subranges, casts and conditionals, and
  [fixed C lvalues for indexed places containing calls](https://github.com/OpenAgentsInc/openagents/commit/e2f8432ea2c57e9fb4106fa79e44033e1f20e2fc).
  Interprocedural and aggregate-contained lifetimes remained unchecked.
- Further follow-ups added
  [`value_barrier(u32|u64)`](https://github.com/OpenAgentsInc/openagents/commit/b9a5f216f0c0ddcf898bfb3ef0261cc4f2788be8),
  an optimizer-opaque identity with scoped native/wasm evidence, and
  [bounded evaluator/native differential generation and reduction](https://github.com/OpenAgentsInc/openagents/commit/63c51caeaba313b07ca77fce34a91ab4291b900f).
  The latter landed before the 0.14.0 bump, although the later catalogue records
  its introduction as 0.14.0.

## 0.12.0 — 2026-10-28

Commit: [`86ec81de01f0ebf540b070f078643d7e68c45d68`](https://github.com/OpenAgentsInc/openagents/commit/86ec81de01f0ebf540b070f078643d7e68c45d68)

- CPU layout reflection: schema-1 `type_info`, `field_info` and `enum_info`
  produce ordinary values, with compile-time tables and bounded selectors.
- Haunted House vertex admission consumed reflected layout. Support was scoped
  to the bootstrap LP64 CPU layout.

## 0.11.0 — 2026-10-28

Commit: [`3837e89ed5948bc72fea70efdb713f00cdc051a4`](https://github.com/OpenAgentsInc/openagents/commit/3837e89ed5948bc72fea70efdb713f00cdc051a4)

- Checked `#load` source graphs: canonical once-only file identity, cycle/budget
  admission, physical-unit syntax validation and source-relative proto imports.
- Original-file diagnostics/traps, per-file authority and private-library
  verification. `coax deps` inventories source and descriptor inputs.

## 0.10.0 — 2026-10-28

Commit: [`1e0ce7636caf5a97876fca98fc02472cd6bf328c`](https://github.com/OpenAgentsInc/openagents/commit/1e0ce7636caf5a97876fca98fc02472cd6bf328c)

- Semantic checking moved into checked Coax: names, scopes, types, layout,
  numeric folding/range/IEEE decisions, calls, control flow, checks and Metal
  admission.
- Added the explicit generated checker seed, bounded indexed graph and exact
  bootstrap regeneration. C++ remained the evaluator, emitter and storage bridge.

## 0.9.0 — 2026-10-28

Commit: [`d7c3a1e9d3011dcd26f35c1930cf3dcb3dc3e4bd`](https://github.com/OpenAgentsInc/openagents/commit/d7c3a1e9d3011dcd26f35c1930cf3dcb3dc3e4bd)

- Parsing and source proto-import admission moved into checked Coax: grammar,
  precedence, diagnostics and a bounded indexed syntax tree.
- Explicit parser bootstrap seed and fixed-point regeneration; the native
  bridge transports tokens and reconstructs syntax-tree storage.

## 0.8.0 — 2026-10-27

Commit: [`b374c75fae5d131df3e30a9324e762f27d3c04c4`](https://github.com/OpenAgentsInc/openagents/commit/b374c75fae5d131df3e30a9324e762f27d3c04c4)

- Lexing, token recovery/diagnostics and exact decimal-to-binary64 conversion
  moved into checked Coax.
- Explicit generated lexer seed, authority/mutation checks and exact bootstrap
  regeneration replaced native lexical/numeric-conversion decisions.

## 0.7.1 — 2026-10-27

Commit: [`1e0e1390632653183dd4fbe6860c3bbcf2ee7bc6`](https://github.com/OpenAgentsInc/openagents/commit/1e0e1390632653183dd4fbe6860c3bbcf2ee7bc6)

- Streaming SHA-256, hexadecimal encoding, runtime-boundary extraction and
  importer provenance/source-envelope assembly moved into checked
  `fingerprint.coax`; native code supplies I/O, allocation and copying.
- Same-version follow-up
  [`03a379cad6`](https://github.com/OpenAgentsInc/openagents/commit/03a379cad69dddc34732b647b9619e1f3a5b2d15)
  guarded scalar-view typedefs so generated headers from two libraries coexist.

## 0.7.0 — 2026-10-27

Commit: [`76b0837c7ca9c45ec30e1155ad3e724000dffea7`](https://github.com/OpenAgentsInc/openagents/commit/76b0837c7ca9c45ec30e1155ad3e724000dffea7)

- Descriptor parsing, validation, option/name/dependency resolution and codec
  generation moved into authority-checked Coax.
- Generated C11 importer bootstrap and exact regeneration replaced the native
  importer implementation, with no old-importer fallback.

## 0.6.1 — 2026-10-27

Commit: [`e190dc4ac18ba7e917b2589d7981632ea2ea6f36`](https://github.com/OpenAgentsInc/openagents/commit/e190dc4ac18ba7e917b2589d7981632ea2ea6f36)

- Restored authoritative checked Coax wire-runtime sources, separating the pure
  policy core from native address wrappers and gating publication on authority
  checks and mutation.
- Separated compatibility evidence from independent interoperability evidence;
  fixed evaluator field access after a failed struct expression.

## 0.6.0 — 2026-10-27

Commit: [`9f5a2a87fcb1bc02026a089a1dd007f465f09dac`](https://github.com/OpenAgentsInc/openagents/commit/9f5a2a87fcb1bc02026a089a1dd007f465f09dac)

- Descriptor importing moved from an external Python helper into the native
  C++20 compiler, with an embedded wire runtime and provenance fingerprints.
  Ordinary compilation/imports became Python-free while preserving generated
  source bytes.
- Atomic compiler publication and an explicit Mac bootstrap target.

## 0.5.1 — 2026-10-27

Commit: [`c103b0852d5846ccfdb2e3270e9e6c7b3ba9836b`](https://github.com/OpenAgentsInc/openagents/commit/c103b0852d5846ccfdb2e3270e9e6c7b3ba9836b)

- Authority coverage requires actual check-time procedure execution. Empty
  checks/domains, uncovered procedures and no reachable mutation sites can no
  longer silently pass.
- Tightened equivalent-marker parsing and expanded exhaustive wire-boundary
  checks and descriptor audit coverage.

## 0.5.0 — 2026-10-27

Commit: [`902a131bb38017f3f96ff234984cc048807c8865`](https://github.com/OpenAgentsInc/openagents/commit/902a131bb38017f3f96ff234984cc048807c8865)

- Check-first `#test`, finite `#forall`, `#authority`, `coax test` and
  `coax mutate`, with bounded check budgets and counterexamples.
- Always-on statement `#assert` and build-time authority checks.

## 0.4.0 — 2026-10-27

Commit: [`f9e377bad6eaf13697cdbe8e5b7bdfb5d8cdb863`](https://github.com/OpenAgentsInc/openagents/commit/f9e377bad6eaf13697cdbe8e5b7bdfb5d8cdb863)

- `#import_proto` accepts selected proto3 descriptor closures and generates
  borrowed typed views/accessors and caller-buffer codecs, including presence,
  oneof and repeated fields, validation and bounds checks.
- Import-time tooling still required Python; generated runtime code did not.
  This was the first descriptor-import release, preceding the later Coax-native
  frontend work recorded by the current catalogue.

## 0.3.0 — 2026-10-27

Commit: [`6d49ae58fc6590aaff6d3a770c6f554d741c6cd0`](https://github.com/OpenAgentsInc/openagents/commit/6d49ae58fc6590aaff6d3a770c6f554d741c6cd0)

- Fixed-width integers/floats, untyped constants/literals, character and bit
  operations, checked/truncating casts, defaulted structs/enums, fixed arrays
  and borrowed views.
- Collection loops, `break`/`continue`, `defer`, `ifx`, `size_of`, ordered operands,
  aggregate `#run`, C exports and `lib`/`header`/`layout`/`explain`/JSON tools,
  plus native-host demos and an expanded runnable book.
- The [native-home branch merge](https://github.com/OpenAgentsInc/openagents/commit/9c0ae696832013260cae86368e0237f0c272dbf7)
  reconciled a parallel 0.3.0 bump and added scalar `#foreign`, suffixed floats,
  numeric conversions, math/byte intrinsics, `c --library --header` and the
  restricted `coax metal` emitter.

## 0.2.0 — 2026-10-27

Commit: [`85e7706684949f54a75f9094e8a5d6a99637ec02`](https://github.com/OpenAgentsInc/openagents/commit/85e7706684949f54a75f9094e8a5d6a99637ec02)

- Bounded scalar `#run`, compile-time `#assert`, immutable top-level constants
  with forward references, `type_name` and `coax check`.
- Formal bootstrap grammar and compile-time diagnostics/tests, imported from
  aethyr-cpp revision `040329b`, retaining the missing-semicolon location fix.

## 0.1.0 — 2026-10-27

Commit: [`6329f3e0b3cf6fb49c0ae0686241097235f6607b`](https://github.com/OpenAgentsInc/openagents/commit/6329f3e0b3cf6fb49c0ae0686241097235f6607b)

- Initial compiler imported into this repository: a C++20 frontend emitting
  self-contained C11, procedures, scalar `int`/`bool`/`string`, locals and control
  flow.
- Checked development arithmetic, wrapping release arithmetic, checked `print`,
  stable diagnostics, `run`/`build`/`c`/`version`, the language specification and
  first book chapters.
