# The smallest machine the CoderOS modules evaluate for: the file systems in
# `stub-hardware.nix`, a host name, and a state version. It stands in for a
# real host and the hardware file it generates with `nixos-generate-config`,
# and it names no account, key, or address.
#
# The flake's checks import it beside `nixosModules.coderos`. A check for a
# new capability adds a module that turns the capability on, the way
# `all-capabilities.nix` does, and passes it to `stubHost` in `flake.nix`.
{ ... }:

{
  imports = [ ./stub-hardware.nix ];

  networking.hostName = "coderos-stub";

  system.stateVersion = "26.05";
}
