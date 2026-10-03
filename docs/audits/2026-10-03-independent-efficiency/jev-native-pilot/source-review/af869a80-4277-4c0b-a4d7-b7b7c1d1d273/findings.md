# Static review: 9bafab36-254c-4d91-a4b6-0b80fe68a5fd

One material malformed-record normalization gap is visible statically. Existing assertions are unchanged.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diff and affected control flow inspected. No candidate code, tests, Cargo, models, or remote jobs executed. No plan, arm mapping, native transcript, run result, or check result inspected. Review completed during the unchanged panel; not supplied to executors.

A second reviewer independently inspected this anonymized packet and pinned public source, without outcomes.

Candidate manifest: `f6cc0df83387e786adb3a41f9225f9bccf75d2df9355473f0dd109bac5992007`. Source: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`.

## P2: Trimming hides a malformed interior JSON record

`crates/atif/src/log.rs:273`. Classification: `demonstrated_static_contract_gap`.

The reader calls str::trim before passing the line to serde_json. Rust whitespace trimming removes form feed (0x0C), which is not valid JSON whitespace. This can normalize a malformed nonempty record into valid JSON without marking any unreadable line or lifecycle violation.

Prepend byte 0x0C to a valid interior step in a clean closed trace, retaining a later valid step and end.

The reader strips the damage, keeps the same step, and leaves unreadable_lines and lifecycle_violations at zero. coderbench::observe copies those counters (lib.rs:1374–1379); judge_record faults only when they are nonzero (1280–1292), so otherwise passing evidence can still pass. The pinned reader parsed the untrimmed nonempty record and counted it unreadable.

The task requires malformed interior damage to remain visible and prevents tolerant recovery from becoming successful strict benchmark evidence.

Static inference only; not executed.

## Existing tests and scope

All existing tests and assertions are unchanged. Three ATIF tests are added for every byte cut before completion of a non-ASCII final record, lifecycle violations after closure and writer append refusal, and malformed interior JSON/UTF-8 followed by a valid end. No new consumer regression test is added.

Only atif/src/log.rs and coderbench/src/lib.rs change, within the allowed crates. No dependencies, unrelated edits, deletions, additions, or mode changes.

## Limits

Steps before the session are counted as lifecycle violations, so this candidate does not have the earlier missing-before-header grading gap. Its new comment says forbidden records are dropped, but a pre-header step is still retained after incrementing the counter (log.rs:315–320); this is a documentation mismatch, not a second material grading defect. The form-feed finding does not imply all malformed or invalid-UTF-8 cases fail. Frozen primary acceptance scores remain unchanged.
