# linux-x64 clean-machine acceptance — owner-observed (rc.25)

- Date: 2026-07-21
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issues: [DIST-08 #8921](https://github.com/OpenAgentsInc/openagents/issues/8921),
  [DIST-12 #8925](https://github.com/OpenAgentsInc/openagents/issues/8925)
- Target: `linux-x64`
- Version: `0.1.0-rc.25`
- Host: `archlinux` (owned Tailnet host, `100.108.56.85`), Linux x86_64, active
  graphical session on display `:0`.

## What was accepted

The promoted rc.25 `linux-x64` AppImage was installed on a clean owned Arch
Linux host and the owner observed the running application on the physical
display.

- Artifact:
  `OpenAgents-0.1.0-rc.25-rc-linux-x64.AppImage`
- SHA-256 (verified on host against the release-set manifest):
  `a6072f0c64aad76ad7ea532a7b6efd7202f65fc02d33100351ae666d5c94bac5`
- Byte length: 225096605
- Source: the immutable rc.25 GitHub release asset, the same artifact identity
  the promoted ReleaseSet v2 references.

## Steps

- Downloaded the AppImage to `~/OpenAgents-rc25/` on the host.
- Verified the on-host SHA-256 matched the release-set manifest exactly before
  launch.
- `chmod +x`, launched with `DISPLAY=:0`.
- A window mapped on the physical display (`wmctrl` listed the `OpenAgents`
  window), and the process stayed resident.
- The owner confirmed observing the running application on the host screen.

## Notes

- Two benign log lines are expected on a host without GPU acceleration: a
  `MESA-LOADER … dri_gbm.so` software-render fallback and an
  `app-server history unavailable` info line. Neither blocks the interface.
- FUSE (`/dev/fuse` + `fusermount`) is present, so the AppImage runs without
  extraction.

## Boundary

This is the `linux-x64` clean-machine install observation only. The macOS and
Windows clean-machine observations, the full cross-platform update/rollback
acceptance, and the first stable channel-pointer promotion remain owner-gated
for a stable cut. RC promotion through the coordinator is proven and served
live (see the atomic-coordinator promotion runbook dated 2026-07-21 and
[#8917](https://github.com/OpenAgentsInc/openagents/issues/8917)).
