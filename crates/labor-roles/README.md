# labor-roles

The [NIP-LAB](../../nips/openagents/NIP-LAB.md) roles after execution:
independently operated worker, reviewer, buyer and resolver hosts, each
with its own key, directory and journal, exchanging signed private `3188`
records. Bounded rework, resolver adjudication under `labor-evidence-v1`,
cancellation and an unavailable buyer, rights, final records, and the
obligation's collection state, funded through `pay-ledger`.

See [the runtime guide](../../docs/coder/runtime/free-labor.md#delivery-roles-after-execution).

```sh
cargo test -p labor-roles
```

`context::admit_rework` checks a repair manifest against the current buyer
review and preceding submission. It preserves the original context entries,
flags, recipient, policy, omissions, and coverage. Appended evidence must be
exact retained review, deliverable, execution, or RUN bytes under the original
rights, with the authorizing review present. Changed source bytes, another
recipient, cancellation, late repairs, and substituted context refuse. This
validates context only; it neither grants execution nor restarts an attempt.

Each participant holds one writer lock and binds its journal to the exact terms.
Replay compares received records with the original signed plaintext and requires
that foreign records retain their original event. A failed write stops further
publication until the host reopens; a duplicate cannot publish an unsaved
record. A missing initialized journal refuses to start a new history. Journals
are bounded to 4,096 records and 16 MiB and are saved with private permissions,
a synced file, an atomic rename, and a directory sync.
