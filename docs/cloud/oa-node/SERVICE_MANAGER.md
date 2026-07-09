# oa-node Service Manager

Status: Cloud MVP scaffold for `CND-018`

`oa-node service` models managed-node service installation and runtime state for
launchd and systemd. The MVP is deliberately unprivileged: it records the
service manager intent, service status, service events, and health events in
local node state. A later privileged installer can consume the same command and
state shape to write real launchd plists or systemd units.

```bash
oa-node service install --manager launchd --json
oa-node service install --manager systemd --service-name openagents-oa-node --json
oa-node service start --json
oa-node service stop --json
oa-node service restart --json
oa-node service status --json
oa-node service uninstall --json
```

The service files are:

```text
node-state.json
service-events.jsonl
health-events.jsonl
```

`install` sets `service_manager` to `launchd` or `systemd` and records an
`installed` service status. `start` and `restart` record `running` and project
node health as `online`. `stop` and `uninstall` project node health as
`offline`; uninstall returns the manager to `manual`.

Every state-changing service action appends a redacted service event and a
health event. Service names are bounded names, not paths, and names containing
raw secret, token, wallet, private-key, or private-topology markers are
rejected before any event is written.
