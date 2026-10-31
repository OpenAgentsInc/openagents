# verse-kms: a realm on the display hardware

`verse-kms` runs a Verse realm straight on a Linux screen: no compositor, no
browser, no GPU API. It takes the display with DRM/KMS dumb buffers, reads
the keyboard from evdev, and rasterizes on every CPU core. The realm's Coax
steps the game and writes its strokes, exactly as it does on the site; the
realm's light, which is WGSL on the site, is scalar Coax here
(`starcatch-light.coax`). `kms.c` is the driver and decides no game rule or
colour.

The first realm is Star Catcher (`verse/realm-template/`). On 2026-10-30 it
ran on `coderos-4080` (RTX 4080, DP-1 at 2560x1440@60): about 40 fps, 14 ms
of drawing a frame on 28 threads, and the compositor resumed after.

| File | What |
| --- | --- |
| `kms.c` | The host: KMS mode set and page flips, evdev keys, the threaded rasterizer, `--headless` and `--shot` |
| `realm.h` | What the host asks of a realm: the browser face's contract plus its light |
| `starcatch-native.c` | Star Catcher's face over `starcatch.coax` and `starcatch-light.coax` |
| `starcatch-light.coax` | Star Catcher's sky, hill and strokes, following `starcatch.wgsl` by hand |
| `build.sh` | Coax to C, then `cc` (with libdrm on Linux; headless only on macOS) |
| `run.sh` | As root: switch to a free VT so the compositor lets go, run, switch back |

## Run it

```sh
verse/kms/build.sh                                  # Mac: generates the C, builds headless
verse/kms/build/verse-kms --headless 2560x1440 --shot /tmp/f.ppm
```

On a NixOS box without `coax`, copy `verse/kms/build/` there and build in
`nix-shell -p gcc pkg-config libdrm` with `build.sh` (it reuses the
generated C). Then, at the machine or over SSH:

```sh
sudo verse/kms/run.sh verse/kms/build --seconds 300
```

Space starts, the arrows move the basket, Escape leaves. The keyboard is
grabbed while it runs, so VT keys don't work; Escape or the time limit
always gives the screen back.

## Next

- Hold 60 fps: draw the next frame while the flip is pending.
- More realms: one `-native.c` and one `-light.coax` each, until Coax emits
  WGSL and the light has one source.
- The GPU: the same light through `coax cuda` (as Psionic runs on this
  card), into the scanout buffer.
- A CoderOS session that boots into it. See
  `docs/os/2026-10-30-coderos-and-coax-audit.md`.
