# Partial native FI HTTPS transport evidence

The `fi_fetch.c` prototype and transport fixtures are uncommitted and excluded
from the partial local-verification commit; the prototype is disabled in normal builds. An
explicitly unsafe isolated-test flag enables its public-HTTPS byte transport;
it is not a provider or FI authorization. It is not wired to snapshot installation,
policy identity, claim validation, NIP-42 or session revalidation. Mesh and
browser transports are unavailable. A normal build refuses even if `FI_DNS_HELPER_PATH` is set. The isolated-test
build additionally requires this path to name an absolute trusted local helper;
an unset or relative path refuses. Its executable identity is not yet bound to the
future locally compiled policy ID, so this cannot be promoted to provider
trust. The adapter requires libcurl with the prerequest callback facility.

The test-only operation obtains a bounded resolver set through a dedicated helper,
uses compiled Coax URI and address admission for every returned answer, pins
the first permitted address, permits one socket, and checks `getpeername`
before HTTP request data. libcurl verifies the original TLS hostname and
certificate; proxy, redirect and connection reuse are disabled. Monotonic
operation deadline, a 524288-byte body cap and a 16384-byte response-header cap
are enforced; a failure publishes no body. There is one in-flight operation
per process. The supervisor exclusively owns its own resolver child and
kills/reaps it on deadline or closed parent pipe. The parent never signals a
potentially recycled PID, but its `waitpid` can still race an uncontrolled
host-wide reaper and reap a different child if the PID is reused. An independent
review identified this as blocking. The existing supervisor is not suitable
for integration until process identity and reaping are safely enforced. A polling caller observes unavailable at expiry;
cleanup assumes the operating system schedules the helper and can terminate
and reap processes. Catastrophic OS stalls or external termination of the
supervisor are not proven bounded. OS `getaddrinfo` is the enumerated resolver
interface; this is not evidence about DNS records the OS resolver omits.

`fi_fetch_native.py` passed in checked and release Coax policy modes on the
local macOS host with the test-only flag. The default-disabled build also
refused as intended. It injects resolver
records (but uses the real compiled Coax address classifier) for mixed
public/private, 17 answers, unsupported family, mapped IPv6, loopback, short
and mismatched sockaddr records, and a 30-second resolver stall. It confirms
that the resolver child is killed/reaped and the single slot becomes reusable.
Live direct TLS checks cover a successful public IPv4 endpoint, wrong-host and
expired certificates (with insecure-connectivity controls), redirect refusal,
poisoned proxy environment, 524288 and 524289 byte bodies, slow body deadline,
and short-output no-write. A live certificate endpoint timed out on one run
and passed on retry; these external checks are not deterministic and do not
replace controlled local endpoint tests. The same harness compiles and runs
socket-length checks and repeated timeout/cancellation including host SIGCHLD
auto-reap under native ASan/UBSan. `-Wall -Wextra -Werror` syntax checks passed
for the host C sources. No Wasm/browser, device or production deployment is
claimed. Independent review found the unresolved parent-reaping race; no transport or
provider approval is claimed. Apple describes launchd-managed XPC services as
an alternative macOS lifecycle boundary: https://developer.apple.com/documentation/xpc .
That option requires a real bundled service, peer authentication, cancellation
and deadline checks and integration tests; it is not implemented.

SHA-256 of tested files:

- `coax/lib/nostr/platform/fi_fetch.c`: `3a9b916b6aad5aa161f15135fadc4b55f4e8254e10cc412ab5d8509e0c0f9bc8`
- `coax/lib/nostr/platform/fi_fetch.h`: `243b81a2100bace3325a2e987fabb8d9f5e1503868865d079e47791dd0dbbbee`
- `coax/lib/nostr/platform/fi_dns_worker.c`: `bc36d2c45a95efde57360bebdc27be86c35f6686daab4b4ae3ed715d8152fe52`
- `coax/tests/nostr/fi_fetch_native.py`: `b56655fe6f948ff9528f89463c5d8a2e11735bae56601eaa8a981afe4b7d202d`
- `coax/tests/nostr/fi_fetch_resolver_fixture.c`: `776b221200f5857e9a49d8d6a821c2f448b19ab180ebf0d0d5fe7ab523671bff`
- `coax/tests/nostr/fi_fetch_sockaddr_fixture.c`: `f92de84aa75ed3ef6d7458a2ebb1bb828409d150f472f31e671cb649dd4a3a6c`
- `coax/tests/nostr/fi_fetch_sanitized.c`: `b21479657acbdc416d159d5995c8c438de020e27f218078f7a61d8dd16e7a8e5`

Independent safe-default review (`fixedbase_rescue`) inspected the current
`fi_fetch.c` hash above and found that the default guard returns unavailable
before URI parsing, slot acquisition, thread creation or spawn. This was a
source review, not approval of the test-only transport. The runtime test
compiles `fi_fetch_sockaddr_fixture.c` with a real absolute helper path and no
test flag and asserts that `fi_fetch_start` returns NULL in checked and release
Coax policy modes. The parent-waitpid race in the enabled prototype remains
unresolved.

