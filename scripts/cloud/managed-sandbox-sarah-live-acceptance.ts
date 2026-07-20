#!/usr/bin/env -S pnpm exec tsx

/**
 * Canonical AssuranceSpec entry point for Sarah's owner-gated live
 * managed-sandbox journey. The implementation stays beside Sarah's Worker
 * tools so it compiles against the same bindings and runtime adapter.
 */

await import("../../apps/openagents.com/workers/api/scripts/managed-sandbox-sarah-live-acceptance.ts");
