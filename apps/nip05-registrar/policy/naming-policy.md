# OpenAgents NIP-05 Naming Policy

This policy governs the `openagents.com` NIP-05 namespace served by the
registrar in `apps/nip05-registrar`.

## Handle format

- Regex: `^[a-z0-9_\-\.]{1,32}$`
- Lowercase only. Uppercase or mixed-case input is rejected (not auto-lowered),
  so the operator and claimer agree on the exact handle string up front.
- Allowed characters: lowercase ASCII letters, ASCII digits, `_`, `-`, `.`.
- Length: 1 to 32 characters inclusive.
- Whitespace is trimmed before validation.

## Reserved names

The following handles are reserved and never claimable at runtime:

```
_, admin, administrator, agent, api, billing, claim, config, contact, help,
info, legal, mail, me, moderator, nostr, openagents, operator, owner,
postmaster, press, privacy, registrar, root, security, staff, status, support,
sysop, system, test, webmaster, well-known, www
```

Additional reserved names can be supplied at boot via the
`NIP05_REGISTRAR_RESERVED_EXTRA` environment variable (comma-separated).

`_` is reserved per NIP-05's "root identifier" convention; we do not currently
serve a `_` entry.

> Note: `agent` is on the reserved list. The booth lead may pre-claim `agent`
> for the official OpenAgents agent identity by adding it to
> `NIP05_REGISTRAR_RESERVED_EXTRA` removal, or by claiming via the admin API
> *before* setting it reserved. The reserved list is a hard runtime block; do
> not attempt to claim a reserved handle through the booth flow.

## Collision and duplicate policy

- A handle is unique. The first successful claim wins; subsequent claims for
  the same handle are rejected with `409 handle_taken`.
- A public key is unique across handles. A claim that would map a second
  handle to a pubkey already in the file is rejected with
  `409 pubkey_taken`. This prevents accidental aliasing and abuse where one
  party tries to register multiple identities under one key without operator
  oversight.
- Comparison is case-insensitive on the hex pubkey but the canonical stored
  form is 64-char lowercase hex.

## Takedown / delete

The registrar exposes an authenticated `DELETE /admin/claim/<name>` endpoint
behind the operator bearer token for runtime rollback. Use it to:

- Remove a typo'd or wrongly-claimed handle.
- Remove a handle that violates this policy or platform terms.
- Free a reserved-by-mistake handle.

Deletes are logged via the operator's terminal/booth notes and reflected
immediately in the live `nostr.json`. There is no undo beyond re-claiming.

## Runtime-write model

- Live writes happen against the **live host's** `nostr.json` only, via
  authenticated HTTP. No Git/CI/PR is in the loop for individual claims.
- The bearer token is held only on the booth device's environment.
- Repo-side `apps/nip05-registrar/data/nostr.json` is **not** the source of
  truth during the event; it is the seed that the host loads on boot.

## Post-event snapshot

After the live event ends:

1. The operator copies the live host's `apps/nip05-registrar/data/nostr.json`
   to a working tree.
2. A separate housekeeping PR commits that snapshot back into the repo.
3. That PR is the only path by which real claims should land in Git history.

This keeps the repo a useful starting point for fresh deployments without
turning every claim into a PR during the event.

## Security boundaries

- The bearer token is a long random string from the operator's environment;
  never commit it. `.env.example` (when added) contains placeholders only.
- All admin endpoints require `Authorization: Bearer <token>` and are checked
  with constant-time comparison.
- Public `GET /.well-known/nostr.json` is open and CORS-permissive
  (`Access-Control-Allow-Origin: *`) so any NIP-05 client can resolve names.
- Admin endpoints are not CORS-permissive; they are intended to be hit from
  the booth device, not the public web.
- Errors return structured JSON (`{ "error": "code", "message": "..." }`) and
  do not leak internal state.
