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
