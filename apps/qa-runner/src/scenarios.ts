// Scripted scenarios (deterministic brain step lists).
//
// These are the journeys the scriptedBrain replays. The headline demo (#6177)
// is `loginRegressionSteps`: verify openagents.com/login renders the sign-in
// form and does NOT redirect to home — the exact regression a headless smoke
// caught. Pointing the same steps at a broken build FAILS honestly (the failed
// assertion is recorded and the failure is visible at the end of the video).

import type { BrainStep } from "./brain";
import type { Commitment } from "./verify";

/**
 * Verify `/login`:
 *   - navigating to /login stays at /login (no redirect to "/")
 *   - the page contains "Log in to OpenAgents"
 *   - (optional) /gym/oss redirects to "/" when logged out
 */
export function loginRegressionSteps(): ReadonlyArray<BrainStep> {
  return [
    { kind: "navigate", url: "/login", label: "open /login" },
    { kind: "wait-for", condition: { kind: "text-visible", value: "Log in to OpenAgents" }, label: "sign-in form renders" },
    { kind: "screenshot", label: "login-page" },
    { kind: "assert", label: "stays at /login (no redirect to home)", check: { kind: "url-includes", value: "/login" } },
    { kind: "assert", label: 'body contains "Log in to OpenAgents"', check: { kind: "text-contains", value: "Log in to OpenAgents" } },
  ];
}

/**
 * A deliberately-wrong variant used to prove the runner FAILS honestly: it
 * asserts the login page redirects away (which it must NOT). Used by the
 * real-chromium proof to show a red is a real red.
 */
export function loginRegressionStepsWrong(): ReadonlyArray<BrainStep> {
  return [
    { kind: "navigate", url: "/login", label: "open /login" },
    { kind: "wait-for", condition: { kind: "text-visible", value: "Log in to OpenAgents" }, label: "sign-in form renders" },
    { kind: "screenshot", label: "login-page" },
    // WRONG on purpose: the page does NOT redirect to home, so this fails.
    { kind: "assert", label: "redirects away from /login (intentionally wrong)", check: { kind: "url-not-includes", value: "/login" } },
  ];
}

/**
 * QS7 executor demo scenario: external production, read-only.
 *
 * The scenario verifies the public executor landing page Rhys can review without
 * mutating his service. It intentionally uses only navigate / wait / screenshot
 * / assert steps so the target adapter can force `read-only` for prod.
 */
export function executorPublicHomeSteps(): ReadonlyArray<BrainStep> {
  return [
    { kind: "navigate", url: "/", label: "open executor landing page" },
    {
      kind: "wait-for",
      condition: { kind: "text-visible", value: "Connect any agent to" },
      timeoutMs: 20_000,
      label: "hero headline renders",
    },
    {
      kind: "wait-for",
      condition: { kind: "text-visible", value: "Executor is an MCP gateway" },
      timeoutMs: 20_000,
      label: "MCP gateway copy renders",
    },
    { kind: "screenshot", label: "executor-public-home" },
    {
      kind: "assert",
      label: 'body contains "Connect any agent to"',
      check: { kind: "text-contains", value: "Connect any agent to" },
    },
    {
      kind: "assert",
      label: 'body contains "Executor is an MCP gateway"',
      check: { kind: "text-contains", value: "Executor is an MCP gateway" },
    },
    {
      kind: "assert",
      label: "body mentions Codex integration",
      check: { kind: "text-contains", value: "Codex" },
    },
  ];
}

/**
 * Deliberately-wrong executor variant for chill-evals: the target page should
 * NOT contain this copy. A failing candidate proves the comparison is honest.
 */
export function executorPublicHomeStepsWrong(): ReadonlyArray<BrainStep> {
  return [
    { kind: "navigate", url: "/", label: "open executor landing page" },
    {
      kind: "wait-for",
      condition: { kind: "text-visible", value: "Executor is an MCP gateway" },
      timeoutMs: 20_000,
      label: "MCP gateway copy renders",
    },
    { kind: "screenshot", label: "executor-public-home" },
    {
      kind: "assert",
      label: "body contains impossible executor copy (intentionally wrong)",
      check: { kind: "text-contains", value: "Executor hides MCP tools from agents" },
    },
  ];
}

// ---------------------------------------------------------------------------
// Commitments (#6192): what the /login scenario must PROVE, declared up front.
// ---------------------------------------------------------------------------
//
// The verify stage checks these against the run's produced steps and emits the
// investigator verdict. A TRUE run satisfies them all -> CONFIRMED. The FALSE
// variant below makes the same claims while running the deliberately-wrong
// steps, so its `redirect` commitment is REFUTED by observed evidence — proving
// a false claim is a finding, not a fake pass.

/**
 * Commitments the HONEST /login scenario proves: the page stays at /login and
 * renders the sign-in copy. Matched against the assert step labels above, so a
 * CONFIRMED verdict rests on OBSERVED ok assertions.
 */
export function loginRegressionCommitments(): ReadonlyArray<Commitment> {
  return [
    {
      id: "no-redirect",
      claim: "/login does NOT redirect to home when logged out",
      evidence: "step-pass",
      match: "stays at /login",
      kind: "assert",
    },
    {
      id: "renders-signin",
      claim: '/login renders "Log in to OpenAgents"',
      evidence: "step-pass",
      match: 'body contains "Log in to OpenAgents"',
      kind: "assert",
    },
  ];
}

/**
 * Commitments for the FALSE-claim proof: the run CLAIMS the login page redirects
 * away from /login (it must not). The verify stage matches this against the
 * deliberately-wrong assert step (which the runner records as FAILED), so the
 * verdict is REFUTED with the contradicting evidence inline — never a fake
 * CONFIRMED. This is the acceptance proof for #6192.
 */
export function loginRedirectClaimCommitments(): ReadonlyArray<Commitment> {
  return [
    {
      id: "claims-redirect",
      claim: "/login redirects away from /login (FALSE claim under test)",
      evidence: "step-pass",
      match: "redirects away from /login",
      kind: "assert",
    },
  ];
}

/** Commitments the honest executor public-home scenario proves. */
export function executorPublicHomeCommitments(): ReadonlyArray<Commitment> {
  return [
    {
      id: "executor-hero",
      claim: 'executor.sh renders the "Connect any agent to" hero copy',
      evidence: "step-pass",
      match: 'body contains "Connect any agent to"',
      kind: "assert",
    },
    {
      id: "executor-mcp-gateway-copy",
      claim: "executor.sh describes Executor as an MCP gateway",
      evidence: "step-pass",
      match: 'body contains "Executor is an MCP gateway"',
      kind: "assert",
    },
    {
      id: "executor-codex-copy",
      claim: "executor.sh names Codex among the agent integrations",
      evidence: "step-pass",
      match: "body mentions Codex integration",
      kind: "assert",
    },
  ];
}

/** Commitments for the deliberately false executor copy claim. */
export function executorImpossibleCopyClaimCommitments(): ReadonlyArray<Commitment> {
  return [
    {
      id: "executor-impossible-copy",
      claim: "executor.sh says Executor hides MCP tools from agents (FALSE claim under test)",
      evidence: "step-pass",
      match: "body contains impossible executor copy",
      kind: "assert",
    },
  ];
}
