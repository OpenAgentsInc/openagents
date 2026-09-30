# Tailscale: a name for this host that survives a network change.
#
# Other machines reach a host by name over ssh, and Coder hosts can serve a
# direct listener on the tailnet. On a LAN a name works through mDNS or a
# `.lan` suffix, and both break the moment a host moves to another network
# or the router hands out a different name. On 2026-09-11 a CoderOS host
# could not resolve a Mac's `.local` name at all, because it carries no mDNS
# resolver, and reached it only by `.lan`.
#
# A tailnet replaces "same LAN" with "same tailnet" and gives every host a
# stable name. It is transport, not authority: it changes who can route to
# whom, and nothing about who may start work on a host or what that work
# may reach. Device grants in `crates/coder-access` decide that.
#
# This is off by default. A host joins a tailnet when its host file says
# so, because joining is a decision about where the machine is reachable
# from, not a property of every CoderOS host.
#
# Authentication is deliberately left out of the closure. `authKeyFile`
# points at a path on the host, and the key never enters the Nix store,
# where it would be world-readable and would follow every copy of the
# system closure. With no key named, the host comes up with the daemon
# running and unauthenticated, and `tailscale up` finishes the join by
# hand.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.tailscale;
in
{
  options.coderos.tailscale = {
    enable = lib.mkEnableOption "joining this host to a tailnet";

    authKeyFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/etc/coderos/tailscale.key";
      description = ''
        A file on this host holding a Tailscale auth key, used once to join
        the tailnet. A string rather than a path, so the key stays out of
        the Nix store. With null, the daemon runs unauthenticated and you
        finish the join with `tailscale up`.
      '';
    };

    ssh = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to let Tailscale answer ssh on the tailnet. Off by default:
        other machines already reach this host through the system's own
        sshd with a key, and turning this on moves who may open a shell
        here from that key to the tailnet's access rules.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to open the UDP port Tailscale uses for direct connections.
        With this off the host still works through a relay, more slowly.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.tailscale = {
      enable = true;
      authKeyFile = cfg.authKeyFile;
      openFirewall = cfg.openFirewall;
      useRoutingFeatures = "client";
    };

    # The command line, so a person on the host can read the state the
    # daemon holds and finish a join.
    environment.systemPackages = [ pkgs.tailscale ];

    services.openssh.enable = lib.mkDefault true;

    # `tailscale ssh` is the daemon answering ssh itself, which is a
    # different door from the system's sshd. It opens only when asked for.
    services.tailscale.extraUpFlags = lib.optional cfg.ssh "--ssh";
  };
}
