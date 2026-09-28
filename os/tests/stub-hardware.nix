# The file systems a real host's `hardware-configuration.nix` declares, and
# nothing else: a root file system and an EFI partition. It stands in for the
# file `nixos-generate-config` writes, so a check can evaluate a host file
# that expects one, such as `examples/workstation/configuration.nix`.
{ ... }:

{
  fileSystems."/" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "ext4";
  };

  fileSystems."/boot" = {
    device = "/dev/disk/by-label/boot";
    fsType = "vfat";
  };
}
