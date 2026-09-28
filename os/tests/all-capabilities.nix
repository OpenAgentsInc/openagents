# Every capability this flake carries, turned on for the stub host, so that
# a check evaluates each module's enabled path as well as its default.
# Add a capability here when its module joins `modules/coderos`.
{ ... }:

{
  users.users.operator = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
  };

  coderos = {
    sudo.wheelNeedsPassword = false;
    git.safeDirectories = [ "/srv/checkouts/openagents" ];

    desktop = {
      enable = true;
      user = "operator";
      android.enable = true;
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
