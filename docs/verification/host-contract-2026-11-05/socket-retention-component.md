# Browser socket copies and leases: component gate

The actual browser `socket-queue-host.js` adapter and real Coax reactor pass
12 cases and all 113 expected fields. The check deliberately uses mocked
send/close SDK effects to hold allocations and observe copy lifetime. It is
not a physical WebSocket/peer verdict and does not yet change `host.js`.
No browser/GPU frame or FPS claim comes from this component.

The [raw receipt](socket-retention-component/receipt.json) covers independent
immutable Unicode copies, bounded `encodeInto` overflow with no prefix
publication, aggregate inbox-byte exhaustion, 64 empty-event exhaustion,
source-memory mutation after an admitted send, 64-command exhaustion, full
32 MiB outgoing retention, four separate terminal cells, hostile physical
source spans, and engine retirement. A callback that retires its engine
observes its active copy still leased; after it returns, the lease and both
queues are released. Repeated callbacks after failure cannot grow another
application retry/closure queue.

All count/byte/refusal choices come from existing `hiq`/`hsq` policy through
`socket-retention.coax`; its 39 root mutants are killed. The independent
113-field observer kills all 137 root mutants without equivalents. Its first
run exposed twelve unchecked negative case/JSON boundaries; the retained
red log and sharpened checks preserve those failures. The initial driver
passes 130 checks; the subsequent source-fenced gate adds the existing shared
input-hash observer and passes with all served SDK/fixture/Coax inputs stable.

The 18,523-byte reactor SHA256 is
`e6a5478e54574015246b20e8f476b1c7e6ed715175b033110de65a8de67dfb36`.
It is built with the original admitted b587 compiler in the genuine 73b
compiler root. Original receipt verification passes before/after product
overlay and after the final build. Seven Coax source inputs match the actual
normal wasm.sh C cache key
`e89ec486b94aa9dfdd8314de2e79dcfb74a153e297f3dc895edb61564950c33d`.
The genuine C/header and every source hash are retained; no missing original
emission profile is inferred. Cache observation uses the existing observer's
new `--unit SOURCE.coax --no-run-steps` projection of wasm.sh's actual
non-render-library key. It changes no compiler/build/load policy.

The final actual served [source fence](socket-retention-component/source-observation.json)
includes the reactor, SDK module/fixture, face, all seven Coax inputs and
driver. Durable artifacts are under
`~/.openagents/aethyrforge/metal-host-12868/browser-sockets/`, including
`emission/`, `browser-final/`, original verification logs and both failed
attempts (a missing C bool header and initial observer boundaries).
The [SHA manifest](socket-retention-component/artifacts.sha256) and
[source check](socket-retention-component/source-join.log) pin the actual
compiler-cache and runtime inputs.

Remaining exits are ordinary browser import/callback adoption, actual peer
stall/pressure/retirement receipts and resource build hooks. Hidden browser,
SDK and kernel buffers remain unmeasured. Public web opt-in and the desktop
default remain unchanged.
