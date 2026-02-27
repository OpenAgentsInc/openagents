---
name: mezo
description: Mezo integration workflows for apps, autonomous agents, and Mezo Earn operations.
metadata:
  oa:
    project: mezo
    identifier: mezo
    version: "0.2.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
---

# Mezo

## Overview

Integrate applications and agent workflows with the Mezo protocol. Use this skill when a task requires Mezo network setup (testnet/mainnet), Hardhat or Foundry configuration, Mezo Passport wallet connection, mezod/validator-kit operations, Mezo Earn automation (lock/vote/claim/poke/incentives), contract deployment verification, or Mezo-specific dApp requirements such as BTC gas and MUSD integration expectations.

## Environment

- Requires `bash` and `curl`.
- Node.js is needed for Hardhat/Passport workflows.
- Go is needed for `mezod` workflows.

Execute this workflow when the user needs practical Mezo integration work, not generic blockchain advice.

## Workflow

1. Pick the integration target first:
- App-level EVM integration (Hardhat/Foundry + RPC + deploy flow).
- Wallet UX integration (standard EVM wallet vs Mezo Passport).
- Node/operator path (`mezod` and validator-kit).
- Mezo Earn operations (veBTC lifecycle, gauge voting, rewards claims, and incentive posting).

2. Configure network and toolchain from [network-and-env](references/network-and-env.md):
- Set correct chain (`31611` testnet or `31612` mainnet).
- Apply Hardhat/Foundry configuration.
- Verify RPC health and chain id with `scripts/check-rpc.sh`.

3. Implement wallet connection path from [passport-and-wallet](references/passport-and-wallet.md):
- If app needs BTC-native + EVM wallet options, use Mezo Passport.
- If Passport is not required, use standard EVM wallet flow and manual network config.

4. If task is Mezo Earn-related, follow [mezo-earn-automation](references/mezo-earn-automation.md):
- Use canonical mainnet/testnet contracts from Mezo docs + tigris deployments.
- Build an epoch-aware automation loop around `vote`, `poke`, and claims.
- Apply safety limits for lock updates, votes, and incentive posting.

5. Complete deployment sanity checks:
- Confirm RPC responds with expected chain id.
- Deploy contract using configured signer/provider.
- Confirm tx on correct explorer (`explorer.test.mezo.org` or `explorer.mezo.org`).

6. Apply Mezo-specific constraints before shipping:
- BTC is the gas asset.
- If user asks about Mezo Market feature readiness, enforce requirements in references (MUSD integration, audit report, mainnet functionality).

7. If task is node/validator-related, follow [mezod-and-validator-kit](references/mezod-and-validator-kit.md):
- Choose deployment mode (docker/native/helm/manual).
- Follow sync and operational requirements.
- Include PoA submission command only when validator onboarding is requested.

## Quick Commands

```bash
# Testnet RPC health + chain id check
scripts/check-rpc.sh https://rpc.test.mezo.org 31611

# Mainnet provider check
scripts/check-rpc.sh https://rpc-http.mezo.boar.network 31612
```

## Reference Files

- [network-and-env](references/network-and-env.md): chain params, RPC endpoints, Hardhat/Foundry config, deployment verification.
- [passport-and-wallet](references/passport-and-wallet.md): Mezo Passport setup and wallet path decisioning.
- [mezo-earn-automation](references/mezo-earn-automation.md): contract map, ABI methods, and agent automation loop for Mezo Earn.
- [mezod-and-validator-kit](references/mezod-and-validator-kit.md): mezod prerequisites, validator-kit modes, sync/PoA operations.
