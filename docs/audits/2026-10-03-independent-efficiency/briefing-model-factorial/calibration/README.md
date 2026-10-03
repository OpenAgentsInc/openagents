# Historical task calibration

The final five-case checker distinguishes the historical base from its fix in
all **11 observations per revision**. Each observation passes 3 of 5 cases on
the base and all 5 on the reference. The two base failures consistently time
out waiting for a catalog notification after importing a populated directory.
These are Linux filesystem and loopback integration checks; scheduling and
native notifications participate.

## Explicit historical test exclusion

The final verifier excludes exactly
`tests::strict_destinations_schemas_and_private_store_permissions` from the
package's own-test command. The exclusion applies to calibration and every
scored arm. All other package tests and all five independent acceptance cases
remain unchanged. The [one-line verifier diff](verifier-exclusion.diff) is
retained beside the [original verifier](verify_remote.py) and
[final verifier](verify_reserve_remote.py).

Before this amendment, the base package suite failed that permission test and
the original notification test: 41 passed, 2 failed, and 5 ignored. The same
permission test passed all three isolated binary runs; the unchanged reference
suite passed 43 tests with 5 ignored, plus 2 integration tests. Ten scratch
probes changed a file from mode `0600` to `0644` without changing `ctime_ns`.
The historical validation cache uses a file stamp that omits mode and includes
ctime. These observations are consistent with a timestamp-sensitive historical
failure outside the notification task. One successful reference run does not
establish the permission test's reliability.

The exclusion was registered before running the final calibration and before
any model call for this task. The earlier observations remain in
[summary.json](summary.json) and [setup-diagnostics.json](setup-diagnostics.json).

## Final checks

| Check | Base | Reference |
| --- | ---: | ---: |
| Package unit tests | 42 passed | 42 passed |
| Additional integration tests | 2 passed | 2 passed |
| Ignored package tests | 5 | 5 |
| Explicitly filtered package test | 1 | 1 |
| Formatting | Passed | Passed |
| First independent acceptance run | 3 passed, 2 failed | 5 passed |
| Ten retained-binary repetitions | 3 passed, 2 failed each | 5 passed each |

The base's original notification test passed during the final package gate,
although it failed in the earlier unamended gate. The independent checker
consistently reproduces the missing notification without depending on a child
file write after the directory enters the watched root.

The source pins are:

- Base: `aeb7f9fb19e0d56c13400702894172ae472e3bcf`.
- Reference: `fd305b01df5add38e2bd0ef3f862ee85cc9950c3`.

Each exact acceptance executable was copied and hashed before compiling the
other revision. Repetitions alternated base and reference, ran sequentially
without rebuilding, and used a fresh scratch home. Every assertion outcome and
sanitized log is retained. These repetitions check reliability in this sandbox;
they do not establish reliability on every filesystem or loaded machine.

## Setup and provenance

The first offline attempt could not download `data-encoding v2.11.1`; neither
test suite ran. The dependency cache was then populated and that failed attempt
was retained. A later HTTP 502 interrupted polling of reference repetition 2.
The original process result was recovered and only unstarted repetitions were
launched. Another HTTP 502 interrupted polling of the scored-source export and
was recovered the same way. No completed observation or export was relaunched.

The [amendment chain](amendments.json) records the move from a private-symbol
checker to public APIs, the change from repeated full builds to retained
executables, and the exact historical test exclusion. No acceptance cases were
added after task model calls.

Before handoff, read-back hashes matched the frozen remote checker and final
verifier. The scored source tree matched all 25,962 pinned Git blobs and modes,
with no extra files, reference changes, or inserted calibration tests. The
[provenance file](provenance.json) binds private originals by digest. Published
logs replace machine and scratch paths with role placeholders; the checker and
verifier copies preserve their exact bytes.
