# Next context policy: bounded declaration neighborhoods

Proposed follow-up, October 3, 2026. This design was written during the native pilot after the coordinator observed omitted caller context. Its source review used public implementation and index structures. It is an untested proposal, separate from the frozen native comparison.

## Recommendation

Keep the selected implementation declaration as an anchor, then offer a small amount of caller and callee evidence around it. Track every exact task clause, cap tests separately, and say when relevant evidence does not fit. First test these packing rules offline with fixed anchors. Do not add another decision call to choose each dependency.

## What exists

- [`syntax.rs`](../../../../crates/briefing-lab/src/syntax.rs) persists declaration kind, simple/lexical qualified name, declaration/signature/body byte and line spans, and parse-error/omission flags. Scope nesting can be inferred from ranges; there is no explicit parent ID, call-site list, import map, visibility field, type resolution, or call graph. Trait implementations can share a lexical name. Use source commit + path + span/kind as identity.
- [`lib.rs`](../../../../crates/briefing-lab/src/lib.rs) binds each file to a Git blob, SHA-256, size, line count, lexical terms, symbols, and optional syntax. The index scans at most 64 MiB / 10,000 files, 512 KiB per file. Its omissions limit any coverage claim.
- [`spans.py`](../../../../bench/jev-lifecycle/spans.py) already verifies pinned blobs, limits signature reads, ranks declaration pointers, and labels oversized bodies partial. It globally sorts implementation and test functions together after clause voting, then renders the first 24 source pointers. It has no implementation/test byte split or caller/callee expansion. A clause choosing a pointer does not guarantee that pointer survives rendering.
- Relevant reusable Rust code already exists outside that persisted index: [`explicit_structure.rs`](../../../../crates/briefing-lab/src/explicit_structure.rs) freshly parses admitted files into private `Unit` records with textual direct-call candidates, mentioned names, simple local shadowing, and unsupported-call counts. `local_links` admits limited same-file targets and supporting type/constant/import context. `closure` traverses up to 64 declarations and treats macro/method/indirect calls as unresolved; bundle rendering omits an entire bundle when it cannot fit. That is useful implementation material, not a persisted or compiler-resolved graph. Its same-file dependency closure and one nearby test policy are not used by Python `spans.pack`.

## Minimal new facts

Reuse the Rust parser and link logic behind a versioned, serializable interface. Cache bounded call-site records with owning declaration identity, exact call-expression span, written callee text, call kind, candidate target IDs, resolution reason, and ambiguity/omission flags. Build a reverse index over those same supported links for potential callers.

A direct call expression is a syntax fact; its target can remain uncertain. Same-file lexical matching does not resolve imported aliases, receiver types, trait dispatch, macro expansion, conditional compilation, or external modules. Retain unresolved call counts instead of selecting a similarly named function silently. A reverse name index remains an incomplete set of caller candidates. Nested function/closure ownership and shadowing need explicit fixtures before reusing the current broad subtree walker for call-site attribution.

For a first bounded version, stay within the same admitted file set and one hop. Do not add cross-file module/import resolution yet. If cross-file calls are important, make that a separately versioned extraction change and measure its extra coverage and index cost. Existing lexical scoped names alone cannot prove those edges.

## Proposed renderer, frozen before new outcomes

