# Operate OpenAgents Lightning addresses

The [design](self-hosted-lightning-addresses.md) describes the account and wallet
contract. The account gateway authorizes names and forwards requests to the
private Breez Rust LNURL server. The website forwards the public payment paths
to that gateway. Never route public registration requests directly to Breez.

## Build and configure

1. Run `scripts/deploy/build-lnurl.sh` from a checkout rebased on current main.
   It builds the exact Breez revision used by the wallet. Retain the image digest
   alongside the OpenAgents commit. The Docker build needs network access and
   does not start a service or make payments.
2. Provision PostgreSQL and an encrypted backup destination. The supplied
   `deploy/lightning-address/compose.yaml` binds the backend only to loopback;
   the database has no published port. Keep both services on the same private
   host, or use private networking and TLS between hosts.
3. Store the database password and URL, Breez service credential, and a stable
   32-byte hex `LNURL_SSP_AUTH_SEED` in the deployment's secret store. Supply
   `LNURL_POSTGRES_PASSWORD`, `LNURL_DATABASE_URL`, `OPENAGENTS_BREEZ_API_KEY`,
   `LNURL_SSP_AUTH_SEED`, and `LNURL_WEBHOOK_DOMAIN` to Compose. URL-encode the
   database password in its connection URL. Do not commit an environment file,
   print `docker compose config`, or put credentials in command arguments.
4. Install `deploy/lightning-address/gateway.json` on the gateway host. Set
   `OPENAGENTS_LIGHTNING_ADDRESS_CONFIG` to that absolute file path and supply
   `OPENAGENTS_BREEZ_API_KEY` to the gateway process. The gateway reads the file
   once at startup. Its account surface must be enabled. The website's existing
   account-service configuration must point to this gateway. Restart after
   changing the configuration or rotating its credential.
5. Configure `lnurl-hooks.openagents.com` with TLS and the restrictions in
   `deploy/lightning-address/webhook.nginx.conf`. Only `POST /webhook` reaches
   the backend; its SSP HMAC check remains required. Do not expose `/lnurlpay`
   on this hostname. Keep request bodies, query strings, authorization headers,
   and Breez signature headers out of access logs.
6. Apply an ingress limit to the website's `/lnurlp/`, `/lnurlpay/`, and
   `/.well-known/lnurlp/` paths: 30 requests per minute per source IP, with a
   small burst, a 16 KiB request limit, and a 20-second request deadline.
   The gateway separately caps concurrency at 64 and payment requests per
   recipient at 120 per minute. Trust a forwarded IP only from the configured
   ingress. Do not cache any of these responses.

The default deployment does not enable Nostr zaps. To enable them, provision a
stable service Nostr key through `BREEZ_LNURL_NSEC`, back it up, and complete the
zap receipt checks below before advertising the feature. This is the service's
receipt key, never a customer's profile key or recovery words.

## Activation checks

Use the dedicated test account and a separate test wallet. Never restore the
owner's wallet or use the owner's account for these checks.

- Before configuration, address controls report receiving as unavailable.
- Claim a name, enable its address in the wallet, and verify that the account
  record and Breez registration identify the same Spark key.
- Fetch the public well-known URL. Check the identifier, amount limits, HTTPS
  callback, no-store header, and the absence of internal backend addresses.
- Request an invoice, decode its mainnet amount, metadata hash, and expiry,
  then make a small payment only with an explicitly funded test wallet.
  Verify receipt in the intended wallet, including when that wallet was offline
  during invoice creation. Record the transaction identifier, not its preimage.
- Disable the address while an invoice request is in progress. New invoice
  responses must stop. Existing invoices can still settle until expiry.
- Release the social name and try to claim it from another test account. A name
  that has received an enrollment must stay unavailable. Delete the first
  account and repeat; its payment name must still be held.
- Test lost registration responses, retry through recovery, and wallet transfer
  with signatures from both wallets. A signature from either wallet alone must
  fail. Requests sent directly to the private backend must be unreachable from
  the public network.
- If zaps are enabled, validate the signed zap request and published receipt,
  recipient, invoice, service receipt key, and duplicate webhook handling.

## Backup, monitoring, and rollback

Back up PostgreSQL, the account store, the SSP authentication seed, and the zap
key together. PostgreSQL holds registrations, permanent released-name holds,
payment metadata, and the generated webhook secret. Restoring only registrations
can lose name holds or payment notifications. Encrypt backups and restrict access:
payment metadata can contain preimages and private comments.

Alert on lookup and invoice failures, timeouts, request saturation, database
backup age, failed webhook deliveries, and unexpected registration volume.
Probe process reachability privately and the public facade with a dedicated
nonpaying check address. Do not infer payment settlement from process health.

To stop new receiving requests, remove the gateway configuration variable and
restart the gateway. Keep the backend and webhook delivery alive until existing
invoices expire and settle. Preserve databases and permanent holds. Restore the
previous verified application image when necessary; do not roll a migrated
database backward without a tested compatible restore. Re-enable only after a
restore rehearsal verifies the same account, name, Spark key, and webhook secret.

## Move an address to a replacement wallet

Use two computers holding the old and new wallets, signed in to the same
OpenAgents account. The old wallet must approve the transfer; account access
alone cannot move payments to a replacement key.

1. On the new wallet, run `openagents wallet lightning key` and copy its public
   key to the old wallet's computer.
2. On the old wallet, run `openagents wallet lightning authorize --to-key KEY`.
   Save the returned JSON to a local approval file. It contains a short-lived,
   address-specific signature, not a recovery phrase or spending key.
3. Give that file to the new wallet's computer and, within five minutes, run
   `openagents wallet lightning transfer --authorization FILE`. This command
   proves the new wallet's key to the account and submits both wallets'
   authorizations to Breez. Receiving pauses until the transfer succeeds.
4. Read `openagents wallet lightning status` and verify a small test receipt
   with a funded test wallet before sharing the address again. Delete the
   approval file. If the transfer timed out, obtain a fresh approval instead
   of assuming it failed.

To publish the address on your profile, edit **Lightning address** under
**Public profile** and save. Your Nostr key signs the change. The profile keeps
fields written by other clients and displays a **Send a tip** link. Disabling an
address stops new invoices; remove it from the profile separately if you no
longer want to display it.
