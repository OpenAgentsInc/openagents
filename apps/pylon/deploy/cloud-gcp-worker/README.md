# `cloud-gcp` Ephemeral Worker-VM Bootstrap

This directory is the minimal run-capable bootstrap for the **plain, already
-proven `cloud-gcp` ephemeral VM lane** (private `cloud/` repo,
`crates/oa-codex-control/src/gce_capacity.rs`, cloud#95/#96/#97): one
`oa-codex-sess-*` VM per session, provisioned and torn down (VM + firewall
rule) with zero leak, verified in
`cloud/docs/bootstrap/2026-06-14-gce-autonomous-control-node.md`.

**This is NOT the Firecracker Agent Computer image** documented in
`../agent-computer/`. That is a separate, not-yet-built microVM guest
kernel/rootfs lane (openagents#8503's Firecracker gap, deliberately not
attempted casually — see that doc's own NEEDS_OWNER framing). This directory
is the much simpler fix for a different, already-real gap: today the
`cloud-gcp` lane's default image (`ubuntu-2404-lts-amd64`) is
provisioner-only — it boots and answers SSH but has no coding-agent-runnable
runtime installed.

## What `bootstrap.sh` does

1. Installs Bun if not already present.
2. Clones the public `openagents` repo at a pinned ref (default `main`,
   overridable via the `openagents-pin-ref` instance metadata attribute).
3. Runs `bun install` at the workspace root.
4. Starts `apps/pylon/src/orchestration/runtime-intent-supervisor.ts` with
   **no** `--owner-user-id`, which selects `org_cloud` executor mode (see that
   file's own `executorMode` default) — the same mode the org-cloud runtime
   spine (#8473) already uses, with `hosted_khala` enabled by default in that
   mode. `hosted_khala` is the simplest lane to prove first: it calls the
   OpenAgents inference gateway from inside the admitted work context rather
   than needing a full Codex/Claude account credential inside the VM.

## Secrets

This script is public-safe and accepts **no secrets** in its own body or on
its command line. The three required values
(`OPENAGENTS_ADMIN_API_TOKEN`, `OPENAGENTS_AGENT_TOKEN`, `OPENAGENTS_BASE_URL`)
are read at boot from the instance's own GCE metadata server
(`metadata_attr`/`require_metadata` in the script), which only that instance
can query. **Wiring the private control plane to actually write short-lived,
run-scoped values onto each ephemeral VM at `instances create` time (mirroring
the session-scoped SSH-metadata pattern `gce_capacity.rs` already uses for SSH,
and cleaning them up at teardown) is a separate, not-yet-done integration
step** — this script only defines the contract it expects on the VM side. Do
not widen this to a long-lived, broadly-scoped credential; each value should
be minted per-session and revoked/discarded with the VM at teardown.

## Local dry-run of the pure helpers

`metadata_attr`/`require_metadata` fall back to an env var named after the
metadata key (dashes replaced with underscores, upper-cased) when not running
on a real GCE instance, so they're testable without a metadata server — see
`apps/pylon/tests/cloud-gcp-worker-bootstrap.test.ts`.

## Still gated

- Wiring `gce_capacity.rs`'s live provisioner to pass this script as
  `--metadata-from-file startup-script=` plus the per-session metadata
  attributes it expects (not done this pass — a private `cloud/` repo change).
- A real end-to-end `hosted_khala` turn proof through this bootstrap requires
  a real admitted work context (mobile session + credit balance) dispatching
  into the `cloud-gcp` lane with `OA_CODEX_GCE_PROVISIONER=live`.
