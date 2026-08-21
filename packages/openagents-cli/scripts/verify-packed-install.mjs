import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const temporaryRoot = mkdtempSync(join(tmpdir(), "openagents-cli-package-"));
const tarballDirectory = join(temporaryRoot, "tarball");
const consumerDirectory = join(temporaryRoot, "consumer");
const packageManifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));

const run = (command, arguments_, options = {}) => {
  const result = spawnSync(command, arguments_, {
    cwd: options.cwd ?? packageRoot,
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });

  if (result.status !== 0) {
    process.stderr.write(result.stdout);
    process.stderr.write(result.stderr);
    throw new Error(`${command} exited with status ${String(result.status)}`);
  }

  return { stdout: result.stdout, stderr: result.stderr };
};

try {
  mkdirSync(tarballDirectory);
  mkdirSync(consumerDirectory);
  writeFileSync(
    join(consumerDirectory, "package.json"),
    `${JSON.stringify({ private: true, type: "module" }, null, 2)}\n`,
  );

  run("pnpm", ["pack", "--pack-destination", tarballDirectory]);
  const tarballs = readdirSync(tarballDirectory).filter((entry) => entry.endsWith(".tgz"));
  if (tarballs.length !== 1) {
    throw new Error(`Expected one packed tarball, found ${String(tarballs.length)}`);
  }

  const tarball = join(tarballDirectory, tarballs[0]);
  run("npm", ["install", tarball, "--no-audit", "--no-fund", "--no-package-lock", "--save-exact"], {
    cwd: consumerDirectory,
  });
  run("npm", ["ls", "--all"], { cwd: consumerDirectory });

  const executable = join(consumerDirectory, "node_modules", ".bin", "openagents");
  const version = run(executable, ["--version"], { cwd: consumerDirectory }).stdout.trim();
  const expectedVersion = `openagents v${String(packageManifest.version)}`;
  if (version !== expectedVersion) {
    throw new Error(`Expected CLI version ${expectedVersion}, received ${JSON.stringify(version)}`);
  }

  const help = run(executable, ["--help"], { cwd: consumerDirectory }).stdout;
  if (!help.includes("Manage OpenAgents repositories") || !help.includes("repo")) {
    throw new Error("The packed CLI help output is incomplete");
  }

  const importHelp = run(executable, ["repo", "import", "--help"], {
    cwd: consumerDirectory,
  }).stdout;
  if (!importHelp.includes("--wait-timeout") || !importHelp.includes("--namespace")) {
    throw new Error("The packed repository import help output is incomplete");
  }

  process.stdout.write("Packed npm install and CLI entry point passed.\n");
} finally {
  rmSync(temporaryRoot, { recursive: true, force: true });
}
