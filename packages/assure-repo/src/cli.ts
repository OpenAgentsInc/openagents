#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  buildFalseGreenReport,
  FALSE_GREEN_REPORT_PATH,
  serializeFalseGreenReport,
} from "./audit.ts";
import { runDriftOracles } from "./drift.ts";
import { coverageByArea } from "./grade.ts";
import { buildInventory, loadPolicy } from "./inventory.ts";
import { runMutation } from "./mutation-runner.ts";
import { DEFAULT_RECEIPT_MAX_AGE_MS, renderReadiness } from "./readiness.ts";
import { decodeSweepReceipt, headCommit, runSweep, serializeSweepReceipt } from "./sweep.ts";
import {
  serializeSurfaceInventory,
  SURFACE_INVENTORY_PATH,
  validateSurfaceInventory,
} from "./schema.ts";
import { repositoryRoot } from "./workspace.ts";

/**
 * assure-repo CLI (AR-0 #9056, AR-1 #9057, AR-2 #9058).
 *
 *   generate        Regenerate docs/assure-repo/surface-inventory.v1.json.
 *   check           Regenerate in memory and byte-compare against the committed
 *                   artifact; validate the no-silent-surface invariant.
 *   summary         Print the inventory coverage summary.
 *   coverage        Print the AR-1 program-area obligation grading report.
 *   audit-generate  Regenerate the AR-2 false-green candidate report.
 *   audit-check     Byte-compare the committed candidate report.
 *   audit           Print the false-green candidate summary.
 *   demonstrate     Run one mutation against a subject/test to prove a
 *                   kill (exit 0) or a surviving weak oracle (exit 1).
 *   drift           Print AR-4 drift-oracle findings over governed documents.
 *   drift-check     Fail (exit 1) on any open (un-dispositioned) broken claim.
 *   sweep           AR-3: re-run the oracles and emit a receipt (--out <path>).
 *   readiness       AR-3: render repo-verification readiness from a receipt
 *                   (--receipt <path>); no/stale receipt renders unknown.
 */

