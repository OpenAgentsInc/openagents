/**
 * CLI: generate a simulated SARAH-NR-09 journey receipt JSON to stdout
 * (or to --out <path>). Does not require a signed Omega install.
 *
 * Usage:
 *   pnpm --dir packages/sarah run generate:journey-receipt
 *   pnpm --dir packages/sarah run generate:journey-receipt -- --out fixtures/sarah-nostr-journey/receipt.simulated.json
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import {
  runSarahNostrJourney,
  serializeSarahNostrJourneyReceipt,
} from "./harness.ts";

const args = process.argv.slice(2);
const outIdx = args.indexOf("--out");
const outPath = outIdx >= 0 ? args[outIdx + 1] : undefined;

const receipt = await runSarahNostrJourney({
  generatedAt: new Date().toISOString(),
});
const json = `${serializeSarahNostrJourneyReceipt(receipt)}\n`;

if (outPath !== undefined && outPath.length > 0) {
  const absolute = resolve(outPath);
  mkdirSync(dirname(absolute), { recursive: true });
  writeFileSync(absolute, json, "utf8");
  process.stderr.write(
    `wrote ${absolute} overall=${receipt.summary.overall} automatedPassed=${receipt.summary.automatedPassed} humanResidual=${receipt.summary.humanResidual}\n`,
  );
} else {
  process.stdout.write(json);
}
