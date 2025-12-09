# Commander

Desktop app for selling your spare compute and training AI agents. Run one command, get a bitcoin wallet and nostr identity, start earning.

## Product Decisions

### Distribution
- Desktop app (macOS first, then Linux/Windows)
- Single shell command to install and run
- No browser, no Electron—native performance

### Identity
- Creates a nostr keypair on first run
- Your npub is your identity across the network
- No email, no password, no account creation

### Wallet
- Built-in self-custodial bitcoin wallet
- Lightning Network for instant payments
- Spark for agent-to-agent microtransactions
- Your keys, your bitcoin

### Compute
- "Go Online" button to sell spare compute
- Swarm network matches jobs to available devices
- Paid per inference job in sats

### Agents
- MechaCoder is your first agent
- Train in the GYM to improve skills
- Publish to Agent Store
- Agents use swarm compute when they run
