# Passport And Wallet Integration

Use this reference for wallet connectivity decisions in Mezo apps.

## Decision Rule

- Choose **Mezo Passport** when the app must support both BTC wallets (for example Xverse/Unisat) and standard EVM wallets in one UX.
- Choose **standard EVM wallet setup** when only EVM wallet flow is needed.

## Mezo Passport Install

```bash
npm install @mezo-org/passport @rainbow-me/rainbowkit wagmi viem@2.x @tanstack/react-query
```

## Mezo Passport React Setup Pattern

```tsx
import { RainbowKitProvider } from "@rainbow-me/rainbowkit";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { WagmiProvider } from "wagmi";
import { getConfig, mezoTestnet } from "@mezo-org/passport";

const queryClient = new QueryClient();

<WagmiProvider config={getConfig({ appName: "Your Mezo dApp" })}>
  <QueryClientProvider client={queryClient}>
    <RainbowKitProvider initialChain={mezoTestnet}>
      {/* app */}
    </RainbowKitProvider>
  </QueryClientProvider>
</WagmiProvider>;
```

## Standard Wallet Path

- Use wallet network config from `references/network-and-env.md`.
- Add Mezo chain manually when needed:
  - Mainnet chain id `31612`
  - Testnet chain id `31611`

## Wallet Capability Caveat

BTC wallets in Mezo can receive a Mezo-associated address and handle selected actions, but sending capabilities may be constrained depending on wallet support and current network feature rollout. Treat BTC-wallet send support as feature-gated and verify current product behavior before assuming parity with EVM wallets.
