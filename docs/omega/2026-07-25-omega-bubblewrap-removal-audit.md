# Omega bubblewrap removal audit

Date: 2026-07-25. Status: proposal, partially landed.

## Why this document exists

Omega is a Zed fork. Zed is a product for a general audience, so it asks
permission a lot: trust gates, confirmation modals, plan upsells, consent
banners. Omega is an owner-operated, agent-driven editor. Most of that asking
is dead weight here, and some of it actively breaks the product by disabling
capability when it is declined.

The direction is: **do what the user tells you.** An interruption has to earn
its place by preventing a loss that cannot be undone. Nothing else survives.

This audit inventories every place Omega still second-guesses the operator,
and says plainly which ones go and which ones stay.

## Product ownership

**Sarah is the product owner of Omega.** The human owner sets direction and
adjudicates disputes, but Sarah holds the ProductSpec, decides what "done"
means for a packet, and is the party a proof is presented to.

Two consequences that this audit treats as binding:

1. **Test state is never the human's problem.** If a proof needs a clean
   identity, an empty profile, a revoked grant, or a downgraded build, the
   harness creates that state itself. Asking the human to reset an identity or
   quit an application to unblock a test is a defect in the harness, not a step
   in a runbook.
2. **Proofs are adversarial by construction.** A packet is reviewed by a
   distinct agent whose job is to falsify it, not by the agent that produced
   it. Self-verification is not verification.

## Removal list

Severity is about product harm, not effort.

| # | Surface | Where | Verdict | Why |
| --- | --- | --- | --- | --- |
| 1 | Restricted Mode + "Unrecognized Project" trust modal | `assets/settings/default.json` `session.trust_all_worktrees` | **REMOVED** | Interrupted every new project; declining silently disabled project settings, language servers, and MCP. Landed as `OMEGA-DELTA-0001`. |
| 2 | Agent tool confirmation on every action | `assets/settings/default.json:1127` `tool_permissions.default: "confirm"` | **REMOVE** | The single largest one. Omega is an agentic editor whose whole point is unattended work; a confirm-by-default tool policy means Full Auto cannot run unattended at all. Should default to `allow`, with `always_confirm`/`always_deny` patterns retained for genuinely destructive operations. |
| 3 | Zed subscription plan UI (Free / Pro / Business / VIP / Student) | `crates/ai_onboarding/src/plan_definitions.rs` | **REMOVE** | Commercial surface for a service Omega does not sell. Also presents Zed as the product, which omega#16 forbids. |
| 4 | "Young account" Pro-trial eligibility banner | `crates/ai_onboarding/src/young_account_banner.rs` | **REMOVE** | Explains why a GitHub account under 30 days old cannot get a Zed Pro trial. The copy already admits "This trial path is not available in Omega", so it is a banner whose only content is that it does not apply. |
| 5 | Agent panel onboarding cards / API-key onboarding | `crates/ai_onboarding/src/agent_panel_onboarding_*.rs`, `agent_api_keys_onboarding.rs` | **REVIEW** | Some is genuine setup Omega needs. Split the Zed-cloud sign-up path from the local provider path and keep only the latter. |
| 6 | Edit-prediction onboarding content | `crates/ai_onboarding/src/edit_prediction_onboarding_content.rs` | **REVIEW** | Keep only if it configures a local capability; drop if it sells a hosted one. |
| 7 | "Sign In" control in the title bar | title bar, drives `services.openagents.invalid` | **REMOVE or REPOINT** | Points at a deliberately non-routable placeholder, so clicking it lands the user on a browser error page. Correct for policy, broken as UX. Either hide it until an owned service exists, or point it at Omega identity. |
| 8 | Collab channel/contact removal confirmations | `crates/collab_ui/src/collab_panel.rs:2504,2535,2570` | **REMOVE (with the feature)** | Zed collab is not an Omega product surface. These die with the feature, not on their own. |
| 9 | "Are you sure you want to quit?" | `crates/zed/src/zed.rs:1777` | **ALREADY OFF** | `confirm_quit` already defaults to `false`. Verify it stays that way — candidate for a delta check. |
| 10 | Telemetry diagnostics / metrics | `assets/settings/default.json:1551` | **ALREADY OFF** | Both default `false`. Candidate for a delta check so a rebase cannot re-enable them. |
| 11 | Debug session terminate confirmation | `crates/debugger_ui/src/debugger_panel.rs:511` | **REMOVE** | Terminating a debug session loses nothing but the session. The user asked. |
| 12 | Workspace restart confirmation | `crates/workspace/src/workspace.rs:11072` | **REMOVE** | Restart is not destructive and is usually deliberate. |

