# everglade-web

Everglade in a browser. This crate is a `cdylib` over Verse's `web` feature:
it fetches the pinned Everglade pack from the page's own origin, checks its
length and SHA-256, installs Everglade directly (no plaza, relay, or studio
host), and draws it with WebGPU, or with WebGL2 where the browser has no
WebGPU adapter. The same module draws the shared Grid ([Grid
mode](#grid-mode)). A native build of the crate holds only the plain data
its tests cover.

## Build

```sh
./scripts/build-everglade-web.sh OUTDIR
```

The script runs `cargo build --release --target wasm32-unknown-unknown -p
everglade-web`, then `wasm-bindgen --target web`, then `wasm-opt -O2` when
Binaryen is installed. It needs:

- The pinned toolchain's `wasm32-unknown-unknown` target.
- The `wasm-bindgen` CLI at exactly the version `Cargo.lock` pins for the
  `wasm-bindgen` crate (0.2.128 when this was written): `cargo install
  wasm-bindgen-cli --version 0.2.128 --locked`. The script refuses another
  version.
- A clang with the WebAssembly target, for secp256k1's C. Apple's clang has
  none; the script finds Homebrew's `llvm`, or set
  `CC_wasm32_unknown_unknown` (and `AR_wasm32_unknown_unknown`).

## Output and the page contract

The script writes exactly these files, directly in `OUTDIR`:

| File | What it is |
| --- | --- |
| `everglade_web.js` | The ES module glue. Its default export, `init`, loads the module. |
| `everglade_web_bg.wasm` | The module (about 11 MB; about 7.7 MB with gzip). |

The page that serves them must:

- Have a `<canvas id="everglade-canvas">`, sized by CSS. The module draws on
  that canvas, sets its drawing-buffer size from its laid-out size and the
  device pixel ratio, and sets `touch-action: none` on it.
- Import the glue from the same origin and call
  `init({ module_or_path: "<url of everglade_web_bg.wasm>" })`. The module
  starts itself when `init` finishes (`#[wasm_bindgen(start)]`).
- Serve the pinned pack at `/everglade/pack/<PACK_SHA256>.vtp` on the same
  origin, where `PACK_SHA256` is
  `verse::zones::everglade_pack::PACK_SHA256` and the file is
  `assets/verse/everglade/<PACK_SHA256>.vtp` (`PACK_BYTES` long). The module
  fetches it without credentials, refuses a redirect, stops reading past
  `PACK_BYTES`, and refuses bytes whose length or SHA-256 differ. A new pack
  digest is a new URL, so the file can be cached as immutable.

Two query parameters help captures: `at=X,Z` or `at=X,Z,YAW` starts the
player at that point of Everglade, and `frames` logs a line to the console
once a second, `Everglade frames {...}`, with the frame gaps, the page's
work per frame, and the town's raised buildings, pieces, and chunks.

Add `?demolition` to the page's URL to open the demolition yard instead of
the glade, as `verse --demolition` does: `1` or a quick tap swings the
sledgehammer, `2` aims Meteor Swarm (a click or a quick tap casts it where
the circle is, and right click or `Esc` cancels), and `R` rebuilds the
cottages.

The glade itself has no offensive spell: its hotbar is Levitate and four
utility spells on keys 1 to 5. The `dev-destruction` feature that puts
Meteor Swarm and the sledgehammer on it does not compile for `wasm32`, so
this module can't carry it.
### Grove mode

The same module starts in the Grove, the druid training field
(`docs/verse/druid-demo.md`), when the page's path ends in `/druid`, as
openagents.com's `/druid` does, or when its URL has the query parameter
`zone=grove`, for example `/?gl&zone=grove`. The Grove is built on the same
pinned pack, so the page serves the same files and nothing else changes.

### Grid mode

The module opens the shared Grid instead when the page's path ends in
`/grid`, as openagents.com's `/grid` does, or with the query parameter
`zone=grid`. The Grid's pack is built into the module, so nothing
downloads. The player joins `verse-bare` over the browser's WebSocket with
the phone's presence session, sees the other players with their names, and
can click a name tag to block or mute that player. The page keeps the
player's key, display name, and block and mute lists in its local storage.
Query parameters:

- `relay=URL` joins another relay (the public relay by default); the
  page's content security policy must admit it.
- `name=NAME` sets and keeps the display name.
- `offline` opens the Grid without joining.

Once a second the module logs `Grid frames {...}` to the console: frame
times, the players drawn, and the session's status and refusals. The
receipt is `docs/verse/verification/2026-10-05-grid-browser/`.

Download progress and errors appear in an element with the ID
`everglade-status`. The module creates one at the bottom left when the page
has none, and hides it when the glade appears.

## Local test

```sh
./scripts/build-everglade-web.sh --with-pack /tmp/everglade
python3 -m http.server -d /tmp/everglade 8080
```

`--with-pack` adds the test page `index.html` from this crate and copies the
committed pack to `everglade/pack/`, so the server root is a complete site.
Open <http://localhost:8080/>.

## Controls

The shared controller, mapped as on desktop:

- `W`/`S` or the up and down arrows walk; `A`/`D` or the left and right
  arrows turn; `Q`/`E` strafe; `Shift` runs; `Space` jumps.
- A left drag orbits the camera, a right drag turns the character, both
  buttons walk forward, and the wheel zooms.
- On a touch screen, one finger turns the character and tilts the camera,
  and two fingers walk forward; pinching them zooms.

In the Grove, keys `1` to `9` cast the druid's spells on the hotbar
(Thunderwave, Gust of Wind, Wind Wall, Wall of Stone, Reverse Gravity, Fire
Bolt, Fireball, Misty Step, and Web), `0` is Long Rest, and a click or tap
on a slot casts it. Spells aim at the dummy nearest the character's facing.

## Authoritative chamber mode

Open `?zone=chamber` to mount the shared authoritative chamber session over
authenticated REACH WebSocket. See [configuration and the platform feature
matrix](../../docs/verse/platform-clients.md). This mode requires explicit host
enrollment and admitted content; the offline glade and Grid presence remain
separate modes. Wasm compilation does not establish browser device acceptance.

## Admitted host terminal

Call `open_host_terminal(config)` with the enrolled device key, host-signed
access, host generation, terminal reference, optional session, advertised
capabilities, mode, and admitted route. The Rust workbench uses the shared
terminal renderer and controls. `close_host_terminal()` detaches without
closing the host terminal; `host_terminal_receipt()` exports content-free
render timing. Resource IDs persist for reattachment; credentials and shell
input do not. Physical browser qualification remains in `NEEDS_OWNER.md`.
