# Partial native FI HTTPS transport evidence

The `fi_fetch.c` prototype is disabled in normal builds. Its original remains
uncommitted outside the product path; an identical isolated copy is retained
under `coax/tests/nostr/fixtures/fi_transport` for tests only. An
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

- `coax/tests/nostr/fixtures/fi_transport/fi_fetch.c`: `3a9b916b6aad5aa161f15135fadc4b55f4e8254e10cc412ab5d8509e0c0f9bc8`
- `coax/tests/nostr/fixtures/fi_transport/fi_fetch.h`: `243b81a2100bace3325a2e987fabb8d9f5e1503868865d079e47791dd0dbbbee`
- `coax/tests/nostr/fixtures/fi_transport/fi_dns_worker.c`: `bc36d2c45a95efde57360bebdc27be86c35f6686daab4b4ae3ed715d8152fe52`
- `coax/tests/nostr/fi_fetch_native.py`: `3e6979b4fd821112bd752580b8f9c4462d4c57485f6ee3107021dc7da93d07ee`
- `coax/tests/nostr/fi_fetch_resolver_fixture.c`: `776b221200f5857e9a49d8d6a821c2f448b19ab180ebf0d0d5fe7ab523671bff`
- `coax/tests/nostr/fi_fetch_sockaddr_fixture.c`: `c74029c4424aa7af330b7734727adfcbd30e5c0d3dc5d4c02ba57f388c65b100`
- `coax/tests/nostr/fi_fetch_sanitized.c`: `4fde3fc93c499d093ef1eac16ee9caf31b4f93ca1a4d74f82c15e3923e7a8451`

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
`186477f4bb2c01eccaa76b206277f435426c7409f8bd60f76519898216a8a04b`,
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

## Active host-reaper XPC scratch experiment

`fi_xpc_reaper_probe.py` is a macOS-only test-bundle
experiment built around the same explicitly unsafe, normally disabled transport
prototype; it is not a deployable service or provider. A client starts a
`waitpid(-1, WNOHANG)` thread and verifies that it reaps 32 host-owned test
children. During a separate one-second XPC resolver request, a producer waits
until the service resolver writes its PID marker before spawning another 32
host children. At least one host-child reap must occur while the request is
in flight after that marker, and the total must be exactly 64. A generated
test-only resolver shim holds an exclusive lock on that unique marker file
while resolving. Immediately after a counted host reap, the reaper checks that
the lock is held; only the shim and that one reaper touch the lock after
startup. The resolver
fixture sleeps 30 seconds; after the unavailable response, the client remains
alive 400 ms, and the harness checks that the recorded resolver PID is absent.
No signal is sent to that PID by the harness; reuse can instead make the final
existence check fail spuriously. The client pins the ad-hoc test service's
CDHash. One finite run with the lock witness passed on the local Mac; an
unlocked-shim negative control failed as intended. Four finite runs of the
preceding marker-only test passed.

The earlier cold-start 100 ms tests returned unavailable before the fixture
wrote its marker and failed their assertions; they are not counted as evidence
of a started resolver. The service was warmed with a refused invalid request
and the final experiment uses a one-second operation deadline. This result
shows host reaping and a service-owned resolver can coexist in the tested
scratch setup. It does not establish that a production service has exclusive
child-reaping ownership, forbid another reaper inside that service, prove PID
reuse safety across arbitrary races, bound OS teardown, authenticate real
clients or snapshots, or support iOS. The unsafe helper remains excluded from
normal builds and no transport or FI authority is granted.

- `coax/tests/nostr/fi_xpc_reaper_probe.py`: `0257a8c4e309a1617a8a2ff1ee4f1edeea47386395cc384b875afecec117e4cb`

