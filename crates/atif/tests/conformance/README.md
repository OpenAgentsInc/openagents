# The conformance corpus

Each `*.json` here is one case that `../conformance.rs` replays against the
crate. The `expect` block was recorded from the implementation and is what the
run must reproduce; a recorded diff is a behavior change, reviewed before it
lands.

A read case carries the log's own bytes:

- `log`: one string per line, joined with newlines. `trailing_newline: false`
  leaves the last line unterminated, which is what a write cut mid-record
  looks like.
- `log_hex`: the whole file as hex, for cases whose bytes are not UTF-8.

A write case carries `write`: a `session`, its `steps`, and an `end` state
(omitted for an interrupted session). The test runs them through
`Log::create_at`, `append`, and `finish`, then reads the log back.

`expect` records what `read` reports: `read_error`, `whole`, `whole_ok`
(whether `read_whole` admits it), `state`, `seconds`, `schema_version`,
`trajectory_id`, `unreadable_lines`, `faults`, `step_count`, `steps_digest`,
and the rendered `document` minus the volatile `exported_at` and
`source_path`. A write case records no `seconds` and its document is compared
without `final_metrics.extra.wall_seconds`: the writer stamps both with its
own clock.

Regenerate expectations after an intentional behavior change:

```sh
ATIF_CONFORMANCE=write cargo test -p atif --test conformance
```

Review the diff: recording is how a change lands, not how it passes.
