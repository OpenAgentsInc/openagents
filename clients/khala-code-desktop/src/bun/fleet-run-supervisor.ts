/**
 * Compatibility re-export from the retired Khala Code desktop source tree.
 *
 * FleetRun scheduling, claims, refill, mixed-harness selection, and restart
 * reconciliation are Pylon engine responsibilities. New work belongs in
 * `apps/pylon/src/orchestration/fleet-run-supervisor.ts`; this legacy path is
 * retained only so extraction-source consumers continue to compile.
 */
export * from "../../../../apps/pylon/src/orchestration/fleet-run-supervisor.js"
