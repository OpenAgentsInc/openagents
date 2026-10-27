# Phone HOST SDK

`phone-host` is a device-only C ABI over the existing Rust HOST/REACH clients.
Aethyr's C++ coordinator owns the task and revision fences; Swift supplies
Keychain storage and byte streams from its embedded Tailscale node. The SDK
redeems invitations, verifies original grants, fetches fresh signed presence,
proves a REACH channel and prepares/exchanges native task create/read/cancel
operations. It does not interpret model-written shell commands.

Schemas live in `proto/openagents/{common,host,reach}/v1`. Protobuf carries the
typed native boundary. It does not replace Nostr JSON, JCS, signatures, NIP-44,
HOST admission or the native task owner. Access projections are compared to the
verified original authorization; a projected right cannot widen a grant.

The first phone path uses unscoped device grants and an authenticated Tailnet
TCP hint for the selected computer. Other hints and scoped grants fail closed.
The Rust call runs off the main thread; the Go callback only reads/writes bytes.
Keys are passed separately from protobuf requests and never enter a task record.

The app saves the exact prepared signed packet before sending it. Reconnects
retry that packet until expiry, then read the stable task identity. New phone
identities contain 32 random bytes. A native dispatch receipt means queued,
not completed. Paged signed task evidence supplies real messages, tool IDs,
statuses and results; the app saves its C++ task before advancing the cursor.
Cancellation remains pending until the task owner confirms its outcome.

`read_only: true` requires the host's `task-read-only` capability. Native task
intent retains that restriction and derives the existing read-only owner policy
on every launch. A later host policy cannot turn it into an editing task.

## Build and test

```sh
cargo test -p phone-host
cargo test -p engine-proto
cargo test -p coder-task read_only
```

Aethyr's `tools/ios-host-build.sh` builds this static library at the exact clean
OpenAgents revision in its imported bundle manifest. Normal engine/Wasm builds
do not link Rust or need an OpenAgents checkout.

## Linux setup and live proof

Use a separate host state/root/task directory and the existing commands:

```sh
coder host serve --state STATE --root ROOT --tasks TASKS \
  --workspace aethyr=/absolute/checkout --listen TAILNET_IP:47113 \
  --allow-nonloopback --advertise tailnet=TAILNET_IP:47113 --no-telemetry
coder host autostart on --root ROOT --workspace aethyr --max-running 1 \
  --route codex:gpt-6-sol --read-only --controller /absolute/microcoder \
  --decision-endpoint https://openagents.com/api --decision-model clef-flash
coder host invite --state STATE --root ROOT --rights observe,operate
```

The decision endpoint and model must match the service named by the execution
grant; the native owner refuses mismatches. Pair from the phone's Agent screen
with the short-lived invitation, select the workspace label, then run a task.
The opt-in `AgentLiveProtoTests` fixture contains only peer ID and invitation;
it uses the actual app coordinator, native SDK, embedded Tailnet, HOST admission,
native owner and Codex. It retains the result in the app's normal `task.pb`,
checks persistence after restart and removes its invitation fixture. No SSH
credential is supplied to this test. Never commit the invitation or device key.
