# Build order

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).

We have about zero users, so this is a greenfield build: we build the new
system, then delete today's login code. No compatibility layer, no dual
reads.

## Code to delete

When steps 3 and 4 land: the phone-QR and words-account routes in
`coax/web/api/private.c` (`/api/auth/challenge`, `approve`, `poll`, `key`,
`me`, `logout`), the `signin` schema, `verse.accounts` and the unverified
`/api/playtest/join` email, the `auth.identities`, `auth.sessions` and
`auth.workos_states` tables and the `/api/auth/v2/*` routes in `access.c`
(including the WorkOS start and callback), `coax/lib/nostr/weblogin.coax`,
and the `/login` page in `apps/openagents-web/private.coax`. The
`/signin`, `/logout` and `/signup` redirects to `/login` stay.

## Steps

Each step ends with its checks passing in `coax/web/api/test.sh` and on
staging. Sizes are rough agent-days. **The launch set is email and Nostr**
(decision D2): steps 0 to 4 ship it on the web and in the apps. Passkeys,
device keys and agent keys come after. There is no SMS step: we never
collect phone numbers (decision D3).

| # | Step | What | Done means | Size |
| --- | --- | --- | --- | --- |
| 0 | **Backups and roles** | Move migrations out of the API's startup into a deploy job, backup project and locked bucket, dump job sealed to the offline key, retain-backups-on-delete, `oa_owner` / `oa_api` / `oa_readonly` / `oa_dump` roles, the DDL event trigger, the first restore drill | A drill restores last night's dump into `oa-drill-*` and its counts match; `oa_api` gets "permission denied" on `DROP TABLE` | 2 |
| 1 | **Missing crypto** | CBOR (deterministic subset), COSE keys, P-256 ECDSA verify, WebAuthn verification, base32 and CRC-32 token codec ([below](#crypto-we-must-write-in-coax)) | Every published vector passes; mutation testing kills every planted bug in the `#authority` files | 3 to 4 |
| 2 | **Core** | `auth` schema, seal and lookup keys, `auth_resolve` middleware with the route rule table, web sessions, revocation, audit log, rate limits with their shown messages | Every API route has a rule; a test lists them; a stolen-cookie replay after logout fails | 3 |
| 3 | **Web login: email and Nostr** (launch) | Email code and same-browser link (Resend), NIP-98, NIP-07, NIP-46, phone-approves-the-web; the new `/login`; then delete the old code above and the WorkOS resources (D7) | Log in each way on staging in headless Chrome; old routes answer 404 | 3 |
| 4 | **App login: email and Nostr** (launch) | Email screen and "Log in with a key" (NIP-46, NIP-55 on Android, make a new key) in the iOS, Mac and Android apps; `oa_at_` and `oa_rt_` tokens with single-use refresh and reuse detection | Log in each way on a simulator and an emulator; a reused refresh token ends the family | 3 |
| 5 | **Lightning Addresses** | `pay.addresses`, the claim and settings UI, `/.well-known/lnurlp/<name>` for the Spark and forward modes, zaps, the shown limits; tips and upgrade shares paid to makers' own addresses ([Lightning Addresses](lightning-addresses.md)) | A claimed address is paid while the phone is offline and the sats arrive in that phone's wallet; a forward address returns the upstream wallet's invoice; a tip never touches our wallet | 4 |
| 6 | **Agents** | Agent accounts, grants, `oa_key_` keys, `openagents login` device approval, NIP-OA attestation, secret-scanning registration, step-up enforcement for human-only routes | An agent key gets 403 on every human-only route (a generated test over the rule table) | 3 |
| 7 | **Recovery and account life** | Recovery codes, the 72-hour delayed recovery, freeze, merge, export, deletion with the 30-day purge | A recovery started by email alone can be cancelled from a Nostr key; a purged account's id is never reused | 3 |
| 8 | **Passkeys** (built, #13215) | Web registration and login, conditional UI, PRF salt per credential, the devices and security settings pages; native passkeys with Associated Domains and `assetlinks.json` | A CDP virtual authenticator (ES256, with PRF) registers, logs out and logs back in; the iOS app uses the same passkey | 3 |
| 9 | **Device keys** (built, #13216) | Secure Enclave and Keystore device keys, device-bound refresh (`OA-Device-Proof`), the devices list | A copied refresh token fails from another device | 2 |
| 10 | **Wallet backup and spend step-up** (built, #13217) | NIP-VAULT passkey-PRF and recovery slots for the wallet seed, spend limits and step-up in the app | A new phone with no wallet logs in with its passkey and has the wallet back after that one prompt (unit-tested in the Simulator with a software passkey; one run on a real iPhone is in `NEEDS_OWNER.md`) | 3 |

Steps 0 and 1 have no dependencies on each other and start first. Step 3
needs 0 and 2; step 2 needs the base32 and CRC-32 codec from step 1, not
the passkey crypto. The invite gate from
[realm invites](2026-10-31-realm-invites-and-auth.md) §4 moves onto
`auth.accounts` in step 2. Step 5 needs step 2 and the app wallet; step 8
needs the rest of step 1.

## Crypto we must write in Coax

The API is C; the crypto is Coax compiled to C (as `nip98.generated.c`
is today) and to WebAssembly for the pages.

**Already in `coax/lib/crypto/` and `coax/lib/nostr/`:** SHA-1, SHA-256,
SHA-512, SHA-3, HMAC, HKDF, PBKDF2, scrypt, ChaCha20-Poly1305,
XChaCha20-Poly1305, AES-GCM, secp256k1 ECDSA and Schnorr, ECDH, RSA
verify, BIP-39, BIP-32, NIP-06, NIP-44, NIP-46, NIP-49, NIP-98, NIP-OA,
NIP-VAULT (tier `user`), base64, bech32, JSON, entropy and zeroize.

**Missing:**

| Piece | Why | Notes |
| --- | --- | --- |
| P-256 (secp256r1) ECDSA verify | Passkeys (ES256), device-key proofs from the Secure Enclave and Keystore | New field and group arithmetic; reuse the secp256k1 code's structure. Vectors: NIST CAVP, Wycheproof |
| P-256 ECDSA sign | APNs token authentication (ES256 JWT) | Constant time; the APNs key lives in Secret Manager |
| CBOR decoder | WebAuthn `attestationObject` and COSE keys | Bounded, deterministic subset only (maps, byte strings, small ints); refuse everything else. Vectors: RFC 8949 appendix A |
| COSE key parsing | Passkey public keys (EC2 P-256, OKP Ed25519, RSA) | RFC 9052/9053 |
| WebAuthn verification | `clientDataJSON` (type, challenge, origin), `authenticatorData` (RP ID hash, UP, UV, BE, BS flags, sign count), signature | Test with the WebAuthn spec's examples and a virtual authenticator |
| Ed25519 verify | Passkeys that choose EdDSA (-8) | Built (#13221): `coax/lib/crypto/ed25519.coax`, libsodium's strict rules (S < L, canonical A and R, no small-order A or R, cofactorless); RFC 8032, Wycheproof, "Taming the many EdDSAs" and a libsodium differential |
| RS256 check | Windows Hello passkeys | `rsa_verify.coax` exists; confirm PKCS#1 v1.5 with SHA-256 against Wycheproof |
| Base32 and CRC-32 | Token format with checksum | Small |

**Not needed:** Argon2 (no passwords; recovery codes use scrypt, which
exists), TOTP (passkeys beat authenticator apps; HMAC-SHA1 exists if the
owner ever wants it), JWT libraries (opaque tokens; the one JWT we sign is
the APNs one), X.509 (we ask for no attestation).

## Outside services we can't avoid

Each one only delivers. None holds a user table, and each sits behind a
one-function interface in the API so swapping it is a config change.

| Service | Why we can't avoid it | Provider | Interface | Fallback and exit |
| --- | --- | --- | --- | --- |
| Email delivery | Inboxes only accept mail from reputable senders; running our own outbound mail server means months of IP reputation work | Resend (already wired: `email.c`, `openagents-resend-api-key`) | `email_send(to, subject, text, idempotency_key)` | A second provider (Amazon SES or Postmark) configured and tested monthly. SPF, DKIM and DMARC are on our domain, so the sender identity is ours whichever provider sends |
| iOS push | Only Apple delivers to iPhones | APNs, token auth | `push_send(device, payload)` | None possible; push is for notices only, never a login factor |
| Android push | Only Google delivers through Play services | FCM HTTP v1, with an access token from the Cloud Run metadata server | Same | None needed; fits Google-first |
| Passkey sync | People's passkeys live in their own password managers | iCloud Keychain, Google Password Manager, 1Password and others | WebAuthn | The person's choice; we never see their keys |
| Database and secrets | Where we run | Cloud SQL, Secret Manager, Cloud Storage | Postgres and the locked dumps | Dumps restore into any Postgres |
| Spark wallet (for now) | The person's wallet until our own passes its gate | Breez Spark | The app's wallet layer | Our Coax wallet, per the [build-everything audit](../research/bitcoin/2026-11-11-build-everything-audit.md) decision 0 |
| Spark receive for Lightning Addresses | An offline phone can't make an invoice; Spark's service provider can, for the person's own Spark wallet | Breez's open-source LNURL server, self-hosted by us, with a Breez API key | `/.well-known/lnurlp/<name>` behind our API | Our own node and LSP with async payments ([Lightning Addresses](lightning-addresses.md#the-long-term-home)) |

We also check that no provider receives more than it needs: the email
provider gets the address and the message, never an account id. We have
no SMS provider, because we never collect phone numbers (decision D3).
