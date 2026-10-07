# pylon

The NIP-PYLON compute provider and its client. A pylon publishes beacons
(`30200`), answers free NIP-CJ conversation jobs (`25900` to `26900`,
NIP-44 encrypted) on a local Psionic model server, and leaves receipts
(`3201`) and pool aggregates (`30201`) to its buyers and aggregators.

| Module | Does |
| --- | --- |
| `provider` | Beacons, admission (allowlist, per-buyer rate, slots, bounds), jobs, encrypted answers. |
| `client` | Discovery from beacons, one encrypted job, the receipt. |
| `field` | `RelayField`: verified pylons as NIP-PYLON `pylon` world states, for Verse. |
| `pool` | Compute, publish, and verify a pool aggregate. |
| `engine` | `Psionic` (OpenAI-compatible loopback HTTP) and the `Echo` test engine. |
| `job`, `relay`, `identity` | The CJ wire, relay plumbing over `nostr-transport`, and `0600` key files. |

The record types and every check live in `nostr::pylon`.

`openagents pylon --help` lists the commands; `cargo build -p pylon --bin
pylon` builds the same commands without the rest of the workspace.
`tests/end_to_end.rs` runs a provider and buyers against an in-process relay
with the echo engine. Read [`docs/compute/pylon.md`](../../docs/compute/pylon.md).
