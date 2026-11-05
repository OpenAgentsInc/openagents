# Host v1 redirect refusal and local reconnect routes

`http-redirect.coax` owns the versioned redirect decision. Version 1
selects no-follow and refuses 301/302/303/307/308 before response
publication. Other versions retain browser compatibility behavior.
The policy kills all 17 root mutants, with no survivors or equivalents.
The browser adapter captures that decision for the concrete request,
uses fetch error mode, and applies the status predicate before reading or
publishing the body. This includes redirect responses without Location.
No-follow beacons use keepalive fetch; they remain callback-free.

The actual muted headless Chrome workload executes 15 requests through
ordinary host.js and 30 GET/POST requests through the same HTTP effects
module and services reactor. Every redirect status is crossed with a
same-origin destination, a second owned loopback origin and no Location.
The peer records all 45 original requests. The 15 POSTs retain their exact
body, Authorization and player header on the original request; the GETs
have none. Both destination counts are zero. All 45 failures return exactly
`0\n` to their owner, with no dropped callback. A normal 200 control returns
`200\nok`; the actual service calls select false for v1 and true for v0.
This is physical loopback evidence; it accesses no production service.

The observation authority kills all 54 root mutants. The original driver
misread escaped JSON string contents, so its initial derived verdict is
retained as false. A separate checked Coax observer decodes the immutable
raw strings and re-admits every physical header, body, callback and count.
It passes the complete 13-word vector
`[30,30,15,15,15,15,15,0,0,1,1,1,1]`. The six original input hashes remain
unchanged; raw peer, browser, failed verdict and new observer hashes are
bound in the corrected receipt. Networking was not repeated to correct
the observer. An earlier partial attempt stopped after nine requests and
is separately retained without a qualification claim.

The services reactor is 20,267 bytes, SHA
`4e1b84f2ded9fcbdd09dad2b8bd186660cc5585383bfd0b5ed4b1d7aa31fae33`.
Its emitted C source table joins all 15 source files to the frozen
snapshot. The unchanged admitted compiler image is SHA
`3f8d0824d17ac7cc5c6ed843c755a7b8aafac741f02c21aeaa799f72cb9fac12`;
the original receipt verifies before and after the product overlay against
its genuine 36d59 compiler root. Later compiler changes requiring
secure_zero are outside this component's frozen library profile. No
compiler receipt was rewritten. The required tools check also passes in
our genuine 9b1 Linux root with the originally checked 0.50 image and the
new tool sources overlaid separately.

The shared local route authorities additionally admit exactly the
producer's existing `local:/api/room/relay/everglade/local-1?re=1`
reconnect URI. The queryless URI retains its meaning. Other query
spellings and extra parameters refuse. The route authority kills 60/60
root mutants and its physical-target writer kills 11/11, with no
survivors or equivalents. This is component admission; actual reconnect
after a seated socket loss remains an integrated host gate.

Raw attempts, all source snapshots, emitted C/header, original receipt
checks and corrected observations are retained under
`~/.openagents/aethyrforge/host-contract-12869/http-redirect/`.
The ordinary Linux `verse/native/net.coax` helper now passes a separate
physical peer workload normally and under ASan/UBSan. Thirty v1 requests
cross the same five statuses, three destinations and GET/POST methods.
Every answer is exactly `0\n`; the peer sees every original path, method,
POST body and two headers, with no destination request. Five pre-v1
no-Location controls retain their original status instead of becoming a
v1 failure. A 200 control returns `200\nok`. All 12 observations pass
`[30,30,15,15,15,15,0,0,5,1,1,1]`; the new native receipt authority kills
50/50 root mutants, with no survivors or equivalents. All 52 helper unit
checks also pass. Only the helper and its linked SDK effects are sanitizer
instrumented; the independent peer/observer remains a normal executable.
This is plaintext private loopback, not an HTTP TLS or authenticated-session
claim. The existing approved TLS implementation is linked unchanged.

The Linux compiler is the originally checked 0.50 image SHA
`5cc82481bf6db529e919a404134b6d18b02c984f443809af7426767af869af9c`,
with its original receipt verifying against the genuine 9b1 compiler root
before/after the separately recorded product overlay and after execution.
The emitted helper source table joins its complete source closure exactly
to the retained snapshot. Build records bind compiler, source/header,
approved OpenSSL selection, C flags, actual executable and source hashes.
Normal/instrumented raw physical requests, responses and complete SDK
records are retained under
`~/.openagents/aethyrforge/host-contract-12869/linux-http/` on both machines.

This checkpoint qualifies the browser and ordinary Linux HTTP helper
adapters and shared policy. Actual URLSession adoption, complete ordinary
Vulkan services, production web opt-in and complete host/default
qualification remain open.
