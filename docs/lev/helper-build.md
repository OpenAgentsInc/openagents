# Build the Lev helper from source

The helper's source is in `swift/lev-bridge`. Build it on an Apple Silicon Mac
with a toolchain that supports the package's macOS 26 target:

```sh
./scripts/build-lev-bridge.sh
```

The script builds the release executable at
`swift/lev-bridge/.build/release/lev-bridge` and verifies its code signature.
Running the helper also requires the Apple Intelligence prerequisites described
in [the Lev documentation](README.md).

Swift's `.build` directory and Python bytecode under `training/` are local build
products. Git ignores them. Do not add them as fixtures or commit a rebuilt
helper binary. Removing them from the index does not remove an operator's local
build or model artifacts. The retained measurement files, authored fixtures,
and `docs/transcripts/` remain tracked.

## Source-only build verification

For audit issue [#9430](https://github.com/OpenAgentsInc/openagents/issues/9430),
a fresh detached checkout of `e7d2eb47c0b826b20bcece666340a516eefd2cde` contained
neither `.build` nor the adapter harness's `__pycache__`. The documented build
script completed with Swift 6.3.3 for `arm64-apple-macosx26.0`; signature
verification passed. `git status --porcelain` was empty after the build, and
`git ls-files` listed no paths in either generated directory.

That build produced SHA-256
`d90d2a4194bb662be1da4cb47d49fc6707ea941d0084dcf9060d90a64655fad5`.
This identifies the verification binary; it is not a claim that different
compiler versions or build directories produce identical bytes. No binary was
published by this cleanup. It removed 79 Swift build paths and one Python
bytecode file from the index without changing the helper's source or build
script. No intentional generated fixture was removed.

## Metadata for a distributed binary

If a release distributes the helper, attach the binary as a versioned release
artifact with its source revision, target, Swift version, macOS version, and
SHA-256 digest. Record the digest after the final signing step. Capture the
metadata from the checkout and binary used for that release:

```sh
git rev-parse HEAD
swift --version
sw_vers
shasum -a 256 swift/lev-bridge/.build/release/lev-bridge
codesign --verify --strict swift/lev-bridge/.build/release/lev-bridge
```

Retain the build and signing procedure with the release. Keep the `.build`
directory out of source control; its machine-specific caches are not release
provenance.
