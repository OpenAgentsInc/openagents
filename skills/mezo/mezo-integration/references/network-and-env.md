# Network And Environment

Use this reference when configuring Mezo networks and local dev toolchains.

## Canonical Network Parameters

## Testnet
- Chain ID: `31611`
- HTTPS RPC: `https://rpc.test.mezo.org`
- WSS RPC: `wss://rpc-ws.test.mezo.org`
- Explorer: `https://explorer.test.mezo.org/`
- Faucet: `https://faucet.test.mezo.org/`

## Mainnet
- Chain ID: `31612`
- Explorer: `https://explorer.mezo.org/`
- Public RPC providers:
  - `https://rpc-http.mezo.boar.network`
  - `https://rpc_evm-mezo.imperator.co`
  - `https://mainnet.mezo.public.validationcloud.io`
  - `https://mezo.drpc.org`

## Hardhat Configuration Pattern

```js
module.exports = {
  defaultNetwork: "mezotestnet",
  networks: {
    mezotestnet: {
      url: "https://rpc.test.mezo.org",
      chainId: 31611,
      accounts: ["YOUR_PRIVATE_WALLET_KEY"]
    }
  },
  solidity: {
    version: "0.8.28",
    settings: { evmVersion: "london", optimizer: { enabled: true, runs: 200 } }
  }
};
```

Swap to mainnet by setting `defaultNetwork`, `url`, and `chainId=31612`.

## Foundry Configuration Pattern

```toml
[profile.default]
chain_id = 31611
eth_rpc_url = "https://rpc.test.mezo.org"
evm_version = "london"
```

Swap to mainnet with `chain_id = 31612` and a chosen mainnet RPC endpoint.

## Deployment Verification Checklist

1. Verify RPC chain id before deploy:
```bash
scripts/check-rpc.sh <rpc_url> <expected_chain_id>
```
2. Confirm signer wallet network.
3. Deploy contract.
4. Confirm tx + contract on correct explorer.
5. Validate contract interactions on the same network.

## Mezo-Specific Constraints

- Gas asset is BTC (18 decimals).
- Wallet/user setup may require manual chain add if Chainlist path is unavailable.
- Mezo Market feature readiness requires:
  - MUSD integration
  - third-party audit report
  - mainnet-functional deployment
