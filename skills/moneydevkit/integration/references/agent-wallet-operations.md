# Agent Wallet Operations

Use this path when an AI agent needs self-custodial Lightning send/receive without an MDK API account.

## Preconditions

- Node.js 20+
- `npx` available
- Persistent filesystem access for wallet state (`~/.mdk-wallet/`)

## One-Time Initialization

```bash
npx @moneydevkit/agent-wallet@latest init --network signet
```

Important:
- Save mnemonic securely.
- `init` does not overwrite existing wallets.
- Network choice is fixed after init unless wallet is reinitialized.

## Operational Commands

```bash
npx @moneydevkit/agent-wallet@latest status
npx @moneydevkit/agent-wallet@latest balance
npx @moneydevkit/agent-wallet@latest receive 1000
npx @moneydevkit/agent-wallet@latest send user@getalby.com 500
npx @moneydevkit/agent-wallet@latest payments
```

## Behavior Notes

- Commands emit JSON to stdout and non-zero exit on failure.
- The daemon auto-starts on first command and runs on localhost.
- Incoming payment polling is handled by the daemon (no webhook setup required).
- Destination auto-detection supports Bolt11/Bolt12/LNURL/Lightning address.

## Recovery / Reinit

```bash
npx @moneydevkit/agent-wallet@latest stop
rm -rf ~/.mdk-wallet
npx @moneydevkit/agent-wallet@latest init --network signet
```

Only reinitialize after backing up mnemonic and understanding funds impact.
