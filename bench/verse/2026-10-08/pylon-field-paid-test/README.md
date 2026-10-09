# The Pylon Field with paid test-sat jobs (#10923)

`pylon-field-paid-test.png` is Everglade's Pylon Field, rendered offscreen
by `everglade_capture`. It shows three relay pylons, each with a TEST coin
over its point, beside this computer's OWNER pylon.

The data is real records on an in-process loopback relay
(`cargo run -p pylon --features fixture --example paid_field`). Three priced
pylons (3 sats a job, regtest) run on fake engines. A buyer pays each job
from `TestLightning`, an in-memory regtest network of worthless sats. Each
`3201` receipt carries a preimage that hashes to its payment hash. Verse's
relay source verifies those receipts, sums the paid msat under `regtest`,
and lights each pylon's coin for 15 seconds after its newest paid receipt.
The coins are pale and marked TEST because the sats are test sats. No live
relay or wallet was involved.

To reproduce:

```sh
cargo build --release -p verse --example everglade_capture
cargo run --release -p pylon --features fixture --example paid_field -- 600
# it prints RELAY ws://127.0.0.1:PORT
VERSE_PYLON_RELAY=ws://127.0.0.1:PORT VERSE_CAPTURE_COMPUTE=live \
  VERSE_CAPTURE_COMPUTE_WAIT=coin \
  "$CARGO_TARGET_DIR/release/examples/everglade_capture" pylon-field-paid-test.png pylons
```