`coax/tests/nostr/fi_xpc_probe.py` SHA-256
`979d8510b6c891acca7755be1cdb421660c0bbd9f178248639b197b24a0b2d41`
constructs a temporary headless macOS app bundle and ad-hoc-signed XPC echo
service. With host SIGCHLD auto-reaping, the client accepted its service's
pinned CDHash, refused a different CDHash, timed out a deliberately stalled
request, and observed the service exit after client exit. A separate run observed the
stalled service still alive 0.4 seconds after connection cancellation while
the client stayed alive. This is finite macOS lifecycle and peer-requirement
evidence, not an authenticated DNS/TLS broker or iOS support. A real service needs deployment-specific signing
identity, replay/binding, deadline, resource and failure tests before it could
be considered by a provider.

A second macOS-only scratch test,
`coax/tests/nostr/fi_xpc_fetch_probe.py` SHA-256
`9b5b2effd565491ac6aeee4780e30440fefefa834d3ed344eaf67e1f5ecd82fd`,
builds a temporary ad-hoc-signed XPC service around the same explicitly unsafe
test-only transport. A client with SIGCHLD auto-reaping pins the service CDHash,
fetches from a real HTTPS endpoint, observes an actual resolver child started
and then reaped when a 30-second injected DNS stall exceeds a 50 ms operation
deadline, and fetches successfully again. This is a finite local proof of data
flow and ordinary cancellation, not proof of exclusive child-reaping in a
shipped service, client authentication, authenticated snapshot/policy binding,
iOS support or readiness to authorize FI.

## macOS DNS-SD observation prerequisite (not a provider)

`fi-dns-observation.coax` is an untrusted bounded callback observation. It
classifies each supplied address with the existing Coax public-address policy,
poisons on an unsafe or excess address, and keeps failure/cancellation terminal.
There is no success or completion state and it returns no destination. Caller
state and address views must be valid, stable and confined to a serial callback
context; a caller can fabricate a value, so this type is not proof or authority.
A cleared DNS-SD `MoreComing` flag is not a complete answer set: Apple says it
only means no additional result is immediately queued, and later results can
still arrive. See <https://developer.apple.com/documentation/dnssd/kdnsserviceflagsmorecoming>.

The macOS-only `fi_dns_sd_probe.c` is a test fixture, not an adapter. It runs a
real `DNSServiceGetAddrInfo` localhost request with both address families, and
sets up and deallocates the reference on the same serial dispatch queue. The
host test process uses `SIGCHLD` auto-reaping and this fixture creates no child
process. It observed callbacks before deallocation and none during a finite
100 ms post-deallocation interval. Synthetic callbacks cover a later private
answer after a quiescent batch, a late event after cancellation, removals,
short and scoped socket addresses, and the seventeenth result. The probe relies
on valid OS-provided sockaddr storage. Test-only alarm and semaphore timeouts
are not a production deadline: no general bound is established for DNS-SD
startup, cancellation, event dispatch, or OS scheduling. This is neither a
complete all-address snapshot nor proof that later or changed answers cannot
arrive. It makes no iOS, browser, transport, provider or authorization claim.
A real provider must establish a complete answer-set contract, connect the
classified exact peer and verified TLS response, enforce its own bounded
lifecycle, and bind the response to trusted policy and snapshot provenance.

On the separately verified 0.21.0 compiler SHA-256
`73d2aeb0e1e57c355d87242946f89e1462f8b765f57087f4307d471a0277e1d4`,
Coax test passed 22/22 groups and the default-budget mutation gate killed
168/168 without waivers. An independent Python state oracle passed 2401
four-event traces plus the 17-event bound in each checked/release native mode.
The macOS DNS-SD fixture passed each checked/release native and ASan/UBSan
mode. These are bounded local observations, not platform or production proof.

- `coax/lib/nostr/block/fi-dns-observation.coax`: `ed87fd12831955383c27714e207468c942b3c392ac1ec7926c64fa4b22dcfdd9`
- `coax/tests/nostr/fi_dns_observation.py`: `cb4086a26c75a55d4ea72c4e4bba40845c448ac4a1dc2181ad589983526ce960`
- `coax/tests/nostr/fi_dns_sd_probe.c`: `69cc720e47c079e2fd3667d9cc748d57d4805b1cab6b067177bce284da122c50`
- `coax/tests/nostr/fi_dns_sd_probe.py`: `d71f5acdd64f43e6b98a0ea45eb983f32dc6b7d78e62ed3343245ff8d51b20b0`

Independent read-only review (`fixedbase_rescue`) found no concrete blocker in
the observation model or original test-only lifecycle fixture at C hash
`3e584b10bde079ac5a042b67ece7f6453e2dc0e1af5bab10aab800c573fe5bf5`.
A narrow delta review of the current C hash found no new blocker: the change
replaced a fixed delay with a bounded first-callback semaphore wait. It also
reviewed the independent trace model. No tests were rerun, and it did not
approve a provider, network behavior or authority. It noted the valid OS
sockaddr assumption and that one finite localhost test does not establish
general quiescence or completeness.
