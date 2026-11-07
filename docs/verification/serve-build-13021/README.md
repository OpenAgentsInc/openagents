# Responsive builds and socket ownership (#13021)

The server's first build and watched rebuilds now run through a nonblocking
pipe in its existing poll loop. While a builder is held, HTTP returns the
existing file bytes and the live WebSocket remains usable. Only a zero exit
status broadcasts `wasm`; failure broadcasts the retained escaped log. The
server drains excess output after its 8 KiB log fills, with a 64 KiB per-turn
budget, so neither a full pipe nor a noisy builder blocks the HTTP loop.
`waitpid(WNOHANG)` reaps build, shaped-response and tunnel children without
discarding build exit status.

Every listener, accepted client, API socket and build pipe is marked
`FD_CLOEXEC` before the next fork. The server is single-threaded, so a build
cannot race between descriptor creation and that mark. The shaped-response
child explicitly releases the build read pipe too. A failed descriptor setup
closes the descriptor rather than leaving it inheritable.

Generated watched files retain their previous rebaseline behaviour, now before
completion is broadcast. This avoids rebuilding recursively from generated
output and preserves an edit made in response to a completion event. This is
not a transactional snapshot of arbitrary build-script writes: builders still
own how they publish files, and already-running clients retain their engine
until a successful build notification.

## Physical checks (2026-11-07, macOS)

The new Coax regression was written and run before the server change. The
original server failed `HTTP during held initial build` (`original-red.txt`).
The fixture is another invocation of the same Coax tool, not a shell script:
it writes 128 KiB (larger than the pipe and retained log), then waits for the
test to release it and maintains a heartbeat. Every HTTP read has a 700 ms
deadline; fixture startup and notification waits have five-second deadlines.
The fixture gives up after ten seconds. Temporary ports and process groups
belong exclusively to the check; it opens no browser.

The optimized fortified C build passes with `-Wall -Wextra -Werror`.
The actual AddressSanitizer/UndefinedBehaviorSanitizer build passes the same
physical test (`sanitized.txt`):

- Existing HTTP bytes and a real WebSocket handshake succeed during the held
  initial build; release produces a success notification.
- A watched edit starts a held rebuild while HTTP remains responsive.
  Nonzero completion sends a parseable error with the compiler-log prefix and
  leaves the existing file response intact. Oversized output cannot deadlock.
- Another edit starts another build. After terminating only the server,
  the builder's heartbeat continues, the same port can bind immediately, and
  the previously accepted WebSocket reaches EOF.

Two deliberate native negative controls independently test the descriptor
assertions (`listener-negative.txt`, `accepted-negative.txt`). Removing only
the listener's mark fails port reuse while WebSocket EOF still passes;
removing only the accepted client's mark fails EOF while port reuse passes.
Both controls still pass the responsiveness checks. These temporary altered
binaries are diagnostic only and are not committed.

The existing independent HTTP oracle passes byte ranges, 416 responses,
latency, simultaneous shaped transfers, API forwarding and invalid argument
refusal (`network.txt`). It was run before the final notification-order-only
cleanup; those unchanged HTTP paths were not redundantly retested.

```sh
# Build the server with serve.sh (or its documented compiler invocation).
coax/web/tool.sh serve-build-check
coax/web/tool.sh serve-build-check /path/to/sanitized-serve
python3 coax/web/tests/serve_network.py
coax/web/tool.sh tools-check
```

The change is POSIX SDK process/socket handling in the existing C server;
the new regression orchestration is Coax. There is no new admission policy
or renderer change. Linux compilation/runtime was not tested on this Mac.
