# Local desk control v2

Identity: `desk-control-v2`, `openagents.os.desk.v2`. This adds generation 2
to the local desk socket without changing generation 1. The semantic schema
is `proto/openagents/os/desk/v2/control.proto`. Transport is one JSON object
per newline on the owner's 0600 Unix socket, with exactly one reply per request.
This is a local user capability, not a network service or game-state authority.

Requests are at most 8191 bytes, use unique known top-level keys and plain
operation names, and require `"generation":2`. Used numeric fields are exact integers
within JSON's safe integer range. Replies carry generation 2, a type and the
current compositor session identifier. Refusals have type `refused` and a code.

- `info`: reports the actual Xwayland display, Wayland socket and control state.
- `list`: reports numeric window handles, lifetime revisions, title/class,
  desk geometry, current buffer dimensions and focus. A reused handle has a
  different revision. A new compositor has a different session identifier.
- `acquire`: requires session, handle, revision, and ms in 1..5000. Focuses
  that window, places the pointer inside its visible content, verifies both
  focus routes and grants this connection exclusive control until its deadline.
  Physical held keys or another controller refuse acquisition. Reacquisition
  by the owner releases its old inputs first.
- `key`: session/handle/revision, evdev code 1..255, explicit boolean down.
- `button`: same ownership, Linux button code 272..279, boolean down.
- `move`: same ownership, absolute desk x/y in the target's visible content.
- `relative`: same ownership, dx/dy each in -2048..2048, clamped to that content.
- `wheel`: same ownership, clicks in -20..20.
- `text`: same ownership, 1..256 US-layout printable ASCII bytes, tab/newline.
  The whole text is checked before any key is emitted.
- `stop`: current session; releases the current controller's inputs and lease.
  A separate connection of the same local owner can use this as an emergency stop.
- `shot`: session/handle/revision and a nonempty output path. Captures the desk
  to PPM and returns the capture's sequence and target geometry. No lease needed.

Each input rechecks connection ownership, session, window lifetime, keyboard
and pointer focus, and monotonic deadline before effects. Owned keys/buttons
are released on stop, disconnect, deadline, window/focus loss, physical input,
legacy input, VT departure and normal shutdown. Events go directly to the
target client and do not execute compositor keyboard shortcuts. Lease expiry
is serviced by the compositor loop; it is not a hard realtime guarantee.

The initial relative operation delivers bounded ordinary pointer motion. It
does not advertise Wayland pointer-lock or relative-pointer protocols, and
continuous camera turning across cursor edges needs separate qualification.
