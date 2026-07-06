#!/usr/bin/env bun
// Agentic PR-evidence entrypoint (#6185): diff-scope -> run eval -> compose PR comment.
// NOT a GitHub Action (see the No-GitHub-Hosted-CI invariant): an agent/operator runs this
// on owned infra and posts the comment itself (e.g. via `gh pr comment`), as on PR #6224.
//
// On a PR, an agent runs this: it maps changed paths to affected
// scenarios (diff-scope), runs the qa-runner eval (fixtures/own-infra by
// DEFAULT — no network, no spend; real-model only when armed + capped upstream),
// uploads the per-variant videos via gh-attach when available, and writes the PR
// comment body to a file (the workflow posts it with `gh pr comment`).
//
// By DEFAULT this is a DRY RUN that prints + writes the comment body; it does NOT
// post. The workflow owns posting (so secrets/tokens stay in CI, not here).
//
// Usage:
//   bun run src/pr-comment-run.ts \
//     --changed "apps/openagents.com/...,packages/..." \
//     --out ./runs/pr-eval --comment-out ./pr-comment.md \
//     [--pro-base-url https://openagents.com] [--repo OpenAgentsInc/openagents]

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { Effect } from "effect";
import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { type EvalVariant, runEval } from "./evals";
import { makeFakeChromium } from "./fake-chromium";
import { makeProcessRunner } from "./gh-attach";
import { composePrComment } from "./pr-comment";
import { publishRunDir } from "./publish-trace";
import { runQaSession } from "./runner";
import {
  loginRedirectClaimCommitments,
  loginRegressionSteps,
  loginRegressionStepsWrong,
} from "./scenarios";
import {
  resolveKhalaSyncAuthFromEnv,
  runKhalaSyncTransportScenario,
} from "./khala-sync-transport-backend";
import { makeTarget } from "./target";
import {
  renderVerdictEvidence,
  renderVerdictLine,
  verifyCommitments,
  type VerifyReport,
} from "./verify";
import {
  learnFromRun,
  type FailureLearningStrategy,
  type FailureSuggestion,
} from "./failure-learning";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith("--")) {
        args[key] = next;
        i++;
      } else args[key] = true;
    }
  }
  return args;
}

// Diff-scope: map changed paths -> scenario ids. Deterministic + bounded (this
// is selection AFTER paths are known, not user-intent routing, so a static map
// is acceptable). The map documents the seam.
//
// ST-6 (#8512): a diff touching the khala-sync packages, the client transport,
// or the auth path scopes in the `khala-sync-transport` scenario — the headless
// seam probe that drives the REAL createHttpKhalaSyncTransport against a live
// target. It runs only when ARMED (QA_KHALA_SYNC_ARM=1 — it needs network + a
// bearer); unarmed it is reported as scoped-but-skipped, honestly, never as a
// fake pass.
const SCOPE_RULES: ReadonlyArray<{ test: (p: string) => boolean; scenario: string }> =
  [
    { test: p => p.includes("login") || p.includes("auth"), scenario: "login-regression" },
    { test: p => p.includes("apps/openagents.com"), scenario: "login-regression" },
    { test: p => p.includes("apps/qa-runner"), scenario: "login-regression" },
    { test: p => p.includes("packages/khala-sync"), scenario: "khala-sync-transport" },
    { test: p => p.includes("transport"), scenario: "khala-sync-transport" },
    { test: p => p.includes("auth"), scenario: "khala-sync-transport" },
  ];

