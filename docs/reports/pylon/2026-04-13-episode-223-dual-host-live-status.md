## Episode 223 dual-host live status

Retained on April 13-14, 2026 while pushing the live `CS336 A1 Demo` path from
local rehearsal into the real Mac + Linux Pylon fleet.

### What is now true

- `pylon-v0.1.1-rc4` exists with both `darwin-arm64` and `linux-x86_64` assets.
- `macbook-pro-m2` is running the `rc4` Darwin binary and is live as a training
  contributor only.
- `archlinux` is running the `rc4` Linux binary and is live as a training
  contributor only.
- On both machines:
  - local Gemma inference is disabled for the Episode 223 path
  - `adapter_training_contributor_enabled = true`
  - `pylon status --json` reports the training contributor row from
    `pylon.serve`
  - the training contributor row is `enabled = true`, `backend_ready = true`,
    `eligible = true`, `delivery_state = "idle"`
  - the advertised environment ref remains
    `psionic.environment.psion_cs336_a1_demo.host_cpu.operator@v1`

### Important config fix

During host rollout, a real config persistence bug showed up in `pylon`:

- `pylon config set ...` wrote the public config JSON surface
- that public surface omitted `adapter_training_contributor_enabled`
- a normal config save therefore stripped the training contributor toggle back
  out of `~/.openagents/pylon/config.json`

The code fix is now retained in the repo:

- expose `adapter_training_contributor_enabled` in the public config JSON
- support `backend.adapter_training_contributor_enabled` in `pylon config set`
- test that saved config files preserve the training contributor toggle

### What is still blocking the live shared run

The blocker is no longer the fleet.

The blocker is the public Nexus training surface:

- `https://nexus.openagents.com/api/training/summary` timed out from here
- `https://nexus.openagents.com/api/stats` also timed out from here in the same
  pass
- the Cloud Build for the Nexus public training-surface image was still
  `WORKING` at:
  - build id `0c4db6a8-e025-4a2d-9d38-67e7cf28064c`

So the honest current state is:

- Mac host ready
- Linux host ready
- local dual-host rehearsal already green
- live public Nexus training read path still not reliable enough to claim the
  final Episode 223 proof run yet
