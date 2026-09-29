# openagents-desktop

**OpenAgents** for Mac: the desktop app a person installs so their phone can
connect to this computer by scanning a QR code. The design is
[Connect a computer by scanning a QR code](../../docs/coder/design/2026-09-29-auto-pairing.md);
the screens are `DSK-01` to `DSK-03` in the
[wireframe](../../docs/product/2026-09-28-app-wireframe.md).

| Screen | What it shows |
| --- | --- |
| `DSK-01` Connect a phone | The QR code, **Scan with the OpenAgents app on your phone.**, **Let this phone open a terminal on this Mac** (off), and **Can't scan? Copy a code instead**. |
| `DSK-02` Connected | **Kai's iPhone is connected.**, **Pick a project for Coder** with **Choose folder…**, whether Codex and Claude Code are signed in, and **Let my phone start Coder here**. |
| `DSK-03` Home | **Online. Your phone can reach this Mac.** or **Offline.**, the phones with **Remove**, Coder's recent tasks, and **Connect another phone**. |
| Adoption | **Use this Mac's existing Coder setup?** on a Mac set up the old way. |

The screens are Rust Native views ([`src/screens.rs`](src/screens.rs)),
drawn by [`rust-native-desktop`](../rust-native-desktop/). The window process
holds no secret key: it talks to the host only over the local control socket
([`src/control.rs`](src/control.rs), `openagents-connect`'s protocol). The
host, `coder host serve`, runs as the login agent the app registers with
`SMAppService`, and reads its keys from the keychain through
[`src/keychain.rs`](src/keychain.rs). Adopting an old-style setup runs in a
separate helper process ([`src/migrate.rs`](src/migrate.rs)).

When a code shows, rotates, and is cancelled is [`src/codes.rs`](src/codes.rs);
`INVARIANTS.md` (Linking devices) states the rules.

## Run it

```sh
# The window against an in-process host; a phone "scans" after 8 seconds.
cargo run -p openagents-desktop -- --fake-host --fake-scan 8

# The window against this Mac's host (its control socket).
cargo run -p openagents-desktop -- --no-login-agent

# Every screen as PNG files.
cargo run -p openagents-desktop -- --capture /tmp/openagents-desktop
```

### The macOS app

```sh
bins/openagents-desktop-macos/bundle.sh
open target/release/OpenAgents.app
```

The script builds `openagents-desktop`, `coder`, and `microcoder` for this
Mac and assembles `OpenAgents.app` with the login agent's plist, signed ad
hoc. `SKIP_CODER=1` leaves the Coder binaries out. The signed, notarized
`.dmg` is `scripts/desktop/package-macos.sh` ([release](../../docs/desktop/release.md)).

On a Mac that already runs Coder from an earlier setup
(`~/.openagents/coder-access/`), the app does not register its own agent; it
asks whether to use that setup, and offers **Use it** only once the bundled
`coder` reads its keys from the keychain.

## Test it

```sh
cargo test -p openagents-desktop
UPDATE_SNAPSHOTS=1 cargo test -p openagents-desktop   # record a screen change
git diff crates/openagents-desktop/snapshots
```

## Screenshots

The bundled app's window, captured with `screencapture -l`: against the
in-process host ([connect](screenshots/dsk-01-connect.png),
[terminal allowed and code copied](screenshots/dsk-01-terminal-copied.png),
[connected](screenshots/dsk-02-connected.png),
[home](screenshots/dsk-03-home.png),
[remove](screenshots/dsk-03-remove.png)), and on a Mac set up the old way
([the question](screenshots/earlier-setup.png),
[home with Coder's real tasks](screenshots/dsk-03-earlier-setup.png)).
