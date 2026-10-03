# Retrospective deep-import diagnostic

The replacement Sonnet briefing candidate misses a notification for a valid deep import. The historical reference and all eight effective Opus and Sonnet control candidates pass the same check. This is a separate quality result; the frozen five-case scores and registered cost comparisons remain unchanged.

## Test and selection

This check was written after source review of replacement run 13 exposed a new directory scan with a depth-four cutoff. Its selection is retrospective. Each of ten revisions ran once: the historical reference, replacement run 13, and all four effective candidates from each control arm. No model reruns, repairs, formatting changes, or broader test suites ran.

The fixture reads an initial catalog, waits for quietness, and atomically imports `sessions/deep-import/one/two/three/four/chat.jsonl`. All child writes finish outside the watched root. It first saves notification success or timeout under the original four-second deadline. It then unconditionally performs a fresh catalog read, proves the chat is visible, and only then asserts the saved notification result. The read cannot change coalescing state before that observation.

The source reader supports this path: it has seven components, below the pinned catalog’s 16-component bound. Replacement run 13 returns before reading the directory at depth four and has no fallback notification when the scan is truncated.

## Results

| Revision | Notification | Fresh catalog visibility | Check wall time, including compilation |
| --- | --- | --- | ---: |
| [Historical reference](logs/historical-reference.log) | Pass | Pass | 7.337 s |
| [13-D-sonnet-treatment](logs/13-D-sonnet-treatment.log) | Timed out at four seconds | Pass | 6.429 s |
| [1-A-opus-control](logs/1-A-opus-control.log) | Pass | Pass | 2.119 s |
| [7-A-opus-control](logs/7-A-opus-control.log) | Pass | Pass | 2.219 s |
| [12-A-opus-control](logs/12-A-opus-control.log) | Pass | Pass | 2.575 s |
| [14-A-opus-control](logs/14-A-opus-control.log) | Pass | Pass | 2.420 s |
| [4-C-sonnet-control](logs/4-C-sonnet-control.log) | Pass | Pass | 2.269 s |
| [6-C-sonnet-control](logs/6-C-sonnet-control.log) | Pass | Pass | 2.218 s |
| [9-C-sonnet-control](logs/9-C-sonnet-control.log) | Pass | Pass | 2.219 s |
| [15-C-sonnet-control](logs/15-C-sonnet-control.log) | Pass | Pass | 2.419 s |

The remote runner took 36.380 seconds in total, including its source export and all ten checks. It made zero model calls and incurred $0 in model charges. The runner did not measure sandbox billing; infrastructure cost remains unavailable rather than assumed zero. A missing credential environment variable blocked the first upload before any API request or trial; the authorized loader resolved it. All ten tests then executed once, with no environment failures or candidate retries.

## Interpretation and retained evidence

Replacement run 13 passed the frozen shallow-import cases but failed this deeper case at the notification assertion, after the fresh-read visibility assertion passed. That is a concrete limitation of this candidate. Passing one extra test does not prove that the control candidates are correct in every case or establish a general model comparison. The unexecuted extensionless-file concern in the original runtime-invalid run 16 remains in the [original review](../quality-review-original.md); it is a different finding.

The [test source](deep_import.rs), [narrow runner](run_depth_remote.py), [Cargo manifest template](Cargo.toml.template), sanitized logs, and [results with hashes](results.json) are retained. The runner uses the existing source-pin and exclusive-lock helper. It restores exact final candidate files on the pinned source, uses a scratch home and the cached Cargo target, and invokes only this test. No owner host is involved.

The selection was made after inspecting candidate code, and only one observation was collected per revision. Keep it separate from prospective acceptance and performance scores.
