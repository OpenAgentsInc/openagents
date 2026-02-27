---
name: integration
description: Integrate applications and agent workflows with the Mezo protocol. Use when a task requires Mezo network setup (testnet/mainnet), Hardhat or Foundry configuration, Mezo Passport wallet connection, mezod/validator-kit operational guidance, contract deployment verification, or Mezo-specific dApp requirements such as BTC gas and MUSD integration expectations.
compatibility: Requires bash and curl; Node.js is needed for Hardhat/Passport workflows and Go is needed for mezod workflows.
metadata:
  oa:
    project: mezo
    nostr:
      identifier: mezo-integration
      version: "0.1.0"
      expiry_unix: "1798761600"
      capabilities_csv: "http:outbound filesystem:read"
---

# Mezo Integration

Execute this workflow when the user needs practical Mezo integration work, not generic blockchain advice.

## Workflow

1. Pick the integration target first:
- App-level EVM integration (Hardhat/Foundry + RPC + deploy flow).
- Wallet UX integration (standard EVM wallet vs Mezo Passport).
- Node/operator path (`mezod` and validator-kit).

2. Configure network and toolchain from [network-and-env](references/network-and-env.md):
- Set correct chain (`31611` testnet or `31612` mainnet).
- Apply Hardhat/Foundry configuration.
- Verify RPC health and chain id with `scripts/check-rpc.sh`.

3. Implement wallet connection path from [passport-and-wallet](references/passport-and-wallet.md):
- If app needs BTC-native + EVM wallet options, use Mezo Passport.
- If Passport is not required, use standard EVM wallet flow and manual network config.

4. Complete deployment sanity checks:
- Confirm RPC responds with expected chain id.
- Deploy contract using configured signer/provider.
- Confirm tx on correct explorer (`explorer.test.mezo.org` or `explorer.mezo.org`).

5. Apply Mezo-specific constraints before shipping:
- BTC is the gas asset.
- If user asks about Mezo Market feature readiness, enforce requirements in references (MUSD integration, audit report, mainnet functionality).

6. If task is node/validator-related, follow [mezod-and-validator-kit](references/mezod-and-validator-kit.md):
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
- [mezod-and-validator-kit](references/mezod-and-validator-kit.md): mezod prerequisites, validator-kit modes, sync/PoA operations.
