import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { formatSteJson, isGovernedPath, readCheckerConfig } from "./ste-core";

interface ProtectedRecord {
  readonly sourceSha256: string;
  readonly normativeKeywords: readonly string[];
  readonly inlineCode: readonly string[];
  readonly urls: readonly string[];
  readonly issueRefs: readonly string[];
  readonly numericValues: readonly string[];
}

interface SemanticBaseline {
  readonly schema: "openagents-ste-semantic-baseline-v1";
  readonly note: string;
  readonly files: Readonly<Record<string, ProtectedRecord>>;
}

const root = resolve(import.meta.dirname, "..");
const baselinePath = `${root}/docs/ste/control-semantic-baseline.v1.json`;
const config = readCheckerConfig(root);
const paths = config.controlPaths.filter((path) => isGovernedPath(path, config)).toSorted();
const capturePaths = process.argv
  .filter((argument) => argument.startsWith("--capture-path="))
  .map((argument) => argument.slice("--capture-path=".length));

const collect = (text: string): ProtectedRecord => ({
  sourceSha256: createHash("sha256").update(text).digest("hex"),
  normativeKeywords: [
    ...text.matchAll(/\b(?:MUST|MUST NOT|SHOULD|SHOULD NOT|MAY|NEVER|ONLY|REQUIRED|PROHIBITED)\b/g),
  ]
    .map((match) => match[0])
    .toSorted(),
  inlineCode: [...text.matchAll(/`([^`\n]+)`/g)].map((match) => match[1]!).toSorted(),
  urls: [...text.matchAll(/https?:\/\/[^\s)>]+/g)].map((match) => match[0]).toSorted(),
  issueRefs: [...text.matchAll(/(?<![A-Za-z0-9])#[0-9]+\b/g)].map((match) => match[0]).toSorted(),
  numericValues: [
    ...text.matchAll(/(?<![A-Za-z])\b[0-9]+(?:\.[0-9]+)*(?:-[0-9]+(?:\.[0-9]+)*)?\b/g),
  ]
    .map((match) => match[0])
    .toSorted(),
});

if (process.argv.includes("--capture") || capturePaths.length > 0) {
  const unknownPaths = capturePaths.filter((path) => !(paths as readonly string[]).includes(path));
  if (unknownPaths.length > 0) {
    throw new Error(`Unknown STE semantic capture path: ${unknownPaths.join(", ")}`);
  }
  const selectedPaths = capturePaths.length > 0 ? capturePaths : paths;
  const priorFiles =
    capturePaths.length > 0
      ? (JSON.parse(readFileSync(baselinePath, "utf8")) as SemanticBaseline).files
      : {};
  const files = {
    ...priorFiles,
    ...Object.fromEntries(
      selectedPaths.map((path) => [path, collect(readFileSync(`${root}/${path}`, "utf8"))]),
    ),
  };
  const output: SemanticBaseline = {
    schema: "openagents-ste-semantic-baseline-v1",
    note: "These token sets protect public control-document conversions. A passing comparison does not prove equal meaning.",
    files,
  };
  writeFileSync(baselinePath, formatSteJson(output));
  console.log(
    `captured ${selectedPaths.length} public control files in docs/ste/control-semantic-baseline.v1.json`,
  );
  process.exit(0);
}

const baseline = JSON.parse(readFileSync(baselinePath, "utf8")) as SemanticBaseline;
const errors: string[] = [];
for (const path of paths) {
  const expected = baseline.files[path];
  if (!expected) {
    errors.push(`${path}: semantic baseline is absent`);
    continue;
  }
  const current = collect(readFileSync(`${root}/${path}`, "utf8"));
  for (const key of [
    "normativeKeywords",
    "inlineCode",
    "urls",
    "issueRefs",
    "numericValues",
  ] as const) {
    if (JSON.stringify(current[key]) !== JSON.stringify(expected[key]))
      errors.push(`${path}: protected ${key} changed`);
  }
}
if (errors.length > 0) {
  for (const error of errors) console.error(error);
  process.exitCode = 1;
} else {
  console.log(`check:ste-semantic OK (${paths.length} public control files)`);
}