### What stays, and why

Being against bubblewrap is not being against all confirmation. These prevent
losses that no undo can recover, so they earn their interruption:

| Surface | Where | Why it stays |
| --- | --- | --- |
| Permanent file deletion | `crates/project_panel/src/project_panel.rs:2600,2654` | Bypasses the trash. Genuinely unrecoverable. |
| Discard uncommitted git changes | `crates/git_ui/src/git_panel.rs:2027,3087` | Uncommitted work has no other copy. |
| `always_confirm` / `always_deny` tool patterns | `assets/settings/default.json` | The mechanism stays even as the *default* flips to `allow`, so `git reset --hard`, force pushes, and `.env` reads can still be gated. Removing item 2 is about the default, not about losing the ability to draw a line. |

The distinction to hold: **confirm on irreversible data loss, never on
capability.** Zed's trust modal failed this because it gated capability, and
gated it silently.

## Enforcement

A removal that a rebase can quietly undo is not a removal. Every entry that
lands becomes a numbered delta in `OMEGA_DELTAS.md` in the omega repository,
with a mechanical check in `crates/omega_deltas` and its own test naming the
upstream value it replaces.

```sh
cargo test -p omega_deltas
```

Currently enforced:

- `OMEGA-DELTA-0001` — `session.trust_all_worktrees` defaults to `true`.

Queued from this audit: items 2, 9, and 10 are pure default flips and should
become deltas in the same shape. Items 3, 4, 7, and 8 are code removals and
need a delta recording that the surface is gone, plus a check that it has not
returned.

## Test-state ownership

The trigger for this audit was a human being asked to reset an identity to
unblock a proof. That is now treated as a harness defect.

Required, in priority order:

1. **A first-run harness.** Onboarding is gated by
   `inspection.custody.state != CustodyState::Ready`
   (`crates/onboarding/src/identity_startup.rs:137`). A test must be able to
   construct a not-Ready custody state against a temporary root and assert
   that onboarding is required, and construct a Ready one and assert that it
   is not. No GUI, no human, no real Keychain.
2. **Identity reset in the harness.** `crates/omega_identity/src/custody.rs`
   already has reset paths under test
   (`reset_requires_the_expected_identity_and_verifies_deletion`). The proof
   harness should drive those rather than expecting a clean machine.
3. **Never probe the login Keychain in an unattended run.** Custody defaults to
   `SystemKeyringStore`; tests must inject a fake store. The identity proof
   matrix already does this correctly — it runs in a disposable keychain — and
   that pattern is the standard.

## Adversarial review, 2026-07-25

A distinct agent reviewed the landed work with the job of falsifying it. It is
worth recording what that caught, because none of it would have surfaced from
the producing agent re-reading its own change.

- **A false claim.** `OMEGA_DELTAS.md` stated that `ToggleWorktreeSecurity`
  still opened the trust modal on demand, so `OMEGA-DELTA-0001` removed only
  the automatic interruption. False: `can_trust`
  (`crates/project/src/trusted_worktrees.rs:469`) returns early before
  populating the `restricted` map, so `has_restricted_worktrees` is permanently
  false and the action is a silent no-op.
- **A real hole.** `handle_restrict_worktrees` called `restrict()`
  unconditionally, so a remote server running upstream Zed could push
  Restricted Mode onto the local machine regardless of the local default.
- **A weaker check than its own documentation claimed.** The registry check
  matched delta IDs as substrings and enforced only one direction.
- **The biggest miss in the first pass:** the agent tool permission default,
  now `OMEGA-DELTA-0002`.
- **Newly inventoried surfaces:** three ambient suggestion nags, a trial-end
  upsell that blocks mouse input, the dormant-but-compiled Restricted Mode UI,
  and the observation that the unsaved-buffer prompt is frequent because
  autosave is off — making autosave the better lever than deleting a prompt
  that guards real data.

All corrections landed in omega `00e16be1c9`. Tracked as omega#54 and children.

## Open, and not claimed

- Items 3–8 are proposals. No code has been removed for them.
- The remote restriction path was fixed by reading, not by running a remote
  session. It is **not** verified end to end against an SSH or WSL host.
- Block comments in `default.json` are unsupported by the delta checker. It
  fails closed rather than mis-parsing, which is the right direction, but a
  legal JSONC block comment would break `cargo test -p omega_deltas` with an
  opaque message.
