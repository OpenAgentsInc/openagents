# NIP-05 Registrar Data Directory

This directory holds the live `nostr.json` served by the NIP-05 registrar at
`https://openagents.com/.well-known/nostr.json`.

## Seeding

The committed `nostr.json` ships **empty** by design. Real names (e.g. `chris`,
`agent`) are claimed at runtime through the booth flow against a live host that
holds the operator bearer token. We deliberately do NOT seed placeholder
public keys here, because:

- A NIP-05 entry with a fake public key advertises a wrong identity to the
  Nostr network for the duration it is published.
- Repo seed data drifts from live data once claims happen; the live file is the
  source of truth.

If you need to pre-seed `chris` or `agent` before opening the booth, post the
real `npub` to `/admin/claim` against the live host, or run
`scripts/add_user.ps1` with real values.

## Post-event snapshot

After the live event, a separate optional housekeeping PR may copy the live
`nostr.json` over this file as a snapshot. That PR is the only path by which
real claims should land in Git. See `policy/naming-policy.md`.
