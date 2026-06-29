# Packaged Runtime Task Smoke

This smoke is the no-spend runtime gate for issue #4661. It is stricter than
`smoke:live-worker-loop`: the assignment must be executed by an installed
`pylon` binary through `pylon assignment run-no-spend`, not by a helper script
posting lifecycle API calls directly.

## Assignment Shape

The operator-created assignment should be a no-spend validation task. The
runtime gate lives inside this public-safe coding assignment payload:

```json
{
  "schema": "openagents.autopilot_coding_assignment.v1",
  "budget": {
    "paymentMode": "unpaid_smoke"
  },
  "objective": {
    "objectiveRef": "objective.public.pylon_runtime_gate.fixture_repair"
  },
  "publicSafe": true,
  "requiredCapabilityRefs": ["cap.gepa.retained.v1"],
  "runtimeGate": {
    "schema": "openagents.pylon.runtime_gate.v0.3",
    "agentKind": "codex_cli_or_fixture",
    "fixtureRef": "fixture.public.pylon.codex_runtime.sum_repair.v1"
  }
}
```

The top-level assignment also carries the controlled-dispatch refs required by
`/api/operator/pylons/assignments`:

```json
{
  "campaignPaused": false,
  "campaignPolicyRefs": ["policy.public.no_spend_smoke"],
  "closeoutPathRefs": ["closeout.public.operator_review_required"],
  "forumAutoPublishAllowed": false,
  "noForumAutoPublishRefs": ["policy.public.no_forum_auto_publish"],
  "paymentMode": "unpaid_smoke",
  "requiredCapabilityRefs": [
    "cap.gepa.retained.v1",
    "capability.public.packaged_binary",
    "capability.public.pylon_runtime_gate"
  ],
  "rollbackRefs": ["rollback.public.cancel_smoke_assignment"],
  "selectionPolicyRefs": ["selection.public.explicit_pylon_ref"],
  "spendCapRefs": ["spend_cap.public.no_spend"]
}
```

The packaged runner recognizes only that runtime gate. It creates a bounded
workspace under local Pylon cache state, repairs the fixture, runs `bun test`,
and reports only stable public refs:

- `artifact.pylon.runtime_gate.fixture_patch.*`
- `proof.pylon.runtime_gate.test_passed.*`
- `command.pylon.runtime_gate.bun_test.*`
- `run.pylon.runtime_gate.*`
- `result.public.pylon_runtime_gate.fixture_repair_passed`
- `summary.public.pylon_runtime_gate.fixture_repair_passed`

Local paths, source contents, command stdout, command stderr, and credentials
must not be sent to OpenAgents.

## Repeatable Smoke

The committed wrapper performs the packaged-binary path end to end: install,
bootstrap, `provider go-online`, register, heartbeat, wallet-readiness report,
operator assignment creation when an admin token is present, then
`pylon assignment run-no-spend`.

```sh
OPENAGENTS_AGENT_TOKEN="<redacted>" \
OPENAGENTS_ADMIN_API_TOKEN="<redacted optional, creates the assignment>" \
bun run smoke:packaged-runtime-task
```

The wrapper packs both local packages used by the installed smoke:

- `@openagentsinc/pylon`
- `@openagentsinc/nip90`, which reuses the workspace `nostr-effect` NIP-90
  implementation instead of rebuilding protocol helpers inside Pylon
- `@openagentsinc/tassadar-executor`, which is a workspace dependency of the
  executor-trace lane and must be installed from the local package until it is
  published

Optional inputs:

- `OPENAGENTS_BASE_URL`: defaults to `https://openagents.com`.
- `PYLON_PACKAGED_RUNTIME_TASK_SMOKE_PYLON_REF`: override the generated public
  Pylon ref.
- `PYLON_PACKAGED_RUNTIME_TASK_SMOKE_ASSIGNMENT_REF`: override the generated
  assignment ref.

If `OPENAGENTS_ADMIN_API_TOKEN` is present, the wrapper creates the no-spend
runtime-gate assignment for the generated Pylon ref. Without it, the wrapper
still installs, bootstraps, registers, heartbeats, and then requires an already
offered compatible assignment.

The wrapper emits only public-safe refs and a compact bootstrap summary. It
does not print local workspace paths, source contents, stdout, stderr, provider
credentials, wallet material, invoices, payment hashes, or preimages.

## Manual Live Procedure

1. Install the published package in a clean temporary directory.

   ```sh
   tmpdir="$(mktemp -d)"
   cd "$tmpdir"
   bun init -y
   bun add @openagentsinc/pylon@0.3.0-rc1
   ```

2. Use a fresh local Pylon home and register the packaged binary.

   ```sh
   export PYLON_HOME="$tmpdir/pylon-home"
   export PYLON_OPENAGENTS_BASE_URL="https://openagents.com"
   export OPENAGENTS_AGENT_TOKEN="<redacted>"
   bunx pylon bootstrap --json \
     --pylon-ref "pylon.public.runtime_gate.example" \
     --display-name "Pylon packaged runtime task smoke" \
     --capability-ref "cap.gepa.retained.v1" \
     --capability-ref "capability.public.packaged_binary" \
     --capability-ref "capability.public.pylon_runtime_gate"
   bunx pylon provider go-online
   bunx pylon presence register --base-url "$PYLON_OPENAGENTS_BASE_URL"
   bunx pylon presence heartbeat --base-url "$PYLON_OPENAGENTS_BASE_URL"
   bunx pylon wallet report-readiness --base-url "$PYLON_OPENAGENTS_BASE_URL"
   ```

3. Have an operator create the no-spend validation assignment above for the
   registered `pylonRef`.

4. Run the installed binary worker-loop command.

   ```sh
   bunx pylon assignment run-no-spend --base-url "$OPENAGENTS_BASE_URL"
   ```

5. Verify the assignment lifecycle contains accept, progress, artifact
   submission, and closeout receipts. The closeout should have
   `status: accepted`, `settlementState: not_applicable`, and
   `payoutClaimAllowed: false`.

## Current Caveat

`bun run smoke:live-worker-loop` can create a compatible runtime-gate assignment,
but it still drives the lifecycle directly and is not enough to close #4661.
The closing evidence must come from the packaged `pylon assignment run-no-spend`
path above.
