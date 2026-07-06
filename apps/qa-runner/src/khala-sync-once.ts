#!/usr/bin/env bun
// One-shot Khala Sync TRANSPORT run against a live deployment (ST-6, #8512).
//
// The seam-explorer entrypoint: drives the REAL `createHttpKhalaSyncTransport`
// (bootstrap -> log page -> WebSocket connect) with a real cookie-less bearer
// and classifies the outcome (live / connect_unauthenticated / connect_denied /
// silent_retry_loop / never_live). Exit code is honest: 0 iff the connect
// reached `live` (or the run was an explicit clean SKIP), 1 on any finding.
//
// Usage:
//   bun run src/khala-sync-once.ts [--target staging|prod|dev|selfhost]
//     [--url https://…]            # explicit base URL instead of the registry
//     [--scope scope.user.<id>]    # explicit scope (default: scope.user.<owner>)
//     [--public]                   # anonymous scope.public.tokens-served run
//     [--register]                 # allow throwaway-agent self-registration
//     [--out ./runs/khala-sync]    [--bound-ms 10000] [--attempts 3]
//
// AUTH (never hardcoded): QA_KHALA_SYNC_TOKEN (+ QA_KHALA_SYNC_OWNER_USER_ID),
// else OPENAGENTS_AGENT_TOKEN, else KHALA_MOBILE_TEST_TOKEN
// (+ KHALA_MOBILE_TEST_OWNER_USER_ID), else — with --register — a throwaway
// agent self-registered against the target (same flow as the predeploy smoke).
// When nothing resolves the run SKIPS CLEANLY (exit 0, an explicit SKIP line,
// no fabricated result) unless --public was requested (anonymous needs no
// bearer). The bearer value is never printed and never written to artifacts.

import {
  resolveKhalaSyncAuthFromEnv,
  runKhalaSyncTransportScenario,
  selfRegisterThrowawayAgent,
  type KhalaSyncRunAuth,
} from "./khala-sync-transport-backend";
import { makeTarget, type Target } from "./target";
import { isTargetName, resolveRegistryTarget } from "./target-registry";
import { renderVerdictEvidence, renderVerdictLine } from "./verify";

const PUBLIC_SCOPE = "scope.public.tokens-served";

function parseArgs(argv: ReadonlyArray<string>) {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--public") args.public = true;
    else if (a === "--register") args.register = true;
    else if (a.startsWith("--")) {
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

function resolveCliTarget(args: Record<string, string | boolean>): Target {
  if (typeof args.url === "string") {
    return makeTarget({ name: args.url, baseUrl: args.url });
  }
  const name = typeof args.target === "string" ? args.target : "staging";
  if (!isTargetName(name)) {
    throw new Error(`unknown target "${name}" (dev|staging|prod|selfhost)`);
  }
  return resolveRegistryTarget(name);
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const target = resolveCliTarget(args);
  const artifactDir = typeof args.out === "string" ? args.out : "./runs/khala-sync";
  const wantPublic = args.public === true;

  let auth: KhalaSyncRunAuth | undefined;
  if (!wantPublic) {
    const fromEnv = resolveKhalaSyncAuthFromEnv();
    if (fromEnv.kind === "resolved") {
      auth = fromEnv.auth;
    } else if (args.register === true) {
      const registered = await selfRegisterThrowawayAgent({
        baseUrl: target.baseUrl,
        runRef: `st6-${Date.now()}`,
      });
      if (registered.kind === "resolved") auth = registered.auth;
      else console.log(`[khala-sync-once] self-registration: ${registered.reason}`);
    }
    if (auth === undefined) {
      // Honest clean SKIP (#8512 req): no bearer -> no run, no fabricated
      // result, exit 0 with an explicit reason.
      console.log(
        `[khala-sync-once] SKIP: ${fromEnv.kind === "unavailable" ? fromEnv.reason : "no auth"}` +
          " — set QA_KHALA_SYNC_TOKEN or pass --register (or --public for the anonymous scope).",
      );
      process.exit(0);
    }
  }

  const scope =
    typeof args.scope === "string"
      ? args.scope
      : wantPublic
        ? PUBLIC_SCOPE
        : auth?.ownerUserId !== undefined
          ? `scope.user.${auth.ownerUserId}`
          : PUBLIC_SCOPE;
  if (!wantPublic && typeof args.scope !== "string" && auth?.ownerUserId === undefined) {
    console.log(
      "[khala-sync-once] note: bearer has no known owner user id — falling back to the " +
        "anonymous-readable public scope. Set QA_KHALA_SYNC_OWNER_USER_ID (or use " +
        "--register, whose response carries the owner id) to drive the authenticated " +
        "scope.user seam the incident class lives on.",
    );
  }

  const outcome = await runKhalaSyncTransportScenario({
    target,
    scope,
    ...(auth !== undefined ? { auth } : {}),
    artifactDir,
    ...(typeof args["bound-ms"] === "string"
      ? { connectBoundMs: Number(args["bound-ms"]) }
      : {}),
    ...(typeof args.attempts === "string"
      ? { connectAttempts: Number(args.attempts) }
      : {}),
  });

  console.log("=== QA KHALA SYNC TRANSPORT RUN (khala-sync-once) ===");
  console.log("target:", target.name, target.baseUrl);
  console.log("scope kind:", scope.startsWith("scope.public.") ? scope : scope.split(".").slice(0, 2).join(".") + ".*");
  console.log("auth source:", auth?.source ?? "anonymous");
  console.log("status:", outcome.result.status);
  console.log("classification:", outcome.classification);
  console.log("result:", outcome.resultPath);
  console.log("report:", outcome.reportPath);
  for (const step of outcome.result.steps) {
    const latency =
      step.detail?.latencyMs !== undefined ? ` ${String(step.detail.latencyMs)}ms` : "";
    console.log(`  - [${step.status}] ${step.label}${latency}`);
  }
  if (outcome.result.failure) console.log("failure:", outcome.result.failure);
  if (outcome.result.verify !== undefined) {
    console.log(renderVerdictLine(outcome.result.verify));
    for (const line of renderVerdictEvidence(outcome.result.verify)) console.log(line);
  }
  process.exit(outcome.result.status === "pass" ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
