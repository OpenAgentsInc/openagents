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
   `openagents.aethyrforge.host/5/listening` to that host origin. This carries no
   authority. The host accepts it only from the configured origin **and its exact
   iframe window**, once per frame load.
3. The host generates a 192-bit random session, sends `hello` with version `[5]`
   and sequence 1 to that exact window/origin, and transfers exactly one MessagePort.
   The engine accepts only its configured parent origin/window, one port, version
   5, and a valid hello. It responds `ready` on that port. All further messages are
   UTF-8 JSON **strings** on the scoped channel, never arbitrary window messages.
4. The host checks release, engine ABI, project format, and capabilities before
   sending `open`. Successful open includes status. Requests and results carry a
   session, strictly increasing sequence in each direction, and correlated request
   ID (`request-` followed by that request envelope's sequence). This binding prevents
   reuse after bounded results are evicted. A project is bound on open; branch mismatch and stale revisions fail before
   dispatch. The host rejects decreasing result revisions and unsolicited results.
5. Unknown fields, tags, capabilities, duplicate JSON keys, invalid numbers,
   incompatible versions, malformed payloads, and replayed messages fail closed.
   Version 6 uses exact ABI/project-format matching. Incompatible schema changes
   require a new wire version and fixtures. This small host schema is independent
   of Aethyrforge's richer authoring and release formats; matching it does not imply
   source, runtime, or saved-world compatibility.

The generated [JSON Schema](../../crates/aethyrforge-host/schema/host-v8.json)
defines closed message shapes. Runtime validation adds UTF-8 byte limits, finite
coordinates, safe integer bounds, state transitions, peer checks, and relationships
between fields. Schema validation alone is insufficient. `Envelope::decode` and
`EngineEndpoint` enforce the complete public decoder contract.

## Native-bridge transport

The OpenAgents mobile app opens a zone's release top-level in a platform web
view and plays the trusted page's role (ADR 0010,
[mobile zones](../aethyr/mobile-zones.md)). The envelopes, schemas, sequence
rules, and bounds are the ones above. Only the carrier and the peer check
change:

1. The shell exposes `window.aethyrHost` (`NATIVE_CHANNEL`) with
   `postMessage(string)` and an `onmessage` handler. Android's
   `WebViewCompat.addWebMessageListener` injects this object only on the
   zone's origin. iOS defines the same object over a `WKScriptMessageHandler`
   and delivers frames with `callAsyncJavaScript`.
2. `EngineEndpoint` uses the channel only when its document is top-level and
   the object has `postMessage`. It posts the bootstrap string
   (`BOOTSTRAP`, `openagents.aethyrforge.host/7/listening`) on the channel,
   accepts `hello` as the first frame, and answers `ready` on the same
   channel. No `MessagePort` is transferred, because the native channel is
   already scoped to one web view. A framed document keeps the
   `postMessage` transport.
3. The shell's `NativeHost` accepts a frame only when the web view reports
   the zone's exact origin and the main frame (`WKScriptMessage.frameInfo`,
   or Android's `sourceOrigin` and `isMainFrame`), and the frame is no
   larger than `MAX_BYTES`. It answers the bootstrap once per document and
   validates everything after it with the same `Host`. Each page load starts
   a new random session.

Releases built before this transport still work in browsers. In a native web
view they load, but no bridge session starts until they are rebuilt against
this SDK.

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

- Version 6 maximum envelope: 524,288 encoded UTF-8 bytes. Actions reserve 8 KiB
  for the frame wrapper (516,096 bytes maximum); grant and authorization HTTP
  bodies allow 540,672 bytes for CSRF and grant metadata. Browser and private
  bridge preflight checks run before queue changes. Frozen v1–v5 artifacts keep
  their original 65,536-byte envelope limit. Source: 48,000
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


Version 5 adds `export_project` and confirmed `restore_project` actions. Frozen
v1–v4 schemas and pinned SDKs remain unchanged. The document includes bounded
creator sources, material values and PNG references, environment, entity/layout
references, and original imported files. It excludes fixed scene collision,
compiled assets, private engine code, credentials, and personal presence. The
asset-set and scene identities must match the target world. Exporting a copy is
read-only; restoring it requires the existing exact-action permission and checks
both the kernel candidate and GPU resources before adoption. A blocked personal
checkpoint uses a validated starting place or refuses the restore.

