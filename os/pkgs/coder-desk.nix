# The `coder-desk` command as a Nix package, built from this repository.
#
# The source is the root workspace, filtered by `pkgs/workspace-src.nix`.
#
# The lockfile's dependencies, including the ones taken from git revisions,
# are vendored by one fixed-output fetch whose hash is `cargoHash`. Any
# change to `Cargo.lock` changes that hash.
#
# The command has no runtime input. Its Hyprland backend speaks the session's
# control socket directly, reading `HYPRLAND_INSTANCE_SIGNATURE` and
# `XDG_RUNTIME_DIR` from the environment the session sets, so it runs no
# `hyprctl` and needs no compositor package on a script's PATH.
{ lib, rustPlatform, stdenv }:

let
  src = import ./workspace-src.nix { inherit lib; };
  workspace = builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"));
in
rustPlatform.buildRustPackage {
  pname = "coder-desk";
  version = workspace.workspace.package.version;
  inherit src;

  # `fetchCargoVendor` vendors the whole lockfile in one fixed-output
  # fetch. `importCargoLock` can't: the lockfile takes some packages at the
  # same name and version from two sources (crates.io and a git fork, or a
  # path and a git checkout), and its vendor directory has one slot per
  # name and version. After a `Cargo.lock` change, set this to
  # `lib.fakeHash`, build, and paste the hash the error prints.
  cargoHash = "sha256-HHUTnPM165MBs9VRd0Ulp0WnsZL2b9qu2bm9SfPgTD8=";

  # The workspace pins Rust 1.97.1 in `rust-toolchain.toml` and states it
  # as every crate's `rust-version`. This package builds with the `rustc`
  # the flake's `nixpkgs` carries, which can be older, and the command uses
  # nothing newer than that compiler has, so Cargo is told not to refuse it.
  # The pinned toolchain is what `cargo test -p coder-desk-cli` runs on.
  cargoBuildFlags = [ "--package" "coder-desk-cli" "--ignore-rust-version" ];

  # The command's tests run the built binary against a fake desk on a Unix
  # socket. A contributor runs them with the pinned toolchain, and a build
  # that ran them here would build the test harness on every host rebuild.
  doCheck = false;

  # cargoBuildHook builds with `--target`, so the binary lands under the
  # target triple's directory rather than `target/release`.
  installPhase = ''
    runHook preInstall
    install -Dm755 \
      "target/${stdenv.hostPlatform.config}/release/coder-desk" \
      "$out/bin/coder-desk"
    runHook postInstall
  '';

  meta = {
    description = "Ask the desktop session what is on its screens, and change one window";
    mainProgram = "coder-desk";
    platforms = [ "x86_64-linux" ];
  };
}
