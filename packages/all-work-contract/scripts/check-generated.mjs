import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";

const packageRoot = resolve(import.meta.dirname, "..");
const definition = JSON.parse(
  await readFile(resolve(packageRoot, "definition/all-work-v1.contract.json"), "utf8"),
);
const generatedRoot = await mkdtemp(join(tmpdir(), "openagents-all-work-contract-"));

const walk = async (root, current = root) => {
  const entries = await readdir(current, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(current, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(root, path)));
    else files.push(relative(root, path));
  }
  return files.sort();
};

try {
  execFileSync(
    process.execPath,
    [resolve(packageRoot, "scripts/generate.mjs"), "--output-root", generatedRoot],
    { stdio: "inherit" },
  );

  const fixtureFiles = definition.fixtures.map((fixture) => `fixtures/${fixture.file}`);
  const canonicalFiles = definition.fixtures
    .filter((fixture) => fixture.canonical === true)
    .map(
      (fixture) =>
        `generated/canonical/${fixture.file
          .split("/")
          .at(-1)
          .replace(/\.json$/u, ".canonical.json")}`,
    );
  const expectedFiles = [
    "src/generated.ts",
    "generated/rust/all_work_v1.rs",
    "generated/json-schema/all-work-v1.schema.json",
    "generated/fixture-index.json",
    "generated/compatibility.json",
    ...fixtureFiles,
    ...canonicalFiles,
  ].sort();

  const actualGeneratedFiles = [
    ...(await walk(resolve(packageRoot, "generated"))).map((path) => `generated/${path}`),
    ...(await walk(resolve(packageRoot, "fixtures"))).map((path) => `fixtures/${path}`),
    "src/generated.ts",
  ].sort();

  if (JSON.stringify(actualGeneratedFiles) !== JSON.stringify(expectedFiles)) {
    throw new Error(
      `generated file inventory drift\nexpected=${expectedFiles.join(",")}\nactual=${actualGeneratedFiles.join(",")}`,
    );
  }

  for (const file of expectedFiles) {
    const [committed, regenerated] = await Promise.all([
      readFile(resolve(packageRoot, file)),
      readFile(resolve(generatedRoot, file)),
    ]);
    if (!committed.equals(regenerated)) throw new Error(`generated artifact drift: ${file}`);
  }
} finally {
  await rm(generatedRoot, { recursive: true, force: true });
}
