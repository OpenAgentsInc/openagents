# Suggestion inbox

Run this service as a dedicated operating-system user. Delegated agents get
their own Nostr keys and HTTP read access; they never get this user's files or
the inbox key. The owner's review key must also differ from the inbox key and
every delegated reader's key.

Generate a fresh inbox key with `openagents playtest keygen --out PATH`. Keep
the key file private. Configure `suggestion-inbox CONFIG.json` with:

```json
{
  "key_file": "/srv/suggestion-inbox/inbox.key",
  "data": "/srv/suggestion-inbox/messages",
  "owner": "OWNER_PUBLIC_KEY_HEX",
  "agents": ["DELEGATED_PUBLIC_KEY_HEX"],
  "relays": ["wss://relay.openagents.com"],
  "public_origin": "https://suggestions.openagents.com",
  "bind": "127.0.0.1:8901"
}
```

Put an HTTPS reverse proxy in front of the bind address. Supervise the process
with restart on failure. A failed storage write stops collection so the next
start replays relay history. Original gift wraps stay encrypted on disk;
screening runs again on reads. Nothing in a suggestion executes a command or
fetches a link.

Our relay's NIP-67 completeness hints keep backfill moving even when its
history cap is smaller than a requested page. Pages include the oldest
timestamp again so records at that timestamp aren't skipped. If a timestamp
holds more records than the relay will return, collection stops with that
timestamp in the error. Existing files and relay history remain available.
Raise both relay limits, restart the relay, then restart this service. For
example, `NOSTR_RELAY_MAX_LIMIT=1000` and
`NOSTR_RELAY_SEND_QUEUE_CAPACITY=2001` recover a bucket of 600 records. The
effective page cap is `min(MAX_LIMIT, (SEND_QUEUE_CAPACITY - 1) / 2)` and must
exceed the bucket size. Valid bounds are 100,000 for `MAX_LIMIT` and 65,536
for `SEND_QUEUE_CAPACITY`; a bucket of 32,767 or more needs a relay cursor or
an operator export rather than another limit increase. Legacy relays without
NIP-67 use the usual short-page heuristic, so use our relay for reliable
backfill.

Set `NOSTR_RELAY_SUGGESTION_INBOX` on the relay to the inbox's public key. This
requires NIP-42 authentication for suggestions and gives this recipient
separate sender and IP flood budgets. Set `OPENAGENTS_SUGGESTION_INBOX` on the
website to the service's base URL. The website serves its public configuration
at `/suggestions/config.json`; clients read it at runtime. The service publishes
the inbox's NIP-17 relay list (kind `10050`).

The service exposes:

- `GET /config.json`: inbox public key, accepted relays, and delegated public keys.
- `GET /v1/suggestions`: normal feedback, quoted as untrusted user content.
- `GET /v1/quarantine`: quarantined feedback, only with the owner's key.

Both read routes require a NIP-98 `Authorization: Nostr …` event over the exact
GET URL. Pages contain at most 100 suggestions (50 by default). Use `limit` and
the returned `next` message ID as `after` to continue. A delegated key cannot
read quarantine, including through query parameters on the normal route.

`openagents suggest TEXT` stores encrypted events before sending. Failed sends
retry on the next send or `openagents suggest --retry`. Even a first offline
send stays encrypted to the device's key until the public configuration can be
read. `--config URL` overrides the public configuration address.

Read with `openagents playtest suggestions --inbox URL --as PROFILE`; add
`--after ID` for another page. The separate owner's profile can deliberately
request `--quarantined`. The Grid and the web, desktop, iOS, and Android menu
links all open the same Rust suggestion composer.
