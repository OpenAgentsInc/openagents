// DIST-13 (#8926 slice 1): release-command step graph, preflight refusals,
// dry-run end-to-end against fixture ports, resume-after-failure, changelog
// integration, receipt schema, and the fail-loud port guard.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { CHANGELOG_DIR, ROUTE_DATA_MODULE_PATH, UNRELEASED_FILE } from "./changelog.js";
import {
  DESKTOP_PACKAGE_JSON_PATH,
  MACOS_SIGNING_CREDENTIAL_NAMES,
  RECEIPT_LINE_MAX_LENGTH,
  RELEASE_OWNER_GATES,
  RELEASE_RECEIPT_SCHEMA,
  RELEASE_SET_SIGNING_ENV_NAMES,
  RELEASE_STEP_GRAPH,
  RELEASE_TRANSACTION_SCHEMA,
  ReleaseGateError,
  ReleasePortNotImplementedError,
  TRANSACTION_REF_PATTERN,
  assertPublicSafeReceiptLine,
  boundReceiptLine,
  buildReleaseReceipt,
  createFixtureCoordinatorPort,
  createFixtureFeedPort,
  createTransactionState,
  gatesForPlan,
  isGateApproved,
  loadTransactionState,
  newTransactionRef,
  releaseChannels,
  releaseReceiptFileName,
  releaseStepIds,
  releaseTargetKeys,
  renderReleaseReceiptMarkdown,
  runPreflightChecks,
  runRelease,
  saveTransactionState,
  type PreflightInput,
  type ReleaseIo,
  type ReleasePlan,
  type ReleasePorts,
} from "./release.js";

const tempDirs: string[] = [];
afterEach(() => {
  while (tempDirs.length > 0) rmSync(tempDirs.pop()!, { recursive: true, force: true });
});

const makeTempDir = (): string => {
  const dir = mkdtempSync(join(tmpdir(), "release-test-"));
  tempDirs.push(dir);
  return dir;
};

const FIXTURE_UNRELEASED = `# Unreleased

## One owner release command (#8926)

- issues: #8926
- commits: abc1234def
- contracts-specs: none
- invariants: none changed
- evidence: none
- lane: fixture-lane

Releasing is now one command that walks the whole pipeline.
`;

/** A temp repo root carrying the changelog fixture + generated-module dir. */
const makeFixtureRoot = (unreleased: string = FIXTURE_UNRELEASED): string => {
  const root = makeTempDir();
  mkdirSync(join(root, CHANGELOG_DIR), { recursive: true });
  writeFileSync(join(root, CHANGELOG_DIR, UNRELEASED_FILE), unreleased);
  mkdirSync(dirname(join(root, ROUTE_DATA_MODULE_PATH)), { recursive: true });
  return root;
};

const CLEAN_ENV: Record<string, string | undefined> = Object.fromEntries([
  ...MACOS_SIGNING_CREDENTIAL_NAMES.map((name) => [name, "present-for-test"]),
  ...RELEASE_SET_SIGNING_ENV_NAMES.map((name) => [name, "present-for-test"]),
]);

const HEAD_SHA = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const cleanPreflightInput = (overrides: Partial<PreflightInput> = {}): PreflightInput => ({
  mode: "dry-run",
  allowUnfrozen: false,
  statusPorcelain: "",
  headSha: `${HEAD_SHA}\n`,
  originMainSha: `${HEAD_SHA}\n`,
  desktopPackageVersion: "0.1.0-rc.18",
  cliVersion: "0.1.0-rc.18",
  channel: "rc",
  nodeVersion: "24.1.0",
  packageManagerPin: "pnpm@11.10.0",
  env: CLEAN_ENV,
  ...overrides,
});

const makePlan = (overrides: Partial<ReleasePlan> = {}): ReleasePlan => ({
  transactionRef: newTransactionRef("0.1.0-rc.18", "rc", new Date("2026-07-16T12:00:00Z")),
  mode: "dry-run",
  version: "0.1.0-rc.18",
  channel: "rc",
  sourceRevision: HEAD_SHA,
  targets: releaseTargetKeys,
  date: "2026-07-16",
  unattended: false,
  approvedGates: [],
  ...overrides,
});

