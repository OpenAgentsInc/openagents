# Meeting action items

This release extracts explicitly marked action items from meeting notes supplied
in the request. Each item retains its owner, task, due date, and source line.
Unassigned items stay unassigned; completed checkboxes are omitted. A person
reviews the result before sending messages or creating tasks.

The workflow has one Wasm step, no required capabilities, and an empty snapshot
on the paid route. It uses the existing Rust action-items guest pinned in
`build-receipt.json`. It reads no connector or local file, sends no message,
and starts no task. Paste the notes themselves; a filename supplies no content.

The host clips notes above 32 KiB and caps instruction fuel at 1,000,000, memory
at 8 MiB, output at 64 KiB, and module bytes at 128 KiB. Truncation is visible
in the result. Extraction recognizes explicit markers, open checkboxes, action
headings, and named commitments; it does not infer hidden decisions or ROI.

Publish through the installed publisher after reviewing the exact package and
choosing the real per-call author fee and supported payout destination:

```sh
openagents plugin publish plugins/meeting-action-items --fee-msat FEE --payout DESTINATION --as PROFILE
```

These are the existing signed EXT fee fields. The receiver adds its separate
endpoint charge and retains the author's full declared fee. Compare both
parts in the quote and approve the exact release before paying. Publishing
new content or changing the fee requires a new package version.

For `POST /v1/plugins/PUBLISHER:meeting-action-items/invoke`, a request without
approval returns `409` with the current `quote` and `quote_digest`, without
an invoice. After reviewing that release and price, submit JSON containing
`{"quote_digest":"<approved digest>","request":"<the actual notes>"}`.
The resulting `402` invoice binds these exact body bytes. Retry those same
bytes with payment proof. A changed quote returns `409` and requires new
approval; an earlier proof cannot buy different release bytes.

The isolated acceptance fixture signs and resolves this package, checks every
blob and the Wasm digest, runs the exact guest on synthetic notes, and checks
the fake payment's author share. It records no real publication, customer
adoption, payment, or payout. Run its focused check with:

```sh
openagents lease build --keep-target-dir -- cargo test -p openagents-cli --bin openagents pay_plugin::tests::useful_release
```

The [retained fixture](../../bench/plugins/meeting-action-items/receipt.json)
records the tested package/program/Wasm digests, throwaway publisher, exact
signed release, fee, payout, and checks. O2/O8 in `NEEDS_OWNER.md` keep the
real offer unavailable until installed execution and settlement are qualified.

After changing these retained package bytes, regenerate the fixture by prefixing
the check with `PAID_PLUGIN_RECEIPT_WRITE=1`, then rerun without that variable.
