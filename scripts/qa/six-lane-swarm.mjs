#!/usr/bin/env node

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const LANE_IDS = [
  "web_routes",
  "desktop_shell",
  "mobile",
  "api_openapi",
  "payments_promises",
  "sync",
];

export function validateRegistry(registry) {
  if (registry?.schema !== "openagents.qa.six-lane-registry.v1") {
    throw new Error("registry schema must be openagents.qa.six-lane-registry.v1");
  }
  const ids = registry.lanes?.map((lane) => lane.id) ?? [];
  if (JSON.stringify([...ids].sort()) !== JSON.stringify([...LANE_IDS].sort())) {
    throw new Error(`registry must declare each lane exactly once: ${LANE_IDS.join(", ")}`);
  }
  for (const lane of registry.lanes) {
    if (!lane.surface || !lane.command || !Array.isArray(lane.probes)) {
      throw new Error(`lane ${lane.id} requires surface, command, and probes`);
    }
  }
  return registry;
}

const sha256 = (value) => createHash("sha256").update(value).digest("hex");

function valueAtPath(value, path) {
  const parts = path.split(".");
  let current = value;
  for (let index = 0; index < parts.length; index += 1) {
    if (parts[index] === "paths" && parts[index + 1]?.startsWith("/")) {
      current = current?.[parts[index]]?.[parts[index + 1]];
      index += 1;
    } else {
      current = current?.[parts[index]];
    }
  }
  return current;
}

export function evaluateProbe(probe, response) {
  const failures = [];
  if (response.status !== probe.status)
    failures.push(`expected status ${probe.status}, got ${response.status}`);
  if (probe.contentType && !response.contentType.includes(probe.contentType)) {
    failures.push(
      `expected content-type containing ${probe.contentType}, got ${response.contentType || "<missing>"}`,
    );
  }
  if (probe.bodyIncludes && !response.body.includes(probe.bodyIncludes)) {
    failures.push(`body did not include ${JSON.stringify(probe.bodyIncludes)}`);
  }
  if (probe.jsonAssertions) {
    let parsed;
    try {
      parsed = JSON.parse(response.body);
    } catch {
      failures.push("response was not valid JSON");
    }
    if (parsed !== undefined) {
      for (const assertion of probe.jsonAssertions) {
        const actual = valueAtPath(parsed, assertion.path);
        if (assertion.present === true && actual === undefined)
          failures.push(`${assertion.path} was absent`);
        if (Object.hasOwn(assertion, "equals") && actual !== assertion.equals) {
          failures.push(
            `${assertion.path} expected ${JSON.stringify(assertion.equals)}, got ${JSON.stringify(actual)}`,
          );
        }
      }
    }
  }
  return failures;
}

async function runCommand(command, logPath, logRef) {
  const startedAt = new Date().toISOString();
  const chunks = [];
  const exitCode = await new Promise((resolveExit, reject) => {
    const child = spawn("/bin/zsh", ["-lc", command], { cwd: process.cwd(), env: process.env });
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    child.on("error", reject);
    child.on("close", resolveExit);
  });
  const output = Buffer.concat(chunks).toString("utf8");
  await writeFile(logPath, output);
  return {
    command,
    startedAt,
    completedAt: new Date().toISOString(),
    exitCode,
    outputSha256: sha256(output),
    log: logRef,
  };
}

async function runProbe(probe) {
  const startedAt = new Date().toISOString();
  try {
    const response = await fetch(probe.url, {
      method: probe.method ?? "GET",
      headers: probe.headers,
      body: probe.body,
      redirect: "manual",
      signal: AbortSignal.timeout(20_000),
    });
    const body = await response.text();
    const observed = {
      status: response.status,
      contentType: response.headers.get("content-type") ?? "",
      body,
    };
    const failures = evaluateProbe(probe, observed);
    return {
      url: probe.url,
      method: probe.method ?? "GET",
      startedAt,
      completedAt: new Date().toISOString(),
      status: response.status,
      contentType: observed.contentType,
      bodyBytes: Buffer.byteLength(body),
      bodySha256: sha256(body),
      failures,
    };
  } catch (error) {
    return {
      url: probe.url,
      method: probe.method ?? "GET",
      startedAt,
      completedAt: new Date().toISOString(),
      status: null,
      contentType: "",
      bodyBytes: 0,
      bodySha256: null,
      failures: [
        `probe infrastructure error: ${error instanceof Error ? error.message : String(error)}`,
      ],
    };
  }
}

async function runLane(lane, outputDirectory) {
  const logRef = `${lane.id}.log`;
  const logPath = resolve(outputDirectory, logRef);
  const [command, probes] = await Promise.all([
    runCommand(lane.command, logPath, logRef),
    Promise.all(lane.probes.map(runProbe)),
  ]);
  const failures = [
    ...(command.exitCode === 0 ? [] : [`command exited ${command.exitCode}`]),
    ...probes.flatMap((probe) =>
      probe.failures.map((failure) => `${probe.method} ${probe.url}: ${failure}`),
    ),
  ];
  let receipt = null;
  if (lane.receiptPrefix) {
    const output = await readFile(logPath, "utf8");
    const line = output.split("\n").find((candidate) => candidate.startsWith(lane.receiptPrefix));
    if (line) receipt = JSON.parse(line.slice(lane.receiptPrefix.length));
    else failures.push(`missing receipt line ${JSON.stringify(lane.receiptPrefix)}`);
  }
  return {
    id: lane.id,
    surface: lane.surface,
    verdict: failures.length === 0 ? "pass" : "finding",
    command,
    probes,
    receipt,
    failures,
  };
}

export async function executeSwarm({ registryPath, outputDirectory, baseSha }) {
  const registry = validateRegistry(JSON.parse(await readFile(registryPath, "utf8")));
  await mkdir(outputDirectory, { recursive: true });
  const startedAt = new Date().toISOString();
  const lanes = await Promise.all(registry.lanes.map((lane) => runLane(lane, outputDirectory)));
  const report = {
    schema: "openagents.qa.six-lane-run.v1",
    runRef: `qa.six-lane.${startedAt.replace(/[-:.]/gu, "").replace("Z", "Z")}`,
    baseSha,
    productionBaseUrl: "https://openagents.com",
    startedAt,
    completedAt: new Date().toISOString(),
    verdict: lanes.some((lane) => lane.verdict === "finding") ? "findings" : "pass",
    lanes,
  };
  await writeFile(resolve(outputDirectory, "run.json"), `${JSON.stringify(report, null, 2)}\n`);
  return report;
}

function parseArgs(argv) {
  const valueAfter = (flag) => {
    const index = argv.indexOf(flag);
    return index === -1 ? undefined : argv[index + 1];
  };
  return {
    registryPath: resolve(valueAfter("--registry") ?? "docs/qa/swarm/six-lane-registry.json"),
    outputDirectory: resolve(
      valueAfter("--out") ?? `runs/qa-six-lane/${new Date().toISOString().slice(0, 10)}`,
    ),
    baseSha: valueAfter("--base-sha") ?? process.env.QA_BASE_SHA ?? "unknown",
  };
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (import.meta.url === invokedPath) {
  const options = parseArgs(process.argv.slice(2));
  executeSwarm(options)
    .then((report) => {
      console.log(
        JSON.stringify({
          runRef: report.runRef,
          verdict: report.verdict,
          report: resolve(options.outputDirectory, "run.json"),
        }),
      );
    })
    .catch((error) => {
      console.error(error);
      process.exitCode = 2;
    });
}
