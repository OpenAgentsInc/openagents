# RX-5 Reactor Install And Air-Gap Update Runbook

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04

Issue: [#8275](https://github.com/OpenAgentsInc/openagents/issues/8275)

## Scope

This is the fleet-executable Reactor install and operations runbook for the
RX-3 fixture node profile. It proves the runbook shape, signed bundle
verification path, policy revalidation, and receipt discipline. It does not
claim a production Hydralisk deployment, customer premises install, compliance
readiness, or public availability.

## Prerequisites

- A clean Linux host or container with Bun available.
- A current clean `OpenAgentsInc/openagents` checkout.
- No production signing secret is required for the smoke. Production bundle
  signing uses the existing OpenAgents release/provenance key described in
  `apps/oa-updates/docs/release-signing-runbook.md`.
- The pinned production public key remains
  `apps/oa-updates/keys/release-pubkey.json`.
- The fail-closed verifier remains
  `apps/oa-updates/scripts/verify-release.ts`.

## Hardware Tier Guidance

The guidance lives in `REACTOR_HARDWARE_TIER_SPECS` and is guidance only, not a
purchase commitment.

| Tier | Guidance |
| --- | --- |
| `workstation` | 16+ modern cores, 128 GB memory, 2 TB NVMe, 1 GbE management with 10 GbE preferred for corpus ingest. |
| `server` | 32+ server cores, 256-512 GB ECC memory, 4-8 TB NVMe, 10 GbE minimum. |
| `rack` | Rack platform sized by GPU count and thermal envelope, 512 GB+ ECC memory, 8 TB+ NVMe, 25 GbE+ fabric. |

## Fresh Install

1. Start from a clean checkout and install dependencies:

   ```sh
   bun install --frozen-lockfile
   ```

2. Run the scripted install smoke:

   ```sh
   bun run --cwd packages/reactor-contracts smoke:install
   ```

3. The smoke creates a clean temporary Reactor node directory, generates a test
   ed25519 keypair, signs a fixture air-gap bundle, verifies the bundle through
   `apps/oa-updates/scripts/verify-release.ts`, confirms a tampered bundle is
   rejected, and writes these receipts under the smoke temp directory:

   - `receipts/bundle.json`
   - `receipts/freshInstall.json`
   - `receipts/upgrade.json`
   - `receipts/rollback.json`

4. A valid fresh install receipt has:

   - `schemaVersion: "openagents.reactor.install_ops_receipt.v1"`
   - `action: "fresh_install"`
   - `status: "succeeded"`
   - verification refs naming the verifier, public key, signature, and policy
     decision
   - no blocker refs

## Signed Air-Gap Bundle Path

Production bundle signing reuses the existing release/provenance machinery:

```sh
bun apps/oa-updates/scripts/sign-release.ts <bundle> > <bundle>.sig.json
bun apps/oa-updates/scripts/verify-release.ts <bundle> <bundle>.sig.json
```

The Reactor bundle manifest is
`openagents.reactor.airgap_update_bundle_manifest.v1`. It must name:

- bundle ref and bundle version
- model ref and RX-3 node profile ref
- policy ref and policy version
- artifact sha256
- signature ref, ed25519 `kid`, verifier ref, and public-key ref
- `callbackRequired: false`

Air-gapped nodes verify the bundle locally before install. They do not need a
callback to OpenAgents infrastructure in the serving or install path.

## Upgrade

1. Transfer the bundle, manifest, and `.sig.json` into the node's offline bundle
   staging directory.
2. Run the verifier against the bytes on disk.
3. Re-run `reactor.model_policy.v1` against the catalog entry and node profile.
4. Refuse before model refresh when the policy no longer admits the model.
5. Record an `openagents.reactor.install_ops_receipt.v1` with
   `action: "upgrade"`.

The RX-5 tests include a Qwen refresh fixture that is refused by the US-only
policy, proving model refresh does not bypass policy revalidation.

## Rollback

1. Keep the previous verified bundle and receipt refs in the node's rollback
   slot.
2. Verify the rollback target bundle with the same ed25519 verifier.
3. Revalidate the rollback target model against the active policy.
4. Record an install-ops receipt with `action: "rollback"`,
   `rollbackFromBundleRef`, and `rollbackToBundleRef`.

Rollback is an operational receipt, not authority to weaken policy or install a
nonconforming model.

## Verification

Commands:

```sh
bun run --cwd packages/reactor-contracts test
bun run --cwd packages/reactor-contracts typecheck
bun run --cwd packages/reactor-contracts smoke:install
```

Source coverage:

- `packages/reactor-contracts/src/index.ts`
- `packages/reactor-contracts/src/index.test.ts`
- `packages/reactor-contracts/scripts/install-smoke.ts`
- `apps/oa-updates/scripts/verify-release.ts`
- `apps/oa-updates/keys/release-pubkey.json`
