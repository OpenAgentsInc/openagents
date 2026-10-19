# The root Cargo workspace's source, filtered to what the CoderOS packages
# built from it read: `pkgs/coder-desk.nix`, `pkgs/coder-compositor.nix`, and
# `pkgs/coderos-camera.nix` take their `src` from here.
#
# `os/` is the flake's root, so the workspace sits one level up. Cargo
# resolves the whole workspace before it builds one package, so the filter
# keeps every manifest Cargo loads: the root manifest and lockfile,
# `crates/`, `vendor/` (the root manifest patches packages in from there),
# and the desktop app's manifest and `apps/openagents/crates/`, which root
# crates take path dependencies on. It also keeps the files outside
# `crates/` that the packages' dependencies embed with `include_str!`: the
# Jev question sets in `questions/`, the NIP schemas in `nips/`, and the
# protocol fixtures in `docs/protocol/fixtures/`. A build that fails with
# "couldn't read" names a file a new dependency embeds; add its directory
# to `keep`.
#
# Everything else, including the build output and the VCS state, is left
# out, so a change to a document or a benchmark record does not rebuild the
# packages.
{ lib }:

let
  root = ../..;
  keep = [
    "Cargo.toml"
    "Cargo.lock"
    "crates"
    "vendor"
    "apps/openagents/Cargo.toml"
    "apps/openagents/crates"
    "questions"
    "nips"
    "docs/protocol/fixtures"
  ];
  kept = relative:
    lib.any
      (k: relative == k || lib.hasPrefix (k + "/") relative
        || lib.hasPrefix (relative + "/") k)
      keep;
in
lib.cleanSourceWith {
  src = root;
  filter = path: type:
    let
      relative = lib.removePrefix (toString root + "/") (toString path);
      name = baseNameOf path;
    in
    kept relative
    && name != "target" && name != "result"
    && lib.cleanSourceFilter path type;
}
