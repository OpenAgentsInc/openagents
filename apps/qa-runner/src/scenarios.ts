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
