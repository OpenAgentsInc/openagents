# The smallest machine the CoderOS modules evaluate for: a root file system,
# an EFI partition, and a state version. It stands in for the hardware file
# a real host generates with `nixos-generate-config`, and it names no
# account, key, or address.
#
# The flake's checks import it beside `nixosModules.coderos`. A check for a
# new capability adds a module that turns the capability on, the way
# `all-capabilities.nix` does, and passes it to `stubHost` in `flake.nix`.
{ ... }:

{
  networking.hostName = "coderos-stub";

  fileSystems."/" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "ext4";
  };

  fileSystems."/boot" = {
    device = "/dev/disk/by-label/boot";
    fsType = "vfat";
  };

  system.stateVersion = "26.05";
}
