// Scripted scenarios (deterministic brain step lists).
//
// These are the journeys the scriptedBrain replays. The headline demo (#6177)
// is `loginRegressionSteps`: verify openagents.com/login renders the sign-in
// form and does NOT redirect to home — the exact regression a headless smoke
// caught. Pointing the same steps at a broken build FAILS honestly (the failed
// assertion is recorded and the failure is visible at the end of the video).

import type { BrainStep } from "./brain";

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
