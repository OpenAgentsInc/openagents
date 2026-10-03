# Static review: 53b718d2-bdc7-414a-9be9-d8d808571c40

One material malformed-record normalization gap is visible statically. No existing assertions were weakened.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. Candidate and checker code were not executed. No Cargo, models, or remote jobs. No plan, arm mapping, native transcript, run result, or check result inspected. Review completed during the unchanged panel; not supplied to executors.

Candidate manifest: `f84dffa1084e0a773661ba0cc1db63144703fcaec5b958e590563ce4c31202cd`. Source: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`.

## P2: Whitespace trimming hides a malformed interior JSON record

`crates/atif/src/log.rs:349`. Classification: `demonstrated_static_contract_gap`.

The reader applies str::trim to every decoded line before serde_json parses it. Rust whitespace trimming includes form feed (0x0C), while JSON whitespace includes only space, tab, LF, and CR. This turns a nonempty invalid JSON record into a valid one without incrementing unreadable_lines.

Prepend byte 0x0C to a valid interior step JSON object in an otherwise clean closed trace, leaving a later step and end record intact.

The malformed prefix is removed. The same Session and Steps are retained, unreadable_lines remains zero, violations stays empty, and verify() succeeds (295–313). coderbench::observe copies those clean counters (1373–1379), and judge_record has no damage to reject (1280–1293). A trace that otherwise passes therefore keeps its successful grade. The pinned reader parsed untrimmed nonempty lines and would have counted this record unreadable.

The task requires malformed interior records to remain visible as damage, and tolerant recovery must not become successful strict benchmark evidence.

Static source inference only; no candidate or diagnostic execution.

## Existing tests and scope

No existing tests or assertions were removed or relaxed. Four ATIF tests and one consumer test are added. The byte-cut loop checks each interior cut of a non-ASCII final record. The invalid-UTF-8 interior test retains a valid end after the corruption. Lifecycle tests include before-session, repeated headers/ends, records after end, and writer append after closure.

Three modified files are within the allowed atif and coderbench crates. Reader/writer lifecycle, consumer faults, and tests are task-related. No dependencies, unrelated edits, deletions, or mode changes.

## Limits

This finding is specific to normalizing non-JSON whitespace before parsing. It does not imply ordinary invalid UTF-8 or malformed JSON recovery fails. The reader deliberately accepts a complete JSON final record without a newline; that documented choice is not treated as a separate defect. Frozen acceptance scores are unchanged.
