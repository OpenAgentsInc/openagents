# The optional Coder Browser: Chromium on SUPER + B.
#
# A host that leaves `coderos.desktop.browser.enable` off gets nothing from
# this file: no Chromium and no bind.
#
# With it on, the host installs `ungoogled-chromium` from nixpkgs with the
# Widevine CDM bundled, and SUPER + B opens it on Wayland, dark, with the
# DevTools Protocol listening on a loopback port. The launcher and every
# flag it passes are `os/bin/coder-browser`, and the one policy the browser
# reads, the white theme colour, is the file this module writes under
# /etc/chromium/policies/managed; nothing about the browser is set on the
# host by hand.
#
# This module writes no grant for a tool to steer the browser. No tool in
# this repository reads one yet, so the port is only the launcher's: it is
# how a second press opens a URL as a tab in the window that is already up.
#
# The port is a weak boundary: anything on this host that can open
# 127.0.0.1:<port> can drive the browser, pages and all. On a host used by
# one person that is the same set of things that can read their files. The
# port is never opened in the firewall, and the assertion below holds that
# rule.
{ config, lib, pkgs, ... }:

let
  desktop = config.coderos.desktop;
  cfg = desktop.browser;

  # `enableWideVine` copies the `widevine-cdm` package into
  # libexec/chromium/WidevineCdm beside the browser binary, the one location
  # the nixpkgs Widevine patch lets Chromium load it from. Without it
  # `navigator.requestMediaKeySystemAccess` rejects `com.widevine.alpha` and
  # DRM streams do not play. The CDM is unfree, which
  # `nixpkgs.config.allowUnfree` in os/modules/coderos/default.nix already
  # permits. The override rebuilds only the wrapper: the browser derivation
  # itself is unchanged.
  browser = pkgs.ungoogled-chromium.override { enableWideVine = true; };

  # The launcher is a real file under os/bin, wrapped so that shellcheck runs
  # at build time and its tools are on PATH by name rather than by whatever
  # a person's profile happens to hold. The port reaches it as
  # `CODER_BROWSER_PORT`.
  launcher = pkgs.writeShellApplication {
    name = "coder-browser";
    runtimeInputs = [ browser desktop.deskPackage pkgs.jq pkgs.curl ];
    runtimeEnv.CODER_BROWSER_PORT = toString cfg.port;
    text = builtins.readFile ../../bin/coder-browser;
  };

  # What an application in a sandbox runs to open a URL: it asks the desk to
  # run the launcher in the host's session. `os/bin/coder-open-url` says
  # why. A host module that runs such an application, such as a game
  # launcher under Wine, names this command as its URL handler.
  openUrl = pkgs.writeShellApplication {
    name = "coder-open-url";
    runtimeInputs = [ desktop.deskPackage ];
    text = builtins.readFile ../../bin/coder-open-url;
  };

  # Wrapper commands that route every name for "open a browser" to the one
  # launcher, so nothing on this host — a `.desktop` file, `xdg-open`, or a
  # shell fallback a program reached for — can start a raw Chromium. A raw
  # Chromium grabs the default profile and makes `coder-browser`'s
  # debug-enabled instance hand off and exit, leaving the DevTools port dead.
  # Each shim hands its arguments to `coder-browser` by its absolute store
  # path, which either focuses the open window or opens one.
  #
  # These must win on PATH: `browser` also ships `chromium` and
  # `chromium-browser`, and a desktop's `xdg-utils` ships `xdg-open`, so the
  # join is `lib.hiPrio` to be the copy the system profile keeps when it
  # collides with the real ones. (`lib.hiPrio` lowers the package's priority
  # number, which is how `buildEnv` resolves a collision without an error.)
  #
  # `coder-browser`'s own `chromium` call is not one of these shims: the
  # launcher is a `writeShellApplication`, whose generated script prepends its
  # `runtimeInputs` to PATH, and `browser` is first in those inputs, so inside
  # the launcher `chromium` resolves to the real browser before the system
  # profile, where these shims live, is ever consulted. So there is no
  # recursion: shim to coder-browser to real chromium, and never shim to
  # coder-browser to shim.
  browserShims = lib.hiPrio (pkgs.symlinkJoin {
    name = "coder-browser-shims";
    paths = map
      (name: pkgs.writeShellScriptBin name ''exec ${launcher}/bin/coder-browser "$@"'')
      [
        "chromium"
        "chromium-browser"
        "google-chrome"
        "google-chrome-stable"
        "chrome"
        "brave-browser"
        "xdg-open"
      ];
  });

  # The toolbar wants the scheme without shouting it: full white on the
  # whole chrome would glare, so the toolbar takes the raised near-black
  # from the low end of the white ladder `desktop.nix` holds for the
  # terminal and the borders.
  chromeTheme = "#1a1a1a";

  # What Chromium reads as managed policy from /etc/chromium/policies/managed.
  # `BrowserThemeColor` seeds the browser's own theme from one colour, which
  # gives the toolbar the scheme without a Web Store extension. Chromium 152
  # lists it on chrome://policy once the file is there; a policy is read on
  # start and on the Reload policies button, so no restart is needed.
  policy = {
    BrowserThemeColor = chromeTheme;
  };
in
{
  options.coderos.desktop.browser = {
    enable = lib.mkEnableOption ''
      the Coder Browser: ungoogled-chromium on SUPER + B, with the DevTools
      Protocol on a loopback port. It needs the desktop session
    '';

    port = lib.mkOption {
      type = lib.types.port;
      default = 9222;
      description = ''
        The loopback port the DevTools Protocol listens on. Only this host
        can reach it, and the firewall never opens it.
      '';
    };
  };

  config = lib.mkIf (desktop.enable && cfg.enable) {
    # The theme. The launcher's flags make the browser dark; this colours it.
    environment.etc."chromium/policies/managed/coderos.json".text = builtins.toJSON policy;

    # The launcher, the command a sandbox opens a URL with, the shims that
    # route every browser name to the launcher (winning on PATH over the real
    # Chromium below by `lib.hiPrio`), and the real Chromium the launcher's
    # own runtime inputs reach.
    environment.systemPackages = [ launcher openUrl browserShims browser ];

    coderos.desktop.launchers = [ "browser" ];

    assertions = [
      {
        assertion = !(builtins.elem cfg.port config.networking.firewall.allowedTCPPorts);
        message = ''
          coderos.desktop.browser.port is ${toString cfg.port}, and
          networking.firewall.allowedTCPPorts opens it. The DevTools port is
          loopback only; anything that reaches it drives the browser.
        '';
      }
    ];
  };
}
