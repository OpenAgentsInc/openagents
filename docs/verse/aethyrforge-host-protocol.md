# Public Aethyrforge host protocol

Implemented for [#12545](https://github.com/OpenAgentsInc/openagents/issues/12545).
The public [`aethyrforge-host`](../../crates/aethyrforge-host/) crate owns the
message contract, Rust/Wasm browser transport, and synthetic conformance fixtures.
Aethyrforge owns its proprietary editor, renderer, worker, compiler, and deployment.
Public builds never fetch or link that implementation. A browser may load an
approved private release separately after deployment configuration permits it.
The current homepage and Haunted House routes are unchanged by this SDK.
Closed source means the engine source repository and licensing remain private;
delivered JavaScript and Wasm can be inspected by browser clients. Bundles contain
no secrets, and private engine source maps are not published.

## Transport and compatibility

1. Mount a separate-origin iframe with `BrowserHost(frame, engineUrl, config,
   onChange)`. Configuration pins `release`, `engine_abi`, `project_format`, and
   `{id, branch, revision}`. HTTPS is required except for loopback HTTP. URLs cannot
   contain credentials, queries, or fragments. Production origins and immutable
   artifact integrity are deployment policy (#12546), not claims made by a release
   string sent by a frame.
2. The private engine loads the operator-pinned public Wasm SDK and constructs
   `EngineEndpoint` with its exact allowed host origin and supported capabilities.
   Once its document has loaded, it posts the fixed public string
   `openagents.aethyrforge.host/4/listening` to that host origin. This carries no
   authority. The host accepts it only from the configured origin **and its exact
   iframe window**, once per frame load.
3. The host generates a 192-bit random session, sends `hello` with version `[4]`
   and sequence 1 to that exact window/origin, and transfers exactly one MessagePort.
   The engine accepts only its configured parent origin/window, one port, version
   4, and a valid hello. It responds `ready` on that port. All further messages are
   UTF-8 JSON **strings** on the scoped channel, never arbitrary window messages.
4. The host checks release, engine ABI, project format, and capabilities before
   sending `open`. Successful open includes status. Requests and results carry a
   session, strictly increasing sequence in each direction, and correlated request
   ID (`request-` followed by that request envelope's sequence). This binding prevents
   reuse after bounded results are evicted. A project is bound on open; branch mismatch and stale revisions fail before
   dispatch. The host rejects decreasing result revisions and unsolicited results.
5. Unknown fields, tags, capabilities, duplicate JSON keys, invalid numbers,
   incompatible versions, malformed payloads, and replayed messages fail closed.
   Version 2 uses exact ABI/project-format matching. Incompatible schema changes
   require a new wire version and fixtures. This small host schema is independent
   of Aethyrforge's richer authoring and release formats; matching it does not imply
   source, runtime, or saved-world compatibility.

The generated [JSON Schema](../../crates/aethyrforge-host/schema/host-v4.json)
defines closed message shapes. Runtime validation adds UTF-8 byte limits, finite
coordinates, safe integer bounds, state transitions, peer checks, and relationships
between fields. Schema validation alone is insufficient. `Envelope::decode` and
`EngineEndpoint` enforce the complete public decoder contract.

## Operations and results

| Operation | Contract |
| --- | --- |
| `open`, `inspect`, `observe`, `close` | Guest exploration, bounded status, lifecycle |
| `propose` | One to eight typed move/material/place/remove proposals |
| `draft`, `compile` | Bounded source proposal and explicit compilation |
| `undo`, `redo`, `save` | Separate history and durable-save requests |
| `publish`, `share` | Explicit visibility and typed release/link results |
| `navigate` | Universe, house, or personal planet request |
| `capture` | Bounded image metadata and separately served image URL |
| `lookup`, `cancel` | Resolve a known request or ask to stop it |

There is no evaluate-JavaScript, raw HTTP, shell, credential, or arbitrary patch
operation. Optional result details carry bounded diagnostics (code, source asset,
line, column, message), published release/link, share link, or capture URL/hash/size.
The host does not automatically navigate or fetch result URLs. The consuming UI
must apply its destination and access policy. `auth_required` requests sign-in for
a pending action; `revoked` ends the session and clears accessible status.
Capabilities describe supported operations, **not permission**. No user role or
bearer credential is accepted in these envelopes. A service must check current
identity, project membership, scope, expiry, and revocation independently before
mutation, publication, sharing private content, or spending. That broker is #12549.
The private adapter currently advertises only `inspect` and `observe`, hides local
editing controls in embeds, and rejects unsupported work. The remaining feature
issues enable operations after their real services and authorization exist.

Results are `applied`, `rejected`, `unsupported`, `cancelled`, or `unknown`. Applied
means the named operation succeeded; it does not merge compilation, source save,
activation, publication, and payment into one success. `save` in status separately
reports unsaved, device-only, project-saved, or failed. A timeout is `unknown`, not
proof of failure or cancellation. Check the original request before retrying a
mutation. `cancel` never implies undo of a finished operation. `lookup` returns the
recorded outcome, or unknown when absent. Session-local results are not a durable
idempotency ledger; future remote command services must retain their own results.

## Bounds and lifetime

- Maximum envelope: 65,536 encoded UTF-8 bytes, including metadata. Source: 48,000
  bytes. IDs: 1–128 ASCII letters, digits, or `-_.:/`. Messages: 2,048 bytes, no NUL.
- At most 16 pending requests and 64 retained results per connection. A request
  times out after 15 seconds (120 seconds for initial world loading); late results can settle a retained unknown result.
  A duplicate request ID is rejected instead of re-executing. Use a new `lookup`
  request to inspect it. Evicted results return unknown, never re-execute.
- Integer identities/revisions/frames are at most 2^53−1. Checkpoint coordinates
  are finite, bounded to ±1,000,000 meters, with a nonzero view direction. The
  engine can impose tighter physical and resource limits.
- Status is a replaceable latest value, not a growing event log. The engine SDK
  sends at most ten updates per second; the private adapter uses one bounded
  subscription. The host retains no history of these updates.
- Panel updates keep the same host, iframe, channel, and worker. Actual frame load
  closes the old channel, creates a new random session, checks compatibility again,
  and reopens with the last validated eye/target/mode checkpoint. This is checkpoint
  recovery, not GPU-object continuity. A full host-page reload requires the app's
  saved checkpoint; this SDK keeps its recovery state in memory only.
- `close`, dropping the host, or leaving the page removes the frame and releases
  listeners, timers, ports, and its contained worker. Page owners call `close` on
  pagehide. Revocation closes the port and clears status; the app may then remove
  the frame. Nested untrusted previews must use a separate origin and receive no
  host/editor credentials. The iframe sandbox denies popups, downloads, navigation,
  camera, microphone, and clipboard by default, permitting scripts, its distinct
  origin, and pointer lock. Production response policies remain required.

## Verification boundary

Native tests exercise both wire peers, invalid identities/shapes/versions, limits,
stale status, replay rejection, timeouts/late results, revocation, and lifecycle.
The public browser checker covers the mock and an optional approved private
artifact using the same Rust endpoint, including panel continuity, reload
checkpoint, forged window messages, pinned-release mismatch, and leaving the frame.
The mock is synthetic and cannot qualify rendering, authorized edits, hosted
persistence, publishing, asset licenses, production headers, or performance.
The [integrated receipt](aethyrforge-host-conformance.json) records the exact tested private
revision and artifact hashes, without publishing its code or binaries.

## Version 2 authoring access

Version 2 adds selected-object status, guest selection, and narrow authorization
and revocation actions. Version 1 consumers reject those shapes, so the new SDK
negotiates only version 2 and uses a separate bootstrap string. The frozen
[v1 schema](../../crates/aethyrforge-host/schema/host-v1.json) and prior pinned
SDK/engine releases remain unchanged. A v2 host cannot drive a v1 engine.

The Rust host keeps sign-in cookies, CSRF tickets, pending edits, and panels on
its own origin. It reuses the site's sign-in popup. Sign-in completion refreshes
account standing and offers confirmation; it never executes the pending edit.
The trusted service issues a five-minute grant bound to the current account
session, tab, engine session/release, loaded project, private draft, action, and
exact action hash. Every use rechecks the account and the durable grant record.
Native bearer callers use the existing account read with no ambient-cookie
CSRF authority. Only the narrow grant reaches the checked engine channel.

Guest inspection and selection remain available. Cancel keeps the view and
request. Revocation ends editing without closing guest exploration. House edits
are not written into shared browser storage; durable project saves belong to the
public persistence service. A renderer acknowledgement alone is not publication
or proof of an account save.

Version 3 adds the bounded creator-owned authoring view. It uses a separate
bootstrap and negotiates only version 3. The frozen v1 and v2 schemas and their
pinned SDKs remain unchanged; their releases keep using those SDKs. New source
drafts, diagnostics, active source, material bindings, and undo availability are
never sent to old strict consumers. A draft does not activate a GPU generation.
`compile` prepares a candidate against the submitted project revision, checks
its material interface, and adopts it only while that revision and compilation
request are current. Failed source remains editable beside the last working
source. Object, material, environment, and activated source changes use the same
project revision and bounded undo history. Save and Publish remain separate.

The `world-effect` source hook applies to every rendered material. The UI and
`effect_scope: "world"` name that scope. Kiln cannot write the hook unless the
person enables `allow_world_effect` for the captured context. By default, it
proposes moves or numeric changes to supplied material IDs. Material IDs are
shared by all surfaces that use them; localized painting is a separate tool.

World-effect compilation supplies `source_sha256` for the exact kept draft.
The grant binds that action hash, and the preview checks the source hash before
preparing a candidate. Changing a source draft cannot reuse consent to compile
different bytes, even when the active world revision has not changed.

Version 4 adds a read-only `view` action and shaded/mesh status. It uses a
separate bootstrap and negotiates only version 4; the frozen v3 schema and SDK
remain unchanged. Mesh inspection draws the compiled triangle edges and does not
change project revisions, collision, or player pose. The host owns window layout,
focus, unsent fields, and session checkpoints; opening a panel uses the existing
engine instance. Mobile uses the same tool DOM with compact tabs.
