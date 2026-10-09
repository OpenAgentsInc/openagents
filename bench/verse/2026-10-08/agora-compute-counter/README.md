# The Agora's compute counter (#10924)

`agora-compute-counter.png` is Everglade's Agora forecourt, rendered
offscreen by `everglade_capture`'s `agora` view. It shows the following:

- The compute counter: four online pylons (three relay pylons and this
  computer's own), their free slots, the day's jobs, the jobs the broker
  bought, and the sats paid on receipts, marked TEST.
- The agent-services wall: three verified NIP-MKT offerings for agent
  services on the pool, each priced in test sats.
- Pale settlement threads that rise from the counter and arc over the hall
  to the Pylon Field, one for each broker job that finished in the last 15
  seconds.

The data is real records on an in-process loopback relay
(`cargo run -p pylon --features fixture --example agent_market`). Three
free pylons run on fake engines and serve only the broker key. Three crew
members publish offerings. The loop alternates two kinds of job:

- Alice hires one of the crew through a NIP-LAB order. The broker buys
  the order's job from the pool, and Alice pays the order's price to the
  receiver after she accepts the answer. The split ledger records the
  seller's fee, the provider's share, and OpenAgents' share, tied to the
  job's `3201` receipt.
- A customer's brokered x402 job, whose receipt carries its payment.

Every sat is a worthless testnet sat in `TestLightning`. No live relay or
wallet was involved.

To reproduce:

```sh
cargo build -p verse --example everglade_capture
cargo run -p pylon --features fixture --example agent_market -- 900
# it prints RELAY ws://127.0.0.1:PORT and BROKER <hex>
OPENAGENTS_PYLON_BROKERS=<hex> VERSE_PYLON_RELAY=ws://127.0.0.1:PORT \
  VERSE_CAPTURE_COMPUTE=live VERSE_CAPTURE_COMPUTE_WAIT=market \
  "$CARGO_TARGET_DIR/debug/examples/everglade_capture" agora-compute-counter.png agora
```

Set `VERSE_PYLON_RELAY` before you run the capture: without it, the capture
reads the production relay.
