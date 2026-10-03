# Blind static patch review: e5fbf039-d10a-42a6-83c7-abcea67ff2b8

Candidate manifest: `a4d93bec527aae1138fc6cdfc11b24727b950a7dc93a4aa8f46acba95be546fd`. Source: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`.

No plan, arm mapping, native transcript, run result, or check result inspected. Review completed during the unchanged panel; not supplied to executors.

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. No Cargo, model, test, or remote execution.

## Conclusion

One material malformed-record normalization gap and one new-test coverage gap are visible statically.

## E1: ASCII trimming hides form-feed corruption before strict JSON validation

Classification: demonstrated_static_contract_gap (P2). Candidate `crates/atif/src/log.rs`, lines 350–357.

The reader replaces each raw line with raw.trim_ascii() before UTF-8 and serde_json parsing. ASCII trimming includes form feed (0x0C), whereas JSON whitespace is only space, tab, LF, and CR. Thus this transformation can turn a malformed record into a valid object without incrementing unreadable_lines or recording any lifecycle violation.

Insert byte 0x0C immediately before a valid interior step’s JSON object in an otherwise clean, closed trace.

The form-feed byte is stripped. The reader returns the same Session and Steps, with unreadable_lines=0, no lifecycle entries, and an intact recording. The consumer propagates only those counters (coderbench/src/lib.rs:1370–1383), so a clean trace’s grade remains unchanged despite the malformed interior record. The pinned reader previously passed the untrimmed nonempty line to serde_json, which would count it unreadable.

The task requires visible unreadable records and strict handling of malformed interior evidence. Tolerant recovery may salvage source, but cannot silently turn the damage into clean benchmark evidence.

Static inference only. ASCII-trim semantics and serde_json whitespace matching were inspected in locally installed library source; no candidate or checker code was executed.


## E2: The new invalid-UTF-8 interior test has no valid record after the corruption

Classification: test_coverage_gap (P3). Candidate `crates/atif/src/log.rs`, lines 700–710.

The test appends the corrupt record after whole_log("one"), then appends whole_log("two")[..0], an empty slice (704). It checks that preceding records survive, but does not test resumption at a later valid record.

The test name and comment claim an interior corruption case. This is a weaker new test than its name suggests, not a removed existing assertion or proof that actual interior recovery fails.

Static test-source review only.


## Tests and scope

No existing tests or assertions were removed or relaxed. Four new ATIF tests and two consumer tests are added. The byte-cut test checks recovery and strict rejection, and a separate case accepts a complete JSON record with only its newline absent. The consumer regressions cover a repeated end and a torn multibyte suffix.

Three modified files within the allowed crates. Reader/writer lifecycle rules, consumer faults, and tests are task-related. No unrelated edits, dependency changes, deletions, or mode changes.

## Limits

The form-feed finding concerns a specific invalid JSON normalization path; it does not imply that the ordinary malformed-JSON or invalid-UTF-8 cases fail. The source-only finding does not change frozen acceptance scores.
