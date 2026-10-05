# Shared engine and dedicated host checks

Evidence for audit V17 and [issue #10636](https://github.com/OpenAgentsInc/openagents/issues/10636).
The dedicated Rust host serves two original worlds through the shared content,
scene, collision, authority, and checkpoint contracts. The actual Verse renderer
uses `verse-pbr` and engine presentation contracts; CPU content compilation lives
in `verse-content` with an optional compiler feature.

Commands use the pinned toolchain and the retained agent Cargo target directory:

```sh
cargo test --lib -p verse-host -p verse-content -p verse-engine -p verse-world \
  -p verse-gfx -p verse-pbr -p verse --no-default-features \
  --features verse/remote-chamber,verse/capture,verse-content/compiler,verse-pbr/imported-surface \
  --no-fail-fast -- --test-threads=1
cargo test -p verse-host --test worlds -- --nocapture
cargo tree -p verse-host --edges normal --prefix none
cargo build -p verse-content --features compiler
cargo build -p verse-host
python3 bench/verse/2026-10-05/engine-boundaries/check-tools.py --binaries "$CARGO_TARGET_DIR/debug"
cargo check -p openagents-desktop -p coder-mobile -p openagents-cli
cargo check -p verse --no-default-features --features web --target wasm32-unknown-unknown
cargo fmt -p verse-content -p verse-host -p verse-engine -p verse-world \
  -p verse-gfx -p verse-pbr -p verse --check
```

The final library suite passes 1,269 tests: Verse 509, content 11, engine 136,
graphics 25, host two, renderer 72, and world 514. Eighteen intentional GPU,
artifact-writing, and subprocess helper checks are ignored. The standalone
integration passes one test. Native, browser, compiler commands, minimal host
build/checks, formatting, and local documentation links pass.

The native check uses the local ALSA development pkg-config directory. Library
checks run serially because parallel software-GPU fixtures crashed in an earlier
run. They cover object-layout preservation and exact legacy sorted-map journal
recovery with the same dependency feature union as the application. The earlier
`world-service.log` records 200 service tests passing after the writer fix;
`tests.log` covers the final legacy compatibility changes and latest integration.
The native/browser checks precede the final Everglade digest literal repin; the
full Verse library suite checks that regenerated artifact afterward. The later
rebase contains only unrelated Grove web and deployment changes.

`worlds.log` records scratch compilation of ritual combat and observatory social
content, generic render-frame admission, 300 scheduled check ticks per world,
actual TLS host processes, enrolled/content-bound clients, live ticks, an
observatory switch mutation, model/texture refusals, and clean SIGTERM shutdown.
`tools.log` records the standalone build and both real compiler commands, with
existing-directory and unknown-recipe refusals. Compiler output and scratch
credentials are deleted after the checks; they are not retained artifacts.
`host-dependencies.txt` is the normal host graph, checked for excluded rendering,
window, font, glTF compiler, private-reader, Coder, and agent dependencies.
`grid-regeneration.log` records the source inventory repin after the extraction.
`tests-initial.log` records 508 passes and one stale generated Everglade pack.
`everglade-regeneration.log` records its rebuild; the Linux digest is repinned,
and the previous reviewed pack remains for the retained platform digest path.
`standalone-host.log` records a separate default-feature host build and 300
check ticks for each compiler-produced world through that exact binary. The
retained `check-tools.py` reproduces those checks without credentials or a
listener; its enrollment is the public secp256k1 generator point.

These checks establish two working engine consumers on this machine. They do
not measure MMO population, raid throughput, arbitrary world authoring,
cross-platform replay, or device presentation. REACH remains an OpenAgents CLI
integration; the independent host uses TLS. Original furnishing collision keeps
its model whitelist, and `Entities` remains an unused helper. No owner host,
display session, device, release gate, Clippy, or GitHub automation was used.
