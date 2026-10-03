# Blind static patch review: b82d458b-c7a2-4c47-beae-6b51b6497309

Candidate manifest: `28219e0318d60a4c7280fa7145677c172b216b86a6f302b923adcd7d9165316f`. Source: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`.

No plan, arm mapping, native transcript, run result, or check result inspected. Review completed during the unchanged panel; not supplied to executors.

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. No Cargo, model, test, or remote execution.

## Conclusion

One material incomplete lifecycle check is demonstrated by source control flow.

## B1: A step before its session header remains verified evidence

Classification: demonstrated_static_contract_gap (P2). Candidate `crates/atif/src/log.rs`, lines 344–350.

The step branch unconditionally pushes a parsed Step without checking opened.is_none(). The only lifecycle counters cover repeated headers, repeated ends and records after end (208–226). verified() checks ended, unreadable_lines and those counters (265–271).

Take a clean session/step/end trace and move its first step before the session header, leaving the step content and later step order unchanged.

The reader retains the same step, later admits the header, and reads the end. unreadable_lines and lifecycle.violations() remain zero, so verified() returns true. observe() copies these same counters (coderbench/src/lib.rs:1373–1388), and judge_record only rejects their nonzero values (1278–1293). A clean trace’s grade is therefore unchanged by this invalid header ordering.

The pinned base log format states that the session record comes first (base crates/atif/src/log.rs:28). The candidate itself claims to validate the order session, step*, end (244) and that verified evidence has records in order (265–267). The public task requires lifecycle integrity and strict handling of damaged evidence.

Static inference only; no counterexample or Cargo test was run.


## Tests and scope

No existing tests or assertions were removed or relaxed. Four new ATIF tests cover byte truncation, a malformed interior JSON record, several post-end violations, and append after finish. No consumer regression test was added in this patch. The new lifecycle test’s duplicate header is after the end; it does not cover a step before the first header.

Two modified source files within the allowed crates. Changes are focused on log reading/writing and consumer integrity state. No unrelated edits, dependency changes, deletions, or mode changes.

## Limits

The issue text enumerates several lifecycle cases without exhaustively listing all record orders; this finding is grounded in the existing public format and the candidate’s new verified() contract, not a demand to copy a reference implementation. Parseable final records without LF are deliberately accepted; that is not marked a defect.
