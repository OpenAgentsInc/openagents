/**
 * CLI: generate a simulated SARAH-CW-09 community journey receipt JSON to stdout
 * (or to --out <path>). Does not require a live outside developer.
 *
 * Usage:
 *   pnpm --dir packages/sarah run generate:community-journey-receipt
 *   pnpm --dir packages/sarah run generate:community-journey-receipt -- --out fixtures/sarah-community-journey/receipt.simulated.json
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import {
  runSarahCommunityJourney,
  serializeSarahCommunityJourneyReceipt,
} from "./harness.ts";

const args = process.argv.slice(2);
const outIdx = args.indexOf("--out");
const outPath = outIdx >= 0 ? args[outIdx + 1] : undefined;

const receipt = await runSarahCommunityJourney({
  generatedAt: new Date().toISOString(),
});
const json = `${serializeSarahCommunityJourneyReceipt(receipt)}\n`;

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
