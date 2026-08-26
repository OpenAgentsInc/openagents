# Release the OpenAgents CLI

`ops/release-cli.sh` builds the `openagents-cli` crate for every platform the
public installer knows how to ask for, signs and notarizes the macOS artifacts,
writes a checksum file, and publishes the result to the release bucket. An
operator runs it from a terminal. There is no hosted build service and no
GitHub Actions workflow.

## What the installer expects

The installer served at `https://openagents.com/install.sh` is the contract.
Under a base URL of `https://openagents.com/releases` it fetches three shapes:

| Path | Body |
| --- | --- |
| `<base>/<channel>` | A bare version string, such as `0.1.0` |
| `<base>/openagents-<version>-<platform>` | The executable |
| `<base>/SHA256SUMS-<version>` | One `<sha256>  <name>` line per platform |

The platform strings are `macos-aarch64`, `macos-x86_64`, `linux-x86_64`,
`linux-aarch64`, and `windows-x86_64`.

Two details of that contract are easy to get wrong, so the script derives both
rather than leaving them to a human:

- The artifact URL never carries a file extension, on any platform. The
  installer computes the URL before it branches on Windows and never revisits
  it.
- The `SHA256SUMS` entry for Windows *does* carry `.exe`, because the installer
  appends that suffix to the name it searches for. The published object and the
  checksum line therefore disagree by design. Changing either one alone breaks
  every Windows install with a checksum mismatch.

## Prerequisites

- A Rust toolchain with the five targets installed. Add a missing one with
  `rustup target add <triple>`.
- `cargo-zigbuild` and `zig`, which cross-compile the Linux and Windows targets
  from macOS.
- `gcloud`, for publishing.
- For macOS artifacts, the Developer ID Application certificate in the login
  keychain and an App Store Connect API key. The script reads the key id,
  issuer, private key path, and signing identity from the operator secret file
  named by `OPENAGENTS_NOTARY_ENV`.

## Build without publishing

Start here. The script stages everything under `dist/releases/<version>/`,
which is ignored by Git, and reports what it built:

```sh
ops/release-cli.sh --version 0.1.0-rc.1
```

Restrict the attempt to a subset while you iterate:

```sh
ops/release-cli.sh --version 0.1.0-rc.1 --targets "macos-aarch64 linux-x86_64"
```

## Publish

```sh
ops/release-cli.sh --version 0.1.0-rc.1 --publish
```

Publishing uploads the artifacts and the checksum file. It does not move any
channel, so nothing that resolves `stable` sees the new version yet.

Point a channel at a version only when you intend readers to receive it:

```sh
ops/release-cli.sh --version 0.1.0 --publish --channel stable
```

## What the script refuses

Each refusal exists because the failure it prevents is silent.

**A binary that does not match its platform name.** After every build the
script reads the produced file's actual Mach-O, ELF, or PE header and compares
it against the header that platform must have. A cross-compilation that fails
while leaving a stale or host-native binary at the output path would otherwise
publish a macOS executable under a `linux-x86_64` name, and the first person to
learn about it would be a reader whose install produced a file their kernel
refuses to run.

**A partial release, unless you say so.** If any requested platform fails to
build, the script names exactly which ones and exits rather than publishing.
The installer reports a bare download failure, not "unsupported platform", so a
channel pointing at a version that only covers three platforms looks to
everyone else like an outage. Pass `--allow-partial` when a partial release is
what you actually want.

**A prerelease claiming a channel.** A version with a suffix, such as
`0.1.0-rc.1`, cannot become the target of a channel without
`--allow-prerelease-channel`. Rehearsals publish release candidates; the
channel a reader resolves by default should keep meaning a real release.

**An unsigned or unnotarized macOS artifact.** Signing failure, a missing
secret file, or a notarization result other than `Accepted` stops the run. Pass
`--skip-notarization` to build macOS artifacts without submitting them, which
is useful while iterating and is recorded as `skipped` in the manifest.

## Why macOS artifacts ship bare rather than zipped

Apple cannot staple a notarization ticket to a bare Mach-O executable. Only a
container such as a `.zip`, `.dmg`, or `.pkg` carries a stapled ticket, and
`xcrun stapler staple` on a bare binary fails with error 73.

The artifact still ships bare, for two reasons. The installer downloads a
single file and marks it executable, so shipping a container would mean
changing a landed contract. And the thing stapling buys — offline Gatekeeper
verification — is not consulted on this install path at all, because `curl`
sets no `com.apple.quarantine` attribute on what it writes.

The artifacts are signed and notarized anyway, so that the paths where
quarantine *does* apply still succeed. `spctl --assess -t install` reports
`source=Notarized Developer ID` against the published binary, which is Apple's
online lookup of the ticket recorded for that code directory hash. The script
prints that assessment during every macOS build.

## Credential handling

The script passes the App Store Connect key, key id, and issuer to
`notarytool` on each invocation rather than storing a `notarytool` keychain
profile. `notarytool store-credentials` copies the private key into the login
keychain, which would leave a second durable copy of a credential that already
exists on disk. Reading the operator secret file at call time leaves nothing
new behind.

Never print the key, its contents, or a `.p12` password into a log, a commit
message, or an issue comment. Reference them by path and variable name.

## The build manifest

Every run writes `dist/releases/<version>/release-manifest.json` recording, per
platform, the Rust target triple, the builder, the SHA-256, the byte count, the
notarization status, and the notarization submission id. It also records the
Git commit the artifacts were built from. Keep it with the release record; it
is the evidence for what shipped.

## Verifying a published release by hand

```sh
curl -fsSL https://openagents.com/releases/SHA256SUMS-0.1.0-rc.1
curl -fsSL -o oa https://openagents.com/releases/openagents-0.1.0-rc.1-macos-aarch64
shasum -a 256 oa
codesign -dv --verbose=4 oa
spctl --assess -vv -t install oa
```

## Known gap: the embedded version

The `--version` value names the release, but the version the binary reports
comes from the `openagents-cli` crate manifest. A release candidate built from
a crate at `0.1.0` installs correctly and then reports `0.1.0` rather than
`0.1.0-rc.1`. The two agree for a real release and diverge for a rehearsal.
Closing this means threading the release version into the build.
