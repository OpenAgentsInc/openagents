# Free labor acceptance evidence

This is synthetic, reproducible evidence for the bounded host described in
[Recoverable free coding orders](../../runtime/free-labor.md). The buyer,
provider, and resolver keys are public test fixtures, controlled by one test
operator. No real account credentials, private source, wallet, model calls,
or public deployment participated.

The [receipt](receipt.json) records one successful order: 10 signed private
relay events, a 47 ms provider process, and a separately granted 60 ms buyer
checker. The provider changed a synthetic Rust function from `41` to `42`;
the buyer compared retained candidate bytes with expected bytes frozen before
dispatch. Both reconstructed the same signed acceptance after relay shutdown.
The price is zero. All-in resource cost remains unknown.

Retained files:

- [Relay events](relay-events.json): exact signed encrypted event objects.
- [Buyer journal](buyer-journal.json) and
  [provider journal](provider-journal.json): original events, artifact closures,
  transitions, locally observed times, and dispatch state.
- [Interrupted provider journal](interrupted-provider-journal.json): explicit
  fault injection of the intent-before-submit boundary. Recovery and duplicate
  dispatch remain unknown and create no task.
- [Provider task](provider-tasks/tasks.json),
  [full provider ATIF trace](provider-tasks/labor-request.1.atif.jsonl), and the
  artifact manifest/blob beside them.
- [Buyer checker task](buyer-check-tasks/tasks.json) and
  [full checker ATIF trace](buyer-check-tasks/buyer-check.1.atif.jsonl).
- [Test log](tests.log), [Clippy log](clippy.log), and
  [decoder regression log](nostr-test.log).
- [File manifest](manifest.json): SHA-256 and byte size for the retained files.

The relay is a real loopback WebSocket fixture with signature validation,
NIP-42 AUTH, private-recipient reads, and duplicate event storage. It is not the
production `nostr-relay` binary or its Postgres deployment. Reconnection occurs
on each read/write. The separate relay test rejects an unrelated reader,
a wrong authenticated publisher, and a modified signed event.

Twelve labor tests passed. The Nostr regression confirms that generic workers
refuse the required labor feature and only the explicit decoder preserves and
opens the original signed request. Strict package Clippy passed. These checks
do not establish physical-host isolation across independent operators or a
production service guarantee.

Temporary absolute paths inside original receipts identify the execution
that produced them. They are historical observations, not portable paths to
follow. The artifact bytes and traces are retained here; use the linked guide
to generate new local paths in a fresh fixture.