A separate read-only review (`fixedbase_rescue`) of the prior probe hash
`5ec4ff580dc71c6193d60401bac6930e599e3b5bcbbb6b0a1d34cf7f7cdc908d` found
no concrete blocker in the stated finite experiment. It noted that the PID
existence check can fail spuriously under reuse and that service-internal
reapers, deployment identity and general lifecycle behavior remain unproven.
The reviewed probe was then strengthened to wait for the resolver marker
before producing overlapping host-child reaps. A separate delta review found
that the marker proves the resolver started before the counted host reaps,
but does not by itself prove it was still alive at each reap; a delayed reply
could outlive it. The reviewer then inspected the lock-witness revision and
found that, under the scratch fixture's sole-lock-holder assumption, a lock
conflict supports resolver liveness at the observation just after `waitpid`.
The reviewer ran no tests and did not expand the scope beyond the finite test.

## Reaperless pipe lifecycle experiment (not a provider)

The following isolated test-fixture files are an unconnected process-lifecycle
experiment, not an enabled transport. `fi_dns_supervisor_experiment.c` is a
single-purpose supervisor that starts and reaps one resolver child. The child
writes into a private internal pipe. Only after that child has been reaped and
internal EOF observed does the supervisor write a bounded terminal response:
`FDR1` contains the child's complete frame, while `FDE1` indicates failure or
cancellation. Incomplete internal EOF or uncertain reaping emits no terminal.
The supervisor's direct-child `kill`/`waitpid` relies on that process being
single-owner and on its SIGCHLD disposition; it does not establish control of
resolver descendants or external OS services.

The separate `fi_pipe_client_experiment.c/h` prototype is disabled unless
explicitly built with `FI_PIPE_TEST_ONLY=1` and an absolute test helper path.
Its consumer never waits on or signals the supervisor PID. A control pipe asks
the supervisor to cancel while the response pipe remains open for the terminal
and EOF. One process-global slot remains occupied during cleanup; only an
exact terminal plus EOF releases it. Missing, oversized or malformed terminal,
including a supervisor crash, permanently quarantines that slot. If teardown
never finishes, the retained thread/state and single occupied slot are bounded,
but availability and a general OS teardown deadline are not guaranteed. This
slot bound is per process. The test-only client now also takes an advisory
lock before spawn and passes the locked file description through supervisor to
resolver; in the crash fixture, a fresh process could not acquire the same
inode until the orphan exited. This is not a production cross-restart bound:
the path and inode are not trusted, and replacement or explicit unlock defeats
the test mechanism. The
service process must arrange automatic or managed reaping of its own direct
children without reusing the supervisor PID. This prototype does not install
that process contract, authenticate the helper or its output, use Coax address
policy on frames, establish peer/TLS identity, or install a snapshot. No frame,
status or slot transition grants FI authority. A real service must bind the
terminal to a locally trusted executable and private descriptors, prevent FD
inheritance, serialize response acceptance against cancellation/deadline, and
handle resolver descendants and target-specific lifecycle behavior.

`fi_supervisor_pipe_probe.py` runs a real host-wide `waitpid(-1)` reaper while
its consumer reads private pipes and never waits or signals the supervisor PID.
A valid frame and a mixed public/private frame traverse as untrusted bytes;
a child that writes a complete forged frame but exits unsuccessfully, and a
child that writes a partial frame then exits successfully, yield only `FDE1`.
Cancellation closes the control pipe while retaining the response pipe until
`FDE1`/EOF. A test-injected supervisor exits itself after the resolver starts:
its response pipe reaches EOF with no terminal while the resolver is still
alive. The test resolver exits naturally, and the consumer must quarantine on
that absent terminal. The test never signals a potentially recycled PID.

`fi_pipe_client_probe.py` exercises the default refusal, delayed post-reap
terminal, occupied slot during cancellation, later reuse, short-output no-write,
expired-result refusal, early free/cancel, and supervisor-crash quarantine.
Both probes passed native and ASan/UBSan builds; the client probe passed with
an active host reaper and with SIGCHLD auto-reap. The crash/liveness and
cancellation tests use finite ordinary scheduling and make no guarantee about
catastrophic OS stalls. Independent scoped source review and its limits are
recorded below.

