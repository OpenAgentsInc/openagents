<!-- openagents-audit-2026-09-19:trace -->

Audit follow-up: A13. Source snapshot: `1843fa6c18a05537bf2b022f69361a9ba3ef12a1`.

[Audit and evidence](https://github.com/OpenAgentsInc/openagents/tree/main/docs/audits/2026-09-19-codebase-audit).

## Problem and evidence

### A13: Recover the valid prefix of a torn trace

The [ATIF reader](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/atif/src/log.rs#L231) skips malformed JSON lines,
but `BufRead::lines()` fails before parsing when a final record ends in partial
UTF-8. A valid session and step followed by a torn two-byte character cause the
whole read to return `InvalidData` in the harness.

Read newline-delimited bytes first, then decode each record under an explicit
recovery policy. Recover a torn final record for interactive viewing while
reporting the damage. Let benchmark ingestion demand stronger integrity rather
than silently treating recovered data as complete. Also reject or report repeated
session headers, records after an end record, and malformed interior records.
Test truncation at every byte of a non-ASCII final record.

## Reproduction

Use the retained [Rust harness and run instructions](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-09-19-codebase-audit/verification.md#focused-reproductions). It uses loopback mocks, a private temporary ledger, and harmless temporary markers; it needs no real credential or paid model call. The matching finding names the observed failure.

## Acceptance

- [ ] Parse newline-delimited bytes before UTF-8 decoding so a torn final multibyte record preserves the valid prefix for an interactive reader.
- [ ] Distinguish tolerant recovery for viewing from strict integrity required for evidence and grading; report unreadable records and interruption explicitly.
- [ ] Specify and enforce session lifecycle rules for repeated headers/end records, steps after end, append after closure, and interior corruption.
- [ ] Property or table-driven tests truncate a non-ASCII final record at every byte and verify the retained prefix and damage metadata.
- [ ] Strict CoderBench ingestion rejects or marks unverifiable damaged/incomplete traces and cannot convert recovery into a successful grade.
- [ ] Keep the retained transcript archive unchanged and preserve compatibility with valid ATIF fixtures.

## Dependencies and scope

Consumer integration is part of A04; coordinate schema or recovery-policy changes rather than independently teaching the grader to ignore damage.
