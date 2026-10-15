# ATIF over Nostr

`atif-nostr` implements NIP-ATIF publication and retrieval. It uses `atif` for
rendering and step digests, `nostr` for signed events and private artifacts,
and `nostr-transport` for authenticated relay connections. It does not execute
recorded actions or supply payment, approval, or recovery authority.

Use `openagents trajectory` or the standalone `atif-nostr` binary. Keys come
from `OPENAGENTS_NOSTR_SECRET` (hex or `nsec`) or `--key-file`; on Unix a key
file must be readable only by its owner.

```sh
# Prepare encrypted events without opening a relay connection.
openagents trajectory pack task.1.atif.jsonl --to npub1... \
  --key-file ./recorder.key --bundle ./private-events.json

# Publish the exact saved events. Chunks precede the declaration.
openagents trajectory retry ./private-events.json --relay wss://relay.example

# Retrieve as the recipient, with that recipient's key.
openagents trajectory fetch EVENT_ID --relay wss://relay.example \
  --key-file ./recipient.key --output ./received.atif.jsonl
```

`publish` combines preparation and relay publication, saving `--bundle` before
sending. Failed publication can retry that file. Output files are written
atomically, are private on Unix, and never replace an existing destination.

Public publication requires `--publish-public` instead of `--to`. It publishes
the supplied bytes; redact a separate copy first when needed. `--metadata FILE`
accepts `derivation`, `task`, `run`, `selector`, `parent`, `children`, and
`previous`. A public redaction normally uses
`{"derivation":{"kind":"redacted","source":null}}`. Use `--document` for an
ATIF JSON document; converted documents and changed/interrupted logs retain
partial coverage. Historical log versions keep their original declaration.

The library validates closed manifests/chunks, exact event and byte identities,
ATIF step digests/counts/state, publisher/recipient/mailbox bindings, and
parent/child and continued-segment relationships. `check_child` and
`check_previous` validate already retrieved linked trajectories; retrieval does
not walk those links or disclose their contents automatically.

Retrieval bounds time, chunks, and bytes. Source-only manifests can use HTTPS
locators only when their hostname is explicitly allowed with `--source-host`;
redirects and URL credentials are refused, and fetched bytes must match the
manifest. Missing or changed content fails before an output file is created.
The retained-history observer remains a separate incremental read path.

Tests use isolated files and an in-process relay, including public/private
round trips, exact retries, missing or forged chunks, large Unicode/escaped
records, historical versions, linked trajectories, and disclosure refusal.
