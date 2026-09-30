# Desktop access to its own Coder

The desktop uses the existing same-user control socket as a broker for its
own computer. The window sends typed NIP-HOST operations and history queries.
The resident host prepares and verifies them with the portable clients the
phones use. Secrets remain in the resident host; the window receives outcomes
and bounded history pages only.

The broker signs task operations as the locally established owner. NIP-HOST
already admits that principal without a device grant. Every request still
passes `Authority::handle`, including signature, owner, freshness, operation
validation, retained-reply, and task-owner checks. The socket's kernel peer
check is required before the broker accepts a message. A network device
cannot use this route or acquire operator authority.

The window supplies a stable request identity. The broker persists the exact
signed pending request in an encrypted cache before dispatch, rejects changed
operations under that identity, and reuses the packet while it is fresh.
After expiry, the same task-owner identity still prevents a second effect.
Cache failure refuses dispatch. Creating a task remains inert except under
the host's existing local auto-start policy.

History stays a separate read-only authority. The broker pairs a temporary
client with the configured observer's Coder source only, prepares the query
with `coder_connect::client::Client`, and verifies the observer's reply. The
same source-bound cursors and page limits apply. It cannot read arbitrary
paths or offer another engine's private history. This connection's key never
leaves the host and grants no task execution.

Using a second enrolled desktop device over loopback would require a secret
in the window or another signing broker, plus grant renewal and enrollment
state for a caller already authenticated by the operating system. The local
owner broker preserves the existing boundary and shares protocol admission
without adding those credentials. Remote computers continue through their
existing device grants and connections.
