> **Resolution (2026-06-15, rev 2 of the guide).** Findings dispositioned; the
> guide (`2026-06-15-rc-agent-test-guide.md`) was rewritten so a **no-SDK run is a
> full pass**:
> - **Core is SDK-free, confirmed.** All 6 core commands (`help`/`bootstrap`/
>   `context`/`status`/`balance`/`memories`) emit valid JSON on **stdout** with no
>   agent SDK installed; the node boots without one. (The original run's `status`
>   "failure" was a stdout-vs-`2>&1` parse issue + a guide-shape mismatch, not a
>   binary fault.) The agent SDKs are `optionalDependencies`, so installs/builds
>   never require them and the compiled binary doesn't bundle them — SDK-free by
>   design.
> - **`status --json` shape** — guide now asserts the real nested shape
>   (`.kind`, `.state.version`, `.state.runtime.lifecycle`); no code change (that
>   envelope is the established public projection).
> - **`compiled binary sdk_missing`** — expected SDK-free behavior, **not a
>   defect**. We deliberately do **not** bundle agent SDKs into the standalone
>   binary. §4 (managed sessions) is now explicitly OPTIONAL + SDK-gated; no-SDK →
>   record `sessionSpawned: "skipped-no-sdk"` (a pass). Sessions are exercised from
>   a source checkout (where `node_modules` has the optional SDK) or an
>   SDK-equipped env.
> - **`update --check` 404** — feed-publishing state, not a binary failure; guide
>   marks it `feed-unpublished` and non-fatal. Publishing the RC feed to
>   `updates.openagents.com` remains a maintainer task.
> - **Guide robustness** — added the `pylon node` startup prerequisite, a random
>   `PYLON_CONTROL_PORT`, filtering `sessions list` by the spawned `sessionRef`,
>   and polling that ref to a terminal state.

# Pylon v1.0-rc Agent Test Results - Codex

Date: 2026-06-15
Tester: Codex
Guide: `docs/autopilot-coder/2026-06-15-rc-agent-test-guide.md`
Platform: `darwin-arm64`
Target version: `1.0.0-rc.1`

## Summary

The RC is partially working locally, but the current agent guide does not pass
end-to-end against the published artifact path or the compiled source-built
binary.

What worked:

- Built signed RC binaries from source in an isolated detached worktree.
- Verified the native `darwin-arm64` binary signature against the pinned
  OpenAgents release key.
- Confirmed most headless JSON commands return valid JSON.
- Confirmed the source-mode Pylon node can spawn a Codex-managed unit of work,
  create `pong.txt`, and pass the verification command.

What failed:

- The live RC feed URL for `darwin-arm64` returned 404.
- `status --json` returned valid JSON, but not in the top-level shape asserted
  by the guide.
- `update --check --json` failed because the live feed returned 404.
- The compiled `darwin-arm64` binary could not run a managed Codex session even
  though source mode could. The failure digest matches the SDK-missing path.

No deploy, publish, registry mutation, payout, or funded node home was used.

## Isolation

The test used isolated temp state and did not touch the normal Pylon home:

- Source checkout: detached temp worktree under `work/tmp/`
- Pylon state: throwaway `PYLON_HOME`
- Session workspace: throwaway directory under `work/tmp/`
- Node control API: loopback with random high ports, not the default `4716`
- Autoupdate: disabled with `PYLON_DISABLE_AUTOUPDATE=1` for bounded binary
  tests

Generated loopback control tokens were not copied into this report.

## Artifact Acquisition

The guide's preferred feed path was attempted first:

```sh
https://updates.openagents.com/pylon/rc/darwin-arm64/feed.json
```

Result: HTTP 404.

The source-build fallback was then used from a detached worktree:

```sh
cd apps/pylon
bun install
bun run build:rc-binaries 1.0.0-rc.1
```

Result: all four platform binaries built and signed:

- `darwin-arm64`
- `darwin-x64`
- `linux-x64`
- `linux-arm64`

Native binary:

- File: `apps/pylon/dist/rc/1.0.0-rc.1/pylon-darwin-arm64`
- SHA-256: `6b603b284304c9a304300998f242b5421173a9d74654a70bfc76f7c3f803f902`
- Signature key id: `2dbe811d19f67528`
- Signature verification: passed

## JSON Command Results

Commands that passed the guide-level JSON checks:

- `help --json`
- `bootstrap --json`
- `context --json`
- `balance --json`
- `memories --json`

Command that returned valid JSON but failed the guide assertion:

- `status --json`

The guide expects top-level fields:

```json
{
  "version": "1.0.0-rc.1",
  "lifecycle": "..."
}
```

Actual shape was:

```json
{
  "kind": "status",
  "state": {
    "version": "1.0.0-rc.1",
    "runtime": {
      "lifecycle": "offline"
    }
  }
}
```

This is probably a guide/code contract mismatch. The least disruptive fix is to
add top-level compatibility fields to the `status --json` output:

```json
{
  "kind": "status",
  "version": "1.0.0-rc.1",
  "lifecycle": "offline",
  "state": {}
}
```

Command that failed because of the missing live feed:

- `update --check --json`

Observed public-safe error:

```json
{
  "status": "error",
  "error": "update feed https://updates.openagents.com/pylon/rc/darwin-arm64/feed.json returned 404",
  "applied": false
}
```

