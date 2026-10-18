# The GCE agent host: a third device for agent chats

Issue [#12116](https://github.com/OpenAgentsInc/openagents/issues/12116),
2026-10-18.

`gce-agent-host-1` is an OpenAgents device on Google Cloud, next to the Mac
(Tron) and `coderos-4080`. It shows in `list_devices`, and `create_chat`
with `device: gce-agent-host-1` runs an agent chat there. Issue work builds
on its disk, not the Mac's.

| | |
| --- | --- |
| Device | `gce-agent-host-1`, id `9e1347ca-0e14-479a-a785-8525a9853b32` |
| VM | `oa-agent-host-1`, `us-central1-a`, on-demand `c3-standard-22` (22 vCPU, 88 GB), 1 TB pd-ssd, no external address (Cloud NAT), Shielded VM, label `openagents-managed=agent-host` |
| Image | `oa-coder-host` (repository clone, toolchains, a warm Cargo target, the Codex and Claude Code CLIs, not logged in) |
| VM account | `oa-coder-host@` (the sccache bucket and the landing queue's prefix; no project role) |
| Made and run by | the automation account `oa-mvp-automation@` (`CLOUDSDK_CONFIG`), never a person's login |

`scripts/cloud/agent-host.sh` does everything below from the Mac:

```sh
scripts/cloud/agent-host.sh status        # VM state, engine, idle stop, Codex login, free disk
scripts/cloud/agent-host.sh start         # after an idle stop
scripts/cloud/agent-host.sh stop
scripts/cloud/agent-host.sh login-codex   # the owner's one step: approve a device code
scripts/cloud/agent-host.sh upgrade       # rebuild the engine from origin/main, restart it
scripts/cloud/agent-host.sh ssh
```

## The engine

The headless engine is built on the host from `apps/openagents` with
`cargo build --locked -p openagents --no-default-features` (Rust 1.98.1,
debug info off; 8 minutes 21 seconds cold), as `coderos-4080`'s was (#12078):

- **Service:** the systemd user unit `openagents-agent-host.service` runs
  `~/.openagents/native-host/current/openagents headless`. It's enabled,
  user lingering is on, and it came back by itself after a reboot.
  `current` points at `versions/<commit>/` (with a `COMMIT` file), so an
  upgrade switches it and the old build stays for rollback.
- **Data:** `~/.local/share/openagents-agent-host`; IPC on loopback port
  42941.
- **Account:** a sign-in key of its own, **GCE agent host oa-agent-host-1
  (12116)** (`openagents keys list`), in
  `~/.config/openagents/agent-host-12116.env` (mode 600, never printed). To
  remove the device, stop the unit and revoke that key with
  `openagents keys revoke 'GCE agent host oa-agent-host-1 (12116)'`.
- **Builds:** the unit sets `CARGO_TARGET_DIR=~/work/openagents-target-1`,
  `OPENAGENTS_AGENT_TARGETS=~/work/agent-targets` and `CARGO_INCREMENTAL=0`.
  Agents source `scripts/dev/cargo-build-env.sh` per build, as on the Mac.
- **IPv4 first:** `/etc/gai.conf` prefers IPv4 because the VM has no IPv6
  route and `sync.openagents.com` answers on both.

The phone's QR pairing host doesn't start on this device ("the owner key
cannot be created": there's no key store on a headless Linux VM). The app
reaches the device through the account's relay, as it does `coderos-4080`.

## Harness logins

The host has no harness login of its own. Logins are the owner's and are
never copied from another computer. The owner runs
`scripts/cloud/agent-host.sh login-codex` once, from a checkout on the Mac,
opens the link it prints and enters the code. Codex then stays signed in on
the host's disk across restarts. Claude Code can be added the same way
(`scripts/cloud/agent-host.sh ssh`, then `claude` and `/login`).

## Idle stop and cost

The root timer `oa-agent-host-idle.timer` checks every minute. The host is
busy while it has a job (`cargo`, `rustc`, `claude`, `codex`, `grok`,
`devin`, `microcoder`, `node`, `python3`), an ssh session, or
`~/.openagents/keep-awake`. After `oa-agent-host-idle-minutes` (instance
metadata, default 60; 0 never stops) with none, it powers itself off, and
the device goes offline. To bring it back, run `scripts/cloud/agent-host.sh
start` (about a minute until the device shows). Change the limit with
`gcloud compute instances add-metadata oa-agent-host-1 --zone us-central1-a
--metadata oa-agent-host-idle-minutes=N`.

List price, us-central1:

| | Per hour | Per day |
| --- | --- | --- |
| Running (`c3-standard-22` on demand) | about $1.13 | about $27 at 24 hours |
| 1 TB pd-ssd (billed while stopped too) | about $0.23 | about $5.60 |
| A 12-hour working day | | about $19 |

## Proof

See [#12116](https://github.com/OpenAgentsInc/openagents/issues/12116).
