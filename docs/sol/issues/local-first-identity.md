# R1-LOCAL: device-local identity with optional OpenAgents account link

- Issue: #8666
- Status: implemented
- Consumers: OpenAgents Desktop and mobile

Native clients create an immutable local identity in their host-owned SQLite
store before any OpenAuth work. Device-local entities are isolated in separate
tables and use `LocalRevision`; the confirmed Sync store remains reconstructible
from the server. Hosted transport rejects device-local scopes.

After the existing server-verification boundary accepts an OpenAgents owner,
the host records an additive account link and starts the existing personal
scope. Disconnect, denial, failed connection, and restart never rewrite the
local identity or remove local rows. No local identity ref, owner ref, token,
row, store, or transport enters either Effect Native view.

Desktop Runtime Gateway v5 projects only `local_only | account_linked |
local_unavailable`. Desktop Settings and mobile Home say “Local device ready”
and present account linking as an optional upgrade for cross-device Sync,
hosted capacity, and network participation rather than an entry gate.

The Tailscale transport exploration remains uncommitted research.