const scopedScenarios = (changed: ReadonlyArray<string>): ReadonlyArray<string> => {
  const set = new Set<string>();
  for (const path of changed) {
    for (const rule of SCOPE_RULES) {
      if (rule.test(path)) set.add(rule.scenario);
    }
  }
  // Default to the headline login scenario when nothing matched (so a PR always
  // gets at least the smoke comparison rather than silently skipping).
  if (set.size === 0) set.add("login-regression");
  return [...set];
};

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const changed =
    typeof args.changed === "string"
      ? args.changed.split(",").map(s => s.trim()).filter(Boolean)
      : [];
  const out = typeof args.out === "string" ? args.out : "./runs/pr-eval";
  const commentOut =
    typeof args["comment-out"] === "string" ? args["comment-out"] : "./pr-comment.md";
  const proBaseUrl =
    typeof args["pro-base-url"] === "string" ? args["pro-base-url"] : "https://openagents.com";
  const repo = typeof args.repo === "string" ? args.repo : undefined;

  const scenarios = scopedScenarios(changed);
  console.log(`[pr-comment] diff-scoped scenarios: ${scenarios.join(", ")}`);

  // CI DEFAULT: deterministic fixtures (no network, no spend).
  const chromium = makeFakeChromium({
    pages: {
      "/login": { text: "Log in to OpenAgents", html: "<form>Log in to OpenAgents</form>" },
    },
  });
  const backend = () => localBackend({ chromium });

  const variants: ReadonlyArray<EvalVariant> = [
    {
      id: "baseline",
      label: "baseline",
      note: "current scenario",
      axis: { kind: "mcp_set", value: "filesystem:on", baseline: true },
      brain: () => scriptedBrain(loginRegressionSteps()),
      backend,
    },
    {
      id: "candidate",
      label: "candidate",
      note: "asserts a redirect that should not happen (regressed)",
      axis: { kind: "mcp_set", value: "filesystem:off" },
      brain: () => scriptedBrain(loginRegressionStepsWrong()),
      backend,
    },
  ];

  const outcome = await Effect.runPromise(
    runEval({
      id: "login-mcp-compare",
      title: "Login scenario: baseline vs candidate",
      target: makeTarget({ name: "ci-fixtures", baseUrl: "https://example.test" }),
      scenario: { id: "login-regression", label: "/login renders sign-in" },
      variants,
      artifactDir: out,
    }),
  );

  // Verify stage (#6192): run the candidate scenario WITH its commitments so the
  // PR comment leads with a real investigator verdict. The candidate CLAIMS
  // /login redirects away (it does not), so this is the acceptance proof: a
  // FALSE claim yields REFUTED, not a fake pass — surfaced before the table.
  const verifyOutcome = await Effect.runPromise(
    runQaSession({
      target: makeTarget({ name: "ci-fixtures", baseUrl: "https://example.test" }),
      brain: scriptedBrain(loginRegressionStepsWrong()),
      backend: backend(),
      artifactDir: `${out}/verify`,
      commitments: loginRedirectClaimCommitments(),
    }),
  );
  // Reconstruct the VerifyReport from the persisted additive `verify` field so
  // the composer renders the verdict + the inline contradicting evidence.
  const verify: VerifyReport | undefined =
    verifyOutcome.result.verify !== undefined
      ? verifyCommitments({
          commitments: loginRedirectClaimCommitments(),
          steps: verifyOutcome.result.steps,
          runStatus: verifyOutcome.result.status,
        })
      : undefined;

  // Failure learning (#6195): when the verify run was REFUTED/failed, capture a
  // public-safe failure pattern + a fix/scenario-update suggestion. The strategy
  // is config-selectable via --fl-strategy (default suggest_in_report); the
  // mutating strategies additionally require --fl-arm-mutations (default OFF), so
  // the CI default can never silently commit/PR. Honest: no failure -> no
  // suggestion (never fabricated).
  const flStrategy: FailureLearningStrategy | undefined =
    args["fl-strategy"] === "auto_commit"
      ? "auto_commit"
      : args["fl-strategy"] === "open_pr"
        ? "open_pr"
        : args["fl-strategy"] === "suggest_in_report"
          ? "suggest_in_report"
          : undefined;
  const learned = learnFromRun(verifyOutcome.result, {
    ...(flStrategy !== undefined ? { strategy: flStrategy } : {}),
    armMutations: args["fl-arm-mutations"] === true,
  });
  const failureSuggestion: FailureSuggestion | undefined = learned?.suggestion;
  if (failureSuggestion !== undefined) {
    console.log(
      `[pr-comment] captured failure pattern ${learned!.pattern.patternRef} (strategy: ${failureSuggestion.resolved.strategy})`,
    );
  }

  // gh-attach is OPTIONAL: in CI without browser cookies it will fail and we
  // fall back to the relative video ref (honest, no broken embed).
  const ghAttach = makeProcessRunner(async (cmd, cmdArgs) => {
    const proc = Bun.spawn([cmd, ...cmdArgs], { stdout: "pipe", stderr: "pipe" });
    const stdout = await new Response(proc.stdout).text();
    const exitCode = await proc.exited;
    return { exitCode, stdout };
  });

  const variantVideoPaths = outcome.result.variants
    .map(v => {
      const video = v.runs.find(r => r.video !== undefined)?.video;
      return video !== undefined
        ? { variantId: v.variantId, filePath: `${out}/${video}` }
        : undefined;
    })
    .filter((x): x is { variantId: string; filePath: string } => x !== undefined);

  // #6210: publish the baseline variant's representative trace -> /trace/{uuid}
  // and use it as the comment's shareable link. ENV-ARMED (QA_TRACE_PUBLISH_URL +
  // an agent token); HONEST NO-OP otherwise (the comment falls back to the
  // operator-console deep link — never a fabricated uuid).
  const baselineRunDir = join(out, `${outcome.result.baselineVariantId}.0`);
  const publishResult = await Effect.runPromise(
    publishRunDir({
      runDir: baselineRunDir,
      sessionId: `${outcome.result.id}-${outcome.result.baselineVariantId}`,
      shareBaseUrl: proBaseUrl,
    }),
  );
  const traceUrl = publishResult.published ? publishResult.url : undefined;
  if (publishResult.published) {
    console.log(`[pr-comment] published shareable trace ${publishResult.url}`);
  } else {
    console.log(`[pr-comment] trace not published (${publishResult.reason})`);
  }

  const body = await composePrComment({
    result: outcome.result,
    proBaseUrl,
    ...(traceUrl !== undefined ? { traceUrl } : {}),
    variantVideoPaths,
    ...(verify !== undefined ? { verify } : {}),
    ...(failureSuggestion !== undefined ? { failureSuggestion } : {}),
    // Only attempt gh-attach when explicitly armed (cookies present); otherwise
    // skip the upload and fall back to relative refs.
    ...(args["gh-attach"] === true ? { ghAttach } : {}),
    ...(repo !== undefined ? { ghAttachOptions: { repo } } : {}),
  });

  // ST-6 (#8512): when the diff scoped in the khala-sync-transport seam probe,
  // run it for REAL only when ARMED (QA_KHALA_SYNC_ARM=1 — network + a bearer);
  // otherwise report scoped-but-skipped honestly (never a fake pass, never a
  // silent drop). A failed/REFUTED seam run REDS the gate below.
  let khalaSyncSection = "";
  let khalaSyncOk = true;
  if (scenarios.includes("khala-sync-transport")) {
    const heading = "### khala-sync-transport seam probe (#8512)";
    if (process.env.QA_KHALA_SYNC_ARM === "1") {
      const resolved = resolveKhalaSyncAuthFromEnv();
      if (resolved.kind === "resolved") {
        const targetUrl =
          process.env.QA_KHALA_SYNC_TARGET_URL ?? "https://staging.openagents.com";
        const scope =
          resolved.auth.ownerUserId !== undefined
            ? `scope.user.${resolved.auth.ownerUserId}`
            : "scope.public.tokens-served";
        const seam = await runKhalaSyncTransportScenario({
          target: makeTarget({ name: "khala-sync-pr", baseUrl: targetUrl }),
          scope,
          auth: resolved.auth,
          artifactDir: join(out, "khala-sync-transport"),
        });
        khalaSyncOk = seam.result.status === "pass";
        const seamVerify = seam.result.verify;
        khalaSyncSection = [
          heading,
          "",
          `Classification: \`${seam.classification}\` — status **${seam.result.status}**`,
          ...(seamVerify !== undefined
            ? ["", renderVerdictLine(seamVerify), ...renderVerdictEvidence(seamVerify)]
            : []),
          "",
        ].join("\n");
        console.log(
          `[pr-comment] khala-sync-transport: ${seam.classification} (${seam.result.status})`,
        );
      } else {
        khalaSyncSection = `${heading}\n\nScoped by this diff but SKIPPED: ${resolved.reason}.\n`;
        console.log(`[pr-comment] khala-sync-transport: skipped (${resolved.reason})`);
      }
    } else {
      khalaSyncSection =
        `${heading}\n\nScoped by this diff but NOT ARMED — set QA_KHALA_SYNC_ARM=1 ` +
        "(with a bearer in env) to drive the real transport against staging.\n";
      console.log("[pr-comment] khala-sync-transport: scoped but not armed — skipped");
    }
  }

  const finalBody = khalaSyncSection === "" ? body : `${body}\n${khalaSyncSection}`;
  writeFileSync(commentOut, finalBody);
  console.log(`[pr-comment] wrote ${commentOut}`);
  console.log("--- PR comment body ---");
  console.log(finalBody);

  // Honest exit code: RED only on a real DEVIATION from each variant's EXPECTED
  // outcome — not merely because a variant failed. The default fixture's
  // `candidate` is a deliberate regression DEMO (it asserts a redirect that does
  // not happen), so it is EXPECTED to fail; gating on "any failure" would turn
  // every PR red (cry-wolf). A real diff-scoped variant has no expected entry →
  // defaults to expect-pass, so a genuine regression still reds the check.
  const EXPECTED_PASS_RATE: Record<string, number> = { baseline: 1, candidate: 0 };
  const asExpected = outcome.result.variants.every(
    v => v.passRate === (EXPECTED_PASS_RATE[v.variantId] ?? 1),
  );
  if (!asExpected) {
    console.log("[pr-comment] a variant deviated from its expected outcome — gate RED.");
  }
  if (!khalaSyncOk) {
    console.log("[pr-comment] khala-sync-transport seam probe failed — gate RED.");
  }
  process.exit(asExpected && khalaSyncOk ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
