import { Effect } from "effect";

// ── Types ─────────────────────────────────────────────────────────────────
//
// Permission gating architecture note:
// Tools run inside a forked Effect fiber while the main chat loop owns stdin,
// so permission prompts CANNOT do synchronous I/O on stdin from within a tool
// handler. Any interactive permission UX must be plumbed through the main loop
// via a side-channel (e.g., pending request queue that the main loop polls
// between turns). For now the default handler always allows.
//
// The type infrastructure and module-level handler setter are here for when
// that integration is built.

export interface PermissionRequest {
  readonly action: "edit" | "write" | "delete";
  readonly filePath: string;
  readonly diff: string;
}

export type PermissionDecision = "allow" | "deny" | "always";

export interface PermissionHandler {
  ask(request: PermissionRequest): Effect.Effect<PermissionDecision, never>;
}

// ── Module-level permission handler ───────────────────────────────────────

let currentHandler: PermissionHandler = {
  ask: () => Effect.succeed("allow"),
};

export function setPermissionHandler(handler: PermissionHandler): void {
  currentHandler = handler;
}

export function getPermissionHandler(): PermissionHandler {
  return currentHandler;
}

export function resetPermissionHandler(): void {
  currentHandler = { ask: () => Effect.succeed("allow") };
}
