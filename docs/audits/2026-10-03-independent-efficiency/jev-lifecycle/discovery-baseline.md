# Source discovery before semantic ranking

This separate development component changes candidate admission before any live
Jev response is inspected. It leaves the earlier frozen study unchanged. The
[baseline record](discovery-baseline.json) binds the assembler, tests, candidate
contexts, source revisions, and deterministic packs. It records zero model calls
and zero Cargo commands.

## Policy and interface

The [assembler](../../../../bench/jev-lifecycle/context.py) reads public task
fields and exact Git blobs without checking out the source. Explicit files and
named packages admit implementation and test files before lexical ranking.
Source-file anchors can admit their owning package; a root document does not
admit the whole workspace. Round-robin admission across implementation, test,
and document roles gives each role an opportunity before the pool fills.
Within each role, each file's strongest declaration precedes that file's
remaining declarations.

The default pool contains at most 32 candidates and 28 KiB of serialized public
task plus candidates. A separate catalog holds up to 64 file records with
omission reasons and read pointers. The catalog is additional state for a later
retrieval decision; it is not included in that initial 28 KiB figure. Assembly
reads at most 48 admitted files, and records all count and byte-budget omissions.

Complete declarations up to 6,000 bytes remain whole. Larger declarations become
explicitly partial, query-selected windows of complete lines up to 2,400 bytes,
with the full declaration's line range preserved. A single line that cannot fit
becomes a read pointer with no clipped source text. Partial evidence never claims
complete declaration or dependency coverage.

`assemble(repo, rev, index, task)` returns the candidate pool, catalog, provenance,
coverage counts, and measured assembly time. `pack(context, scores=None)` packs
that pool deterministically; supplying one finite score for every candidate ID
changes ranking over the same pool. Both use a maximum 16,384-byte payload,
including labels and provenance. `read_source(...)` returns a bounded exact-line
slice from the same pinned source. Complete applicable instructions are supplied
separately from this optional evidence.

The CLI takes `--repo`, `--rev`, `--index`, `--task-manifest`, optional `--task-id`,
and a new `--output` directory. It writes `context.json`, `baseline.md`, and
`baseline-pack.json`. The optional `--state-bytes` supports a separately declared
pool up to 64 KiB; these observations use the default 28 KiB.

## Observed coverage

These three tasks have already been inspected. They supply development feedback,
not unseen confirmation. The fourth proposed case, reserved A, remains unrun
because no retained local syntax index was located. No substitute index was
fabricated or rebuilt for this component.

| Task | Old / new candidates | New implementation / test / document units | New state bytes | Deterministic pack bytes | Local assembly seconds |
|---|---:|---:|---:|---:|---:|
| Alpha | 13 / 14 | 6 / 6 / 2 | 28,591 | 16,371 | 2.204 |
| Beta | 16 / 11 | 6 / 4 / 1 | 28,535 | 14,702 | 1.537 |
| Gamma | 14 / 13 | 6 / 5 / 2 | 28,400 | 16,238 | 2.291 |

The result is partial recovery, with remaining omissions:

- **Alpha:** `Host::execute` now appears as a labeled partial declaration in the
  pool and deterministic pack. `Host::request_enrollment` remains present.
  `Host::redeem` and `Host::handle_with_clock`, which were available to the old
  ranker, are absent from the new pool. Role balance trades off other evidence
  under the same state ceiling.
- **Beta:** the ATIF `read` implementation enters the pool, so a semantic ranker
  can now select it. The deterministic pack still omits it. `Log::append` and
  `Log::finish` remain absent from the pool.
- **Gamma:** the pool now includes six implementation units from six SDK source
  files. The old pool contained no unit from `jev/src`. However, the named
  `SystemOneResponse::decode`, `decode_answer`, `Client::system_one`, and
  `BlockingClient::system_one` functions remain absent. Package admission alone
  has not established the intended behavior coverage.

The source index contains the named omitted functions. All three new pools hit
the serialized state budget, leaving 393, 259, and 397 eligible units out,
respectively. The omission catalog provides a bounded opportunity to recover
additional evidence. It does not guarantee that its suggested read window covers
the right function.

## Verification and timing limits

Twelve synthetic tests pass in 15.608 seconds. They cover package and file-anchor
admission, document scope, implementation/test roles, dirty-worktree exclusion,
path/blob and content mismatches, complete-line slices, oversized-line pointers,
shared pack limits, candidate identity, deterministic ordering, and explicit
omission accounting. No Rust build or model call is part of those tests.

The measurements start inside `assemble`, after input JSON loading. They include
Git reads, source validation, candidate selection, and canonical index hashing.
They exclude Python startup, index construction, input loading, packing, and
output writes. Packing has separate timings in the JSON. These are first local
observations, not a controlled cold-cache benchmark or a matched speed comparison
with the prior remote previews.

Candidate presence is a limited coverage diagnostic. No native coding trial,
semantic ranking benefit, complete dependency recall, or net efficiency win is
established here. The implementation and these contexts were frozen before live
responses; subsequent semantic outputs do not change this baseline.
