# Bare Agent -> Pylon MCP -> Khala E2E Smoke

Issue: #6286  
Epic: #6273

This smoke proves the no-spend path where a vanilla coding agent becomes a
Khala-network client by adding the Pylon MCP config, issuing a coding request,
letting the owner-linked local Pylon execute it, and resuming the durable result
stream.

## Fixture Harness

Run the bounded fixture without live spend:

```sh
bun test apps/pylon/tests/khala-mcp-end-to-end.test.ts
```

The harness uses a temp Pylon home and an in-memory OpenAgents edge. It exercises:

- local Pylon MCP JSON-RPC `khala.request`
- Worker Khala MCP catalog assignment creation
- caller-owned linked Pylon routing
- Pylon `assignment run-no-spend` Codex fixture execution
- durable stream seeding on closeout
- local Pylon MCP `khala.resume`
- second-account durable resume denial before replay

No production token, wallet, or paid settlement is used.

## Vanilla Agent Setup

Emit the MCP config:

```sh
pylon mcp config --base-url https://openagents.com
```

Add the returned server entry to Codex or Claude Code and provide the owner
agent token through the environment variable shown in the config:

```sh
export OPENAGENTS_AGENT_TOKEN="<owner agent token>"
```

The local server entry is enough for a bare agent:

```json
{
  "mcpServers": {
    "openagents-khala-local": {
      "command": "pylon",
      "args": ["mcp"],
      "env": {
        "OPENAGENTS_AGENT_TOKEN": "${OPENAGENTS_AGENT_TOKEN}",
        "PYLON_OPENAGENTS_BASE_URL": "https://openagents.com"
      }
    }
  }
}
```

## Manual Smoke

Bring the local owner Pylon online with Codex capability and heartbeat fresh:

```sh
OPENAGENTS_AGENT_TOKEN="<owner agent token>" \
PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
pylon provider online
```

From the bare agent MCP client, call `khala.request` with:

```json
{
  "prompt": "Repair the public fixture through my linked Pylon.",
  "workflow": "codex_agent_task",
  "targetPylonRef": "<owner linked pylon ref>"
}
```

Run the no-spend local assignment loop for MCP-issued leases:

```sh
OPENAGENTS_AGENT_TOKEN="<owner agent token>" \
PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
pylon assignment run-no-spend
```

For the equivalent CLI request path, fixture smoke intent must be explicit:

```sh
OPENAGENTS_AGENT_TOKEN="<owner agent token>" \
PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
pylon khala request \
  --prompt "Repair the public fixture through my linked Pylon." \
  --workflow codex_agent_task \
  --pylon-ref "<owner linked pylon ref>" \
  --fixture \
  --json
```

The CLI path auto-runs the returned no-spend assignment unless `--no-run` is
present, so successful JSON includes both `autoRun` and `assignmentRun`. Use
`--no-run` only when intentionally testing lease creation or manual recovery.

Public issue or repository work must use complete workspace pins instead of the
fixture flag:

```sh
pylon khala request \
  --prompt "Implement public issue #NNNN and run the named verification." \
  --workflow codex_agent_task \
  --pylon-ref "<owner linked pylon ref>" \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "<current origin/main sha>" \
  --verify "bun test apps/pylon/tests/khala-requester.test.ts" \
  --json
```

Resume from the durable handle returned by `khala.request`:

```json
{
  "durableRequestId": "<durable request id>",
  "offset": 0
}
```

Resolve the owner-scoped Pylon/Codex closeout proof for the assignment:

```sh
OPENAGENTS_AGENT_TOKEN="<owner agent token>" \
PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
pylon khala closeout "<assignmentRef>" --json
```

The closeout projection composes assignment trace status and assignment proof.
Its `closeoutChecklist.ok` should be `true` only after the final owner-only
trace, raw event summary, exact token rows, no-spend/not-applicable closeout
policy with `payoutClaimAllowed: false`, and closed-out lifecycle all agree.

The underlying proof projection is still available when a narrower token/trace
proof is useful:

```sh
OPENAGENTS_AGENT_TOKEN="<owner agent token>" \
PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
pylon khala proof "<assignmentRef>" --json
```

Expected proof shape is public-safe only: exact `tokenUsage` row counts and
totals for `provider: "pylon-codex-own-capacity"`, `model:
"openagents/pylon-codex"`, `usageTruth: "exact"`, `demandKind:
"own_capacity"`, `demandSource: "khala_coding_delegation"`, owner-only trace
refs/counts, and private raw-event refs/counts/byte totals. It must never return
raw Codex events, prompts, command args, shell output, local paths, keys,
`trajectory_json`, `safe_metadata_json`, R2 keys, or trace bodies.

Expected result:

- the request creates `assignment.public.khala_coding.*`
- the assignment carries `request.public.khala_coding.<durable id>`
- the local Pylon closeout includes
  `result.public.pylon.codex_agent_task.fixture_repair_passed`
- `khala.resume` returns the durable closeout frame and `streamClosed: true`
- a different OpenAgents agent token receives a 403 denial before durable replay

## Invariants

- Own capacity only: assignment routing is limited to Pylons linked to the caller
  through the OpenAuth/agent account relation.
- No resale: the no-spend fixture uses the owner's local Codex capacity and does
  not broker shared platform credentials.
- Semantic, not keyword: `khala.request` carries typed workflow intent
  (`codex_agent_task`) instead of prose keyword routing.
- Default-on delegation: the MCP request path delegates when linked, fresh Codex
  Pylon capacity is available.
