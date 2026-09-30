# Linux 1.0.0: published packages and the updater (2026-09-30)

[#10025](https://github.com/OpenAgentsInc/openagents/issues/10025), part of
[#10003](https://github.com/OpenAgentsInc/openagents/issues/10003). The
procedure is in [release.md](../../release.md#releasing-for-linux).

## Build

Commit `a934905d9c801ef3ee79ebda426566fefb27b136`, built on coderos-4080
(NixOS, x86_64) from a scratch clone under `/tmp`, with
`scripts/desktop/build-linux-release.sh` (image
`rust:1.97.1-bullseye@sha256:02d78ca3…42e0`, AppImage runtime 20251108
`2fca8b44…260d`, `SOURCE_DATE_EPOCH=1790800540`, container at CPU share 64
and `nice -n 19`). The release build took 2 min 52 s. The binaries need
glibc 2.30 or newer.

Built twice, the second time with its own empty Cargo home and target
directory: `SHA256SUMS` identical.

```text
8d06bf68f61a9a9114a13fe59b88f54c46926aa5456c6c9193e7ce26f6977890  openagents-1.0.0-linux-x86_64.tar.gz
38a9a0505684a40dca15d926b957ae9764b83b5eb0583497faf74d3202c5e2ce  openagents_1.0.0_amd64.deb
1d1d329b82c0b663556ccd0def890a00d0b031a26ae565c88aaa7144abbb0141  OpenAgents-1.0.0-x86_64.AppImage
```

## Published

Signed on the release Mac with `scripts/desktop/sign-manifest-linux.sh
--version 1.0.0 --upload` (key `desktop-update-2026-09`, public key
`b9c688e6…8b84`, the one in `TRUSTED_KEYS`):

- https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/OpenAgents-1.0.0-x86_64.AppImage
- https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/openagents_1.0.0_amd64.deb
- https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/1.0.0/openagents-1.0.0-linux-x86_64.tar.gz
- `SHA256SUMS`, `SHA256SUMS.sig`, `openagents-desktop-update.pub.pem`,
  `BUILDINFO`, `manifest.json` beside them
- https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/manifest.json
  (`cache-control: no-cache, max-age=0`)

## Checks

Downloaded from the public URLs on coderos-4080:

- `sha256sum -c SHA256SUMS`: all three OK.
- `openssl pkeyutl -verify … -in SHA256SUMS -sigfile SHA256SUMS.sig`:
  `Signature Verified Successfully`; the published public key's raw bytes
  are `b9c688e6…8b84`.
- The manifest payload names `"platform":"linux"`, version 1.0.0, and the
  AppImage and `.deb` by the SHA-256 and size above.

AppImage on coderos-4080 (NixOS, FUSE mount, nix-ld), in a scratch session:
its own `HOME` and XDG folders under `/tmp`, no display, no session bus, and
a `systemctl` on `PATH` that only records its arguments, so nothing reached
the owner's session, host unit, or screen:

- `--help` printed the usage; `coder --version` ran the bundled `coder`
  through `AppRun`.
- `--check-update`: `OpenAgents 1.0.0 is up to date.` (the app fetched the
  published manifest and verified it with the compiled key).
- `--capture DIR`: wrote 12 screens; [Settings](shell-settings.png) shows
  `Version 1.0.0`, and [the code screen](dsk-01-connect.png).

`.deb` in `debian:11` (glibc 2.31) and `ubuntu:22.04` containers:
`apt-get install --no-install-recommends ./openagents_1.0.0_amd64.deb`
installed `/usr/lib/openagents/{openagents-desktop,coder,microcoder}` and
`/usr/bin/openagents-desktop`; `--check-update` said up to date and
`--capture` wrote 12 screens on both. The AppImage also ran in
`ubuntu:22.04` (`APPIMAGE_EXTRACT_AND_RUN=1`).

## The updater, against a newer test release

A test release `1.0.1-test.1`: the published 1.0.0 AppImage repacked with
`usr/share/openagents/UPDATE-TEST`, and the `.deb` with that version and
file, signed with the real key by
`sign-manifest-linux.sh --prefix desktop/linux-test --upload` and read
through `OPENAGENTS_UPDATE_MANIFEST_URL`. The test objects were deleted
afterwards (the manifest URL now answers 404).

AppImage, in the scratch session above:

```text
before: 1d1d329b82c0b663556ccd0def890a00d0b031a26ae565c88aaa7144abbb0141
$ OpenAgents-1.0.0-x86_64.AppImage --check-update
OpenAgents 1.0.1-test.1 is available: https://…/desktop/linux-test/1.0.1-test.1/OpenAgents-1.0.1-test.1-x86_64.AppImage
$ OpenAgents-1.0.0-x86_64.AppImage --update
Downloading OpenAgents 1.0.1-test.1…
Updated /tmp/oa-10025/scratch/home/Applications/OpenAgents-1.0.0-x86_64.AppImage to OpenAgents 1.0.1-test.1.
after:  883a882504bc6945d0546d8e0c1b6bd938fdcd7017482e93784a88ef36a0d7dd  (the signed test AppImage)
mode 755, no leftover .incoming file
systemctl calls: systemctl --user try-restart com.openagents.desktop.host.service
UPDATE-TEST inside the replaced file: "update test 1.0.1-test.1"
```

Refusals, from a local server on `127.0.0.1`: the same envelope with its
payload's version changed printed `the update manifest's signature does not
verify`, and one naming another key `the update manifest is signed by an
unknown key`; both exited 1 and left the AppImage byte-for-byte as it was.

`.deb` 1.0.0 in `ubuntu:22.04`: `--check-update` and `--update` both
printed `OpenAgents 1.0.1-test.1 is available: …/openagents_1.0.1-test.1_amd64.deb`
and `Install it with your package manager.`; the installed files and the
package version (1.0.0) were unchanged.

Not run here: the window's **Restart to update** button and its relaunch
(it calls the same `Updater::install` as `--update`, then starts the new
file after the window exits); opening the window on a Linux desktop. Both
are owner checks in `NEEDS_OWNER.md`.

Tests: `cargo test -p openagents-desktop` and `cargo clippy -p
openagents-desktop --all-targets --no-deps -D warnings` pass on macOS and in
the Linux build image.