const usage = (): never => {
  process.stderr.write(
    "usage: assure-repo <generate|check|summary|coverage|audit|audit-generate|audit-check|demonstrate|drift|drift-check|sweep|readiness> [...]\n",
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

  if (command === "audit-generate") {
    const report = buildFalseGreenReport(root);
    writeFileSync(resolve(root, FALSE_GREEN_REPORT_PATH), serializeFalseGreenReport(report));
    process.stdout.write(
      `wrote ${FALSE_GREEN_REPORT_PATH} (${report.summary.filesScanned} test files, ${report.summary.candidateCount} candidate leads)\n`,
    );
    return;
  }

  if (command === "audit-check") {
    const expected = serializeFalseGreenReport(buildFalseGreenReport(root));
    const target = resolve(root, FALSE_GREEN_REPORT_PATH);
    if (!existsSync(target)) {
      process.stderr.write(
        `${FALSE_GREEN_REPORT_PATH} is missing; run pnpm run audit:assure-repo\n`,
      );
      process.exit(1);
    }
    if (readFileSync(target, "utf8") !== expected) {
      process.stderr.write(`${FALSE_GREEN_REPORT_PATH} is stale; run pnpm run audit:assure-repo\n`);
      process.exit(1);
    }
    const report = JSON.parse(expected) as {
      summary: { filesScanned: number; candidateCount: number };
    };
    process.stdout.write(
      `false-green candidate report OK (${report.summary.filesScanned} files, ${report.summary.candidateCount} leads)\n`,
    );
    return;
  }

  if (command === "audit") {
    const report = buildFalseGreenReport(root);
    if (json) {
      process.stdout.write(`${JSON.stringify(report.summary, null, 2)}\n`);
      return;
    }
    process.stdout.write(`Scanned ${report.summary.filesScanned} test files.\n`);
    process.stdout.write(
      `Candidate false-green LEADS (heuristic, not findings): ${report.summary.candidateCount}\n`,
    );
    process.stdout.write(`By mode: ${JSON.stringify(report.summary.byMode)}\n`);
    process.stdout.write(
      "\nA lead becomes a finding only when demonstrated by a surviving mutation (`assure-repo demonstrate`).\n",
    );
    return;
  }

  if (command === "demonstrate") {
    const at = (flag: string): string | undefined => {
      const idx = argv.indexOf(flag);
      return idx >= 0 ? argv[idx + 1] : undefined;
    };
    const subjectPath = at("--subject");
    const target = at("--target");
    const replacement = at("--replacement") ?? "";
    const testIdx = argv.indexOf("--test");
    const testCommand = testIdx >= 0 ? argv.slice(testIdx + 1) : [];
    if (!subjectPath || target === undefined || testCommand.length === 0) {
      process.stderr.write(
        "usage: assure-repo demonstrate --subject <path> --target <str> --replacement <str> --test <cmd...>\n",
      );
      process.exit(2);
    }
    const outcome = runMutation(root, { subjectPath, target, replacement, testCommand });
    process.stdout.write(`${JSON.stringify(outcome, null, 2)}\n`);
    process.exit(outcome.result === "survived" ? 1 : 0);
  }

  if (command === "drift" || command === "drift-check") {
    const policy = loadPolicy(root);
    const governed = [
      ...policy.governedDocuments,
      "docs/assure-repo/README.md",
      "packages/assure-repo/README.md",
    ];
    const report = runDriftOracles(root, governed, policy.driftDispositions);
    if (json) {
      process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    } else {
      process.stdout.write(
        `Drift oracles over ${report.summary.documentsChecked} governed documents: ${report.summary.broken} broken (${report.summary.dispositioned} dispositioned, ${report.summary.brokenUndispositioned} open), ${report.summary.unverifiable} unverifiable.\n`,
      );
      for (const finding of report.findings.filter((f) => f.verdict === "broken")) {
        const disp = policy.driftDispositions[`${finding.file}:${finding.claim}`];
        process.stdout.write(
          `  ${disp ? "DISPOSITIONED" : "BROKEN"} ${finding.file}:${finding.line} [${finding.kind}] ${finding.claim} — ${disp ?? finding.detail}\n`,
        );
      }
    }
    if (command === "drift-check" && report.summary.brokenUndispositioned > 0) {
      process.stderr.write(
        `\n${report.summary.brokenUndispositioned} open broken documented claim(s); fix the docs, the referenced target, or add a policy disposition.\n`,
      );
      process.exit(1);
    }
    return;
  }

  if (command === "sweep") {
    const outIdx = argv.indexOf("--out");
    const receipt = runSweep(root, new Date().toISOString());
    const serialized = serializeSweepReceipt(receipt);
    if (outIdx >= 0 && argv[outIdx + 1]) {
      writeFileSync(resolve(argv[outIdx + 1]!), serialized);
      process.stdout.write(
        `wrote sweep receipt to ${argv[outIdx + 1]} (overall=${receipt.overall}, evidence=${receipt.evidenceClass})\n`,
      );
    } else if (json) {
      process.stdout.write(serialized);
    } else {
      process.stdout.write(
        `Sweep overall=${receipt.overall} (evidence: ${receipt.evidenceClass}, commit ${receipt.commit.slice(0, 10)})\n`,
      );
      for (const outcome of receipt.oracleOutcomes) {
        process.stdout.write(
          `  ${outcome.outcome.toUpperCase().padEnd(5)} ${outcome.oracle} — ${outcome.detail}\n`,
        );
      }
    }
    process.exit(receipt.overall === "red" ? 1 : 0);
  }

  if (command === "readiness") {
    const receiptIdx = argv.indexOf("--receipt");
    let receipt;
    if (receiptIdx >= 0 && argv[receiptIdx + 1]) {
      const receiptPath = resolve(argv[receiptIdx + 1]!);
      if (existsSync(receiptPath)) {
        try {
          receipt = decodeSweepReceipt(JSON.parse(readFileSync(receiptPath, "utf8")));
        } catch (error) {
          process.stderr.write(
            `invalid sweep receipt: ${error instanceof Error ? error.message : String(error)}\n`,
          );
          process.exit(1);
        }
      }
    }
    const readiness = renderReadiness(
      receipt,
      Date.now(),
      DEFAULT_RECEIPT_MAX_AGE_MS,
      headCommit(root),
    );
    if (json) {
      process.stdout.write(`${JSON.stringify(readiness, null, 2)}\n`);
      return;
    }
    process.stdout.write(
      `Repo verification readiness: ${readiness.state.toUpperCase()} — ${readiness.reason}\n`,
    );
    return;
  }

  usage();
};

main();
