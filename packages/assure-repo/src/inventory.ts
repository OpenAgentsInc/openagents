import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import {
  compareStrings,
  summarize,
  SURFACE_INVENTORY_FORMAT_VERSION,
  SURFACE_POLICY_PATH,
  type OracleRef,
  type SurfaceInventoryDocument,
  type SurfaceKind,
  type SurfaceRow,
  type UnverifiedReason,
} from "./schema.ts";
import {
  allBehaviorContracts,
  codeSurfaceOracles,
  crateHasTests,
  type FlatContract,
} from "./oracles.ts";
import { cargoCrates, pnpmPackages, releasePipelines, trackedFiles } from "./workspace.ts";

/**
 * Surface policy: the non-derivable classifications. Keeps the generator
 * deterministic while letting a human/owner state why a surface is unverified
 * or out of scope, instead of the generator guessing.
 */
export type SurfacePolicy = {
  readonly schemaVersion: 1;
  /** Surface id -> explicit unverified reason (overrides the default). */
  readonly unverifiedReasonOverrides: Record<string, UnverifiedReason>;
  /** Curated governed-document paths that AR-4 drift oracles will own. */
  readonly governedDocuments: ReadonlyArray<string>;
  /** Surface id -> disposition note for intentionally out-of-scope surfaces. */
  readonly outOfScope: Record<string, string>;
};

export const emptyPolicy: SurfacePolicy = {
  schemaVersion: 1,
  unverifiedReasonOverrides: {},
  governedDocuments: [],
  outOfScope: {},
};

export const loadPolicy = (root: string): SurfacePolicy => {
  const path = join(root, SURFACE_POLICY_PATH);
  if (!existsSync(path)) return emptyPolicy;
  const json = JSON.parse(readFileSync(path, "utf8")) as Partial<SurfacePolicy>;
  return {
    schemaVersion: 1,
    unverifiedReasonOverrides: json.unverifiedReasonOverrides ?? {},
    governedDocuments: (json.governedDocuments ?? []).slice().sort(compareStrings),
    outOfScope: json.outOfScope ?? {},
  };
};

