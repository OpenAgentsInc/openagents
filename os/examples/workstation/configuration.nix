# An example CoderOS workstation: one person's desktop machine that runs
# Coder in a tiling Hyprland session, keeps the resident Coder host running,
# and rebuilds `coder` from this repository on a timer.
#
# It is a real host file with its identity replaced. Copy it into a private
# flake of your own, as `README.md` in this directory describes, and change
# every value marked "yours". The hardware, the file systems, and the boot
# devices come from `hardware-configuration.nix`, which `nixos-generate-config`
# writes on the machine and `flake.nix` lists beside this file.
#
# `nix flake check ./os` evaluates this file against a stub hardware file, so
# every option it sets exists and every assertion holds.
{ config, pkgs, ... }:

{
  # Yours: the name the machine answers to on the network and the tailnet.
  networking.hostName = "coderos";

  # Yours: the SSH public keys that may reach root, for example
  # "ssh-ed25519 AAAA... you@laptop". Password authentication is off, so with
  # this list empty the console is the only way in until you add a key.
  coderos.authorizedKeys = [ ];

  # Yours: the account you log in as. It reaches SSH with the same keys.
  users.users.you = {
    isNormalUser = true;
    extraGroups = [ "wheel" "docker" "networkmanager" ];
    openssh.authorizedKeys.keys = config.coderos.authorizedKeys;
  };

  # `nixos-rebuild` runs as root, and your account owns the checkout, so root's
  # git has to trust it for the `--override-input` edit loop in `README.md`.
  coderos.git.safeDirectories = [ "/home/you/openagents" ];

  # A stable name for this host on your tailnet, so your other machines reach
  # it after a network change. Finish the join once with `tailscale up`, or
  # name an auth key file with `coderos.tailscale.authKeyFile`.
  coderos.tailscale.enable = true;

  # The machine has a monitor. The console login on tty1 is what starts the
  # desktop session below.
  services.getty.autologinUser = "you";

  # The screen is a tiling workspace whose first window is Coder, and
  # SUPER + RETURN opens another one in a new tile. `os/README.md` in the
  # OpenAgents repository covers the session and its keys.
  coderos.desktop = {
    enable = true;
    user = "you";

    # Coder opens in a checkout, not in the home directory. A writing
    # delegation locks the directory it writes in, and outside a repository
    # there is no worktree to hand the next one, so every writer would wait on
    # one lock on `~`. In a checkout each writer gets a worktree of its own.
    directory = "/home/you/openagents";

    # The `screen-record` command, and push-to-talk dictation on SUPER + V.
    screenRecording.enable = true;
    dictation.enable = true;

    # Dictation uses the default microphone until you name one. Name it when
    # the machine has more than one, such as a desk microphone beside a
    # webcam, so that dictation never records from the wrong one. Read the
    # node names with `pw-cli ls Node` and the USB IDs with `lsusb`:
    #
    #   microphone = {
    #     node = "alsa_input\\.usb-Example_Microphone.*";
    #     name = "Desk mic";
    #     exclude = [ "alsa_input\\.usb-Example_Webcam.*" ];
    #     usbVendor = "1234";
    #     usbProduct = "5678";
    #     gain = { card = "Microphone"; value = 10; };
    #   };

    # SUPER + P sets the screen up to be watched and puts it back.
    presentation.enable = true;

    # SUPER + B opens the Coder Browser, with the DevTools Protocol on a
    # loopback port that Coder's browser tool steers.
    browser.enable = true;

    # SUPER + A opens the Android emulator on KVM. Turn it on if you build the
    # Android apps on this machine; it adds an SDK and a system image to the
    # store and turns Xwayland on for the session.
    #
    #   android.enable = true;

    # The Coder compositor, the camera, and hand tracking stay off. Hyprland
    # is the session until the Coder compositor has earned tty1 on your
    # hardware; `coderos.desktop.compositor` and `trialTty` switch it, and
    # `os/README.md` describes both.
  };

  # The resident Coder host, `coder host serve`, as a systemd user unit that
  # starts at boot. It waits until you run `coder host init` once; `os/README.md`
  # lists the steps.
  coderos.coderHost = {
    enable = true;
    user = "you";
  };

  # A timer that builds `coder` from this repository's main branch and hands
  # it to the Coder host service, which trials it and rolls back on failure.
  # The build runs under memory and task limits, so a runaway build fails
  # without taking the desktop down.
  coderos.coderUpdate.enable = true;

  # What follows is hardware a machine may or may not have. Each block is off;
  # turn on the ones that match yours.

  # An NVIDIA card. The proprietary driver is the conservative choice for
  # recent GeForce cards. Keep one generation without a driver to fall back
  # to, by building one without this block first. The container toolkit lets
  # Docker hand the card to a container that requests `nvidia.com/gpu`.
  #
  #   hardware.graphics.enable = true;
  #   hardware.graphics.enable32Bit = true;
  #   services.xserver.videoDrivers = [ "nvidia" ];
  #   hardware.nvidia = {
  #     modesetting.enable = true;
  #     nvidiaSettings = false;
  #     open = false;
  #     powerManagement.enable = false;
  #     package = config.boot.kernelPackages.nvidiaPackages.stable;
  #   };
  #   hardware.nvidia-container-toolkit.enable = true;
  #   environment.systemPackages = [ pkgs.nvtopPackages.nvidia ];

  # CPU bounds, for a CPU that runs hotter than its cooler can hold or one
  # that logs machine checks under load. The module caps package power, boost
  # frequency, and build parallelism. Measure your own part before you set a
  # value; `os/modules/coderos/cpu-limits.nix` holds one measured example.
  #
  #   coderos.cpuLimits = {
  #     enable = true;
  #     watts = 125;
  #     maxPerfPct = 88;
  #   };

  # A record of machine checks, for a CPU or memory that faults. rasdaemon
  # decodes each event and keeps its own database, so the evidence outlasts
  # the journal, which a warranty claim needs.
  #
  #   hardware.rasdaemon = {
  #     enable = true;
  #     record = true;
  #   };

  # Compressed swap in RAM, for a machine with no swap that runs large builds
  # or test suites. It gives the kernel room to page cold memory out before
  # the out-of-memory killer takes the desktop session. With zstd a page takes
  # about a third of its size. It is headroom, not a bound.
  #
  #   zramSwap = {
  #     enable = true;
  #     algorithm = "zstd";
  #     memoryPercent = 25;
  #   };

  # Set once, at install, to the release you installed. It is a compatibility
  # marker for stateful services, not a version to keep current.
  system.stateVersion = "26.05";
}
