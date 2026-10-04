# boltz-client

Boltz client for cross-chain swaps.

Reverse-only swaps from Lightning (sats) to a stablecoin (USDT or USDC) via [boltz.exchange](https://boltz.exchange). Uses a two-hop architecture:

```
Lightning -> tBTC (Boltz reverse swap) -> stablecoin (DEX swap on Arbitrum) -> destination
```

A Router contract makes claim + DEX atomic — one Alchemy-sponsored EVM transaction claims tBTC from the ERC20Swap contract, executes the DEX swap on Arbitrum, and bundles cross-chain delivery in the same call. How the stablecoin reaches the user depends on the destination's bridge:

- **Direct** — delivered on Arbitrum (USDT, or USDC-on-Arbitrum); no cross-chain hop.
- **OFT** — LayerZero USDT0 bridge (native or legacy mesh) to other EVM chains.
- **CCTP** — Circle CCTP v2 burn + mint (USDC; EVM chains + Solana).

OFT/CCTP swaps complete only once cross-chain delivery is confirmed (CCTP via Circle Iris, OFT via LayerZero Scan); Direct swaps complete immediately.

## Key Design Choices

| Decision | Choice |
|----------|--------|
| EVM keys | SDK-managed, derived from seed via BIP-32 |
| Gas | EIP-7702 via a configurable gas-sponsor URL (wraps Alchemy) — users don't need ETH |
| ABI + signing | alloy-sol-types + k256 (WASM-compatible) |
| Swap status | WebSocket (real-time updates) |

## Building

```bash
make build          # Build workspace
make check          # Run all checks (fmt, clippy, tests)
```

## Testing

```bash
make test           # Unit tests
make itest          # Integration tests (requires Docker)
```

## External References

- [Boltz API docs](https://api.docs.boltz.exchange)
- [Boltz web app source](https://github.com/BoltzExchange/boltz-web-app)
- [Boltz regtest environment](https://github.com/BoltzExchange/regtest)
- [boltz-core contracts](https://github.com/BoltzExchange/boltz-core)

## License

[MIT](LICENSE)
