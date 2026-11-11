# Build order

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).

We have about zero users, so this is a greenfield build: we build the new
system, then delete today's login code. No compatibility layer, no dual
reads.

## Code to delete

When step 3 lands: the phone-QR and words-account routes in
`coax/web/api/private.c` (`/api/auth/challenge`, `approve`, `poll`, `key`,
`me`, `logout`), the `signin` schema, `verse.accounts` and the unverified
`/api/playtest/join` email, the `auth.identities`, `auth.sessions` and
`auth.workos_states` tables and the `/api/auth/v2/*` routes in `access.c`
(including the WorkOS start and callback), `coax/lib/nostr/weblogin.coax`,
and the `/login` page in `apps/openagents-web/private.coax`. The
`/signin`, `/logout` and `/signup` redirects to `/login` stay.

## Steps

Each step ends with its checks passing in `coax/web/api/test.sh` and on
staging. Sizes are rough agent-days.

| # | Step | What | Done means | Size |
| --- | --- | --- | --- | --- |
| 0 | **Backups and roles** | Move migrations out of the API's startup into a deploy job, backup project and locked bucket, dump job sealed to the offline key, retain-backups-on-delete, `oa_owner` / `oa_api` / `oa_readonly` / `oa_dump` roles, the DDL event trigger, the first restore drill | A drill restores last night's dump into `oa-drill-*` and its counts match; `oa_api` gets "permission denied" on `DROP TABLE` | 2 |
| 1 | **Missing crypto** | CBOR (deterministic subset), COSE keys, P-256 ECDSA verify, base32 and CRC-32 token codec ([below](#crypto-we-must-write-in-coax)) | Every published vector passes; mutation testing kills every planted bug in the `#authority` files | 3 to 4 |
| 2 | **Core** | `auth` schema, seal and lookup keys, `auth_resolve` middleware with the route rule table, web sessions, audit log, rate limits with their shown messages | Every API route has a rule; a test lists them; a stolen-cookie replay after logout fails | 3 |
| 3 | **Web login: email and Nostr** | Email code and same-browser link (Resend), NIP-98, NIP-07, NIP-46, phone-approves-the-web; the new `/login`; then delete the old code above | Log in each way on staging in headless Chrome; old routes answer 404 | 3 |
| 4 | **Passkeys on the web** | Registration and login, conditional UI, PRF salt per credential, the devices and security settings pages | A CDP virtual authenticator (ES256, with PRF) registers, logs out and logs back in | 2 |
| 5 | **Native apps** | Email and phone screens, Secure Enclave and Keystore device keys, device-bound refresh with reuse detection, Associated Domains and `assetlinks.json`, native passkeys | A copied refresh token fails from another device; reuse ends the family | 4 |
| 6 | **SMS** | Delivery interface with two providers, US and Canada, the monthly budget stop, domain-bound message format | A send over budget is refused with the shown message; failover to the second provider works | 1 to 2 |
| 7 | **Agents** | Agent accounts, grants, `oa_key_` keys, `openagents login` device approval, NIP-OA attestation, secret-scanning registration | An agent key gets 403 on every human-only route (a generated test over the rule table) | 3 |
| 8 | **Recovery and account life** | Recovery codes, the 72-hour delayed recovery, freeze, merge, export, deletion with the 30-day purge | A recovery started by SMS alone can be cancelled from email; a purged account's id is never reused | 3 |
| 9 | **Wallet backup and spend step-up** | NIP-VAULT passkey-PRF and recovery slots for the wallet seed, spend limits and step-up in the app | Delete the app, reinstall, restore the wallet with one passkey prompt | 3 |

Steps 0 and 1 have no dependencies on each other and start first. The
invite gate from [realm invites](2026-10-31-realm-invites-and-auth.md) §4
moves onto `auth.accounts` in step 2.

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
| Ed25519 verify | Passkeys that choose EdDSA (-8) | Lower priority: nearly every platform authenticator uses ES256 |
| RS256 check | Windows Hello passkeys | `rsa_verify.coax` exists; confirm PKCS#1 v1.5 with SHA-256 against Wycheproof |
| Base32 and CRC-32 | Token format with checksum | Small |
| E.164 phone parsing | Normalize numbers for lookup | Bounded: country code, length per country we allow |

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
| SMS delivery | Only carriers deliver SMS | Twilio Programmable Messaging or Telnyx (decision D3); plain messaging, not their verification products, so codes stay ours | `sms_send(to, text)` | The other provider as failover. Google has no SMS product, and Firebase Phone Auth would put users in Firebase, so Google-first doesn't apply here |
| iOS push | Only Apple delivers to iPhones | APNs, token auth | `push_send(device, payload)` | None possible; push is for notices only, never a login factor |
| Android push | Only Google delivers through Play services | FCM HTTP v1, with an access token from the Cloud Run metadata server | Same | None needed; fits Google-first |
| Passkey sync | People's passkeys live in their own password managers | iCloud Keychain, Google Password Manager, 1Password and others | WebAuthn | The person's choice; we never see their keys |
| Database and secrets | Where we run | Cloud SQL, Secret Manager, Cloud Storage | Postgres and the locked dumps | Dumps restore into any Postgres |
| Spark wallet (for now) | The person's wallet until our own passes its gate | Breez Spark | The app's wallet layer | Our Coax wallet, per the [build-everything audit](../research/bitcoin/2026-11-11-build-everything-audit.md) decision 0 |

We also check that no provider receives more than it needs: the email
provider gets the address and the message; the SMS provider gets the
number and the message; neither gets an account id.
