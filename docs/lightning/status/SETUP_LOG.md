# Lightning Agent Tools — Setup Log

Log of following the [Lightning Labs instructions](https://lightning.engineering/posts/2026-02-11-ln-agent-tools/) to clone the repo, install lnd and lnget skills, set up a Lightning node with remote signer, bake a pay-only macaroon, configure lnget, and (optionally) install Aperture for selling. The clone lives in `~/code/lightning-agent-tools` (outside this repo). This doc lives in the OpenAgents repo under `docs/lightning/status/` for reference.

**Date:** 2026-02-11

---

## 1. What I Did

### Clone

- Cloned `https://github.com/lightninglabs/lightning-agent-tools.git` into `~/code/lightning-agent-tools`.
- Clone completed successfully.

### Install lnd skill

- Ran: `cd ~/code/lightning-agent-tools && skills/lnd/scripts/install.sh`
- **Result:** Failed. Default install pulls the Lightning Terminal (litd) Docker image; Docker daemon was not running (`Cannot connect to the Docker daemon at unix:///.../docker.sock`).
- **Alternative:** Run `skills/lnd/scripts/install.sh --source` to build lnd + lncli from source (requires Go 1.21+). No Docker required for install; running the node still typically uses Docker via `start-lnd.sh` unless you use `--native`.

### Install lnget skill

- Ran: `cd ~/code/lightning-agent-tools && skills/lnget/scripts/install.sh`
- **Result:** Failed. Script runs `go install github.com/lightninglabs/lnget/cmd/lnget@latest`. The lnget module requires Go 1.25+ and uses `replace` directives in go.mod, so `go install ...@latest` from outside the module fails with: *"The go.mod file for the module providing named packages contains one or more replace directives..."*.
- **Alternatives:**
  - Build lnget from source inside a clone of the [lnget repo](https://github.com/lightninglabs/lnget): `git clone https://github.com/lightninglabs/lnget.git && cd lnget && make install` (ensure Go 1.25+).
  - Or use a prebuilt binary if Lightning Labs publish one; check the lnget releases.

### Install lightning-security-module (remote signer)

- Did not run (depends on Docker for default path). Same as lnd: `skills/lightning-security-module/scripts/install.sh` pulls `lightninglabs/lnd:v0.20.0-beta`. Use `--source` to build lnd for the signer without Docker.

### Macaroon bakery / Aperture / commerce

- Did not run (depend on a running lnd node and, for Aperture, Go install). Documented the exact commands below from the skill docs.

---

## 2. Exact Steps to Run (When Docker Is Available)

Use these steps when the Docker daemon is running. All paths and script names are relative to `~/code/lightning-agent-tools`.

### 2.1 Install lnd and lnget skills

```bash
cd ~/code/lightning-agent-tools

# Pull litd Docker image (default)
skills/lnd/scripts/install.sh

# Install lnget: if go install fails, build from lnget repo with Go 1.25+
skills/lnget/scripts/install.sh
```

### 2.2 Set up Lightning node with remote signer (key isolation)

**Two-container local setup (both on same machine):**

```bash
# Start litd (watch-only) + signer containers
skills/lnd/scripts/start-lnd.sh --watchonly

# Set up signer wallet and export credentials (first run only)
skills/lightning-security-module/scripts/setup-signer.sh --container litd-signer

# Import credentials into watch-only node and create wallet
skills/lnd/scripts/import-credentials.sh --bundle ~/.lnget/signer/credentials-bundle
skills/lnd/scripts/create-wallet.sh --container litd

# Verify
skills/lnd/scripts/lncli.sh getinfo
```

**Two-machine setup (signer on a separate machine):**
On the signer machine: `install.sh`, `start-signer.sh`, `setup-signer.sh`, then copy the credentials bundle to the agent machine. On the agent machine: `import-credentials.sh`, `start-lnd.sh --watchonly`, `create-wallet.sh`.

### 2.3 Bake a pay-only macaroon (scope spending permissions)

```bash
# Bake pay-only macaroon (auto-detects litd container)
skills/macaroon-bakery/scripts/bake.sh --role pay-only

# Optional: save to a specific path for lnget/config
skills/macaroon-bakery/scripts/bake.sh --role pay-only --save-to ~/.lnget/pay-only.macaroon
```

Use this macaroon (and not `admin.macaroon`) for the agent so it can only pay invoices, not create them or manage channels.

### 2.4 Configure lnget and pay for L402 APIs

```bash
# Initialize config (auto-detects local lnd; edit ~/.lnget/config.yaml if paths differ)
lnget config init

# Point lnget at pay-only macaroon: in ~/.lnget/config.yaml set ln.lnd.macaroon to the pay-only macaroon path.

# Check Lightning backend
lnget ln status

# Fetch an L402-gated URL (cap spend per request)
lnget --max-cost 1000 https://api.example.com/paid-data.json
```

Config file is `~/.lnget/config.yaml`. The skill docs note that `lnget config init` may generate wrong YAML keys (`tlscertpath`/`macaroonpath` vs `tls_cert`/`macaroon`); verify against the example in `skills/lnget/SKILL.md`.

### 2.5 (Optional) Install Aperture and set up a paid endpoint

```bash
# Install Aperture (Go)
skills/aperture/scripts/install.sh

# Generate config (connects to local lnd)
skills/aperture/scripts/setup.sh

# Ensure invoice-only macaroon exists for Aperture
skills/macaroon-bakery/scripts/bake.sh --role invoice-only \
  --save-to ~/.lnd/data/chain/bitcoin/mainnet/invoice.macaroon

# Start Aperture
skills/aperture/scripts/start.sh

# Test with lnget (insecure for local dev)
lnget -k --no-pay https://localhost:8081/api/test
```

Aperture sits in front of your backend; it returns 402 with an L402 challenge, then proxies after the client pays.

---

## 3. What I Learned

### Repo layout

- **`skills/`** — One directory per skill: `lnd`, `lnget`, `lightning-security-module`, `macaroon-bakery`, `aperture`, `lightning-mcp-server`, `commerce`. Each has `SKILL.md` and `scripts/` (e.g. `install.sh`, `start-*.sh`, `bake.sh`).
- **`docs/`** — Architecture, security, L402/lnget, MCP server, commerce, two-agent setup, quickref.
- **`lightning-mcp-server/`** — Go MCP server (read-only node access via LNC). Can be run via `npx -y @lightninglabs/lightning-mcp-server` without cloning.
- **`versions.env`** — Pinned Docker image tags (litd, lnd, aperture, bitcoin-core).

### Remote signer flow

- **Watch-only node (litd):** Runs Neutrino, manages channels, routes payments. No private keys; it talks to the signer over gRPC for any signing.
- **Signer node (lnd):** Holds the seed and keys; only signs. No P2P, no channels. Exports a credentials bundle: `accounts.json` (xpubs), `tls.cert`, `admin.macaroon`. The watch-only node imports this and creates a wallet from the xpubs.
- **Security:** Compromise of the agent machine does not expose keys; the signer can be on a separate, locked-down machine.

### Macaroon roles

- **pay-only** — Pay invoices, decode payreqs, get node info. No invoice creation, no channel ops.
- **invoice-only** — Create/lookup invoices, get info. No paying, no channel ops. Use for Aperture/seller.
- **read-only** — Balances, channels, peers, payments (observe only).
- **channel-admin** — Read-only + open/close channels, connect peers.
- **signer-only** — For the remote signer’s RPC; only signing/derivation. Use instead of admin macaroon on the signer in production.

### L402 flow (lnget)

1. Request hits a URL; server responds 402 with `WWW-Authenticate: L402 macaroon="...", invoice="..."`.
2. lnget parses the challenge, checks amount against `--max-cost`, pays the invoice via lnd (or LNC/Neutrino).
3. Stores token (macaroon + preimage) under `~/.lnget/tokens/<domain>/`.
4. Retries request with `Authorization: L402 <macaroon>:<preimage>`.
5. Subsequent requests to the same domain reuse the cached token.

### Commerce loop

- **Buyer:** lnd (funded) + lnget. Run `lnget --max-cost N <url>` to buy from L402-gated APIs.
- **Seller:** lnd (with invoice capability) + Aperture in front of backend. Aperture issues invoices and validates L402 tokens; backend stays payment-agnostic.
- **docs/commerce.md** walks through funding, channels, and running both sides.

### Prerequisites (from repo)

- **Docker** — Default for lnd/litd and signer containers.
- **Go** — For lnget, Aperture, MCP server from source. lnget currently requires Go 1.25+ and has go.mod replace directives (so `go install` from outside fails).
- **jq** — Used by several scripts.

---

## 4. References

- Repo: [github.com/lightninglabs/lightning-agent-tools](https://github.com/lightninglabs/lightning-agent-tools)
- Article: [The Agents Are Here and They Want to Transact](https://lightning.engineering/posts/2026-02-11-ln-agent-tools/)
- In this repo: [LIGHTNING_AGENT_TOOLS.md](../reference/LIGHTNING_AGENT_TOOLS.md) (integration plan), [GLOSSARY.md](../../GLOSSARY.md) (L402, lnget, Aperture, etc.)

---

## 5. Retry with Docker Running (2026-02-11)

Docker was started and the setup was retried from `~/code/lightning-agent-tools`.

### 5.1 Install lnd (litd)

- **Command:** `skills/lnd/scripts/install.sh`
- **Result:** Success. Pulled `lightninglabs/lightning-terminal:v0.16.0-alpha`. Verified with `litd version 0.16.0-alpha`.

### 5.2 Install lightning-security-module (signer)

- **Command:** `skills/lightning-security-module/scripts/install.sh`
- **Result:** Success. Pulled `lightninglabs/lnd:v0.20.0-beta` for the signer container.

### 5.3 Install lnget

- **Command:** `skills/lnget/scripts/install.sh`
- **Result:** Failed again. Same error: lnget module requires Go 1.25+ and has `replace` directives in go.mod, so `go install ...@latest` cannot be used from outside the module. Build from a clone of the [lnget repo](https://github.com/lightninglabs/lnget) with Go 1.25+ and `make install` to get the binary.

### 5.4 Start litd + signer (watch-only)

- **Command:** `skills/lnd/scripts/start-lnd.sh --watchonly`
- **Result:** Success. Created network `templates_litd-watchonly`, volumes for signer and litd data, started containers `litd-signer` and `litd`. Both running in background.

### 5.5 Set up signer wallet and export credentials

- **Command:** `skills/lightning-security-module/scripts/setup-signer.sh --container litd-signer`
- **Result:** Success. Generated passphrase (saved to `~/.lnget/signer/wallet-password.txt`), created wallet seed (saved to `~/.lnget/signer/seed.txt`), waited for signer RPC (took ~30 retries as signer was still starting). Exported credentials bundle to `~/.lnget/signer/credentials-bundle/` (accounts.json, tls.cert, admin.macaroon) and created portable base64 bundle at `~/.lnget/signer/credentials-bundle.tar.gz.b64`.

### 5.6 Import credentials and create watch-only wallet

- **Command:** `skills/lnd/scripts/import-credentials.sh --bundle ~/.lnget/signer/credentials-bundle`
- **Result:** Success. Credentials copied into container `litd`.

- **Command:** `skills/lnd/scripts/create-wallet.sh`
- **Result:** Failed at first with: `line 251: /../../lib/rest.sh: No such file or directory`. The script uses `source "$SCRIPT_DIR/../../lib/rest.sh"` but **`create-wallet.sh` never sets `SCRIPT_DIR`** (bug in upstream). Workaround: run with `SCRIPT_DIR` set:
  - **Command:** `SCRIPT_DIR="$(cd skills/lnd/scripts && pwd)" skills/lnd/scripts/create-wallet.sh`
- **Result:** Success. Generated litd passphrase, imported 259 accounts from signer, watch-only wallet created. No seed on this machine.

### 5.7 Verify node

- **Command:** `skills/lnd/scripts/lncli.sh getinfo`
- **Result:** Success. Node identity, testnet, block_height 686000, synced_to_chain false (Neutrino still syncing). No channels or peers yet.

### 5.8 Bake pay-only macaroon

- **Command:** `skills/macaroon-bakery/scripts/bake.sh --role pay-only` (without `--container`)
- **Result:** Failed. Script requires `lncli` on host PATH; we only have lncli inside the litd container.

- **Command:** `skills/macaroon-bakery/scripts/bake.sh --role pay-only --container litd --save-to ~/.lnget/pay-only.macaroon`
- **Result:** Success. Macaroon saved to `~/.lnget/pay-only.macaroon` (mode 0600). Permissions: SendPaymentSync, SendPaymentV2, DecodePayReq, GetInfo, GetVersion. Ready for lnget config (point `ln.lnd.macaroon` at this file).

### 5.9 lnget config and L402

- **Skipped:** lnget is not installed (install failed). Once lnget is built from source, run `lnget config init` and set `~/.lnget/config.yaml` to use `~/.lnget/pay-only.macaroon` and the litd gRPC host (e.g. `localhost:10009` with TLS cert and network testnet). Then `lnget --max-cost N <url>` can pay for L402 APIs.

### 5.10 Aperture (seller side)

- **Command:** `skills/aperture/scripts/install.sh`
- **Result:** Failed. Same as lnget: Aperture’s go.mod has `replace` directives and requires Go 1.24.9+, so `go install` from outside the module fails. Build from a clone of the [aperture repo](https://github.com/lightninglabs/aperture) with Go 1.25+ to run Aperture.

### 5.11 Summary (retry)

| Step | Status | Notes |
|------|--------|--------|
| Clone | Done earlier | `~/code/lightning-agent-tools` |
| lnd install | OK | Docker image litd v0.16.0-alpha |
| Signer install | OK | Docker image lnd v0.20.0-beta |
| Start litd + signer | OK | Containers litd, litd-signer |
| Setup signer | OK | Wallet + credentials bundle |
| Import creds | OK | Into litd |
| Create watch-only wallet | OK | After setting SCRIPT_DIR (upstream bug) |
| lncli getinfo | OK | Node running, testnet |
| Bake pay-only macaroon | OK | With `--container litd` |
| lnget install | Fail | go.mod replace; build from lnget repo |
| lnget config / L402 pay | Skipped | Requires lnget binary |
| Aperture install | Fail | go.mod replace; build from aperture repo |

**Takeaways:** (1) Full remote-signer + watch-only + pay-only macaroon path works with Docker. (2) `create-wallet.sh` must be run with `SCRIPT_DIR` set until upstream adds `SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"`. (3) Macaroon bakery needs `--container litd` when lncli is only in the container. (4) lnget and Aperture must be built from their respective source repos; the skill `go install` path is broken for modules with replace directives.
