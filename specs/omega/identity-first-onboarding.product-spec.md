---
spec_format_version: "0.1"
title: "Omega Identity-First Onboarding"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-24T00:00:00Z"
updated_at: "2026-07-24T00:00:00Z"
linked_github_repo: "OpenAgentsInc/omega"
applies_to:
  - component: "omega-identity"
  - component: "omega-onboarding"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "rollout"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
tool_metadata:
  openagents_plan: "docs/omega/2026-07-23-identity-first-onboarding-roadmap.md"
  openagents_status: "proposed; implementation exists; candidate admission and release remain pending"
---

## Problem

Omega inherits an editor setup that can open before the user has a durable
local identity. The inherited flow does not define secret custody, recovery,
identity loss, or startup intent release. It also does not separate a
sovereign person identity from a device identity or a hosted account.

Omega needs one identity-first journey. The journey must create or recover a
local person identity before normal editor use. It must keep editor setup
available without changing the proven Theme and Agent Setup controls.

## Hypothesis

If Omega makes an explicit local identity action the first onboarding task,
users can start with durable signing identity and clear custody facts. If Omega
keeps editor setup in the same native structure, it can add this value without
changing familiar theme and agent controls.

## Scope

In scope: one Nostr-only person identity, operating-system secret storage,
public identity records, encrypted recovery, explicit reset, identity-first
startup routing, the native Omega onboarding section, and Editor Onboarding
replay.

Out of scope: wallets, relay publication, Nostr command authority, cloud
custody, cloud recovery, hosted account ownership, raw secret export, device
pairing, and a general editor setup redesign.

Cut: startup key generation, a plain-text secret fallback, a normal interface
that returns a secret, a replacement identity after loss, one key that owns
all identity roles, a network requirement, and a Zed credential or data root.

## User Experience

On first start, Omega shows its identity section before Theme. The page tells
the user that the public identity is safe to share. It also tells the user
that the signing secret has no password-reset path.

The user selects Create my identity or Use an existing identity. Create makes
one local Nostr identity after the action. Recovery accepts an encrypted
recovery artifact. An advanced flow can accept a masked Nostr secret.

After success, Omega shows the public `npub`, a short fingerprint, and recovery
status. The user can make an encrypted recovery file or finish with a visible
Recovery needed state. Editor Onboarding remains available for later use.

## Solution

Omega uses profile `openagents.omega.nostr_only.v1`. This profile creates no
wallet material. The device-local identity, sovereign person identity, and
hosted owner identity remain separate. A Nostr public key does not become an
OpenAuth owner ID.

The custody service stores the person secret in the operating-system secret
store. The keyring account is `omega-sovereign-identity-v1`. The service names
are `com.openagents.omega.credentials.dev`,
`com.openagents.omega.credentials.nightly`,
`com.openagents.omega.credentials.rc`, and
`com.openagents.omega.credentials` for the matching application channels.
RC and stable do not share a secret slot.

The normal application interface receives only public identity facts, custody
state, recovery state, and public receipt references. A custody-only interface
accepts an import secret and makes an encrypted recovery artifact. The normal
application interface never receives a mnemonic, an `nsec`, a seed, a raw
private key, or a wallet secret.

Creation and recovery use a durable transaction. Success requires a secret
write, a secret read-back, a matching public key, an atomic public manifest,
and a completion record for the same identity. The service serializes
mutations and resumes an incomplete transaction without a new key.

The user can defer recovery protection. Omega must keep Recovery needed
visible until a verified encrypted recovery file exists. Reset is the only
in-product deletion action. It requires the expected public identity, deletes
the managed secret and public records, verifies deletion, and requires a new
process before new identity creation. Uninstall is not an identity deletion
promise.

## Acceptance Criteria

- **OMEGA-AC-01:** Omega keeps the device-local identity, sovereign person
  identity, and hosted owner identity as three roles with separate authority.
- **OMEGA-AC-02:** Create makes one Nostr-only identity only after an explicit
  user action, without a wallet, network request, relay event, or profile
  publication.
- **OMEGA-AC-03:** The operating-system secret store uses the exact
  channel-specific service and account locators, and the normal application
  interface cannot read or return secret key material.
- **OMEGA-AC-04:** Create and recovery serialize mutations, verify the stored
  secret by read-back, commit matching public records atomically, and resume
  incomplete work without an identity rotation.
- **OMEGA-AC-05:** Locked, unavailable, lost, conflict, incomplete,
  reset-failed, relaunch-required, absent, and ready states stay distinct and
  deny signing when custody is not ready.
- **OMEGA-AC-06:** Recovery uses a verified encrypted artifact by default,
  keeps advanced secret import masked, requires explicit conflict selection,
  and keeps Recovery needed visible when the user defers protection.
- **OMEGA-AC-07:** Reset verifies the expected identity and deletion, keeps a
  durable failure state, and requires a new process before identity creation.
- **OMEGA-AC-08:** First-run onboarding puts the Omega identity section before
  the unchanged Theme section and preserves the current registry Agent Setup
  structure and behavior.
- **OMEGA-AC-09:** One startup coordinator inspects identity before all launch
  intents, opens at most one onboarding surface, and releases each saved
  intent only after durable identity completion.
- **OMEGA-AC-10:** Identity completion and Editor Onboarding completion use
  independent versioned records, and Editor Onboarding remains available for
  replay.
- **OMEGA-AC-11:** Omega uses isolated application, data, cache, log, protocol,
  and credential identities and does not read or change Zed state during the
  identity journey.
- **OMEGA-AC-12:** The installed candidate completes create, restart,
  recovery, reset, offline, accessibility, update, rollback, uninstall, and
  reinstall checks without secret disclosure or an unsupported Zed label.

## Success Metrics

```productspec-success-metrics
- id: omega_identity_first_run_completion
  metric: admitted_installed_candidate_completes_identity_first_run
  target: "= true"
  window: each release candidate
  segment: supported Omega desktop targets
  source: admitted_identity_assurance_receipt
- id: omega_identity_secret_disclosure
  metric: detected_secret_disclosures
  target: "= 0"
  window: each release candidate
  segment: logs telemetry clipboard UI diagnostics and crash output
  source: admitted_identity_privacy_receipt
```

## Rollout

The source implementation and its automated tests can land before candidate
admission. A release candidate cannot use this ProductSpec as proof. The
candidate must pass the companion AssuranceSpec with exact artifact and
environment bindings.

## custom-owner-gates

The owner must accept this ProductSpec revision and the user journey. The
owner must observe the installed identity-first journey on an exact candidate.
The owner must make a separate release decision after independent
verification. This proposal does not record any of these decisions.

## custom-receipts

Required receipt classes are an exact source-test receipt, a package identity
receipt, a manual custody and lifecycle receipt, an accessibility receipt, a
privacy tripwire receipt, an owner-observation receipt, and an independent
verification receipt. A source test result is not an installed-candidate
receipt.
