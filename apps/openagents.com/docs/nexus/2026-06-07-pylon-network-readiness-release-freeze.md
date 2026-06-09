# Pylon Network Readiness Release Freeze

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`
Related issues: #499, #500, #501, #502, #503, #504, #505

## Decision

State: `limited_launcher_release_shipped`

`@openagentsinc/pylon@0.2.5` is now published and npm `latest` points at
`0.2.5`. This closed the release freeze for a limited downloadable launcher
release after #500 through #505 proved install, registration, heartbeat,
wallet readiness, assignment, accepted-work proof, bitcoin payout, public
receipts, multi-host smokes, and rollback documentation.

This does not mean every broader claim is live. Native Windows, WSL Ubuntu,
hosted MDK direct programmatic payouts, unrestricted earning, and autonomous
Artanis production operation remain blocked until separately proven.

This document remains as the historical freeze checklist and rollback guardrail
for future releases. Future Pylon GitHub binary releases, npm package releases,
npm `latest` promotions, and broad earning announcements should reuse this
checklist before widening claims.

## Canonical Release-Unfreeze Checklist

The next Pylon release can be considered only after all of these are proven by
retained public-safe evidence:

1. Fresh install: a new operator can install and run the Pylon launcher without
   relying on a local source checkout.
2. Stable identity: first run creates or reuses a stable Pylon identity.
3. Registration: the Pylon registers through the OpenAgents product surface Pylon Agent API or an
   equivalent scoped onboarding path.
4. Heartbeat: the Pylon posts heartbeat, capability, and stale/offline status
   transitions that OpenAgents product surface can inspect.
5. Public-safe visibility: public stats and operator surfaces show online,
   offline, stale, blocked, and not-ready states without private host details.
6. Wallet and payout target: the Pylon can set up an MDK-backed bitcoin receive
   path, submit redacted payout readiness, and receive operator approval.
7. Job assignment: OpenAgents product surface can offer a real assignment to an eligible Pylon.
8. Job execution: the Pylon accepts the lease and executes the bounded job.
9. Proof upload: the Pylon submits artifact/proof refs without private paths,
   prompts, outputs, wallet data, or credentials.
10. Accepted-work closeout: OpenAgents product surface can mark the work accepted or rejected from
    retained evidence.
11. Bitcoin payout: accepted work can dispatch a real small bitcoin payout
    through approved payment authority.
12. Receipt projection: the public receipt API/page shows terminal
    public-safe payment state without raw invoice, payment hash, preimage,
    mnemonic, exact balance, private payout target, customer data, or provider
    secret material.
13. Repeated use: at least two distinct Pylons complete paid jobs through the
    normal network path, not manual database insertion.
14. Host breadth: local macOS and at least one reachable second host pass the
    clean install/registration/job/payment path; Linux, WSL Ubuntu, and native
    Windows are either proven or retained as explicit release blockers.
15. Failure drills: offline Pylon, stale heartbeat, stale wallet readiness,
    invalid proof, duplicate assignment, duplicate payout, adapter pause, and
    bad public projection all produce the expected blocked, retryable, or
    failed states.
16. Rollback posture: npm latest, GitHub release metadata, public copy, Forum
    posts, receipt projection, and scheduler/tick mistakes all have retained
    rollback instructions and at least critical-path drill evidence.

## Issue Sequence

| Issue | Required outcome |
| --- | --- |
| #499 | Freeze new releases and define this checklist. |
| #500 | Proved source-level self-serve install, registration, heartbeat, and status visibility on one clean local host and one reachable Arch Linux host; Linux resolved to `pylon-v0.2.2`, so platform asset alignment still belongs to #505. |
| #501 | Proved source-level MDK agent-wallet setup, redacted wallet readiness, and payout-target admission for a registered Pylon. |
| #502 | Proved live OpenAgents product surface assignment lease, owned Pylon assignment list, execution progress, artifact/proof refs, accepted-work closeout, and post-closeout public-safe payment-evidence refs for `pylon.issue502.local.20260608024927`. |
| #503 | Settle accepted Pylon work with real bitcoin payouts and public receipts. |
| #504 | Run repeated multi-Pylon, multi-host network smokes and failure drills. |
| #505 | Published `@openagentsinc/pylon@0.2.5`, moved npm `latest`, verified package flags, ran macOS and Arch Linux package-launcher registration and wallet-readiness smokes, and documented rollback. |

## Allowed Current Copy

```text
@openagentsinc/pylon@latest is a downloadable launcher at 0.2.5. The launcher
supports OpenAgents registration and MDK wallet-readiness reporting. macOS
arm64 and Linux x86_64 package-launcher smokes are proven, and two distinct
Pylons have accepted-work bitcoin receipts. Native Windows, WSL Ubuntu, hosted
MDK direct programmatic payouts, unrestricted earning, and autonomous Artanis
production operation are still not public-ready claims.
```

## Disallowed Current Copy

```text
Download Pylon now and you will earn bitcoin.
Pylon v0.2 is ready for everyone.
The Pylon network is unrestricted.
Artanis is autonomously assigning paid Pylon jobs.
Native Windows and WSL Ubuntu are fully proven.
```

## Guardrails

- Do not delete existing releases as part of the freeze.
- Do not publish a new release to prove the network.
- Do not move npm `latest` until #505 explicitly permits it.
- Do not treat payment proof as a substitute for accepted-work proof.
- Do not treat clean launcher smoke as a substitute for network readiness.
- Do not treat a Forum post as release, payment, settlement, or scheduler
  authority.