- `coax/tests/nostr/fixtures/fi_transport/fi_dns_supervisor_experiment.c`: `5362f135999480e11ac2ea665e1e1253eb88c86383e07091d0ed84bbc9f6f0b3`
- `coax/tests/nostr/fixtures/fi_transport/fi_pipe_client_experiment.c`: `34fead662874429639bd89975bb0099819ef9cd2d8fa0324c7c2569e12a3d74e`
- `coax/tests/nostr/fixtures/fi_transport/fi_pipe_client_experiment.h`: `1c5c3ab2b573246d8a7585dd31b1c0471e0153e7167f19f67ffd7fb9368c242b`
- `coax/tests/nostr/fi_supervisor_pipe_probe.py`: `5b91be19a9a10d29e9791f3af4036ef2b4bd1ae7ab7fdcf2a0e4bee3e928efc2`
- `coax/tests/nostr/fi_pipe_client_probe.c`: `c90874c3b56caece8b5c9109a0bd300fd81efd29efd653b593243528b97c9c3e`
- `coax/tests/nostr/fi_pipe_client_probe.py`: `7f5e1ef12eda8380bbc73783d1c5b0047cfa8245c81888bc45c8036495e51d2c`

The above hashes identify the first reviewed reaperless revision. The newer
lock-inheritance test-fixture revision has these hashes:

- `coax/tests/nostr/fixtures/fi_transport/fi_dns_supervisor_experiment.c`: `b48759113efcd5a76d350f1fbdee466b76886712df1ab941d0c50a122449a483`
- `coax/tests/nostr/fixtures/fi_transport/fi_pipe_client_experiment.c`: `bf4298c70269bb7b409d454456f729df9528db880c4daa6bf81e4bf9310ad8f4`
- `coax/tests/nostr/fi_pipe_client_probe.c`: `573312f65eae735a5f8d9c681e64f9f91f7d19b569b5ba63b89801a98487d39d`
- `coax/tests/nostr/fi_pipe_client_probe.py`: `4fd1d77f588a4914b948b2afb539a349fe77cc847641c5bf7c55e02f5e9a60f5`
- `coax/tests/nostr/fi_flock_restart_probe.c`: `04d3bab7e7d84d98f43b81f07a4efaea3bcd380abc84fb205bd484caedc37bc9`
- `coax/tests/nostr/fi_flock_restart_probe.py`: `e6e79eb20054b02564a5e932c136b1808ed131e20a8d253eb9912e07cbe8f584`

The lock revision passed native and ASan/UBSan client tests with active host
reaping and SIGCHLD auto-reap, including a restart observer blocked while the
crash-orphan retained the lock and succeeding after it exited. The first ASan
run failed a 250 ms positive-response timing assumption; the test now allows
one second before checking refusal after expiry, and the full rerun passed.
The standalone macOS lock probe passed fork/exec inheritance, exit before
spawning and early service exit. Its negative controls demonstrate that path
replacement or explicit child unlock defeats the advisory lock. An independent
review of the original lock probe noted that `before` is an ordinary exit,
not a signal crash, and that PID-existence tests may fail spuriously on reuse.
The current test revision requires a newline-terminated PID marker to avoid
parsing a partial write; an independent delta review confirmed this change.
Independent review of the integrated client and supervisor confirmed fd 64
inherits the locked open description under the stable-inode and cooperative
helper assumptions. It noted fd existence does not authenticate the lock or
helper; the pipe CLOEXEC window and host reaper requirement still apply.

An explicit untrusted-helper negative control now reproduces the trust failure:
the helper emits `FDE1`, closes its inherited lock in a live descendant, and
exits; the test-only client accepts the fake terminal and starts a second
helper while both descendants are alive. This counterexample reproduced in
native and ASan/UBSan modes with both active and automatic host reaping. The
client is therefore **not safe to enable as a provider** until the helper,
private channels and stable lock identity are authenticated and enforced.

Independent review of the previous fixture hashes `2c3a573d...` and
`e91030de...` confirmed the fake-helper control demonstrates this narrow
counterexample. It noted that a client-self-exit case witnesses the inherited
helper lock, but does not establish that a resolver had already started; and
that PID-existence checks may be confused by reuse. The latest fixture requires
newline-terminated PID markers rather than accepting partial numeric writes;
the packaged native and sanitizer tests passed, and this test-only delta has
not been independently reviewed.

