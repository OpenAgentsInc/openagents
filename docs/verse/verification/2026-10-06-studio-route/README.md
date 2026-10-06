# Declared Studio route acceptance, 2026-10-06

The [scripted receipt](simulated.json) passed against source commit
`2816ea673709c00545f4a3e0c124cf8d61beb7e6`. Five declared routes reached the
existing scratch host: goal submission, decision answer, review read, stale
merge refusal, and valid local merge. Each retained result projected in the
shared workbench. Exact repeats returned the same result. The scratch origin
stayed unchanged, and all five created tasks were archived.

The receipt includes each immutable admission, placement binding, typed
operation, request ID, host result, and workbench rows. Their canonical digests
were recomputed and matched, as did the original artifact manifest and ATIF
trace hashes. The shared opening, reopen, and reconnect checks also passed.

This is a headless semantic capture with a scripted engine, no model call, and
no inference spend. It took 914 ms. It used a temporary HOME, root, task store,
repository, keys, and socket, with only PATH, HOME, and TMPDIR in the child
environment. Limits were 32 scripted passes, 10 seconds per socket request,
and a 90-second outer deadline. Native and separately granted real-engine
qualification remain unverified in [NEEDS_OWNER.md](../../../../NEEDS_OWNER.md).

## Reproduce

Use the pinned toolchain and the external target directory and isolated runner
in the [shared-sheet receipt](../2026-10-06-studio-workbench/README.md). Add
`--routed` as the last argument to `studio-acceptance`, and select a new output
directory. Omitting that flag exercises the existing direct-intent path.

Validation: 168 chat unit tests passed (one existing test ignored), 51
route-contract tests passed, and nine studio-adapter tests passed. These
include acknowledgment loss across journal reopening, exclusive writer
locking, conflicting request identity, missing decision rights, shell rights
without merge authority, and restart refusal. Verse and terminal-app consumer
checks passed with existing unrelated warnings. Formatting and diff checks
passed. No release gate or real engine was run.
