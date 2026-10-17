# layertree-web

The LayerTree demo in a browser (#11987), served at
[openagents.com/layertree](https://openagents.com/layertree). A shared
Bitcoin coin's Merkle-sum tree is drawn as a Verse scene on the engine
renderer (WebGPU, or WebGL2 where the browser has no WebGPU adapter):

- Thousands of members join, move their share to a new key, pay each other,
  split their share, and leave with a payout. Each change lights its
  member and climbs the tree to the root.
- The signer stands beside the tree with its check result. A signed
  transition sends a pulse to the shared coin; a refresh sends a wave down
  the tree.
- Light clients circle the tree and check each new block.
- Once a run, the signer is switched off. After the unroll delay the tree
  is broadcast in three levels and every member is paid their exact
  balance.

The run is a seeded, simulated `layertree.events.v1` stream from
`src/demo.rs`: a depth-12 tree with 2,600 members, 420 transactions with
blocks, light-client checks, refreshes and signer checks, then the
switch-off, 73 unroll transactions and one payout per member. The same seed
gives the same run; when a run ends the page starts the next seed. Add
`?run=signet` to replay the real signet run
(`crates/layertree-tee-lab/fixtures/events/signet-kill-paid.jsonl`) instead,
or press `R` on the page to switch. `?seed=N` picks another simulated run,
and `?gl` forces WebGL2.

The replay is the Verse viewer's own (`crates/verse/examples/layertree/`),
compiled in by path, so a run that the desktop viewer refuses is refused
here too. The demo's root hashes are SHA-256 Merkle-sum hashes over the
simulated leaves in this crate's own encoding; its keys and transaction IDs
are random. The scene says the run is simulated.

The engine renderer draws at most 1,024 instances a frame, so a large tree
draws its members and its lowest five levels as points in the overlay and
the seven levels above them as spheres and links. Trees with at most 64
slots, such as the signet run's, draw every member as a sphere.

## Build

```sh
./scripts/build-layertree-web.sh OUTDIR
```

It needs what `scripts/build-everglade-web.sh` needs: the
`wasm32-unknown-unknown` target, the `wasm-bindgen` CLI at the version
`Cargo.lock` pins, and a clang with the WebAssembly target for secp256k1's C.
It writes `layertree_web.js`, `layertree_web_bg.wasm`, `start.js`, and a
gzip copy of each.

## The page contract

The page must have a `<canvas id="layertree-canvas">` sized by CSS and load
`start.js` as a module from the same directory as the glue. Progress and
errors go to an element with the ID `layertree-status`. The site's
`/layertree` page does this (`crates/openagents-web/src/pages/layertree.rs`)
from the directory given with `--layertree DIR`, or `/srv/layertree` in the
image.

## Local test

```sh
out="$(openagents scratch)/layertree"
./scripts/build-layertree-web.sh --with-page "$out"
python3 -m http.server -d "$out" 8080
```

Open <http://localhost:8080/>. Drag to turn the camera, scroll or pinch to
zoom, and use the arrow keys to turn and zoom. The camera turns slowly on
its own after a few seconds without input.

## Tests

```sh
cargo test -p layertree-web
```

The tests replay the generated run end to end (every balance, leaf, and
payout checked), check that the seed decides the run, play the stage to the
end of the demo and of the real run, check the layout, and run the site's
copy guard over every caption.