Independent read-only review (`fixedbase_rescue`) of the reaperless prototype
confirmed two deployment constraints: `pipe()` followed by `fcntl(FD_CLOEXEC)`
has a multithreaded inheritance window, so an eventual service needs atomic
close-on-exec creation or process-wide spawn discipline plus authenticated
channels; and a client that never reaps its supervisor requires an explicit
host reaper or SIGCHLD auto-reap to prevent accumulating zombies. The reviewer
noted caller-serialized poll/free and the mutex/expiry recheck in the test-only
client. No tests were rerun; no production, provider or teardown approval.

For test-only packaging, the six worker/fetch/supervisor/client files were
copied byte-for-byte from the untracked platform prototypes into the fixture
directory. The packaged supervisor and pipe-client probes passed their native
and ASan/UBSan cases; the packaged XPC reaper probe passed on macOS. The adapted
checked native fetch test also passed its real TLS, proxy, redirect, stream and
resolver cases, and the adapted XPC fetch test passed locally. Those external
network checks remain nondeterministic. Independent review confirmed the
three reaper/supervisor/client runner changes were path-only and the six source
copies were byte-identical; no production or provider scope was added.

## Mutual XPC peer-requirement scratch test

`fi_xpc_mutual_probe.py` is a macOS-only ad-hoc-signed local test, not a
provider. The service requires a client signing identifier and the client pins
the service CDHash using `xpc_connection_set_peer_code_signing_requirement`.
A matching client received a reply, a different client identifier did not,
and a permissive-service control accepted that same different client. The
instrumented service observed `XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT` for
the mismatched client. A wrong service CDHash likewise produced that exact
error on the client. The permissive control is a different service instance.
The local SDK declares this API available on
macOS 12+ and explicitly unavailable on iOS; this experiment makes no iOS claim.
Apple documents that received messages are checked against the connection peer
requirement: <https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_requirement>.

The decisive negative control uses a distinct ad-hoc-signed client executable
with a different CDHash but the same allowed signing identifier: it was accepted.
An ad-hoc identifier alone is spoofable and cannot identify an authorized
client. The test does not supply a production signing team, trusted requirement,
service packaging, helper/executable binding, cancellation or snapshot authority.

- `coax/tests/nostr/fi_xpc_mutual_probe.py`: `c0489d3ef1722fa7d7ed1505c5157f44b9ff13d67d7760f401f8016c34b249df`

One finite local run passed the matching, mismatching, permissive and spoofing
controls. Independent read-only review confirmed the same-identifier,
different-CDHash spoofing observation; a later delta review confirmed the
instrumented exact-error checks address its earlier attribution concern. The
reviewer noted the then-immediate service-marker read could fail spuriously;
the latest revision waits up to one second for complete marker contents and
has passed locally. A further independent delta review found the bounded wait
does not weaken the exact-error assertion. No tests were rerun by the reviewer,
and no deployment approval is claimed.

A further macOS-only scratch test, `fi_xpc_codehash_probe.py`, individually
ad-hoc-signs a client executable and pins that exact client CDHash in a signed
XPC service; the client also pins the service CDHash. A matching client and a
byte-identical copied client both received replies. A different executable
with the same signing identifier received no reply, and an exact peer-signing
error marker appeared at the service. Independent review noted that the marker path is shared across
connections; a delayed earlier callback cannot be ruled out solely by the
pre-request absence check, so exact attribution is not conclusive. This is finite code-identity evidence: the identical-copy control
shows it does not establish application location, user intent, team identity,
or deployment authorization. The test passes server requirement as a test
argument and has no trusted build manifest, helper/snapshot binding, iOS or FI
claim. Independent review and the other requirements remain open.

- `coax/tests/nostr/fi_xpc_codehash_probe.py`: `b31c675fa8af9a7823cd9df8801ca649c5ebaef94b9f91acaee39b232d6b7f4c`

The latest source additionally checks both signing identifiers and that the
service error marker was absent before the changed-code test. Independent
review found the shared-marker limitation above; it ran no tests and gave no
deployment or authorization approval. One finite local run passed.
