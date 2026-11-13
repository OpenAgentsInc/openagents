# Linux socket worker effects

Linux and Metal compile the same portable
`apps/metaverse-engine-desktop/MetalSockets.c` worker implementation.
`metaverse/native/host-sockets.c` projects the realm's current Coax transport
owner into that SDK. It supplies no WebSocket parser, queue-size policy or
reconnect policy. Protocol, retained-payload leases, command admission and
main-thread drain budgets remain the shared Coax modules.

Each engine owns a concrete SDK context. The process owns four persistent
workers, with at most one current and one pending job per logical slot.
Replacement frees a pending job immediately and stops the executing job;
that job keeps its immutable bytes and old context reference until its
effects return. Engine retirement closes its context without waiting for
network I/O. Final process teardown joins all workers.

The incoming lease queue holds at most 64 event nodes and four inboxes of
payload bytes. A full queue blocks the native reader before another event
copy or read. Four fixed terminal-owner cells separately retain scalar
error/close outcomes for SDK allocation failure and local close; these
contain no payload. The outbound queue leases at most 64 commands/32 MiB,
with 8 MiB per command. Refusal becomes the shared explicit error/close
outcome. These are application-owned storage bounds. Kernel buffers and
peer-side buffering remain separately observed.

`rhn_socket_poll` borrows one SDK event, admits its complete owner through
`ht_socket_deliver`, copies the admitted bytes through the ordinary realm
dispatcher, then releases the lease. Empty terminal events carry no byte
pointer; the dispatcher skips their zero-length copy before delivering the
actual empty message. Deferred callbacks cannot retain a replacement engine
or release its leases. Local route resolution still uses the exact bound
development room authority.

`native-network-build.coax` stages the same worker source/header for ordinary
shared realms. `build-vulkan.coax` links it for conformance. For explicitly
retained admitted-compiler emissions, `build-linux-socket-effects.coax`
builds either the physical worker driver or, with `--transport`, the actual
realm dispatcher. Its source/header/profile and executable manifest is
written before execution. `--sanitize` instruments generated Coax C and all
linked product/fixture effects with ASan/UBSan; runtime leak detection stays
enabled. The pressure tool's `--peer`/`--echo` and transport tool's `--server`
accept prebuilt Coax fixtures, keeping compiler preparation separate from
the measured workload.

The driver retains the existing `aethyr.metal.socket-effects.v1` fixture
wire schema and executable basename for the common plan runner, and reports
`platform: linux`. This name does not claim a Metal measurement. Linux
world Origin/cookie selection, browser queue adoption, ordinary Vulkan
world/room integration and final compiler/default admission remain separate.
