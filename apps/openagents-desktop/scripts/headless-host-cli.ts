/**
 * #9161: the operator CLI over `createHeadlessHost` — the documented command
 * that runs conversations and Full Auto through the PRODUCTION Desktop host
 * services with no renderer, and emits public-safe + private evidence
 * receipts. A thin pass-through: it wires a lane, drives one turn or starts a
 * Full Auto run, and prints the receipt JSON.
 *
 * Usage (package script `pnpm --dir apps/openagents-desktop run headless-host`):
 *   # ordinary turn through a real owner-local Codex turn (spends capacity):
 *   node --import tsx scripts/headless-host-cli.ts codex-turn \
 *     --message "hey who are you" [--model gpt-5.6-terra] [--effort medium] \
 *     [--sandbox read-only|workspace-write] [--root <dir>] [--private]
 *
 *   # start a Full Auto run (creates exactly one durable run record):
 *   node --import tsx scripts/headless-host-cli.ts full-auto-start \
 *     --objective "Implement #NNNN and run the named verification." \
 *     [--title "..."] [--done "..."] [--root <dir>]
 *
 * Output: the public-safe receipt JSON on stdout by default; `--private`
 * additionally prints the private receipt (frames + answer) to stderr. Raw
 * answer text and local paths never appear on stdout.
 */

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { makeCodexHeadlessLane } from "../src/codex-headless-lane.ts";
import { createHeadlessHost } from "../src/desktop-headless-host.ts";
import { deriveHeadlessReceipts } from "../src/desktop-headless-receipt.ts";

const argv = process.argv.slice(2);
const command = argv[0];

const flag = (name: string): string | undefined => {
  const index = argv.indexOf(name);
  return index === -1 || index + 1 >= argv.length ? undefined : argv[index + 1];
};
const hasFlag = (name: string): boolean => argv.includes(name);

const USAGE = `usage: headless-host-cli <command> [flags]

  codex-turn        --message <text> [--model <id>] [--effort low|medium|high]
                    [--sandbox read-only|workspace-write] [--root <dir>] [--private]
  full-auto-start   --objective <text> [--title <t>] [--done <t>] [--root <dir>]

Prints the public-safe receipt JSON on stdout. --private also prints the
private receipt (frames + answer) to stderr. Codex turns spend real
owner-local provider capacity.`;

const fail = (message: string): never => {
  process.stderr.write(`${message}\n`);
  process.exit(1);
};

const run = async (): Promise<void> => {
  const root = flag("--root") ?? mkdtempSync(join(tmpdir(), "oa-headless-host-"));

  if (command === "codex-turn") {
    const message = flag("--message");
    if (message === undefined) fail("codex-turn requires --message");
    const workspace = mkdtempSync(join(tmpdir(), "oa-headless-work-"));
    const sandbox = flag("--sandbox") === "workspace-write" ? "workspace-write" : "read-only";
    const host = createHeadlessHost({ root });
    const lane = makeCodexHeadlessLane({
      workspace,
      model: flag("--model") ?? "gpt-5.6-terra",
      reasoningEffort: flag("--effort") ?? "medium",
      sandbox,
      timeoutMs: 300_000,
    });
    const thread = host.createThread("headless codex turn");
    const result = await host.submitOrdinaryTurn({
      lane,
      threadRef: thread.id,
      turnRef: "turn-1",
      message: message as string,
    });
    const { publicReceipt, privateReceipt } = deriveHeadlessReceipts("turn-1", thread.id, result);
    process.stdout.write(`${JSON.stringify(publicReceipt, null, 2)}\n`);
    if (hasFlag("--private")) {
      process.stderr.write(`${JSON.stringify(privateReceipt, null, 2)}\n`);
    }
    process.exit(publicReceipt.coherence.disposition === "fail" ? 1 : 0);
  }

  if (command === "full-auto-start") {
    const objective = flag("--objective");
    if (objective === undefined) fail("full-auto-start requires --objective");
    const host = createHeadlessHost({ root });
    const run = host.startFullAutoRun({
      title: flag("--title") ?? "headless Full Auto run",
      objective: objective as string,
      doneCondition:
        flag("--done") ??
        "Complete the objective, run relevant verification, and report the result or a concrete blocker.",
    });
    process.stdout.write(
      `${JSON.stringify({ runRef: run.runRef, state: run.state, objective: run.objective, root }, null, 2)}\n`,
    );
    process.exit(0);
  }

  process.stderr.write(`${USAGE}\n`);
  process.exit(command === undefined || command === "--help" || command === "-h" ? 0 : 2);
};

void run();
