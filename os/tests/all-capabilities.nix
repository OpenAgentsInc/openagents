# Every capability this flake carries, turned on for the stub host, so that
# a check evaluates each module's enabled path as well as its default.
# Add a capability here when its module joins `modules/coderos`.
{ ... }:

{
  users.users.operator = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
  };

  # The console login on tty1 starts the desktop session.
  services.getty.autologinUser = "operator";

  coderos = {
    sudo.wheelNeedsPassword = false;
    git.safeDirectories = [ "/srv/checkouts/openagents" ];

    desktop = {
      enable = true;
      user = "operator";
      directory = "/srv/checkouts/openagents";
      android.enable = true;
      presentation.enable = true;
      browser.enable = true;
      screenRecording.enable = true;
      dictation.enable = true;
      microphone = {
        node = "alsa_input.usb-Example_Microphone.*";
        name = "Desk mic";
        exclude = [ "alsa_input.usb-Example_Webcam.*" ];
        usbVendor = "1234";
        usbProduct = "5678";
        gain = {
          card = "Microphone";
          value = 10;
        };
      };
    };

    tailscale = {
      enable = true;
      ssh = true;
    };

    cpuLimits = {
      enable = true;
      watts = 125;
      maxPerfPct = 88;
      nixJobs = 6;
      nixCores = 4;
      cargoJobs = 20;
    };

    coderHost = {
      enable = true;
      user = "operator";
    };

    coderUpdate = {
      enable = true;
      jobs = 16;
    };
  };
}
