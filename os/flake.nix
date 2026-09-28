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
      };

      # A host with only the base module, and a host with every capability
      # this flake carries turned on. `tests/stub-host.nix` says how to add
      # another.
      checks.${system} = {
        stub-host = evaluates "stub-host" (stubHost [ ]);
        stub-host-all = evaluates "stub-host-all" (stubHost [ ./tests/all-capabilities.nix ]);
      };

      # The shell that builds the Android apps: `nix develop ./os#android`
      # from the repository root, then `scripts/build-coder-android.sh` or
      # `scripts/build-openagents-android.sh`. The Rust toolchain comes from
      # the machine's `rustup`, which reads `rust-toolchain.toml`, with the
      # `aarch64-linux-android` and `x86_64-linux-android` targets added.
      devShells.${system}.android = pkgs.mkShell {
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
    };
}
