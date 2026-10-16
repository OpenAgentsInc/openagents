# LayerTree visualizer

The `layertree` Verse example shows a tree of balances from a
`layertree.events.v1` file. It uses the engine Grid floor, gold for value,
and white for the Bitcoin output. Leaves grow with their balance; small
stubs mark empty subtrees. Gold dots carry the sums toward the root.

![A LayerTree regtest run above the Verse Grid](captures/layertree/tree.jpg)

Run these commands from the repository root:

```sh
# Open the scene. Arrow keys orbit and zoom; hover a leaf for its exit key.
cargo run -p verse --no-default-features --features imported-desktop \
  --example layertree -- \
  --events crates/layertree-tee-lab/fixtures/events/regtest.jsonl --at 8 --visual

# Write a 1080p frame without opening a window.
layertree_scratch="$(openagents scratch)"
cargo run -p verse --no-default-features --features imported-desktop \
  --example layertree -- \
  --events crates/layertree-tee-lab/fixtures/events/regtest.jsonl --at 8 \
  --size 1920x1080 --scale 1.5 --capture "$layertree_scratch/layertree-tree.png"

# Render the five-second orbit to a 1080p film clip.
cargo run -p verse --no-default-features --features imported-desktop,crypt-fight \
  --example film_shot -- docs/verse/film/verse-shots/layertree-tree.json \
  --out "$HOME/work/film-out/verse/layertree-tree.mp4"
```

`--at N` selects the state after event N; omitting it selects the final
state. `--size WIDTHxHEIGHT` sets physical pixels, and `--scale` sets text
scale from 1 to 4. Captures do not open a window. The image above uses
`fixtures/events/regtest.jsonl`, event 8, with test keys and regtest coins.

The viewer supports depths from 1 to 16. It collapses empty subtrees and
shows groups for large trees. The layout checks every parent sum against
its children; the root's displayed balance must equal the run's output
balance. The reader refuses skipped events, mixed networks, mismatched
leaves, and inconsistent balances. It reads public JSON only and does not
perform Bitcoin or hardware-attestation verification. There is no Cargo
dependency on the excluded lab crate.

The example and film scene share the same replay, layout, geometry, and
palette. They are development examples and do not ship in a Coder binary.

## Playback

Add `--play --from 0 --to 17 --rate 0.6` to step through a run. The rate
is events per second, including block and client events. A single PNG with
`--play` shows the final selected event; add `--capture-every 1` to write
numbered frames to the directory passed to `--capture`.

The transition animation drops joining leaves into place, resizes updated
leaves with two signature pulses, sends exits to their exit keys, shows a
lease-expiry pause before eviction, and draws voucher arcs with their new
sequence number. Heartbeats move the output along the rail. Refused
operations flash red and leave the balances unchanged. Displayed balances
always belong to the checked destination event; only geometry interpolates.

`docs/verse/film/verse-shots/layertree-transitions.json` renders a 24-second
demonstration of every move. Its source,
`docs/verse/fixtures/layertree-transitions.jsonl`, is simulated throughout.
The viewer's `simulation` event extends the public log for illustration:
it requires `synthetic: true`, uses the same old/new leaves and balances
as a transition, and also accepts `split`, `merge`, `rewrite`, and `detach`.
The lab does not accept this illustration format as a signed record. The
scene labels it **simulated**; its hashes and transactions are illustrative.
For confirmed transactions, select the regtest log instead:

```sh
cargo run -p verse --no-default-features --features imported-desktop \
  --example layertree -- \
  --events crates/layertree-tee-lab/fixtures/events/regtest.jsonl \
  --play --from 0 --to 17 --rate 0.6 --visual
```
