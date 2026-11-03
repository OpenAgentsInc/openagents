# Current consumer reports

Each file here is a generated report, named for the day it ran. A new run
adds a new file; no report is edited after it is written. Chapter 97's
report is a separate, frozen teaching example from `73966b6359`.

A report reads the consumer registry,
[coax/lib/nostr/consumers.coax](../../../coax/lib/nostr/consumers.coax), against
the tree. For each real consumer (the CLI, the Pylon, the API's NIP-98
check, the homepage, the project page, web sign-in, the hosted runner, the
unlock opener, the iOS app and the coder host) and each of its four axes
(sign, verify, key storage, transport), it says how the axis is done, in
which file, and whether a record from that consumer's target shows it
running. A consumer's profile is complete only when every axis it uses is
done in Coax with a record, or, for key storage and transport, by the host
platform with a record.

Make one from the repository root:

```sh
coax/web/tool.sh coax/tests/nostr/consumer_report.coax > nostr/book/reports/consumers-YYYY-MM-DD.md
coax/web/tool.sh coax/tests/nostr/consumer_report.coax --json > nostr/book/reports/consumers-YYYY-MM-DD.json
```

The tool exits 1 when the registry is stale: a source or its symbol is gone,
a build script is gone, or a named record is missing or doesn't name its
source.

Records are test-key or public-event evidence. A complete profile is not
approval to hold a production key or to move money; those stay owner
decisions in [NEEDS_OWNER.md](../../../NEEDS_OWNER.md).

| Report | Base commit | Complete profiles |
| --- | --- | --- |
| [consumers-2026-11-03.md](consumers-2026-11-03.md) ([JSON](consumers-2026-11-03.json)) | see the report | 0 of 10 |
| [consumers-2026-11-03-2.md](consumers-2026-11-03-2.md) ([JSON](consumers-2026-11-03-2.json)) | see the report | 2 of 10 (the API's NIP-98 check, the homepage) |
| [consumers-2026-11-03-3.md](consumers-2026-11-03-3.md) ([JSON](consumers-2026-11-03-3.json)) | see the report | 3 of 10 (the API's NIP-98 check, the homepage, the project page) |