Version 6 defines instance-scoped face material inspection, temporary preview,
confirmed apply, and restore with at most eight checked source bundles. Preview
is read-only and returns its action hash and rendered frame; it does not advance
project history. Each generated or handmade layer is a 256 × 256 PNG up to
16 KiB. Saved material references use that same limit, while generic project
resources retain their existing bounds. Saved material JSON can keep its exact
text and hash up to the existing 24 KiB per-source and 48 KiB whole-project
limits; formatting and JSON key order do not change its meaning. Frozen v1–v5
schemas and matching pinned releases remain unchanged.

Account saves use the existing project membership and private project-file store.
Each accepted version has a server-stamped actor, idempotent command, expected
base version, source hash, and timestamp. Conflicts never silently replace a
newer save. Compensating undo changes only the actor's still-applied fields and
refuses fields another author changed. Camera, selection, tools, and unsent text
are separate per-account checkpoints. A fetched version becomes the local base
only after a confirmed restore succeeds. A Save response advances its accepted
base but marks any text typed during the request as unsaved. Confirmation and
late receipts stay bound to the original account, project context, and engine
session; switching context ends the old private world before it can be adopted.

The older editor must export its files before import on the new host origin.
The explicit converter maps only supplied old asset names to current catalog IDs
and preserves original scene/material/change files and unsupported fields. It
never labels old primitives as kit assets. Device copies are account/project
scoped; failed storage/network writes offer an editable export rather than a
successful project-save label. PNG resources are hash-checked, decoded within
4 MiB/2048-pixel limits, and privately stored under the same project authority.

## Version 8 characters (NPCs)

Version 8 adds the world's characters to status and one `talk` action. Adding a
field to the closed status shape breaks strict v7 decoders, so v8 is a new wire
version with its own bootstrap (`openagents.aethyrforge.host/8/listening`) and
the [v8 schema](../../crates/aethyrforge-host/schema/host-v8.json). The frozen
v1–v7 schemas and the SDKs pinned with earlier releases remain unchanged; a v8
host cannot drive a v7 engine. Publication still accepts playable runtimes pinned
at version 7 or 8.

`status.npc` is present only in worlds that have characters (the Haunted House
and personal planets):

```json
{
  "npcs": [{ "id": "ghost", "name": "Ghost" }],
  "nearby": "ghost",
  "lines": [
    { "sequence": 7, "npc": "ghost", "name": "Ghost", "kind": "greet",
      "text": "Oooh. You made it all the way to the gallery." },
    { "sequence": 8, "npc": "ghost", "name": "Ghost", "kind": "emote",
      "text": "twirls slowly through the air", "emote": "spin" }
  ]
}
```

- `npcs`: at most 16, each with a bounded `id` and a `name` of at most 64 bytes.
- `nearby`: the character within talking distance, or `null`.
- `lines`: the newest four lines, oldest first, with strictly increasing
  `sequence` for the engine session. `kind` is `greet` (the player walked up),
  `hint` (the next step), `reply`, or `emote`. `emote` (`wave`, `hop`, `spin`,
  `bow`) is present exactly when `kind` is `emote`. `text` is at most 512 bytes.

The `talk` action, `{"type": "talk", "npc": "ghost" | null, "intent": "reply" |
"hint" | "emote"}`, needs only the `inspect` capability, so guests and players
can use it. A missing `npc` means the nearest character. The result is `applied`
with status whose `npc.lines` include the new line; the message is
`"Name: text"`. An unknown name or nobody nearby fails with a readable reason.

The browser SDK (`mount_studio`, `mount_player`, and the demo host) adds the
following; they answer only in worlds with characters (today the house and
personal-planet editor releases, not published player pages):

- `npc_talk(intent, npc?)`: sends `talk`.
- `npc_chat(text)`: a chat message that starts with an exact `@id` mention (for
  example `@ghost hello`) asks that character to `reply`. This is a bounded
  ID parse, not intent routing; other messages are not for characters.
- A window `CustomEvent` named `aethyr-npc` per new line, in sequence order,
  whose `detail` is that line's JSON (the shape above). It fires for greetings,
  keyboard and button requests in the world, and host requests alike, so a chat
  card can show every line by listening once.

Lines come from the checked-in dataset
[`apps/aethyr/packages/runtime/npc-dataset.json`](../../apps/aethyr/packages/runtime/npc-dataset.json)
(`openagents.aethyr.npcs.v1`): places, characters with positions and greet,
reply, and emote lines, and ordered hints keyed on places visited and characters
met. No model is called. Figures are original box geometry drawn by the engine;
no licensed asset is involved.
