# Corrected structured development preview

The corrected `ExplicitStructureV1` preview includes the complete fault
declaration, two specification sections, and one complete nearby test with
partial fixture context for development issue #10166. Its **15,045-byte**
optional payload fits the 16 KiB bound. This is a preparation and retrieval
check; it provides no model success, cost, or repair-quality result.

The exact [payload](treatment.md) and [measurement](preparation.json) were
reviewed for private host paths and copied without changing their bytes.
The measurement records input, index, binary, script, and payload hashes,
selection provenance, and all 15 retained coverage warnings. Source is pinned
to `4705102273140a5f381fb75e17a29965662629c8`. No checker, model patch, heldout
task, or reserve task informed this review.

## What is included

All eight rendered source ranges match the pinned files byte for byte.
Their combined source size is **13,035 bytes**; headers, fences, provenance,
and coverage text account for the remaining 2,010 bytes.

| Pinned source | Lines | Bytes | Coverage |
| --- | --- | ---: | --- |
| [`git.rs`](https://github.com/OpenAgentsInc/openagents/blob/4705102273140a5f381fb75e17a29965662629c8/crates/background/src/git.rs#L4) | 4–41; 61–118 | 3,380 | Imports, `Undo`, the `git` helper, and the complete `removable` declaration with its documentation. The issue's 75–78 anchor is included. |
| [Background specification](https://github.com/OpenAgentsInc/openagents/blob/4705102273140a5f381fb75e17a29965662629c8/docs/background/2026-10-02-background-processes.md#L158) | 158–195; 219–239 | 4,823 | Complete “Safety” and “Candidate classes, in order” sections, including the unsaved-work and undo requirements. |
| [`tests.rs`](https://github.com/OpenAgentsInc/openagents/blob/4705102273140a5f381fb75e17a29965662629c8/crates/background/src/tests.rs#L409) | 4–72; 89–133; 239–256; 409–429 | 4,832 | Imports; complete `Fixed`, `Idle`, and `Home` declarations and implementations; `low`, `target`, `age`, and `git` helpers; and one complete attributed test. |

The selected test is
`a_checkout_target_needs_cachedir_tag_and_git_ignore` (409–429). It checks
that cleanup removes a tagged, ignored Cargo target and preserves an untagged
target. It provides related cache-policy context. Selection used the recorded
**scope-limited lexical fallback**; no direct call from this test to
`git::removable` was established. Default lexical and symbol ranking was
enabled within the admitted files. Only the two explicitly named files and
the owning package's `tests.rs` entered the candidate pool.

## Coverage limits

The retained declarations are complete, but the fixture dependency set is
incomplete. The selected test calls `env` at line 424; the helper at 74–87 is
absent. The packer's conservative shadowing check treats `let env = env(...)`
as potentially ambiguous, although the new binding is not in scope in its
initializer. The rendered payload reports this unresolved call. This is a
known dependency-recall defect, not evidence that every fixture helper was
included.

The existing worktree cleanup and undo test
`unsaved_worktrees_stay_and_a_clean_pushed_one_goes_and_comes_back` (300–346)
and its `repo` fixture (258–298, including its documentation) are absent.
They would provide closer worktree-lifecycle context than the selected
cache-target test. The policy admits at most one automatically ranked test;
ten other ranked tests were omitted. The two-section document bound also
omitted 19 ranked sections. The separate “What it never touches” section at
240–252 is absent.

Cross-file definitions, method dispatch, macros, and other unresolved calls
remain outside the syntax dependency expansion. The test excerpts are useful
source context, not a standalone compilable test bundle. Complete mandatory
instructions and the original issue must be supplied separately and equally
to both experiment arms; this artifact does not verify that injection.

## Comparison with the initial preview

The [initial preview](../development-preparation-initial/README.md) is retained
as development failure evidence. Documentation incorrectly activated the root
virtual workspace as a package scope, admitting unrelated benchmark tests.
The 24-file read bound then excluded the actual package test file. The
correction verifies a real `[package]` table and derives automatic test scopes
only from explicit Rust or Cargo paths. Documents cannot activate a scope.
Rendered warning details are now capped, with full bounded records retained
in the measurement.

| Measure | Initial preview | Corrected preview |
| --- | ---: | ---: |
| Optional payload | 16,223 B | 15,045 B |
| Source excerpts | 8,203 B | 13,035 B |
| Other payload bytes | 8,020 B | 2,010 B |
| Selected test declarations | 0 | 1 |
| Warm preview wall time | 225.925 ms | 231.668 ms |
| Fresh index wall time | 1,943.755 ms | 1,692.405 ms |

These are individual preparation observations, not latency distributions.
Warm preview timing includes process startup and output writes. The corrected
preview's internal assembly stage took 30.183 ms. Building a fresh index is
measured separately and exceeds one second. “Fresh” means a new index artifact;
operating-system page caches were not cleared, and the warm preview ran after
that build. Neither preparation made a model call, ran a task command, or
appended an external Git probe.

The new binary also reproduced the frozen V2 `--focused --no-lexical
--no-symbols` payload byte for byte. Both copies have SHA-256
`0ebc6e55dc2944420bdcd18e41840a0dc09d06863acc35b3aa7b0c41da17bb9a`.
That check applies to this development input; it is not a universal behavioral
proof. This corrected payload has SHA-256
`291cdd81af50e275b316fc7a3f3d7946d0edfdf2ead8b1e1929625b8063d3c9e`.

The measured policy is preserved with these coverage limits. The source and
preparation scripts were not tuned after this output review.
