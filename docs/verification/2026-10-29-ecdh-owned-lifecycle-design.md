# ECDH owned scratch lifecycle: design in progress

This is a design sketch, not an implemented or accepted API. The existing
`coax_ecdh_raw` and exported `ecdh_stage` retain their current contracts.
The former refuses malformed metadata before workspace ownership, leaving
prefilled caller storage unchanged; an injected erase failure may leave work
dirty and returns cleanup pending. The latter is a lower-level arithmetic
stage and does not own or wipe caller work. Repository source search finds no
non-test caller of the host wrapper, but does not establish that no external
caller uses either export. Neither surface can be silently treated as retired.

A versioned owner could provide a bounded private slot and a separate result
staging region. Initialization must complete before secret input is accepted.
An opaque handle identifies the owner, slot and generation; generation
exhaustion must refuse, never wrap to an old live handle. All transitions,
including cancellation, retry, close and callbacks, must serialize. Unknown,
stale or cross-owner handles must not touch any live generation. A copied or
guessed currently valid handle is indistinguishable from its owner, so the
host must enforce authorization or treat exclusive possession as a caller
premise. Capacity exhaustion must fail before accepting a secret. Slots cannot be reused, freed
or destroyed while any owned region remains dirty or a wipe is pending.

For a successful operation, work must be wiped before releasing a staged
result. To complete transfer, the owner validates a live, exclusive,
nonoverlapping caller output, copies while that output remains private, then
erases staging before returning success. The dirty-output transition must
precede the first copied byte, including a partial-copy failure. The ownership
transfer that marks output published must be serialized and infallible, or a
separate publish-pending state must retain the cleanup obligation. A failed staging erase quarantines
the same generation and the already-written output until all required wipes
finish; it cannot become an ordinary success or failure. Invalid scalar,
invalid peer, cancellation and partial computation take the error path only
after all owned secret-bearing regions are wiped. A failed wipe remains an
explicit incomplete state bound to the same generation and retains every
necessary region for retry. Early metadata refusal must either occur before
any secret-bearing slot exists or leave that slot quarantined, never claim a
completed wipe. A caller's original key remains caller-owned.

Numeric pointer checks alone cannot establish real allocation, exclusivity or
lifetime; those are host/caller premises, and concurrent caller mutation must
be excluded. Actual erasure depends on the effect implementation and valid
storage. Crash, process termination, inaccessible memory, hidden compiler,
register or ABI copies, target timing and production-secret approval remain
separate. A reference policy, host implementation, all exposed return paths,
source-to-effect correspondence and adversarial failure/cancellation races
require independent review and tests. A new interface alone does not discharge
the literal ECDH task while a legacy externally accessible path retains its
contradictory error semantics.
