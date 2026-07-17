import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const packageRoot = resolve(import.meta.dirname, "..");
const temporary = mkdtempSync(join(tmpdir(), "fast-follow-spec-pack-"));
const run = (command: string, args: string[], cwd: string) => {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", stdio: "pipe" });
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")} failed\n${result.stdout}\n${result.stderr}`);
  return result.stdout.trim();
};
try {
  const output = run("pnpm", ["pack", "--pack-destination", temporary], packageRoot);
  const tarball = output.split(/\r?\n/).at(-1)!;
  const listing = run("tar", ["-tzf", tarball], packageRoot);
  for (const required of [
    "package/package.json",
    "package/src/index.ts",
    "package/fixtures/conformance/0.1/valid/minimal.md",
  ]) {
    if (!listing.split(/\r?\n/).includes(required))
      throw new Error(`packed package is missing ${required}`);
  }
  const extracted = join(temporary, "extracted");
  mkdirSync(extracted);
  run("tar", ["-xzf", tarball, "-C", extracted], packageRoot);
  const manifest = JSON.parse(readFileSync(join(extracted, "package/package.json"), "utf8")) as {
    name?: string;
    private?: boolean;
  };
  if (manifest.name !== "@openagentsinc/fast-follow-spec" || manifest.private === true)
    throw new Error("distribution manifest is not publicly consumable");
  const consumer = join(temporary, "consumer");
  mkdirSync(consumer);
  writeFileSync(
    join(consumer, "package.json"),
    JSON.stringify({
      private: true,
      type: "module",
      dependencies: { "@openagentsinc/fast-follow-spec": `file:${tarball}` },
      devDependencies: { tsx: "4.20.6" },
    }),
  );
  run("pnpm", ["install", "--offline", "--ignore-scripts"], consumer);
  writeFileSync(
    join(consumer, "verify.mjs"),
    'import { parseFastFollow, starterFastFollow } from "@openagentsinc/fast-follow-spec";\nif (!parseFastFollow(starterFastFollow("Consumer", "consumer.fast_follow")).valid) process.exit(1);\n',
  );
  run("node", ["--import", "tsx", "verify.mjs"], consumer);
  console.log("ok packed public artifact and clean installed consumer");
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
