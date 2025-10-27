#!/usr/bin/env node
/**
 * OpenAgents NPX placeholder (no Node WebSockets, no bridge spawn).
 *
 * For now, `npx openagents` only prints guidance. The Rust bridge
 * remains the single source of truth and is started separately.
 */

function main() {
  console.log("OpenAgents — Desktop Bridge Placeholder\n");
  console.log("This command is a placeholder for the upcoming CLI.\n");
  console.log("To use OpenAgents today:");
  console.log("  1) Clone the repo: https://github.com/OpenAgentsInc/openagents");
  console.log("  2) From the repo root, start the bridge: cargo bridge");
  console.log("  3) In the mobile app Settings, set Bridge URL to ws://localhost:8787/ws");
  console.log("  4) Open Session in the app and start chatting\n");
  console.log("Tip: If desktop and phone aren’t on the same LAN, use Tailscale.");
}

// Intentionally avoid referencing Node's `process` type so @types/node isn't required.
// If something throws here, Node will print the stack and exit.
main();
