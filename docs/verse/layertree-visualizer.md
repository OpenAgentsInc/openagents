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
