# The `coder-desk` command as a Nix package, built from this repository.
#
# `os/` is the flake's root, so the workspace the command belongs to sits one
# level up. The source filter keeps what Cargo reads to resolve the
# workspace, the root manifest, the lockfile, and `crates/`, and leaves out
# the build output, the VCS state, and the rest of the repository, so a
# change to a document or a benchmark record does not rebuild the command.
#
# The lockfile takes some packages from git revisions. Each revision gets
# one `outputHashes` entry, keyed by the name and version of a package the
# lockfile takes from it, because a revision's hash covers every package
# the fetch holds. A revision bump in a manifest names a new key, and
# `importCargoLock`'s missing-hash error prints it.
#
# The command has no runtime input. Its Hyprland backend speaks the session's
# control socket directly, reading `HYPRLAND_INSTANCE_SIGNATURE` and
# `XDG_RUNTIME_DIR` from the environment the session sets, so it runs no
# `hyprctl` and needs no compositor package on a script's PATH.
{ lib, rustPlatform, stdenv }:

let
  root = ../..;
  src = lib.cleanSourceWith {
    src = root;
    filter = path: type:
      let
        relative = lib.removePrefix (toString root + "/") (toString path);
        name = baseNameOf path;
      in
      (relative == "Cargo.toml" || relative == "Cargo.lock"
        || relative == "crates" || lib.hasPrefix "crates/" relative)
      && name != "target" && name != "result"
      && lib.cleanSourceFilter path type;
  };
  workspace = builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"));
in
rustPlatform.buildRustPackage {
  pname = "coder-desk";
  version = workspace.workspace.package.version;
  inherit src;

  cargoLock = {
    lockFile = src + "/Cargo.lock";
    outputHashes = {
      "bitcoin-payment-instructions-0.6.0" = "sha256-kJz+7u55DiSy+/7Rpvm9aiB2ZGpds91M3QQ0CzOi9q4=";
      "ldk-node-0.7.0" = "sha256-qrPzuK+JNloEPKi1R9/qLNmXsZCHgbAjW+01qQV0nTU=";
      "lightning-0.2.5" = "sha256-wTWWuQjp6kdtzL+ndmTgld00HY3jqzIWZFcAJOKhfhY=";
      "lightning-0.3.0+git" = "sha256-uiZaVn2W3LyEh4TTsp7NN6pIcwJ0rWxkR7eZW6cF/9o=";
      "musig2-0.1.0" = "sha256-+ksLhW4rXHDmi6xkPHrWAUdMvkm1cM/PBuJUnTt0vQk=";
    };
  };

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
