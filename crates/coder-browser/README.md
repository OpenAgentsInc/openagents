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
