import { execFileSync } from "node:child_process";

import {
  backgroundAgentsContractRegistry,
  khalaSyncContractRegistry,
  openAgentsAppsContractRegistry,
  sarahRetiredContractRegistry,
} from "@openagentsinc/behavior-contracts";

import { compareStrings } from "./schema.ts";
import type { OracleRef } from "./schema.ts";

/**
 * Oracle enumeration and surface binding. The strongest deterministic signals
 * are (a) tracked test files under a surface path and (b) behavior contracts
 * whose `bun-test` oracle refs point into a surface path. Everything the
 * generator cannot bind becomes an explicit `unverified` reason upstream.
 */

export type FlatContract = {
  readonly contractId: string;
  readonly state: "enforced" | "pending" | "retired";
  readonly oracleRefs: ReadonlyArray<string>;
};

/**
 * Flatten every behavior-contract registry into id + state + test-path oracle
 * refs. There is no aggregate export upstream, so the four registries plus the
 * audio contract array are combined here.
 */
export const allBehaviorContracts = (): ReadonlyArray<FlatContract> => {
  const documents = [
    openAgentsAppsContractRegistry,
    sarahRetiredContractRegistry,
    backgroundAgentsContractRegistry,
    khalaSyncContractRegistry,
  ];
  const flat: FlatContract[] = [];
  const seen = new Set<string>();
  const push = (contract: {
    contractId: string;
    state: "enforced" | "pending" | "retired";
    oracles: ReadonlyArray<{ kind: string; ref: string }>;
  }): void => {
    if (seen.has(contract.contractId)) return;
    seen.add(contract.contractId);
    flat.push({
      contractId: contract.contractId,
      state: contract.state,
      oracleRefs: contract.oracles
        .filter((oracle) => oracle.kind === "bun-test" || oracle.kind === "script")
        .map((oracle) => oracle.ref)
        .sort(compareStrings),
    });
  };
  for (const document of documents) {
    // `openAgentsAppsContractRegistry` already spreads in the audio contracts.
    for (const contract of document.contracts) push(contract);
  }
  return flat.sort((a, b) => compareStrings(a.contractId, b.contractId));
};

// `*.test.*`/`*.spec.*` plus the repo's `*.node-suite.ts` / `*.suite.ts`
// `node --test`/vite-plus suite convention.
const isTestPath = (path: string): boolean =>
  /\.(test|spec)\.(ts|tsx|mts|cts|mjs|js)$/.test(path) ||
  /(?:\.node-suite|\.suite)\.(ts|mts)$/.test(path);

const withinPath = (path: string, owningPath: string): boolean =>
  path === owningPath || path.startsWith(`${owningPath}/`);

/** Tracked test files that live under a surface path. */
export const surfaceTestFiles = (
  owningPath: string,
  tracked: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  tracked.filter((path) => withinPath(path, owningPath) && isTestPath(path)).sort(compareStrings);

/** Behavior contracts whose oracle refs point into a surface path. */
export const surfaceContracts = (
  owningPath: string,
  contracts: ReadonlyArray<FlatContract>,
): ReadonlyArray<FlatContract> =>
  contracts
    .filter((contract) => contract.oracleRefs.some((ref) => withinPath(ref, owningPath)))
    .sort((a, b) => compareStrings(a.contractId, b.contractId));

/**
 * Whether a Rust crate carries in-tree tests. Deterministic `git grep` over the
 * crate path; the crate's completion evidence is the workspace `cargo test`.
 */
export const crateHasTests = (root: string, cratePath: string): boolean => {
  try {
    // Fixed-string match: the literal attribute forms, not a regex char class.
    execFileSync(
      "git",
      [
        "-C",
        root,
        "grep",
        "-l",
        "-F",
        "-e",
        "#[test]",
        "-e",
        "#[cfg(test)]",
        "--",
        `${cratePath}/`,
      ],
      {
        stdio: "pipe",
      },
    );
    return true;
  } catch {
    return false;
  }
};

/**
 * Compose the oracle refs for a code surface: one aggregated `test` oracle when
 * test files exist, plus one `behavior-contract` oracle per bound contract.
 */
export const codeSurfaceOracles = (
  owningPath: string,
  tracked: ReadonlyArray<string>,
  contracts: ReadonlyArray<FlatContract>,
): ReadonlyArray<OracleRef> => {
  const oracles: OracleRef[] = [];
  const tests = surfaceTestFiles(owningPath, tracked);
  if (tests.length > 0) {
    oracles.push({
      type: "test",
      ref: `${owningPath} (${tests.length} tracked test file${tests.length === 1 ? "" : "s"})`,
    });
  }
  for (const contract of surfaceContracts(owningPath, contracts)) {
    oracles.push({ type: "behavior-contract", ref: contract.contractId });
  }
  return oracles;
};