1. Keep full task, instructions, and required contracts common and outside the optional source pack, as in the native pilot. For the initial packing ablation retain the current optional contract allowance too; removing its duplication is a different change.
2. One bundle starts with one selected implementation declaration. Add at most two supported caller candidates and two supported callee candidates, one hop, with deterministic qualified-name/path/span ties. Prefer implementation callers over tests; do not prefer an async/blocking function by task-specific name. Include minimal containing impl/trait signatures and directly referenced type definitions only within the same context allowance. Show link reasons and unresolved alternatives.
3. Keep the 16 KiB rendered ceiling, including provenance and omissions. A concrete first allocation is 10 KiB implementation, 2 KiB tests, 3 KiB optional contract excerpts, 1 KiB headings/coverage. No test borrowing from implementation in this first comparison. Unused test capacity may stay unused; report underfill. Do not let a separate type/import budget expand the total.
4. Preserve complete declarations when they fit. Anchor admission comes first; caller/callee additions are separately complete units. If a dependency cannot fit, retain the anchor and mark the bundle incomplete. If the anchor itself cannot fit, report it unserved; compare clipping versus whole-anchor refusal separately rather than silently change the current partial-body policy.
5. Maintain a record for every exact clause: selected anchor, included whole/partial/omitted/no-match, caller/callee evidence present or unresolved, and test-only evidence. This is source-delivery coverage, not requirement satisfaction. Compound clauses remain compound. One anchor may cover several clause records without duplicating its bytes.
6. To avoid a popular anchor exhausting space before other clauses, test a separate round-robin pass over clause order: admit each distinct anchor once before any neighborhood additions. Then distribute caller/callee additions in rounds. A capped test pass runs last. Neither the AST nor a Choice response can guarantee that every semantic requirement has implementation evidence; fail that coverage claim explicitly when evidence is absent.

## Isolate effects

First use fixed synthetic/public anchor selections, no new inference, identical catalog and 16 KiB limit:

- R0: retained current renderer.
- R1: R0 plus the implementation/test budget split only.
- R2: R1 plus one-hop caller/callee context; keep anchor order unchanged.
- R3: R2 plus clause-first round-robin admission.

Record all deltas and omissions. These offline comparisons establish byte allocation and evidence delivery, not better coding. Use the existing exposed tasks only for development and label them as such. Freeze the resulting extraction/packing policy before choosing new evaluation tasks and before seeing their reference fixes or checker contents.

A later native 2×2 can isolate selection and neighborhood effects: deterministic versus one Jev selection call, crossed with neighborhood off versus on, all using the same role budgets and common contracts. Hold the model, tools, task source, final checks, and total context budget fixed. This does not separately establish a native benefit from the role split; that requires its own control if claimed. Pair repetitions/task order prospectively and retain failures; do not expand or stop based on favorable outcomes.

## Measurements and interpretation

Measure cold index/extraction separately from warm assembly. For every pack record selected blob reads/bytes, cache size, parse/link time, lookup/materialization/render time, serialized selector request bytes, rendered bytes by role, and all caps. Measure p50/p95 warm full-process assembly from fixed repeated inputs, with first run reported separately. Price and time the one optional selection call separately and include it in native endpoints.

Coverage measures: anchors present, clause records with implementation evidence rather than tests only, whole versus partial declarations, supported caller/callee candidates included, unresolved edges, omitted units, and path/role diversity. Validate source spans and link classifications on synthetic ambiguity/trait/alias/macro/nested-function fixtures. On new tasks, independent reviewers can predeclare relevant public entry paths before seeing treatment output; retain uncertainty. None of these counts proves behavioral coverage.

Native metrics remain independent final acceptance, total inference cost including failed attempts and preparation, time through final checks and cleanup, cost per accepted patch, and matched task/repetition differences. Extra source bytes or additional read tools are mechanisms, not a win by themselves.

Leakage controls: pinned pre-task source only; no reference patches, checker symbols, new tests from the reference, model outcomes, or review labels in selection. Namespace/method heuristics and budget constants must be generic and frozen. The current caller-omission observation can motivate this development policy, but cannot turn the same exposed task into an unseen validation case. Catalog names and complete functions may contain public tests/comments; treat them as evidence, not instructions or correctness labels.

## Reviewed source identities

- `crates/briefing-lab/src/lib.rs`: `863e4a0d44af2f7db7f158da89443933a61e5ddcbe55e29012850ca71ae80901`
- `crates/briefing-lab/src/syntax.rs`: `bd0589314de817c4801273f8ce03e60fe096574745dcc6876ed0b4d3c578941b`
- `crates/briefing-lab/src/explicit_structure.rs`: `6f3d5ad89bd051f61a75a040d3266d775c51429a85565a97513368263312c36b`
- `bench/jev-lifecycle/spans.py`: `304ea0d77911c993555a431e2148c7e6fbb627c394ea75726fe4d46bb4a900dc`
