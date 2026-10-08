# Bring your own Claude: Claude Code in Cloud computers

Written October 8, 2026. This page defines how a Cloud computer can run Claude
Code on the user's own Claude plan or API key, within Anthropic's published
terms. It is the policy source for the BYO-Claude issues listed at the end.

## What Anthropic's terms allow

Source: [Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)
and [authentication](https://code.claude.com/docs/en/authentication), read on
October 8, 2026. Recheck both before each availability decision.

- A platform may preinstall and run Claude Code in hosted sandboxes, under
  Anthropic's Commercial Terms of Service.
- The binary must be installed and run as published. We may not remove,
  disable, or restrict any of its sign-in methods.
- Each end user signs in with their own Claude plan, their own Anthropic API key,
  or their own Bedrock, Vertex, or Foundry credential. Usage is billed to them.
  We may not pay for, resell, or intermediate that usage.
- Third-party developers may not offer Claude.ai login in their own
  applications, route requests through Free, Pro, or Max credentials on behalf
  of their users, or **collect, store, or intermediate Claude.ai credentials or
  session tokens**. Sign-in must complete through Anthropic's own flow.
- Pro and Max limits assume ordinary, individual use.
- We may say in plain text that a computer "runs Claude Code". We may not use
  the Claude Code or Anthropic names or logos in our product names or logos, or
  suggest endorsement.

## Rules for OpenAgents

1. **Sign-in happens inside the user's computer.** The user runs `claude` (or
   `/login`) in their own computer's terminal and completes Anthropic's browser
   flow. In a hosted terminal, the browser shows a code that the user pastes
   back into that same terminal. Our web app never shows its own Claude login
   form and never asks for a token.
2. **We never collect a Claude.ai credential.** No openagents.com field, API,
   Secret Manager entry, or Coder setting accepts a claude.ai OAuth token or a
   `claude setup-token` value. The login lives only in that user's computer
   (`~/.claude/.credentials.json` in its isolated home).
3. **No path reads it back out.** Evidence capture, terminal recording, ATIF,
   export, logs, crash reports, support tooling, and saved environment images
   exclude Claude login files and redact `CLAUDE_CODE_OAUTH_TOKEN` values.
   Terminal input during a sign-in is never retained.
4. **It persists only in that user's own computer.** A working-computer
   checkpoint may carry the login so the user signs in once per computer. It
   never enters a reusable or shared environment image, another user's
   computer, or an operator copy.
5. **The binary is unmodified.** Runtime images install the published Claude
   Code release and pin its version. We do not patch, wrap away, or disable its
   sign-in choices.
6. **Usage stays the user's.** We do not meter, price, or resell Claude usage
   that runs on the user's plan or key. Our charges cover our computer, not
   their model usage.
7. **Plans get individual-use concurrency.** Automated turns on a Claude plan
   run one at a time per computer by default, respect usage-limit errors by
   pausing until the reported reset, and never fan out a fleet on one plan.
   Parallel or fleet work uses an API key or cloud-provider credential.
8. **API keys are a different custody class.** A user's own Anthropic API key,
   or Bedrock/Vertex/Foundry credential, may be stored in our secret custody
   for that user's own computers, billed to the key owner, revocable by the
   user, and never shared or resold.

## Product features within these rules

| Feature | How it stays compliant |
| --- | --- |
| Choose Claude Code as a computer's engine | Unmodified pinned binary in the runtime image. |
| One-click "Sign in to Claude" | Opens the computer's terminal and runs `claude`; Anthropic's flow does the rest. |
| Signed-in status, plan, and expiry warnings | Read by running the binary inside the computer; only status text leaves it, never credentials. |
| Sign in once per computer | Login persists in that user's own checkpoint (CMP-01). |
| Background tasks on your Claude plan | Coder drives the unmodified `claude -p` inside the user's computer on their login, one at a time, with pause-until-reset on limits. |
| Usage-limit display | Shows the reset time Claude Code reports; we keep no usage ledger for their plan. |
| Parallel fleets | Require the user's own API key or cloud credential. |
| Phone and web observation | Observers see the task and its evidence, never the credential. |

## Existing docs this supersedes

- `2026-10-02-boat-sdk-plan.md` describes connecting Claude Pro or Max on Boat's
  dashboard, with Boat writing `~/.claude/.credentials.json` into sandboxes, and
  lists `CLAUDE_CODE_OAUTH_TOKEN` as an injectable key. That path may only serve
  the owner's own internal work on the owner's own plan. It must not carry
  customer Claude plans.
- `2026-10-02-cloud-parallel-execution-audit.md` lists the long-lived
  `claude setup-token` value for burst hosts. Customer Cloud must not collect or
  inject that token. Burst and fleet hosts use API keys.

## Issues

See the BYO-Claude issues referenced from
[#10964](https://github.com/OpenAgentsInc/openagents/issues/10964). Owner action:
accept Anthropic's Commercial Terms for OpenAgents before any customer
availability (recorded in the workspace `NEEDS_OWNER.md`).