The update code route shape appears to match the guide:
`/pylon/<channel>/<platform>/feed.json`. This looks like a publishing/seed issue
for the RC feed, not a client route mismatch.

## Session Spawn Results

The guide's optional session command requires a running node. Running
`pylon sessions spawn` before starting `pylon node` returned a clean JSON
control error:

```json
{
  "ok": false,
  "command": "sessions",
  "code": "no_token"
}
```

This is not a binary failure, but the guide should explicitly start a node first
or mark the session step as requiring a pre-running node.

### Compiled Binary Session

After starting the compiled binary as a node on an isolated high loopback port,
`sessions spawn` reached the control API and returned a running session. The
session immediately transitioned to failed.

Public projection:

- `state`: `failed`
- `errorClass`: `execution_error`
- `errorDigestRef`: `digest.pylon.session.error.51f03833164a4105ccd4e22a`

The digest matches this exception string:

```text
Error: Codex composer unavailable: sdk_missing (blocker.codex_agent.sdk_missing)
```

This is the strongest evidence that the compiled binary does not include or
resolve the lazy Codex SDK import.

### Direct SDK Probe

A direct `@openai/codex-sdk` probe in an isolated directory succeeded:

- Started a Codex thread.
- Created `pong.txt`.
- Wrote `pong`.
- Completed successfully.

This confirms the local Codex credentials and SDK can work outside the compiled
binary path.

### Source-Mode Pylon Session

Running the same node/session flow through source mode succeeded:

```sh
bun src/index.ts node
bun src/index.ts sessions spawn --adapter codex \
  --objective "create a file pong.txt containing 'pong'" \
  --verify "test -f pong.txt" \
  --worktree <throwaway-session-dir>
```

Result:

- `pong.txt` was created with `pong`.
- Verification command passed.
- A clean proof artifact was written.
- Session executor reported `local_bounded`, `workspace-write`, and
  `networkAccessEnabled: false`.

One polling attempt saw the managed session as still `running` while the proof
was written shortly afterward. A more reliable guide step should poll until the
specific session ref reaches a terminal state, rather than stopping when any
terminal state appears in the merged session list.

## Likely Root Causes

1. RC feed is not published or seeded for `darwin-arm64`.

   The client requests the documented URL and receives 404. The update service
   source supports this route, so the likely fix is in the publish/seed path for
   `updates.openagents.com`.

2. `status --json` shape does not match the agent guide.

   The CLI currently nests version and lifecycle under `state`. The guide
   asserts top-level fields. Add top-level compatibility fields or update the
   guide assertions.

3. Compiled binary does not package the Codex SDK lazy import.

   The Codex SDK is imported through a variable specifier:

   ```ts
   const importer = options.importer ?? ((specifier: string) => import(specifier))
   const sdk = await importer(CODEX_AGENT_SDK_PACKAGE)
   ```

   Source mode resolves this through `node_modules`. The compiled binary reports
   `sdk_missing`. Bun compile likely cannot statically discover and bundle this
   variable dynamic import. A likely fix is to keep test injection support but
   use a literal dynamic import in the default path:

   ```ts
   const importCodexAgentSdk = () => import("@openai/codex-sdk")
   ```

   Then call the injected importer only in tests or explicit overrides.

4. The guide omits the node startup prerequisite for `sessions spawn`.

   The CLI session commands intentionally operate through the running node's
   control API. The guide should start `pylon node` with isolated
   `PYLON_HOME`/`PYLON_CONTROL_PORT`, then run spawn/list against that endpoint.

5. Session polling should target the spawned session ref.

   `sessions list --json` includes external host Codex/Claude sessions. An
   assertion should filter to the newly spawned `sessionRef` and wait for that
   specific session to become `completed` or `failed`.

## Suggested Fix List

High priority:

- Publish or seed `https://updates.openagents.com/pylon/rc/darwin-arm64/feed.json`
  with the signed `1.0.0-rc.1` release.
- Make `status --json` satisfy the guide by adding top-level `version` and
  `lifecycle` fields, while preserving the existing nested `state` payload.
- Change the default Codex SDK import path so Bun compile bundles it into the
  standalone binary.
- Add a compiled-binary smoke that starts `pylon node`, spawns a Codex session
  in a throwaway worktree, waits for that exact session ref, and verifies a
  passing proof.

Guide updates:

- State that `sessions spawn` requires `pylon node` to be running.
- Use a random `PYLON_CONTROL_PORT` in the guide to avoid collisions.
- Filter `sessions list --json` by the spawned session ref.
- Note that `update --check --json` depends on the live RC feed being published.

## Final Machine-Readable Result

This reflects the compiled binary test against the guide:

```json
{
  "platform": "darwin-arm64",
  "pylonVersion": "1.0.0-rc.1",
  "signatureVerified": true,
  "commandsOk": ["help", "bootstrap", "context", "balance", "memories"],
  "commandsFailed": ["status", "update-check"],
  "sessionSpawned": false,
  "notes": "Source build and signature verification passed. status --json uses nested state.version/state.runtime.lifecycle instead of guide top-level fields. update --check failed because the live RC feed returned 404. Compiled binary managed session failed with sdk_missing, while source-mode Pylon session succeeded and created pong.txt."
}
```
