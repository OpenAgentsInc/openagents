# B1 retained stores — completed qualification, 2026-11-01

B1 checks retained descriptor stores in every concrete checked procedure, including void and uncalled bodies, hidden checks and generic instances. It rejects known callee-local backing retained in caller or static storage at the original RHS or substituted call. Addressed storage remains distinct from the old descriptor. Caller parameter obligations are projected through actual calls; supported inline-array sources retain their actual storage origin.

The qualified source is `b83517e3f071cc8227347568d18bc3bfd1a3751b`, checker `177892429ac07bc7eac88a948e2a0b4be43fb9cb606d41224e669a1dfb5ac895`. Its compiler fingerprint is `1fb5436c6168362ea395419b8951aed1d98d04973d7e1e84f4c24f0acee2b32e`; the actual qualified compiler image is `3b2b62250751bc21f23566cff2dd8305c82185534da9d4966b4e4f155734abfa`.

| Gate | Actual result |
| --- | --- |
| Authored checker checks | 984/984, including authority coverage on the fresh stage0 |
| Full authority mutation | 9,253/9,253 killed; eight shards and actual merge; zero survivors |
| Fresh normal bootstrap | All 165 required statements; no gate-cache or changed-line tolerance |
| Determinism and package | C/header seed comparisons, compiler aggregate fixed point, final re-emission comparisons, provenance and all 17 package artifacts |
| Source diagnostics | All 33 B1 and 44 returned-borrow cases, including original physical witnesses |
| Native retained-store observations | 19 source cases plus disabled/short controls, 21 per profile at O0, O2 and ASan/UBSan |
| Header layout | 21 records/198 offsets; exact byte-identical-header continuity plus fresh native profile guards |
| Existing consumers | Legacy checker 10; System One three profiles with 129 digit and 45 stderr cases each; 1,000 independent returns; all eight diagnostic-schema tests |

The matching development image compiled the complete compiler input using 1,047,475 of the unchanged 1,048,576 work steps, 14,561 of 16,384 query rows and 8,251 of 65,536 origins. The debugger observed the native analysis return and compiler exit zero. This quantitative observation is separate from the subsequently completed normal qualification. Remaining work headroom is 1,101 ticks; subsequent compiler growth needs measured cost reduction.

The cost fixes reuse frozen type/field facts, normalize completed raw facts only for source classification, avoid redundant direct-storage memo rows and classify each canonical effect's formal array storage once. Source-derived eligibility and eager seeding replace redundant lazy/state spellings. Per-pair manual relations reuse already admitted Coax policy predicates. All original capacities remain. No equivalent-mutant labels, native semantic fallbacks or authority-scope reductions were introduced.

The earlier 959-check diagnostic covered a different inventory: 9,273 mutants with 135 survivors. Its failed records remain. The final 9,253-mutant result is a fresh whole-source observation, not a same-inventory score adjustment. Exact seeded defect checks and reviewed dead-code simplifications drove the repair. Four duplicated authored blocks were coalesced while retaining all nine semantic cases and public assertions. Impossible oversized aggregate fixtures were replaced by the production scalar admission predicate and real bounded views; normal C emission limits remain unchanged. Preparation errors and insufficient first fixtures remain in the proof history.

An independent agent reviewed the complete 61-path source change and found no blocking correctness, regression or check-weakening issue. The final integration retains the qualified compiler fingerprint and every one of the 145 repository inputs pinned by the final gate collection. Incoming unrelated main work is preserved.

B2 local-carrier helper writes followed by returned-field reads, precise SoPost composition, B3 request defaults/ownership/completion/cancellation, foreign retention and general region/fixed-point lifetime guarantees remain separate open work. B1 does not establish general memory safety or automatically discharge host-entry obligations.

The [record](record.json) binds actual producers and receipts. The frozen raw proof archive and member manifest retain current qualification, independent source review, earlier failures, source variants and their identities. Historical stopping archives remain unchanged: [first stop](../coax-audit-stopping-point-2026-11-01/assessment.md) and [cost-fix resume](../coax-retained-stores-B1-resume-2026-11-01/assessment.md).
