# A private host flake for one CoderOS workstation. Copy this directory into
# a repository of your own; `README.md` beside it describes the steps.
#
# The OpenAgents flake supplies the CoderOS modules and pins nixpkgs, and this
# flake follows that pin, so your host builds against the packages the
# OpenAgents checks evaluated.
{
  inputs.openagents.url = "github:OpenAgentsInc/openagents?dir=os";
  inputs.nixpkgs.follows = "openagents/nixpkgs";

  outputs = { nixpkgs, openagents, ... }: {
    nixosConfigurations.coderos = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        openagents.nixosModules.coderos
        ./configuration.nix
        # Written on the machine by `nixos-generate-config`.
        ./hardware-configuration.nix
        # Modules of your own that stay private, such as a video client or a
        # game launcher, go here. They add their chords and window rules
        # through `coderos.desktop.extraBinds` and `extraWindowRules`.
      ];
    };
  };
}
