#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { coverageByArea } from "./grade.ts";
import { buildInventory } from "./inventory.ts";
import {
  serializeSurfaceInventory,
  SURFACE_INVENTORY_PATH,
  validateSurfaceInventory,
} from "./schema.ts";
import { repositoryRoot } from "./workspace.ts";

/**
 * assure-repo CLI (AR-0 #9056, AR-1 #9057).
 *
 *   generate   Regenerate docs/assure-repo/surface-inventory.v1.json.
 *   check      Regenerate in memory and byte-compare against the committed
 *              artifact; validate the no-silent-surface invariant. Exit 1 on
 *              staleness or any validation issue.
 *   summary    Print the coverage summary for the committed artifact.
 *   coverage   Print the AR-1 program-area obligation grading report.
 */

const usage = (): never => {
  process.stderr.write(
    "usage: assure-repo <generate|check|summary|coverage> [--root <dir>] [--json]\n",
  );
  process.exit(2);
};

const main = (): void => {
  const argv = process.argv.slice(2);
  const command = argv[0];
  const rootFlag = argv.indexOf("--root");
  const root =
    rootFlag >= 0 && argv[rootFlag + 1] ? resolve(argv[rootFlag + 1]!) : repositoryRoot();
  const json = argv.includes("--json");
  const path = resolve(root, SURFACE_INVENTORY_PATH);

  if (command === "generate") {
    const document = buildInventory(root);
    const validation = validateSurfaceInventory(document);
    if (!validation.ok) {
      process.stderr.write(
        `generated inventory failed validation:\n${validation.issues.map((i) => `  - ${i.message}`).join("\n")}\n`,
      );
      process.exit(1);
    }
    writeFileSync(path, serializeSurfaceInventory(document));
    process.stdout.write(
      `wrote ${SURFACE_INVENTORY_PATH} (${document.summary.totalSurfaces} surfaces, ${document.summary.unverified} unverified)\n`,
    );
    return;
  }

  if (command === "check") {
    const expected = serializeSurfaceInventory(buildInventory(root));
    const expectedValidation = validateSurfaceInventory(JSON.parse(expected));
    if (!expectedValidation.ok) {
      process.stderr.write(
        `inventory generation is not self-consistent:\n${expectedValidation.issues.map((i) => `  - ${i.message}`).join("\n")}\n`,
      );
      process.exit(1);
    }
    if (!existsSync(path)) {
      process.stderr.write(
        `${SURFACE_INVENTORY_PATH} is missing; run pnpm run generate:assure-repo\n`,
      );
      process.exit(1);
    }
    const actual = readFileSync(path, "utf8");
    const committedValidation = validateSurfaceInventory(JSON.parse(actual));
    if (!committedValidation.ok) {
      process.stderr.write(
        `committed inventory fails validation:\n${committedValidation.issues.map((i) => `  - ${i.message}`).join("\n")}\n`,
      );
      process.exit(1);
    }
    if (actual !== expected) {
      process.stderr.write(
        `${SURFACE_INVENTORY_PATH} is stale; run pnpm run generate:assure-repo\n`,
      );
      process.exit(1);
    }
    process.stdout.write(
      `assure-repo inventory OK (${committedValidation.document!.summary.totalSurfaces} surfaces, 0 silent)\n`,
    );
    return;
  }

  if (command === "summary") {
    const document = existsSync(path)
      ? JSON.parse(readFileSync(path, "utf8"))
      : buildInventory(root);
    const validation = validateSurfaceInventory(document);
    if (!validation.ok) {
      process.stderr.write(
        `inventory invalid:\n${validation.issues.map((i) => `  - ${i.message}`).join("\n")}\n`,
      );
      process.exit(1);
    }
    const summary = validation.document!.summary;
    if (json) {
      process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
      return;
    }
    process.stdout.write(`Surfaces: ${summary.totalSurfaces}\n`);
    process.stdout.write(`  with oracle: ${summary.withOracle}\n`);
    process.stdout.write(`  unverified:  ${summary.unverified}\n`);
    process.stdout.write(`By kind: ${JSON.stringify(summary.byKind)}\n`);
    process.stdout.write(`By unverified reason: ${JSON.stringify(summary.byUnverifiedReason)}\n`);
    if (Object.keys(summary.byObligationState).length > 0)
      process.stdout.write(`By obligation state: ${JSON.stringify(summary.byObligationState)}\n`);
    return;
  }

  if (command === "coverage") {
    const document = existsSync(path)
      ? JSON.parse(readFileSync(path, "utf8"))
      : buildInventory(root);
    const validation = validateSurfaceInventory(document);
    if (!validation.ok) {
      process.stderr.write(
        `inventory invalid:\n${validation.issues.map((i) => `  - ${i.message}`).join("\n")}\n`,
      );
      process.exit(1);
    }
    const report = coverageByArea(validation.document!.surfaces);
    if (json) {
      process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
      return;
    }
    process.stdout.write(
      "Obligation coverage by program area (mapped/designed/observed/accepted/inconclusive/out-of-scope):\n",
    );
    for (const area of report) {
      const s = area.byState;
      process.stdout.write(
        `  ${area.area.padEnd(26)} total=${area.total}  m=${s.mapped} d=${s.designed} o=${s.observed} a=${s.accepted} inc=${s.inconclusive} oos=${s["out-of-scope"]}\n`,
      );
    }
    process.stdout.write(
      "\nNote: `designed` means an executable oracle is authored, not observed. `observed`/`accepted` require an AR-3 sweep receipt or owner acceptance and are never set by grading.\n",
    );
    return;
  }

  usage();
};

main();
