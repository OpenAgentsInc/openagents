# Autopilot Remote Control

Autopilot Remote Control is the Expo mobile client for observing and steering a paired Pylon Autopilot Coder node. It is the open-source, single-operator companion app described by roadmap CL-4 / issue #4906.

This is a vanilla Expo scaffold, not an Ignite app. Future work can mine Ignite for selected pieces such as MMKV persistence, Maestro flows, and theming structure (the drawer/nav was already harvested), while keeping the protocol and streaming layers centered on `@openagentsinc/autopilot-control-protocol`. Builds are **local (our infra)** and JS updates ship over **our own OTA server** — EAS / Expo cloud is not used (see `TESTFLIGHT.md`).

The App Store convenience binary is intended to be a $4.99 paid download. The source remains available in this repository.

Pairing credentials and node bearer material must be stored with `expo-secure-store`; MMKV is for non-secret state such as paired-node metadata, cached projections, and stream cursors.

## Operator Integration

Dependencies are intentionally declared but not installed by this scaffold. Run Expo installation and build steps only during the later operator integration step.