const makeIo = (rootDir: string): ReleaseIo & { readonly lines: string[] } => {
  const lines: string[] = [];
  return {
    rootDir,
    scratchDir: join(rootDir, ".release"),
    log: (line) => lines.push(line),
    env: CLEAN_ENV,
    now: () => new Date("2026-07-16T12:00:00Z"),
    lines,
  };
};

const fixturePorts = (
  options: { failOnceOn?: string } = {},
): ReleasePorts & {
  coordinatorCalls: string[];
  feedCalls: string[];
} => {
  const coordinator = createFixtureCoordinatorPort(options);
  const feed = createFixtureFeedPort(options);
  return { coordinator, feed, coordinatorCalls: coordinator.calls, feedCalls: feed.calls };
};

// ---------------------------------------------------------------------------
// Step graph
// ---------------------------------------------------------------------------

describe("release step graph", () => {
  test("covers exactly the nine issue steps in order", () => {
    expect(RELEASE_STEP_GRAPH.map((step) => step.id)).toEqual([
      "preflight",
      "worker_bring_up",
      "fan_out",
      "test_gates",
      "candidate",
      "changelog",
      "promote",
      "public_surface_verify",
      "final_receipt",
    ]);
    expect(RELEASE_STEP_GRAPH.map((step) => step.id)).toEqual([...releaseStepIds]);
  });

  test("declares real vs port implementation status per step", () => {
    const kinds = Object.fromEntries(RELEASE_STEP_GRAPH.map((step) => [step.id, step.kind]));
    expect(kinds.preflight).toBe("real");
    expect(kinds.changelog).toBe("real");
    expect(kinds.final_receipt).toBe("real");
    for (const id of [
      "worker_bring_up",
      "fan_out",
      "test_gates",
      "candidate",
      "promote",
      "public_surface_verify",
    ]) {
      expect(kinds[id]).toBe("port");
    }
  });

  test("dependencies form a valid topological order (each step depends only on earlier steps)", () => {
    const seen = new Set<string>();
    for (const step of RELEASE_STEP_GRAPH) {
      for (const dependency of step.dependsOn) {
        expect(seen.has(dependency)).toBe(true);
      }
      seen.add(step.id);
    }
  });
});

// ---------------------------------------------------------------------------
// Mirror guard — local vocabulary must match the app release contracts
// ---------------------------------------------------------------------------

