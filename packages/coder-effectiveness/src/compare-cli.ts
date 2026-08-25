/**
 * `coder-effectiveness compare` — read a bench-results store and say what its
 * rows do and do not prove about each other.
 *
 *     pnpm run effectiveness:compare -- bench-results/tb2-cross-section.jsonl
 *
 * The chain is verified before anything is compared, and a store that does not
 * verify is refused rather than compared. A trend line computed over a history
 * that has been rewritten is worse than no trend line, because it reads exactly
 * like one that has not.
 *
 * EXIT CODES. 0 the store verified and was compared, 1 the store could not be
 * read, 2 the store does not verify. The second and third are different
 * findings: a missing file is an operator typo, a broken chain is evidence.
 */

import { compareRuns } from "./compare.ts";
import { renderComparison } from "./render-compare.ts";
import { readResultRows, verifyResultChain } from "./results-store.ts";

const USAGE = `Usage: coder-effectiveness compare <store> [options]

Arguments:
  <store>                An append-only bench-results JSONL store, the file
                         \`effectiveness:report --append\` writes.

Options:
  --suite <name>         Compare only rows recorded under this suite name.
  --baseline-lane <lane> Lane every other lane is measured against.
                         Default: proxy, or the first lane in the group.
  --json                 Emit the comparison as JSON instead of text.
  -h, --help             Show this help.

Exit codes: 0 compared, 1 the store could not be read, 2 the store does not
verify. A store that does not verify is not compared: its history was rewritten
after it was written, and a trend over a rewritten history reads exactly like a
trend over an honest one.`;

interface Arguments {
  readonly storePath: string;
  readonly suite: string | null;
  readonly baselineLane: string | null;
  readonly json: boolean;
}

const parseArguments = (argv: ReadonlyArray<string>): Arguments | "help" => {
  let storePath: string | null = null;
  let suite: string | null = null;
  let baselineLane: string | null = null;
  let json = false;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]!;
    if (argument === "--") continue;
    if (argument === "-h" || argument === "--help") return "help";
    if (argument === "--json") {
      json = true;
      continue;
    }
    if (argument === "--suite") {
      suite = expectValue(argv, (index += 1), "--suite");
      continue;
    }
    if (argument === "--baseline-lane") {
      baselineLane = expectValue(argv, (index += 1), "--baseline-lane");
      continue;
    }
    if (argument.startsWith("-")) throw new Error(`unknown option: ${argument}`);
    if (storePath !== null) throw new Error(`unexpected extra argument: ${argument}`);
    storePath = argument;
  }

  if (storePath === null) throw new Error("missing required <store> argument");
  return { storePath, suite, baselineLane, json };
};

const expectValue = (argv: ReadonlyArray<string>, index: number, option: string): string => {
  const value = argv[index];
  if (value === undefined || value.startsWith("-")) throw new Error(`${option} needs a value`);
  return value;
};

const main = (argv: ReadonlyArray<string>): number => {
  let parsed: Arguments | "help";
  try {
    parsed = parseArguments(argv);
  } catch (error) {
    process.stderr.write(`${(error as Error).message}\n\n${USAGE}\n`);
    return 1;
  }
  if (parsed === "help") {
    process.stdout.write(`${USAGE}\n`);
    return 0;
  }

  let rows;
  try {
    rows = readResultRows(parsed.storePath);
  } catch (error) {
    process.stderr.write(`coder-effectiveness: ${(error as Error).message}\n`);
    return 1;
  }

  const verdict = verifyResultChain(rows);
  if (!verdict.ok) {
    process.stderr.write(
      `coder-effectiveness: ${parsed.storePath} does not verify (${verdict.break.kind}).\n  ${verdict.break.detail}\nNothing was compared.\n`,
    );
    return 2;
  }

  const comparison = compareRuns(rows, {
    ...(parsed.suite === null ? {} : { suite: parsed.suite }),
    ...(parsed.baselineLane === null ? {} : { baselineLane: parsed.baselineLane }),
  });

  process.stdout.write(
    parsed.json
      ? `${JSON.stringify({ verified: verdict, comparison }, null, 2)}\n`
      : renderComparison(comparison),
  );
  return 0;
};

process.exitCode = main(process.argv.slice(2));
