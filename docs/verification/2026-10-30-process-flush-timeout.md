# Bounded child-input flush, 2026-10-30

`cxproc_flush_timeout(handle, timeout_ms)` is an additional native process
effect for the Chapter 26 private witness pipe. Coax chooses the checker,
arguments, admission and budget. C performs owned pipe I/O and buffering.
The existing `cxproc_flush` source body is unchanged.

The new operation accepts a started library handle whose stdin is a writable
FIFO on fd >= 3. It returns 0 on completion, -2 on timeout, -3 on error. Negative
and larger-than-INT_MAX budgets refuse; zero permits one nonblocking write.
One monotonic deadline spans partial writes, poll waits and interruptions.
A timeout/error preserves only unsent bytes; retry or appending bytes cannot
repeat a prefix already delivered to the pipe. Consumed buffer bytes are
retired, and public mutable file-status flags are restored. OS-maintained
status bits after writing are compared against the existing legacy write.
A noisy checker’s stderr is drained in bounded pieces into the existing
ring, without forwarding opaque output or changing the total deadline.

Checks were written first: the initial fixture failed because the effect did
not exist. The final exact fixture passes on Mac Clang and Linux GCC 15.2
with ASan/UBSan and -Werror. It exercises actual child processes and kernel
pipes: a stalled reader, partial timeout/retry with appended data, zero
budgets, repeated EINTR, preexisting nonblocking state, closed/read-only/
regular/standard-stream refusal, opaque noisy stderr and owned fd/reaping
cleanup. A parent alarm bounds even a broken test implementation. GCC’s
fixture indentation warnings were fixed; no warnings were suppressed.

```sh
coax/tests/process/flush_timeout.sh
```

[Exact input hashes and logs](data/process-flush-timeout-2026-10-30/) bind
both platform results. Linux hashes were compared to the local final inputs.
No passing compiler/crypto/model gate was repeated. These are native effect
tests, not a claim of memory secrecy for all callers or of Coax authority
policy verification; Chapter 26 owns the private-witness policy.
