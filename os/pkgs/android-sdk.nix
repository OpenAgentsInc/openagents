# One SDK composition for the build shell and the optional desktop module.
# Package revisions, including JDK and Gradle, follow os/flake.lock.
{ pkgs, build ? true, emulator ? true, platformVersion ? "35",
  systemImageType ? "google_apis", abi ? "x86_64" }:
let
  ndkVersion = "27.1.12297006";
  jdk = pkgs.jdk17;
in
{
  inherit ndkVersion jdk;
  buildPackages = [ jdk (pkgs.gradle_8.override { java = jdk; }) pkgs.cargo-ndk pkgs.protobuf ];
  sdk = ((pkgs.androidenv.override { licenseAccepted = true; }).composeAndroidPackages {
    platformVersions = [ platformVersion ];
    buildToolsVersions = if build then [ "35.0.0" ] else [ ];
    includeNDK = build;
    ndkVersions = [ ndkVersion ];
    includeCmake = build;
    cmakeVersions = [ "3.22.1" ];
    includeEmulator = emulator;
    includeSystemImages = emulator;
    systemImageTypes = [ systemImageType ];
    abiVersions = [ abi ];
    toolsVersion = null;
  }).androidsdk;
}
