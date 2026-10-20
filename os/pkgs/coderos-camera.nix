# The CoderOS camera daemon as a Nix package, built from this repository.
#
# The source filter, the vendored dependencies' `cargoHash`, and
# `--ignore-rust-version` are the same as `pkgs/coder-compositor.nix`, for
# the reasons that file gives.
#
# The hand tracker in `crates/coder-hands` runs its landmark model through
# `ort`, whose build script downloads a static ONNX Runtime from pyke's CDN
# at build time. A build in the sandbox has no network, so this file fetches
# the same archive by the digest `ort-sys` records in its `dist.tsv`,
# unpacks it, and names the directory in `ORT_LIB_LOCATION`.
# `crates/coder-hands/Cargo.toml` pins `ort` to one exact version for this
# reason: bump the URL, the digest, and that version together.
#
# The camera reader `v4l` binds the kernel's `videodev2.h` with bindgen at
# build time, so `rustPlatform.bindgenHook` provides libclang and the C
# library's headers.
#
# At run time the daemon runs `ffmpeg` for a recording and `ffprobe` for
# its receipt, and `curl` once to fetch the model `crates/coder-hands/
# model.toml` pins, so the wrapper puts them on `PATH`. The model itself is
# data and stays out of the store: the first `hands on` fetches it to
# `~/.openagents/quest/models/` and checks its digest.
{ lib
, rustPlatform
, stdenv
, makeWrapper
, pkg-config
, fetchurl
, runCommand
, xz
, gnutar
, ffmpeg
, curl
}:

let
  src = import ./workspace-src.nix { inherit lib; };
  workspace = builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"));

  # The static ONNX Runtime `ort-sys` 2.0.0-rc.13 would download: the row
  # for `x86_64-unknown-linux-gnu` with no features in its `dist.tsv`. The
  # archive is a raw LZMA2 stream over a tar with one file,
  # `libonnxruntime.a`, which the build script reads with a 64 MiB
  # dictionary.
  onnxruntimeArchive = fetchurl {
    name = "onnxruntime-1.28.0-x86_64-unknown-linux-gnu.tar.lzma2";
    url = "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/x86_64-unknown-linux-gnu.tar.lzma2";
    hash = "sha256-5FT3EPikn1OqW0/1HjRUrhg1d35DHGw1xSVc5vIF/Wg=";
  };
  onnxruntime = runCommand "onnxruntime-static-1.28.0" { nativeBuildInputs = [ xz gnutar ]; } ''
    mkdir -p "$out"
    xz -dc --format=raw --lzma2=dict=64MiB ${onnxruntimeArchive} | tar -C "$out" -x
    test -f "$out/libonnxruntime.a"
  '';
in
rustPlatform.buildRustPackage {
  pname = "coderos-camera";
  version = workspace.workspace.package.version;
  inherit src;

  # `fetchCargoVendor` vendors the whole lockfile in one fixed-output
  # fetch. `importCargoLock` can't: the lockfile takes some packages at the
  # same name and version from two sources (crates.io and a git fork, or a
  # path and a git checkout), and its vendor directory has one slot per
  # name and version. After a `Cargo.lock` change, set this to
  # `lib.fakeHash`, build, and paste the hash the error prints.
  cargoHash = "sha256-HHUTnPM165MBs9VRd0Ulp0WnsZL2b9qu2bm9SfPgTD8=";

  # The workspace pins Rust 1.97.1; this package builds with the `rustc` the
  # flake's `nixpkgs` carries, which can be older. The daemon uses nothing
  # newer than that compiler has, so Cargo is told not to refuse it. The
  # pinned toolchain is what `cargo test -p coderos-camera` runs on.
  cargoBuildFlags = [ "--package" "coderos-camera" "--ignore-rust-version" ];

  # The daemon's tests bind sockets and run threads. A contributor runs
  # them with the pinned toolchain, and a build that ran them here would
  # build the test harness on every host rebuild.
  doCheck = false;

  nativeBuildInputs = [ makeWrapper pkg-config rustPlatform.bindgenHook ];

  env.ORT_LIB_LOCATION = "${onnxruntime}";

  # cargoBuildHook builds with `--target`, so the binary lands under the
  # target triple's directory rather than `target/release`.
  installPhase = ''
    runHook preInstall
    install -Dm755 \
      "target/${stdenv.hostPlatform.config}/release/coderos-camera" \
      "$out/bin/coderos-camera"
    runHook postInstall
  '';

  postFixup = ''
    wrapProgram "$out/bin/coderos-camera" \
      --prefix PATH : "${lib.makeBinPath [ ffmpeg curl ]}"
  '';

  meta = {
    description = "One owner of the camera on a CoderOS desktop: a loopback node, a recording, and hand landmarks on a socket";
    mainProgram = "coderos-camera";
    platforms = [ "x86_64-linux" ];
  };
}
