# Browser host sessions

`Admission` verifies an existing host-signed device grant. `browser::direct`
proves a NIP-REACH WebSocket route; `browser::Socket` authenticates to the
relay named in that grant and carries exact sealed NIP-TERM artifacts.
`Direct` and `Relayed` share portable requests, capability checks, current
host generation, bounded output, and explicit uncertainty after dispatch.
Neither queues input or retries an uncertain operation. Reconnect with a new
verified admission, then reattach/read retained host resources.

Keys stay in Rust page memory. The browser has no local shell, host backend,
provider credential, or persistent input queue. Host rights and typist checks
remain authoritative on every operation. The DOM allocates received messages
before Rust can check their bounds; the adapter bounds retained bytes.

`pairing::Pairing` creates a fresh device key for one page and checks public
host, relay, generation, and route pins before enrollment. `browser::enroll`
redeems an explicit native host invitation with NIP-42 and verifies its
original NIP-HOST reply. Production routes require WSS. The loopback fixture
exception requires explicit opt-in and the page's same loopback host.
Terminal access is host-wide; the workspace label is navigation context.
`Admission::prepare_thread` additionally requires the original grant's
Observe right and returns an owned, bounded read context. Thread reads never
send, run, or stop a thread.

Mounts check `Admission::current` while idle and drop their transport on
retirement. Pairing, admission, and thread client keys are erased on drop.
Workbench watch mode never types. Typist loss clears pending input; lost
admission clears private output, history, records, proposals, and clipboard
state without closing the host PTY. Late callbacks cannot restore that state.
