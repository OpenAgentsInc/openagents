# Browser v1 socket retention and physical pressure

The ordinary v1 host now consumes the checked `socket-retention.coax`
companion before retaining incoming events or outgoing commands. Pre-v1
keeps its existing adapter. There are at most 64 leased incoming events and
four inboxes of retained payload, plus four separate scalar terminal-owner
cells. Full delivery storage queues an explicit empty error and close after
the already admitted FIFO. An oversized Unicode message produces one empty
error with no prefix. Outgoing storage remains 64 commands, 8 MiB per command
and 32 MiB total. Refusal closes with an explicit error; it cannot grow a
promise/closure list. GPU recovery leaves callbacks in the leased queue until
the consumer is available. Replaced concrete owners cannot deliver into the
new realm.

The real ordinary host passes the unchanged core and 79-row socket/HTTP
transport check, exact initial GPU readback, physical HTTP targets and engine
replacement. The common Coax checker accepts the retained
[core](browser-socket-queues/core.json) and
[transport](browser-socket-queues/transport.json) receipts. Current and retired
[application queue owners](browser-socket-queues/replacement-owners.json)
have zero leases. This is a recording-face dispatch/retention receipt, not a
new realm performance measurement or default migration.

The finite loopback peer and actual ordinary `host.js` then pass
[physical pressure](browser-socket-queues/pressure.json):

- A real 150 ms blocked main thread precedes 65 empty incoming frames. The
  application retains exactly 64, then delivers those 64 plus error/close.
- Five 128-byte frames retain exactly four/512 bytes, followed by error/close.
  All four delivered FNV32 payload identities remain in authored FIFO order.
- A 132-byte UTF-8 message is refused without a prefix; a following valid
  text frame still arrives. Binary data publishes no text callback.
- Held sending admits 64 empty commands; the next refuses before another
  retained copy. Engine replacement releases four held incoming frames and
  no old payload reaches the replacement.
- Four actual 8 MiB WebSocket sends observe 32 MiB in the browser's
  `bufferedAmount`. Application copies stay leased through actual send, then
  retirement releases their copies and the real connection closes.
- A real Worker with a deliberately delayed consumer holds all 64 empty
  command leases until actual acknowledgment. No byte count can make an
  unacknowledged empty command appear complete.

The final peer is deliberately unread for two seconds. Its closed socket
still reports **32,113,432 SDK buffered bytes** in this Chrome build, while
application leases are zero. This is retained as a separate physical
observation. Application-owned copies/terminal cells are bounded; these
receipts neither measure nor promise bounds for hidden browser, message-port,
socket or kernel queues. The SDK can retain transport bytes after application
retirement.

The actual bundled room Worker also passes
[64-command completion](browser-socket-queues/local-worker.json): all 64
empty sends acquire leases, its real room dispatch emits 64 acknowledgments,
the queue reaches zero, the room remains open without errors, and close has
zero retained output. This uses the unchanged frozen 1,034,562-byte room Wasm
from the qualified desktop preview. It does not relabel that worker as a new
current room build. The Worker SDK adapter now records pending commands and
bytes until the returned acknowledgment; the previous adapter exposed no
completion signal and would have held v1 output leases indefinitely.

`browser-socket-pressure.coax` checks every authored pressure field and kills
90/90 root mutants. The real-room completion observer kills 16/16. The first
physical run's [refused receipt](browser-socket-queues/first-counting-refusal.json)
is retained: its Unicode total included the fixture's own command callback
(three rows rather than two socket callbacks). The corrected observer filters
the actual socket-kind range; no product limit or expected payload changed.
Initial observer mutation survivors exposed missing exact-100 ms and absent
time boundaries; the strengthened checks kill all mutants without equivalents.

The independent 18,523-byte reactor remains exactly the admitted component
from [41ccdb3](socket-retention-component.md), SHA256
`e6a5478e54574015246b20e8f476b1c7e6ed715175b033110de65a8de67dfb36`.
No Coax source in that seven-file generated closure changed. The new web and
desktop hooks build/copy `build/host-sockets.wasm`, with a separate 24,000-byte
budget. No realm, shader, compiler or build/load policy was changed.

Actual input/runtime identities are in the
[ordinary](browser-socket-queues/ordinary-sources.json),
[pressure](browser-socket-queues/pressure-sources.json) and
[room Worker](browser-socket-queues/local-worker-sources.json) source fences;
all remained stable during their runs. The pressure SDK source bytes are
frozen under `~/.openagents/aethyrforge/metal-host-12868/browser-sockets/physical-sdk/`
and match the recorded hashes. The later room-only acknowledgment counter
and fixture entry are frozen separately under `frozen-sdk/` and their own
source fence. The original b587 image's genuine 73b receipt remains the tool
profile; it does not claim compiler admission for later protected sources.
Syntax/build-hook checks and tools-check pass. All Chrome, finite peer and
static-server children exit after each run; all browsers are headless/muted.

Rerun only changed slices with `coax/web/tool.sh
aethyr/host/check-browser-socket-pressure.coax ROOT PREPARED NEW_OUTPUT`;
`--local-room` selects the separate real-room acknowledgment workload.
Prepared inputs are fixture Wasm/WGSL, services/sockets reactors and (for that
last mode) the frozen room Wasm. Full ordinary dispatch uses
`aethyr/host/check-browser.coax PREPARED`. These component receipts leave
remaining #12869 pack/clip/Linux/service/default exits separate.
