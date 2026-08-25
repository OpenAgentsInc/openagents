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

/** Subcommand names from `--help`, which lists them one per line after SUBCOMMANDS. */
const subcommands = (help) => {
  const section = help.split("SUBCOMMANDS")[1];
  if (section === undefined) return [];
  return [...section.matchAll(/^\s{2}([a-z][a-z-]*)\s{2,}/gm)].map((match) => match[1]).sort();
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

  // A workspace specifier that survives packing makes the published package
  // uninstallable: `npm install` cannot resolve `catalog:` or `workspace:` and
  // fails with EUNSUPPORTEDPROTOCOL, which names the protocol and not the
  // mistake. `pnpm pack` rewrites them and `npm pack` does not, so this is
  // exactly what a publish packed the wrong way looks like. Read from the
  // packed manifest rather than the source one, because the rewrite is the
  // thing under test.
  const packedManifest = JSON.parse(
    run("tar", ["-xOf", tarball, "package/package.json"]).stdout,
  );

  const unresolved = ["dependencies", "peerDependencies", "optionalDependencies"].flatMap(
    (field) =>
      Object.entries(packedManifest[field] ?? {})
        .filter(([, range]) => /^(catalog:|workspace:)/.test(String(range)))
        .map(([name, range]) => `${field}.${name}: ${String(range)}`),
  );

  if (unresolved.length > 0) {
    throw new Error(
      "The packed manifest carries workspace specifiers npm cannot resolve, so " +
        "the published package would not install:\n  " +
        unresolved.join("\n  ") +
        "\nPack and publish with pnpm, never npm: `pnpm publish` rewrites these " +
        "to concrete versions and `npm pack` leaves them.",
    );
  }

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

  // The published package once carried a `forum` command that existed in no
  // source file, because nothing compared what shipped against what a clean
  // build produces. Comparing the two command surfaces is that check.
  const packedSubcommands = subcommands(help);
  const localHelp = run("node", [join(packageRoot, "dist", "main.js"), "--help"]).stdout;
  const localSubcommands = subcommands(localHelp);

  const onlyPacked = packedSubcommands.filter((name) => !localSubcommands.includes(name));
  const onlyLocal = localSubcommands.filter((name) => !packedSubcommands.includes(name));
  if (onlyPacked.length > 0 || onlyLocal.length > 0) {
    throw new Error(
      "The packed CLI and a clean build disagree about their commands. " +
        `Only in the tarball: ${onlyPacked.join(", ") || "none"}. ` +
        `Only in the build: ${onlyLocal.join(", ") || "none"}.`,
    );
  }

  process.stdout.write(
    `Packed npm install, CLI entry point, and command surface passed (${packedSubcommands.join(", ")}).\n`,
  );
} finally {
  rmSync(temporaryRoot, { recursive: true, force: true });
}
