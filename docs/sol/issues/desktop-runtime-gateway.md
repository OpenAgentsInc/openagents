# D1-A: closed Desktop Runtime Gateway protocol and lifecycle

- Issue: #8655
- Parent track: #8574
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-openagents-desktop-product-architecture.md`](../2026-07-10-openagents-desktop-product-architecture.md)

## Landed boundary

The signed tokenless renderer reaches Electron main through one versioned,
closed Effect Schema query/command/event protocol. Main validates the bundled
top-level frame. Bootstrap reports truthful capability availability;
unsupported conversation interruption returns `unavailable`; lifecycle events
are sequenced and disposable. No token, credential, URL, raw event, arbitrary
IPC, `MessagePort`, filesystem/process handle, or argv can enter the contract.

The enforced behavior contract is
`openagents_desktop.seam.runtime_gateway_closed_protocol.v1`. The normal
Desktop `verify` gate runs its e2e schema round trip, mechanical boundary
oracle, bundle, and real Electron bootstrap smoke.

## Explicit residual

This leaf does not claim OpenAgents authentication, Khala Sync, or durable
provider streaming. Those services must compose behind this stable gateway in
later D1 leaves without widening renderer authority.
