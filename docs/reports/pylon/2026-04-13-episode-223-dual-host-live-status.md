## Episode 223 dual-host live status

Retained on April 13-14, 2026 while pushing the live `CS336 A1 Demo` path from
local rehearsal into the real Mac + Linux Pylon fleet.

### 2026-04-14 rc6 live deploy update

- `pylon-v0.1.1-rc6` now exists on GitHub with both required assets:
  - `pylon-v0.1.1-rc6-darwin-arm64.tar.gz`
  - `pylon-v0.1.1-rc6-linux-x86_64.tar.gz`
- `macbook-pro-m2` is now running:
  - `/Users/christopherdavid/code/pylon-v0.1.1-rc6-darwin-arm64/pylon serve`
- `archlinux` is now running:
  - `/home/christopherdavid/.openagents/pylon/releases/pylon-v0.1.1-rc6-linux-x86_64/pylon serve`
- The retained live host proof still shows the bounded training path honestly:
  - both hosts are `desired_mode = "online"`
  - both hosts report `training_operator_status.contributor_supported = true`
  - both hosts show the inventory row
    `psionic.cluster.training.adapter_contributor.cluster_attached` as
    `enabled = true`, `backend_ready = true`, `eligible = true`,
    `delivery_state = "idle"`
- The final Nexus deploy branch is:
  - `codex/ep223-live-ship-v2`
- That branch is based on the newer
  `origin/codex/ep223-live-combined` head, so it keeps the separate treasury
  hotfix line instead of rolling it back.
- The validation Nexus image from that branch was built and deployed:
  - Cloud Build `f2680bf2-3628-4c74-97ad-091efd5938a4`
  - live image tag `nexus-ep223-live-v2-20260414-130340`
- Public Nexus training reads are now live again:
  - `https://nexus.openagents.com/api/stats` returns `200`
  - `https://nexus.openagents.com/api/training/summary` returns `200`
  - `https://nexus.openagents.com/api/training/runs/run.cs336.a1.demo` returns
    the named `CS336 A1 Demo` run
  - `https://nexus.openagents.com/api/training/windows?training_run_id=run.cs336.a1.demo`
    currently returns `[]`
- The still-open live blocker shifted after that deploy:
  - the run is visible again, but the real Pylon hosts are still failing their
    automatic admission / heartbeat loop before they can start the first live
    training window

### What is now true

- `pylon-v0.1.1-rc6` exists with both `darwin-arm64` and `linux-x86_64` assets.
- `macbook-pro-m2` is running the `rc6` Darwin binary as a training
  contributor only.
- `archlinux` is running the `rc6` Linux binary as a training contributor only.
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

The current blocker is no longer release packaging, lane contract, or the
public GET routes. It is the live host POST path under real Pylon payloads.

- A compact synthetic training-node admission now succeeds publicly:
  - `POST https://nexus.openagents.com/api/training/nodes/admission` returns
    `200 admitted` for a valid Episode 223-shaped request
- But the real live `rc6` Pylons are still logging:
  - failed provider heartbeat POSTs
  - failed training node admission POSTs
  - occasional failed training lease claim POSTs
- The strongest concrete cause found in code is that the live Pylon clients are
  still using very aggressive HTTP deadlines:
  - provider presence client timeout: `2s`
  - training coordinator client timeout: `5s`
- Those deadlines are too small for the current live Nexus path through
  Cloudflare plus the heavier real payloads, even though the same routes now
  work for lighter manual probes.
- The retained branch now carries the next fix:
  - increase provider presence HTTP timeout to `15s`
  - increase training coordination HTTP timeout to `20s`
  - keep behavior otherwise unchanged
- The relevant Pylon tests are green on that patch:
  - `training_assignment_intake_claims_and_acks_assignment_and_updates_status`
  - `provider_presence_reports_online_and_offline_to_nexus`

So the honest current state is:

- both target hosts are on `rc6`
- the public Nexus read surfaces are live again
- the named `CS336 A1 Demo` run is visible again
- the run is still stalled before the first active window
- the next concrete move is to ship the timeout-adjusted Pylon build, roll both
  hosts forward, and then verify that the first live window is finally created
