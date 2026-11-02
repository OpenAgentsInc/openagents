{
  description = "Verse OS: NixOS modules for a machine that boots into the Verse (docs/os/2026-11-02-verse-os.md)";

  # The nixpkgs coderos-4080 runs (~/coder/os's pin), so a host importing these
  # modules builds the system it built before, plus what they add.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/c25784012c9982bca5b3e0de87e90bbdac8927d3";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      # A host with nothing but the module, for the evaluation check: a
      # stand-in disk and boot loader, one account, the Verse on tty3.
      example = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.verse-os
          {
            boot.loader.grub.enable = false;
            boot.loader.systemd-boot.enable = true;
            fileSystems."/" = { device = "/dev/disk/by-label/verse-os"; fsType = "ext4"; };
            users.users.pilot = { isNormalUser = true; };
            services.getty.autologinUser = "pilot";
            verseos.session = {
              enable = true;
              user = "pilot";
              dir = "/home/pilot/.local/share/verse-os/current";
              stateDir = "/home/pilot/.local/state/verse";
            };
            system.stateVersion = "26.05";
          }
        ];
      };
      pkgs = nixpkgs.legacyPackages.${system};
      # The realms from a tree `verse/gpu/realm.sh local ship DIR` wrote.
      verseRealms = src: pkgs.callPackage ./pkgs/verse-realms.nix { inherit src; };
      # The image (V7): the session on tty1 from the realms package. Its realms are the tree in
      # ./ship when one was shipped there (git-ignored), else a stand-in that only evaluates.
      shipped = if builtins.pathExists ./ship/realms then ./ship else ./tests/ship-stub;
      image = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.verse-os
          self.nixosModules.verse-os-image
          {
            verseos.image.enable = true;
            verseos.session.package = verseRealms shipped;
            hardware.graphics.enable = true;
            system.stateVersion = "26.05";
          }
        ];
      };
    in
    {
      nixosModules.verse-os = ./modules/verse-os.nix;
      nixosModules.verse-os-image = ./modules/verse-os-image.nix;
      nixosModules.default = self.nixosModules.verse-os;

      lib.verseRealms = verseRealms;
      packages.${system}.verse-realms = verseRealms shipped;

      nixosConfigurations.verse-os-example = example;
      nixosConfigurations.verse-os-image = image;

      # `nix flake check ./os/nix` evaluates the example host to its system:
      # the options type-check, the assertions hold, the login script renders.
      checks.${system}.example = example.config.system.build.toplevel;
      # the image evaluates to its system (building the disk is `config.system.build.image`)
      checks.${system}.image = image.config.system.build.toplevel;
    };
}
