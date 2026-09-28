{
  description = "CoderOS: NixOS modules and shells for OpenAgents hosts";

  # Pinned to one revision so that a rebuild reproduces the system it was
  # tested on instead of following wherever the release branch has moved.
  # Advance it deliberately, and update `flake.lock` in the same change.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/c25784012c9982bca5b3e0de87e90bbdac8927d3";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      lib = nixpkgs.lib;

      # The Android SDK packages are unfree, and the composition accepts
      # Google's SDK license the way `modules/coderos/android.nix` does.
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };

      # The SDK that builds `bins/coder-android` and `bins/openagents-android`:
      # the platform and build tools that match `compileSdk`, and the NDK
      # their `ndkVersion` names, so Gradle finds it without a download. The
      # emulator and its system image stay in the module's SDK, the one
      # /etc/coderos/android.json names.
      androidNdkVersion = "27.1.12297006";
      androidSdk = ((pkgs.androidenv.override { licenseAccepted = true; }).composeAndroidPackages {
        platformVersions = [ "35" ];
        buildToolsVersions = [ "35.0.0" ];
        includeNDK = true;
        ndkVersions = [ androidNdkVersion ];
        includeEmulator = false;
        includeSystemImages = false;
        includeCmake = false;
        toolsVersion = null;
      }).androidsdk;

      androidSdkRoot = "${androidSdk}/libexec/android-sdk";

      # A NixOS system from the CoderOS modules, a stub for the hardware, and
      # whatever a check adds. It is the smallest host that can evaluate, so
      # a check proves the modules evaluate for a machine that is not yours.
      stubHost = modules: lib.nixosSystem {
        inherit system;
        modules = [
          self.nixosModules.coderos
          ./tests/stub-host.nix
        ] ++ modules;
      };

      # A check that evaluates a host's toplevel without building it. Nix
      # computes the derivation path, which forces every option and
      # assertion, and the check writes that path to its output.
      evaluates = name: host:
        pkgs.runCommand "coderos-${name}-evaluates" { } ''
          echo ${builtins.unsafeDiscardStringContext host.config.system.build.toplevel.drvPath} > $out
        '';

      # The shell that builds and tests the Coder compositor with the pinned
      # toolchain: `nix develop ./os#compositor` from the repository root,
      # then `cargo test -p coder-wm -p coder-compositor`. It carries the
      # libraries Smithay links, and the ones the nested backend opens by
      # name at run time.
      compositorShell = pkgs.mkShell {
        packages = [ pkgs.pkg-config ];
        buildInputs = with pkgs; [
          libxkbcommon
          wayland
          systemdLibs
          libinput
          seatd
          libgbm
        ];

        LD_LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [
          libxkbcommon
          wayland
          libglvnd
          vulkan-loader
        ]) + ":/run/opengl-driver/lib";
      };
    in
    {
      # The CoderOS module set. A host flake imports this and adds its own
      # hardware, accounts, and choices. Every capability is off by default.
      nixosModules = {
        coderos = ./modules/coderos;
        default = self.nixosModules.coderos;
      };

      # Packages built from this repository. Each later module that runs a
      # program from this workspace adds its build here, as
      # `pkgs.callPackage ./pkgs/<name>.nix { }`.
      packages.${system} = {
        # The desk command every CoderOS script asks the session through.
        coder-desk = pkgs.callPackage ./pkgs/coder-desk.nix { };
        # The Coder Wayland compositor, which a host runs with
        # `coderos.desktop.compositor = "coder"` or on its trial TTY.
        coder-compositor = pkgs.callPackage ./pkgs/coder-compositor.nix { };
      };

      # A host with only the base module, and a host with every capability
      # this flake carries turned on. `tests/stub-host.nix` says how to add
      # another.
      checks.${system} = {
        stub-host = evaluates "stub-host" (stubHost [ ]);
        stub-host-all = evaluates "stub-host-all" (stubHost [ ./tests/all-capabilities.nix ]);

        # The Coder compositor on tty1 and Hyprland on a trial TTY, with the
        # compositor and its session script at their defaults. The host
        # installs both, starts the session on tty1 and Hyprland on tty2,
        # and puts `Xwayland` on `PATH` for the compositor to start.
        coder-compositor-host =
          let
            host = stubHost [
              ./tests/all-capabilities.nix
              {
                coderos.desktop.compositor = "coder";
                coderos.desktop.trialTty = 2;
              }
            ];
            config = host.config;
            names = map (p: p.name or "") config.environment.systemPackages;
            grant = builtins.fromJSON (builtins.unsafeDiscardStringContext
              config.environment.etc."coderos/compositor.json".text);
            ok =
              lib.elem "coder-compositor-0.1.0" names
              && lib.elem "coder-compositor-session" names
              && config.programs.xwayland.enable
              && lib.hasSuffix "/bin/coder-compositor" grant.compositor
              && !(config.environment.variables ? LIBRARY_PATH);
          in
          if !ok then
            throw "the Coder compositor host lacks its package, session, grant, or Xwayland: ${builtins.toJSON names}"
          else
            evaluates "coder-compositor-host" host;

        # The Coder compositor built from a checkout: the host adds the
        # libraries Smithay links, for the link and for the run through
        # `nix-ld`, and installs no compositor package.
        coder-compositor-checkout =
          let
            host = stubHost [
              ./tests/all-capabilities.nix
              {
                coderos.desktop.compositor = "coder";
                coderos.desktop.trialTty = 2;
                coderos.desktop.compositorBinary = "/srv/checkouts/openagents/target/release/coder-compositor";
              }
            ];
            config = host.config;
            libraries = map (p: p.pname or p.name or "") config.programs.nix-ld.libraries;
            ok =
              config.environment.variables ? LIBRARY_PATH
              && config.environment.variables ? PKG_CONFIG_PATH
              && lib.all (name: lib.elem name libraries) [ "libinput" "seatd" "libxkbcommon" "wayland" ]
              && !(lib.elem "coder-compositor-0.1.0" (map (p: p.name or "") config.environment.systemPackages));
          in
          if !ok then
            throw "a checkout-built compositor lacks its libraries: ${builtins.toJSON libraries}"
          else
            evaluates "coder-compositor-checkout" host;

        # The example workstation, `examples/workstation/configuration.nix`,
        # with stub file systems in place of the hardware file its flake
        # lists, so a machine that is not yours can copy it and build.
        example-workstation = evaluates "example-workstation" (lib.nixosSystem {
          inherit system;
          modules = [
            self.nixosModules.coderos
            ./examples/workstation/configuration.nix
            ./tests/stub-hardware.nix
          ];
        });

        # A host's own launchers and window rules, set through
        # `coderos.desktop.extraBinds` and `extraWindowRules` the way a
        # private host flake sets them, reach Hyprland as the exact lines
        # such a host's session reads today, and reach the Coder
        # compositor's grant as the same entries.
        extension-points =
          let
            host = stubHost [ ./tests/extension-points.nix ];
            etc = host.config.environment.etc;
            conf = etc."coderos/hyprland.conf".text;
            grant = builtins.fromJSON (builtins.unsafeDiscardStringContext etc."coderos/compositor.json".text);
            expected = [
              "bind = SUPER SHIFT, D, exec, coder-deck-open"
              "bind = SUPER, Z, exec, coder-zoom"
              "bind = SUPER, G, exec, coder-battlenet"
              "windowrule = match:class ^(battle\\.net\\.exe|Battle\\.net\\.exe|steam_app_battlenet)$, float on, center on"
              "windowrule = match:class (?i)^(wow.*|world of warcraft.*|steam_app_.*wow.*)$, tile on, suppress_event maximize fullscreen"
              "windowrule = match:title (?i)^(World of Warcraft.*)$, tile on, suppress_event maximize fullscreen"
              "windowrule = match:class (?i)^(sc2.*|starcraft.*|steam_app_.*sc2.*)$, tile on, suppress_event maximize fullscreen"
              "windowrule = match:title (?i)^(StarCraft II.*)$, tile on, suppress_event maximize fullscreen"
            ];
            lines = map lib.trim (lib.splitString "\n" conf);
            missing = lib.filter (line: !(lib.elem line lines)) expected;
            grantOk =
              map (bind: bind.key) grant.extraBinds == [ "D" "Z" "G" ]
              && builtins.length grant.extraRules == 5
              && (lib.head grant.extraRules).effects.center
              && grant.launchers == [ "dictation" "presentation" "browser" "android" ];
          in
          if missing != [ ] then
            throw "hyprland.conf is missing: ${lib.concatStringsSep "; " missing}"
          else if !grantOk then
            throw "compositor.json does not carry the host's entries: ${builtins.toJSON grant}"
          else
            evaluates "extension-points" host;
      };

      # The shell that builds the Android apps: `nix develop ./os#android`
      # from the repository root, then `scripts/build-coder-android.sh` or
      # `scripts/build-openagents-android.sh`. The Rust toolchain comes from
      # the machine's `rustup`, which reads `rust-toolchain.toml`, with the
      # `aarch64-linux-android` and `x86_64-linux-android` targets added.
      devShells.${system} = {
        android = pkgs.mkShell {
          packages = [
            androidSdk
            pkgs.jdk17
            pkgs.gradle
            pkgs.cargo-ndk
            pkgs.watchexec
          ];

          ANDROID_HOME = androidSdkRoot;
          ANDROID_SDK_ROOT = androidSdkRoot;
          ANDROID_NDK_HOME = "${androidSdkRoot}/ndk/${androidNdkVersion}";
          JAVA_HOME = pkgs.jdk17.home;

          shellHook = ''
            echo "Android SDK: $ANDROID_HOME"
            echo "NDK: $ANDROID_NDK_HOME"
            echo "Build with scripts/build-coder-android.sh or scripts/build-openagents-android.sh."
          '';
        };

        compositor = compositorShell;
      };
    };
}
