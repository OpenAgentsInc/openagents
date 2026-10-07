# Compute workbench

Portable, read-only account, top-up, quote, usage, and receipt values over the
existing central ledger. Exact millisatoshis display as credits: one credit is
one sat. Unknown charges remain unknown; released funds are available again,
not a refund. Public aggregates omit account and execution records.

The `host` feature reads `pay-ledger` and `retail-cloud`. Every refresh resolves
the principal's current account read right. Execution check and cancellation
evidence additionally requires the host's independent current observation right
through `read_observing`. Account reads grant no execution, disclosure, or spend.

Both `openagents-terminal --compute-workbench CONFIG` and
`verse --compute-workbench CONFIG` mount the same account and receipt
adapters. Use F16 to return to product panes, F2 to refresh, arrows to select, and
Page Up or Page Down to scroll. The configuration is a private JSON file with
`ledger` and `journal` paths, `principal`, its credential digest in `credential`,
and optional immutable `offers`. Existing files must exclude group and other
access; missing files are refused. Credentials never go on the command line.

The optional `client` feature builds the native `retail-client` binary over
[`POST /v1/retail`](../../docs/cloud/retail-service.md#selected-native-client).
It requires an explicit private configuration, retail principal and bearer file,
service origin, and client state directory. Its separate `quote` and `confirm`
controls pin reviewed source, effects, recipients, payer, quote, expiry,
credential digest, and service custody; read-only policy refuses mutations.
It stores bounded reviews and cursors, and uses the service's existing ledger
and funded execution on reconnect. The actual binary/HTTP fixture uses only
fake funds and providers.

The read-only panes offer no spending action. A valid displayed quote is informational;
the owner's separate offer control must recheck current terms and authority.
Browser, phone, and CLI adapters can consume the portable values without linking
SQLite or a wallet. Physical native rendering and real payments remain unverified.
