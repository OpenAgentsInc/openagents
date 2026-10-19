# The Coder compositor as a Nix package, built from this repository.
#
# The source filter, the vendored dependencies' `cargoHash`, and
# `--ignore-rust-version` are the same as `pkgs/coder-desk.nix`, for the
# reasons that file gives.
#
# Smithay links `libxkbcommon`, and for the hardware backend `libudev`,
# `libinput`, `libseat`, and `libgbm`, at build time, so they are
# `buildInputs` here and the package links them whatever the host sets. GBM
# loads the driver's backend at run time from `/run/opengl-driver/lib/gbm`,
# the path `libgbm` was built to read, so a proprietary driver's GBM backend
# is found there. At run time the binary opens the Wayland client library,
# EGL, and the Vulkan loader by name, and a client that draws through Vulkan
# opens the loader as well, so the wrapper names them on `LD_LIBRARY_PATH`
# with `/run/opengl-driver/lib` last, where a NixOS host keeps the driver
# the running kernel loaded.
{ lib
, rustPlatform
, stdenv
, makeWrapper
, pkg-config
, libxkbcommon
, wayland
, libglvnd
, vulkan-loader
, systemdLibs
, libinput
, seatd
, libgbm
}:

let
  src = import ./workspace-src.nix { inherit lib; };
  workspace = builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"));

  # What the binary opens by name while it runs. The driver directory is
  # last, so a library the store holds is the one the binary loads and the
  # driver supplies what only the host has.
  libraries =
    lib.makeLibraryPath [ libxkbcommon wayland libglvnd vulkan-loader ]
    + ":/run/opengl-driver/lib";
in
rustPlatform.buildRustPackage {
  pname = "coder-compositor";
  version = workspace.workspace.package.version;
  inherit src;

  # `fetchCargoVendor` vendors the whole lockfile in one fixed-output
  # fetch. `importCargoLock` can't: the lockfile takes some packages at the
  # same name and version from two sources (crates.io and a git fork, or a
  # path and a git checkout), and its vendor directory has one slot per
  # name and version. After a `Cargo.lock` change, set this to
  # `lib.fakeHash`, build, and paste the hash the error prints.
  cargoHash = "sha256-aYn7JwSP0QmbNzpaY7iCG5jQjbY6j8ObvELbT7bytSA=";

  # The workspace pins Rust 1.97.1; this package builds with the `rustc` the
  # flake's `nixpkgs` carries, which can be older. The compositor uses
  # nothing newer than that compiler has, so Cargo is told not to refuse it.
  # The pinned toolchain is what `cargo test -p coder-compositor` runs on.
  cargoBuildFlags = [ "--package" "coder-compositor" "--ignore-rust-version" ];

  # The compositor's tests bind sockets, spawn programs, and run the layout
  # over pixels. A contributor runs them with the pinned toolchain, and a
  # build that ran them here would build the test harness on every host
  # rebuild.
  doCheck = false;

  nativeBuildInputs = [ makeWrapper pkg-config ];
  buildInputs = [ libxkbcommon wayland systemdLibs libinput seatd libgbm ];

  # cargoBuildHook builds with `--target`, so the binary lands under the
  # target triple's directory rather than `target/release`.
  installPhase = ''
    runHook preInstall
    install -Dm755 \
      "target/${stdenv.hostPlatform.config}/release/coder-compositor" \
      "$out/bin/coder-compositor"
    runHook postInstall
  '';

  postFixup = ''
    wrapProgram "$out/bin/coder-compositor" \
      --prefix LD_LIBRARY_PATH : "${libraries}"
  '';

  meta = {
    description = "The Coder Wayland compositor for a CoderOS desktop";
    mainProgram = "coder-compositor";
    platforms = [ "x86_64-linux" ];
  };
}
