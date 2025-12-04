#!/usr/bin/env bun
import { checkTasksIntegrity } from "../src/tasks/integrity.js";

interface CliOptions {
  json: boolean;
  fix: boolean;
  rootDir: string;
}

const parseArgs = (argv: string[]): CliOptions => {
  const opts: CliOptions = {
    json: false,
    fix: false,
    rootDir: process.cwd(),
  };

  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    switch (arg) {
      case "--json":
        opts.json = true;
        break;
      case "--fix":
        opts.fix = true;
        break;
      case "--dir":
      case "--root":
        if (next) {
          opts.rootDir = next;
          i++;
        }
        break;
    }
  }

  return opts;
};

const formatIssues = (label: string, items: { message: string; hint?: string }[]): string =>
  items
    .map(
      (item) =>
        `- ${label}: ${item.message}${item.hint ? `\n    hint: ${item.hint}` : ""}`,
    )
    .join("\n");

const main = async () => {
  const opts = parseArgs(process.argv);
  const result = await checkTasksIntegrity({
    rootDir: opts.rootDir,
    fix: opts.fix,
  });

  if (opts.json) {
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.ok ? 0 : 1);
  }

  if (result.issues.length === 0 && result.warnings.length === 0) {
    console.log("tasks.jsonl integrity OK");
  }

  if (result.issues.length > 0) {
    console.error(
      formatIssues("issue", result.issues),
    );
  }

  if (result.warnings.length > 0) {
    console.warn(
      formatIssues("warning", result.warnings),
    );
  }

  process.exit(result.ok ? 0 : 1);
};

main().catch((error) => {
  console.error(`tasks:integrity failed: ${(error as Error).message}`);
  process.exit(1);
});
