# Verse OS as an image (docs/os/2026-11-02-verse-os.md, "Base system" and V7): a disk image a machine
# boots straight into the Verse from, which nobody configures and nothing rebuilds on the machine.
#
#   - The disk: an ESP (systemd-boot and a unified kernel image), a read-only root holding the
#     system's store (slot A), and room for a second (slot B). Built on our machines with
#     image.repart; `nix build .#nixosConfigurations.verse-os-image.config.system.build.image`.
#   - Updates: systemd-sysupdate fetches the next version's root and UKI into the slot not in use,
#     and the boot loader tries it next boot. A version that does not reach the Verse is not
#     blessed (boot counting), and the loader falls back to the one before.
#   - Your data: /home on its own partition, made and encrypted (TPM2) by systemd-repart on first
#     boot. An update never touches it.
#   - The session: tty1 logs `pilot` in and runs verse-session (verse-os.nix) from the realms
#     package, which is built from `realm.sh local ship` output (pkgs/verse-realms.nix).
{ config, lib, pkgs, modulesPath, ... }:

let
  cfg = config.verseos.image;
  efiArch = pkgs.stdenv.hostPlatform.efiArch;
in
{
  imports = [ "${modulesPath}/image/repart.nix" ];

  options.verseos.image = {
    enable = lib.mkEnableOption "Verse OS as a bootable image with A/B updates";

    version = lib.mkOption {
      type = lib.types.str;
      default = "0";
      description = "This image's version: the root partition and the UKI are named by it.";
    };

    updateUrl = lib.mkOption {
      type = lib.types.str;
      default = "https://openagents.com/verse-os/";
      description = ''
        Where systemd-sysupdate looks for the next version: a SHA256SUMS and the
        verse-os_VERSION.root.raw.xz and verse-os_VERSION.efi files it names.
      '';
    };

    rootSize = lib.mkOption {
      type = lib.types.str;
      default = "8G";
      description = "Each root slot's size; the second slot is reserved at this size too.";
    };
  };

  config = lib.mkIf cfg.enable {
    system.image.id = "verse-os";
    system.image.version = cfg.version;

    # Nothing on the machine rebuilds it.
    nix.enable = false;
    system.switch.enable = false;
    boot.loader.grub.enable = false;
    boot.initrd.systemd.enable = true;

    fileSystems."/" = {
      device = "/dev/disk/by-partlabel/verse-os_${cfg.version}";
      fsType = "ext4";
      options = [ "ro" ];
    };
    fileSystems."/home" = {
      device = "/dev/disk/by-partlabel/verse-home";
      fsType = "ext4";
      neededForBoot = false;
      options = [ "nofail" "x-systemd.device-timeout=30s" ];
    };
    # what the read-only root still needs written
    boot.tmp.useTmpfs = true;
    fileSystems."/var" = { device = "tmpfs"; fsType = "tmpfs"; options = [ "mode=0755" ]; };

    image.repart = {
      name = "verse-os";
      partitions = {
        "10-esp" = {
          contents = {
            "/EFI/BOOT/BOOT${lib.toUpper efiArch}.EFI".source = "${pkgs.systemd}/lib/systemd/boot/efi/systemd-boot${efiArch}.efi";
            "/EFI/Linux/verse-os_${cfg.version}.efi".source = "${config.system.build.uki}/${config.system.boot.loader.ukiFile}";
          };
          repartConfig = { Type = "esp"; Format = "vfat"; SizeMinBytes = "256M"; };
        };
        "20-root-a" = {
          storePaths = [ config.system.build.toplevel ];
          repartConfig = { Type = "root"; Format = "ext4"; Label = "verse-os_${cfg.version}"; Minimize = "guess"; SizeMaxBytes = cfg.rootSize; };
        };
      };
    };

    # On the machine, at first boot: slot B reserved, and /home made and encrypted.
    systemd.repart = {
      enable = true;
      partitions = {
        "30-root-b" = { Type = "root"; Label = "_empty"; SizeMinBytes = cfg.rootSize; SizeMaxBytes = cfg.rootSize; };
        "40-home" = { Type = "home"; Label = "verse-home"; Format = "ext4"; Encrypt = "tpm2"; };
      };
    };

    systemd.sysupdate = {
      enable = true;
      transfers = {
        "10-uki" = {
          Source = { Type = "url-file"; Path = cfg.updateUrl; MatchPattern = "verse-os_@v.efi"; };
          Target = { Type = "regular-file"; Path = "/EFI/Linux"; PathRelativeTo = "esp"; MatchPattern = "verse-os_@v+@l-@d.efi verse-os_@v+@l.efi verse-os_@v.efi"; Mode = "0444"; TriesLeft = 3; TriesDone = 0; InstancesMax = 2; };
        };
        "20-root" = {
          Source = { Type = "url-file"; Path = cfg.updateUrl; MatchPattern = "verse-os_@v.root.raw.xz"; };
          Target = { Type = "partition"; Path = "auto"; MatchPattern = "verse-os_@v"; MatchPartitionType = "root"; ReadOnly = 1; };
        };
      };
    };
    # an image that reached the Verse is good: systemd-bless-boot marks it once boot-complete.target is
    # reached, which verse-session's first frame on tty1 stands for
    systemd.targets.boot-complete.wantedBy = [ "multi-user.target" ];

    # one person, logged in on tty1, where the Verse runs
    users.users.pilot = { isNormalUser = true; extraGroups = [ "input" "video" ]; };
    services.getty.autologinUser = "pilot";
    verseos.session = {
      enable = true;
      user = "pilot";
      tty = 1;
      leaveTo = 2;
      stateDir = "/home/pilot/.local/state/verse";
    };
  };
}