describe("mirrored release vocabulary", () => {
  const repoRoot = join(import.meta.dirname, "..");
  test("target keys and channels match the Desktop release contracts", () => {
    const releaseSetSource = readFileSync(
      join(repoRoot, "apps/openagents-desktop/src/release-set-contract.ts"),
      "utf8",
    );
    for (const target of releaseTargetKeys) {
      expect(releaseSetSource).toContain(`"${target}"`);
    }
    const updateContractSource = readFileSync(
      join(repoRoot, "apps/openagents-desktop/src/update-contract.ts"),
      "utf8",
    );
    for (const channel of releaseChannels) {
      expect(updateContractSource).toContain(`"${channel}"`);
    }
  });

  test("the version authority file exists and carries a valid release version", () => {
    const manifest = JSON.parse(
      readFileSync(join(repoRoot, DESKTOP_PACKAGE_JSON_PATH), "utf8"),
    ) as { version?: string };
    expect(manifest.version).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Preflight refusals
// ---------------------------------------------------------------------------

describe("preflight checks", () => {
  test("clean input yields no refusals", () => {
    const checks = runPreflightChecks(cleanPreflightInput());
    expect(checks.filter((check) => check.severity === "refusal")).toEqual([]);
  });

  test("dirty tree is a refusal", () => {
    const checks = runPreflightChecks(cleanPreflightInput({ statusPorcelain: " M foo.ts\n" }));
    expect(checks.find((check) => check.id === "clean_tree")?.severity).toBe("refusal");
  });

  test("HEAD not at origin/main is a refusal", () => {
    const checks = runPreflightChecks(cleanPreflightInput({ originMainSha: "b".repeat(40) }));
    expect(checks.find((check) => check.id === "frozen_at_origin_main")?.severity).toBe("refusal");
  });

  test("--allow-unfrozen downgrades ONLY the freeze rows, ONLY in dry-run", () => {
    const dry = runPreflightChecks(
      cleanPreflightInput({
        allowUnfrozen: true,
        statusPorcelain: " M foo.ts\n",
        originMainSha: "b".repeat(40),
      }),
    );
    expect(dry.find((check) => check.id === "clean_tree")?.severity).toBe("warning");
    expect(dry.find((check) => check.id === "frozen_at_origin_main")?.severity).toBe("warning");
    const real = runPreflightChecks(
      cleanPreflightInput({
        mode: "real",
        allowUnfrozen: true,
        statusPorcelain: " M foo.ts\n",
      }),
    );
    expect(real.find((check) => check.id === "clean_tree")?.severity).toBe("refusal");
  });

  test("version conflict with the Desktop version authority is a refusal", () => {
    const checks = runPreflightChecks(cleanPreflightInput({ cliVersion: "0.1.0-rc.99" }));
    const row = checks.find((check) => check.id === "version_resolution");
    expect(row?.severity).toBe("refusal");
    expect(row?.detail).toContain(DESKTOP_PACKAGE_JSON_PATH);
  });

  test("channel/version disagreement is a refusal in both directions", () => {
    const stableWithRc = runPreflightChecks(cleanPreflightInput({ channel: "stable" }));
    expect(stableWithRc.find((check) => check.id === "version_resolution")?.severity).toBe(
      "refusal",
    );
    const rcWithStable = runPreflightChecks(
      cleanPreflightInput({ cliVersion: "0.1.0", desktopPackageVersion: "0.1.0", channel: "rc" }),
    );
    expect(rcWithStable.find((check) => check.id === "version_resolution")?.severity).toBe(
      "refusal",
    );
  });

  test("missing signing-credential PRESENCE refuses in real mode, warns in dry-run, never prints values", () => {
    const env = { ...CLEAN_ENV, OA_DEVELOPER_ID_APPLICATION: undefined };
    const real = runPreflightChecks(cleanPreflightInput({ mode: "real", env }));
    const realRow = real.find((check) => check.id === "signing_credentials_present");
    expect(realRow?.severity).toBe("refusal");
    expect(realRow?.detail).toContain("OA_DEVELOPER_ID_APPLICATION");
    expect(realRow?.detail).not.toContain("present-for-test");
    const dry = runPreflightChecks(cleanPreflightInput({ env }));
    expect(dry.find((check) => check.id === "signing_credentials_present")?.severity).toBe(
      "warning",
    );
  });

  test("toolchain pin drift is a refusal", () => {
    const checks = runPreflightChecks(cleanPreflightInput({ nodeVersion: "22.0.0" }));
    expect(checks.find((check) => check.id === "toolchain_pins")?.severity).toBe("refusal");
  });
});

// ---------------------------------------------------------------------------
// Port guard — fail loudly outside dry-run without a real implementation
// ---------------------------------------------------------------------------

describe("typed port guard", () => {
  test("a real-mode run refuses at the first fixture-port call and never reaches promote", async () => {
    const root = makeFixtureRoot();
    const io = makeIo(root);
    const ports = fixturePorts();
    const plan = makePlan({ mode: "real", unattended: true });
    await expect(
      runRelease({
        plan,
        preflightInput: cleanPreflightInput({ mode: "real" }),
        ports,
        io,
      }),
    ).rejects.toThrow(ReleasePortNotImplementedError);
    // The refusal happened at preflight's worker-inventory port check —
    // structurally before any build, candidate, or channel-pointer step.
    expect(ports.coordinatorCalls).not.toContain("promoteChannelPointer");
    expect(ports.feedCalls).toEqual([]);
    const state = loadTransactionState(io.scratchDir, plan.transactionRef);
    expect(state.steps.preflight.status).toBe("failed");
    expect(state.steps.promote.status).toBe("pending");
  });

  test("the guard message names the owning issue for each port", () => {
    expect(
      new ReleasePortNotImplementedError("ReleaseCoordinatorPort", "fanOutTargets").message,
    ).toContain("#8917");
    expect(
      new ReleasePortNotImplementedError("ReleaseFeedPort", "verifyPublicSurfaces").message,
    ).toContain("#8922");
  });
});

// ---------------------------------------------------------------------------
// Dry-run end-to-end
// ---------------------------------------------------------------------------

describe("dry-run end-to-end", () => {
  test("walks all nine steps against fixtures, writes a DRY-RUN receipt, mutates nothing", async () => {
    const root = makeFixtureRoot();
    const io = makeIo(root);
    const ports = fixturePorts();
    const plan = makePlan();
    const result = await runRelease({
      plan,
      preflightInput: cleanPreflightInput(),
      ports,
      io,
    });

    for (const id of releaseStepIds) {
      expect(result.state.steps[id].status).toBe("succeeded");
    }
    // Every step logged a bounded receipt line.
    for (const id of releaseStepIds) {
      expect(result.state.steps[id].receiptLines.length).toBeGreaterThan(0);
      for (const line of result.state.steps[id].receiptLines) {
        expect(line.length).toBeLessThanOrEqual(RECEIPT_LINE_MAX_LENGTH);
      }
    }
    // DRY-RUN receipt lands in the scratch dir, clearly marked; docs/deploy
    // receives nothing.
    expect(result.receiptPath).toContain(join(".release", "receipts", "DRY-RUN-"));
    const receiptText = readFileSync(result.receiptPath, "utf8");
    expect(receiptText).toContain("DRY-RUN — NOT A RELEASE RECEIPT");
    expect(receiptText).toContain(RELEASE_RECEIPT_SCHEMA);
    expect(existsSync(join(root, "docs/deploy/receipts"))).toBe(false);
    // The changelog fixture was previewed, not mutated.
    expect(readFileSync(join(root, CHANGELOG_DIR, UNRELEASED_FILE), "utf8")).toBe(
      FIXTURE_UNRELEASED,
    );
    // Promotion was simulated through the fixture port only.
    expect(ports.coordinatorCalls).toContain("promoteChannelPointer");
    expect(result.state.steps.promote.receiptLines.join(" ")).toContain("SIMULATED");
    // Owner gates are surfaced by name in the output.
    expect(io.lines.join("\n")).toContain("changelog_human_review");
    expect(io.lines.join("\n")).toContain("rc_promotion");
  });

  test("dry-run receipt includes per-step status and the port/real split", async () => {
    const root = makeFixtureRoot();
    const io = makeIo(root);
    const result = await runRelease({
      plan: makePlan(),
      preflightInput: cleanPreflightInput(),
      ports: fixturePorts(),
      io,
    });
    const receiptText = readFileSync(result.receiptPath, "utf8");
    expect(receiptText).toContain("1. preflight — succeeded (real)");
    expect(receiptText).toContain("3. fan_out — succeeded (port)");
    expect(receiptText).toContain("7. promote — succeeded (port)");
    expect(receiptText).toContain("9. final_receipt — succeeded (real)");
  });
});

// ---------------------------------------------------------------------------
// Resume after failure
// ---------------------------------------------------------------------------

describe("resume after failure", () => {
  test("a mid-graph failure persists resumable state; resume skips succeeded steps", async () => {
    const root = makeFixtureRoot();
    const io = makeIo(root);
    const failing = fixturePorts({ failOnceOn: "runReleaseGates" });
    const plan = makePlan();

    await expect(
      runRelease({ plan, preflightInput: cleanPreflightInput(), ports: failing, io }),
    ).rejects.toThrow("fixture failure injected on runReleaseGates");

    const failed = loadTransactionState(io.scratchDir, plan.transactionRef);
    expect(failed.schema).toBe(RELEASE_TRANSACTION_SCHEMA);
    expect(failed.steps.preflight.status).toBe("succeeded");
    expect(failed.steps.worker_bring_up.status).toBe("succeeded");
    expect(failed.steps.fan_out.status).toBe("succeeded");
    expect(failed.steps.test_gates.status).toBe("failed");
    expect(failed.steps.test_gates.failure).toContain("fixture failure injected");
    expect(failed.steps.promote.status).toBe("pending");
    expect(io.lines.join("\n")).toContain(`--resume ${plan.transactionRef}`);

    // Resume with fresh ports: earlier steps are NOT re-executed.
    const resumed = fixturePorts();
    const result = await runRelease({
      plan,
      preflightInput: cleanPreflightInput(),
      ports: resumed,
      io,
      resumeState: failed,
    });
    expect(resumed.coordinatorCalls).not.toContain("checkWorkerInventory");
    expect(resumed.coordinatorCalls).not.toContain("bringUpWorkers");
    expect(resumed.coordinatorCalls).not.toContain("fanOutTargets");
    expect(resumed.coordinatorCalls).toContain("runReleaseGates");
    for (const id of releaseStepIds) {
      expect(result.state.steps[id].status).toBe("succeeded");
    }
    // Receipt lines from the pre-failure steps survived the resume.
    expect(result.state.steps.fan_out.receiptLines.length).toBe(6);
  });

  test("resume refuses a state file with the wrong schema", () => {
    const root = makeTempDir();
    const scratch = join(root, ".release");
    const state = createTransactionState(makePlan(), new Date());
    (state as { schema: string }).schema = "openagents.other.v1";
    saveTransactionState(scratch, state, new Date());
    expect(() => loadTransactionState(scratch, state.transactionRef)).toThrow("schema");
  });
});

// ---------------------------------------------------------------------------
// Changelog step integration
// ---------------------------------------------------------------------------

describe("changelog step", () => {
  test("refuses when UNRELEASED has no entries (per DIST-14 semantics)", async () => {
    const root = makeFixtureRoot("# Unreleased\n\nNo entries yet.\n");
    const io = makeIo(root);
    const plan = makePlan();
    await expect(
      runRelease({ plan, preflightInput: cleanPreflightInput(), ports: fixturePorts(), io }),
    ).rejects.toThrow("no entries to roll");
    const state = loadTransactionState(io.scratchDir, plan.transactionRef);
    expect(state.steps.changelog.status).toBe("failed");
    expect(state.steps.promote.status).toBe("pending");
  });

  test("real mode rolls the fixture changelog dir and requires the review gate before promote", async () => {
    const root = makeFixtureRoot();
    const io = makeIo(root);
    // Real coordinator/feed stand-ins so real mode can pass the port guard in
    // this bounded test (the fixtures below assert changelog file effects).
    const coordinator = { ...createFixtureCoordinatorPort(), kind: "real" as const };
    const feed = { ...createFixtureFeedPort(), kind: "real" as const };
    const plan = makePlan({ mode: "real", unattended: true });
    // Unattended real mode stops at the changelog_human_review gate (NOT safe
    // for --yes) — after the roll has produced the reviewable draft.
    await expect(
      runRelease({
        plan,
        preflightInput: cleanPreflightInput({ mode: "real" }),
        ports: { coordinator, feed },
        io,
      }),
    ).rejects.toThrow(ReleaseGateError);
    const releaseFile = join(root, CHANGELOG_DIR, "2026-07-16-desktop-0.1.0-rc.18.md");
    expect(existsSync(releaseFile)).toBe(true);
    expect(readFileSync(join(root, CHANGELOG_DIR, UNRELEASED_FILE), "utf8")).not.toContain("#8926");
    const state = loadTransactionState(io.scratchDir, plan.transactionRef);
    expect(state.steps.changelog.status).toBe("succeeded");
    // The gate refusal is recorded on the promote step and is resumable.
    expect(state.steps.promote.status).toBe("failed");
    expect(state.steps.promote.failure).toContain("changelog_human_review");
  });
});

// ---------------------------------------------------------------------------
// Owner gates
// ---------------------------------------------------------------------------

describe("owner gates", () => {
  test("rc promotion is safe for unattended --yes; stable and review are owner-explicit", () => {
    const rcPlan = makePlan({ unattended: true });
    const gates = gatesForPlan(rcPlan);
    expect(gates.map((gate) => gate.id)).toEqual(["changelog_human_review", "rc_promotion"]);
    expect(isGateApproved(gates.find((gate) => gate.id === "rc_promotion")!, rcPlan)).toBe(true);
    expect(
      isGateApproved(gates.find((gate) => gate.id === "changelog_human_review")!, rcPlan),
    ).toBe(false);

    const stablePlan = makePlan({ channel: "stable", version: "0.1.0", unattended: true });
    const stableGates = gatesForPlan(stablePlan);
    expect(stableGates.map((gate) => gate.id)).toContain("first_stable_promotion");
    expect(
      isGateApproved(stableGates.find((gate) => gate.id === "first_stable_promotion")!, stablePlan),
    ).toBe(false);
    expect(
      isGateApproved(
        stableGates.find((gate) => gate.id === "first_stable_promotion")!,
        makePlan({ channel: "stable", approvedGates: ["first_stable_promotion"] }),
      ),
    ).toBe(true);
  });

  test("every gate is declared before the promote step or earlier", () => {
    for (const gate of RELEASE_OWNER_GATES) {
      expect(releaseStepIds.includes(gate.beforeStep)).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// Receipt schema and public-safety
// ---------------------------------------------------------------------------

describe("release receipt", () => {
  test("receipt file name follows ProductSpec §11.1", () => {
    expect(releaseReceiptFileName("2026-07-16", "0.1.0-rc.18", "rc")).toBe(
      "2026-07-16-openagents-desktop-v0.1.0-rc.18-rc.md",
    );
    expect(releaseReceiptFileName("2026-08-01", "0.1.0", "stable")).toBe(
      "2026-08-01-openagents-desktop-v0.1.0-stable.md",
    );
  });

  test("transaction refs are pattern-bounded", () => {
    expect(TRANSACTION_REF_PATTERN.test(newTransactionRef("0.1.0-rc.18", "rc", new Date()))).toBe(
      true,
    );
    expect(TRANSACTION_REF_PATTERN.test("../../etc/passwd")).toBe(false);
  });

  test("receipt lines are bounded and flattened", () => {
    expect(boundReceiptLine(`a${" b".repeat(400)}`).length).toBeLessThanOrEqual(
      RECEIPT_LINE_MAX_LENGTH,
    );
    expect(boundReceiptLine("multi\nline\ttext")).toBe("multi line text");
  });

  test("a receipt line carrying a present credential value is rejected", () => {
    const env = { OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D: "super-secret-d-value" };
    expect(() => assertPublicSafeReceiptLine("signed with super-secret-d-value", env)).toThrow(
      "OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D",
    );
    expect(() => assertPublicSafeReceiptLine("-----BEGIN EC PRIVATE KEY-----", env)).toThrow(
      "private-key",
    );
    expect(assertPublicSafeReceiptLine("ok line", env)).toBe("ok line");
  });

  test("rendered receipt binds schema, transaction, version, channel, revision, gates, and steps", () => {
    const plan = makePlan();
    const state = createTransactionState(plan, new Date("2026-07-16T12:00:00Z"));
    state.steps.preflight.status = "succeeded";
    state.steps.preflight.receiptLines = ["ok: clean_tree — working tree clean"];
    const receipt = buildReleaseReceipt(state, plan, new Date("2026-07-16T12:00:00Z"));
    expect(receipt.schema).toBe(RELEASE_RECEIPT_SCHEMA);
    const markdown = renderReleaseReceiptMarkdown(receipt);
    expect(markdown).toContain(`- transaction: ${plan.transactionRef}`);
    expect(markdown).toContain("- version: 0.1.0-rc.18");
    expect(markdown).toContain("- channel: rc");
    expect(markdown).toContain(`- source-revision: ${HEAD_SHA}`);
    expect(markdown).toContain("## Owner gates");
    expect(markdown).toContain("changelog_human_review");
    expect(markdown).toContain("DRY-RUN — NOT A RELEASE RECEIPT");
  });
});
