#!/usr/bin/env node
/**
 * SARAH-NR-03 load-proof CLI.
 *
 * Local (default): start startTestRelay or mock, measure, print JSON, exit 0/1.
 *
 *   pnpm --dir packages/sarah run load-proof
 *   node --import tsx packages/sarah/scripts/relay-load-proof.ts
 *
 * Remote:
 *
 *   RELAY_URL=wss://<owned-relay-host> pnpm --dir packages/sarah run load-proof
 *
 * Force mock host:
 *
 *   LOAD_PROOF_MOCK=1 pnpm --dir packages/sarah run load-proof
 */
import { writeFileSync } from "node:fs";
import {
  runLoadProof,
  startLocalLoadProofHost,
  type LoadProofReport,
} from "../src/relay-load-proof/index.js";

const parseNumber = (value: string | undefined, fallback: number): number => {
  if (!value) return fallback;
  const n = Number(value);
  return Number.isFinite(n) && n > 0 ? n : fallback;
};

const main = async (): Promise<number> => {
  const remoteUrl = process.env.RELAY_URL?.trim();
  const preferMock = process.env.LOAD_PROOF_MOCK === "1";
  const durationMs = parseNumber(process.env.LOAD_PROOF_DURATION_MS, 5_000);
  const publishers = parseNumber(process.env.LOAD_PROOF_PUBLISHERS, 4);
  const subscribers = parseNumber(process.env.LOAD_PROOF_SUBSCRIBERS, 2);
  const outPath = process.env.LOAD_PROOF_OUT?.trim();

  let hostStop: (() => Promise<void>) | null = null;
  let report: LoadProofReport;

  try {
    if (remoteUrl) {
      report = await runLoadProof({
        relayUrl: remoteUrl,
        hostMode: "remote",
        remote: true,
        config: { durationMs, publishers, subscribers },
      });
    } else {
      const host = await startLocalLoadProofHost({ preferMock });
      hostStop = host.stop;
      report = await runLoadProof({
        relayUrl: host.relayUrl,
        hostMode: host.mode,
        nostrEffectPin: host.nostrEffectPin,
        remote: false,
        config: { durationMs, publishers, subscribers },
      });
    }
  } finally {
    if (hostStop) {
      try {
        await hostStop();
      } catch {
        /* best-effort stop */
      }
    }
  }

  const json = JSON.stringify(report, null, 2);
  console.log(json);
  if (outPath) {
    writeFileSync(outPath, `${json}\n`, "utf8");
  }

  if (!report.pass) {
    console.error("SARAH-NR-03 load proof FAILED:");
    for (const failure of report.failures) {
      console.error(`  - ${failure}`);
    }
    return 1;
  }

  console.error(
    `SARAH-NR-03 load proof PASS publish_rps=${report.publish.rps.toFixed(1)} subscribe_rps=${report.subscribe.rps.toFixed(1)} mode=${report.hostMode}`,
  );
  return 0;
};

main()
  .then((code) => {
    process.exit(code);
  })
  .catch((error) => {
    console.error(
      "SARAH-NR-03 load proof crashed:",
      error instanceof Error ? error.stack ?? error.message : error,
    );
    process.exit(2);
  });
