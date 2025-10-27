#!/usr/bin/env node
/**
 * OpenAgents NPX placeholder (no Node WebSockets, no bridge spawn).
 *
 * For now, `npx tricoder` only prints guidance. The Rust bridge
 * remains the single source of truth and is started separately.
 */
import chalk from "chalk";

function main() {
  console.info(chalk.bold("OpenAgents Tricoder - Desktop Bridge"));
}

// Intentionally avoid referencing Node's `process` type so @types/node isn't required.
// If something throws here, Node will print the stack and exit.
main();