/** Classify a pnpm package path into a surface kind. */
const classifyPackage = (path: string): SurfaceKind => {
  if (/^apps\/openagents\.com\/workers\//.test(path)) return "worker";
  if (/^apps\/[^/]+$/.test(path) || path === "apps/openagents.com") return "app";
  return "package";
};

const makeUnverified = (
  id: string,
  policy: SurfacePolicy,
  fallback: UnverifiedReason,
  fallbackNote: string,
): { reason: UnverifiedReason; note: string } => {
  const override = policy.unverifiedReasonOverrides[id];
  if (override) return { reason: override, note: `policy: ${override}` };
  return { reason: fallback, note: fallbackNote };
};

/** The public-API contract surfaces derived from known machine-readable modules. */
const PUBLIC_ENDPOINT_SURFACES: ReadonlyArray<{
  id: string;
  path: string;
  title: string;
  test: string;
}> = [
  {
    id: "public-endpoint:exact-route-registry",
    path: "apps/openagents.com/workers/api/src/index.ts",
    title: "Cloud Run monolith exact route registry",
    test: "apps/openagents.com/workers/api/src/worker-exact-routes.test.ts",
  },
  {
    id: "public-endpoint:openapi",
    path: "apps/openagents.com/workers/api/src/openagents-openapi-routes.ts",
    title: "Public OpenAPI manifest and routes",
    test: "apps/openagents.com/workers/api/src/openagents-openapi-routes.test.ts",
  },
];

const productSpecPaths = (tracked: ReadonlyArray<string>): ReadonlyArray<string> =>
  tracked.filter((path) => path.endsWith(".product-spec.md")).sort(compareStrings);

/** Build the full surface list (unsorted here; caller sorts and summarizes). */
const buildSurfaces = (
  root: string,
  tracked: ReadonlyArray<string>,
  contracts: ReadonlyArray<FlatContract>,
  policy: SurfacePolicy,
): ReadonlyArray<SurfaceRow> => {
  const surfaces: SurfaceRow[] = [];

  // 1. pnpm workspace packages / apps / workers.
  for (const pkg of pnpmPackages(root, tracked)) {
    const kind = classifyPackage(pkg.path);
    const id = `${kind}:${pkg.name}`;
    const oracles = codeSurfaceOracles(pkg.path, tracked, contracts);
    const row: SurfaceRow = {
      id,
      kind,
      owningPath: pkg.path,
      title: pkg.name,
      derivation: "derived",
      oracles,
      ...(oracles.length === 0
        ? {
            unverified: makeUnverified(
              id,
              policy,
              "no-oracle-authored",
              "no tracked test files and no bound behavior contract",
            ),
          }
        : {}),
    };
    surfaces.push(row);
  }

  // 2. Rust crates (completion evidence: workspace cargo test).
  for (const crate of cargoCrates(root)) {
    const id = `crate:${crate.name}`;
    const oracles = crateHasTests(root, crate.path)
      ? [{ type: "test" as const, ref: `${crate.path} (cargo test --workspace)` }]
      : [];
    surfaces.push({
      id,
      kind: "crate",
      owningPath: crate.path,
      title: crate.name,
      derivation: "derived",
      oracles,
      ...(oracles.length === 0
        ? {
            unverified: makeUnverified(
              id,
              policy,
              "no-oracle-authored",
              "no in-tree Rust tests found",
            ),
          }
        : {}),
    });
  }

  // 3. Public API contract surfaces.
  for (const endpoint of PUBLIC_ENDPOINT_SURFACES) {
    if (!existsSync(join(root, endpoint.path))) continue;
    const hasTest = tracked.includes(endpoint.test);
    const oracles = hasTest ? [{ type: "test" as const, ref: endpoint.test }] : [];
    surfaces.push({
      id: endpoint.id,
      kind: "public-endpoint",
      owningPath: endpoint.path,
      title: endpoint.title,
      derivation: "derived",
      oracles,
      ...(oracles.length === 0
        ? {
            unverified: makeUnverified(
              endpoint.id,
              policy,
              "no-oracle-authored",
              "no exact-route/openapi test found",
            ),
          }
        : {}),
    });
  }

  // 4. Release pipelines.
  for (const pipeline of releasePipelines(root)) {
    const id = `release-pipeline:${pipeline.name}`;
    surfaces.push({
      id,
      kind: "release-pipeline",
      owningPath: "package.json",
      title: `pnpm run ${pipeline.name}`,
      derivation: "derived",
      oracles: [],
      unverified: makeUnverified(
        id,
        policy,
        "no-oracle-authored",
        "release pipeline has no standing acceptance oracle yet (DIST chain / AR-3 sweep)",
      ),
    });
  }

  // 5. Product specs (validated by the product-spec sweep; assurance companion when present).
  for (const specPath of productSpecPaths(tracked)) {
    const id = `document:${specPath}`;
    const companion = specPath.replace(/\.product-spec\.md$/, ".assurance-spec.md");
    const oracles: OracleRef[] = [{ type: "product-spec", ref: "pnpm run test:product-spec" }];
    if (tracked.includes(companion)) oracles.push({ type: "assurance-obligation", ref: companion });
    surfaces.push({
      id,
      kind: "document",
      owningPath: specPath,
      title: specPath.split("/").pop() ?? specPath,
      derivation: "derived",
      oracles,
    });
  }

  // 6. Curated governed documents (AR-4 drift oracles will bind these).
  for (const docPath of policy.governedDocuments) {
    if (!existsSync(join(root, docPath))) continue;
    const id = `document:${docPath}`;
    if (surfaces.some((surface) => surface.id === id)) continue;
    surfaces.push({
      id,
      kind: "document",
      owningPath: docPath,
      title: docPath.split("/").pop() ?? docPath,
      derivation: "annotated",
      oracles: [],
      unverified: makeUnverified(
        id,
        policy,
        "no-oracle-authored",
        "governed document; AR-4 drift oracles will bind checkable claims",
      ),
    });
  }

  return surfaces;
};

const COVERAGE_NOTES: ReadonlyArray<string> = [
  "AR-0 rev 1 enumerates surfaces at package/app/worker/crate/public-endpoint/release-pipeline/document granularity derived from the workspace, Cargo, contract, and product-spec graphs.",
  "Individual HTTP routes, Electron IPC channels, and per-endpoint contracts are represented by their owning worker/app/public-endpoint surface, not yet subdivided; fine-grained route/IPC enumeration is a bounded follow-up.",
  "CLI entrypoints are covered by their owning package surface rather than counted separately.",
  "Obligation grading (state field) is populated by AR-1 #9057; drift oracles for governed documents by AR-4 #9060; standing verdicts by AR-3 #9059.",
];

export const buildInventory = (root: string): SurfaceInventoryDocument => {
  const tracked = trackedFiles(root);
  const contracts = allBehaviorContracts();
  const policy = loadPolicy(root);
  const surfaces = [...buildSurfaces(root, tracked, contracts, policy)].sort((a, b) =>
    compareStrings(a.id, b.id),
  );
  const summary = summarize(surfaces, COVERAGE_NOTES);

  const digestInput = surfaces
    .map(
      (surface) =>
        `${surface.id} ${surface.owningPath} ${surface.oracles.map((oracle) => `${oracle.type}:${oracle.ref}`).join("|")} ${surface.unverified?.reason ?? ""}`,
    )
    .join("\n");
  const sourceDigest = `sha256:${createHash("sha256").update(digestInput).digest("hex")}`;

  return {
    schemaVersion: SURFACE_INVENTORY_FORMAT_VERSION,
    repository: "OpenAgentsInc/openagents",
    sourceDigest,
    surfaces,
    summary,
  };
};
