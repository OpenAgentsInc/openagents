# NIP-VAULT — Sealed Personal Data

`draft` `optional` — v1, 2026-10-10. **Designed; nothing is implemented.**
The [shared contracts](contracts.md) are normative. The design, the threat
model and the honest limits are in the
[sensitive data vault](../../docs/security/sensitive-data-vault.md).

This NIP defines how a client stores a person's sensitive files so that a
storage operator holds only ciphertext. The files are then opened either:

- only by the person's own devices (tier `user`);
- only by an attested workload, and only while the person's client supplies
  its share (tier `sealed`);
- or by an operator key that is logged at every use (tier `operator`).

It fixes:

- the object format and the content encryption;
- the key wraps for each tier;
- key slots, which let several devices and a recovery code open one master
  key;
- the session, request and lease formats for asking an attested workload
  ([NIP-ATT](NIP-ATT.md)) to open objects.

Independent clients can read and write the same vault from these formats.

It allocates no kinds. Vault objects are byte blobs, kept in any blob store,
including Blossom (NIP-B7), addressed by the SHA-256 of their bytes. Slots
and headers that travel over Nostr use the shared private artifact `3188`,
self-addressed.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [contracts](contracts.md) `3188` | Carries slots, key indexes and leases between a person's own devices, self-addressed. Relay privacy rules apply. |
| [ATT](NIP-ATT.md) | The `sealed` tier's workload is an ATT workload. Sessions are opened only to endpoints the client verified, at level `tee-cloud`. |
| [POL](NIP-POL.md) | A lease is a single-run approval: exact objects, purpose and route, consumed once, not a bearer grant. |
| [SOV](NIP-SOV.md) custody | Each open request fixes the ciphertext, who receives the plaintext, and the scope, as SOV decryption operations do. |
| NIP-44, NIP-49, NIP-51 | The Nostr-key slot is NIP-44 v2 to self, as NIP-51 private lists are. The recovery slot stretches with scrypt, as NIP-49 does. |
| NIP-17 kind 15 | Same idea, a per-file AES-GCM key under an encrypted envelope, extended with chunking, tiers and slots. |

## Encoding

JSON objects follow [contracts](contracts.md): required `v`, `requires`, and
optional `meta`. Unknown fields refuse. Binary values are standard base64
with padding. `"<digest>"` is the lowercase hex SHA-256. IDs are 32 random
bytes as 64 lowercase hex characters.

## Keys

| Key | Size | Made by | Lives |
| --- | --- | --- | --- |
| `VMK` vault master key | 32 B | random, on the person's first device | the person's devices; wrapped in slots |
| `K_user_wrap` | 32 B | `HKDF-SHA256(ikm=VMK, salt=empty, info="openagents.vault.v1/user-wrap")` | derived when needed |
| `S_user` user share | 32 B | `HKDF-SHA256(ikm=VMK, salt=empty, info="openagents.vault.v1/user-share")` | derived when needed; sent only inside an ATT session |
| `user_key_id` | 16 B hex | first 16 bytes of `SHA-256("openagents.vault.v1/key-id\0" ‖ K_user_wrap)` | public |
| `SK` system key | X25519 pair | the attested workload, one per person per epoch | private half only as Cloud KMS (or equivalent) ciphertext, decryptable only by the attested workload |
| `DEK` data key | 32 B | random, per object | only inside wraps |

`VMK` rotation produces a new `VMK`, new slots and new wraps. Object content
is not re-encrypted.

## Vault object

An object is: `magic ‖ header_len ‖ header ‖ chunks`.

- `magic` is the 8 bytes `OAVAULT1`.
- `header_len` is a big-endian u32.
- `header` is the JCS JSON object below.
- `chunks` follow the header.

```json
{
  "v": "openagents.vault-object.v1",
  "requires": [],
  "core": {
    "object": "<id>",
    "vault": "<id of the person's vault>",
    "tier": "sealed",
    "content": {
      "alg": "aes-256-gcm-chunked-v1",
      "chunk_bytes": 65536,
      "nonce_prefix": "<7 bytes base64>",
      "chunks": 3
    },
    "media": "application/pdf",
    "created_at": 0
  },
  "wraps": [ ]
}
```

- `core_digest = SHA-256(JCS(core))`. It binds every chunk and every wrap.
  The `wraps` array can change (rotation, a new device) without touching
  the content.
- `media` is optional. File names are never in the header; a name is a
  separate small object. Size is visible from the blob length anyway.

**Content, `aes-256-gcm-chunked-v1`.** Plaintext is split into chunks of
`chunk_bytes`. The last chunk may be shorter, but not empty unless the
plaintext is empty. Chunk `i` (from 0) is encrypted with AES-256-GCM under
`DEK`:

- nonce = `nonce_prefix (7 B) ‖ be32(i) ‖ last (1 B: 0x01 for the final chunk, else 0x00)`;
- associated data = `"openagents.vault.v1/chunk\0" ‖ core_digest`;
- output = ciphertext ‖ 16-byte tag.

A reader MUST refuse a stream that ends without a chunk marked final, a
chunk count different from `content.chunks`, or a chunk past the final one.
This stops truncation and extension.

## Wraps

Each wrap opens `DEK` one way. An object carries one or more.

**`user`** (tier `user`, and the device copy in tier `sealed`):

```json
{ "mode": "user", "key": "<user_key_id>", "nonce": "<12 B>", "dek": "<48 B: ct+tag>" }
```

AES-256-GCM under `K_user_wrap`, with associated data
`"openagents.vault.v1/wrap-user\0" ‖ core_digest`.

**`sealed`** (tier `sealed`):

```json
{
  "mode": "sealed",
  "system_key": "<id of SK.pub>",
  "user_key": "<user_key_id>",
  "suite": "x25519-hkdf-sha256/hkdf-sha256/aes-256-gcm",
  "enc": "<32 B>",
  "dek": "<48 B>"
}
```

HPKE (RFC 9180), in `mode_psk`:

- KEM `0x0020` DHKEM(X25519, HKDF-SHA256), KDF `0x0001` HKDF-SHA256, AEAD
  `0x0002` AES-256-GCM.
- `pkR` = `SK.pub`.
- `psk` = `S_user`, and `psk_id` = `"openagents.vault.v1/" ‖ user_key_id`.
- `info` = `"openagents.vault.v1/wrap-sealed\0" ‖ core_digest`.
- `aad` = `core_digest`.

Opening needs `SK.priv` **and** `S_user`. A client can make this wrap
without contacting the workload. It does need `SK.pub` from a system key
certificate it has verified (below).

**`operator`** (tier `operator`):

```json
{ "mode": "operator", "kms_key": "<KMS key version resource name>", "dek": "<base64 KMS ciphertext>" }
```

The DEK is encrypted by the named KMS key, with additional authenticated data
`core_digest`. Anyone holding decrypt on that key can open it, and every use
is logged by the KMS. Clients MUST label this tier as readable by the
operator.

A `sealed` object SHOULD also carry a `user` wrap (device copy) unless the
person turned that off. An object MUST NOT carry an `operator` wrap unless
its tier is `operator`.

## System key certificate

The workload publishes `SK.pub` for a person inside its attested session
(below), as:

```json
{
  "v": "openagents.vault-system-key.v1",
  "requires": [],
  "vault": "<id>",
  "system_key": "<id>",
  "public": "<32 B>",
  "epoch": 1,
  "release": "<NIP-ATT 3202 event id>"
}
```

The workload signs it with the session's endpoint key, and the client checks
that signature against the attested endpoint. The client stores the
certificate inside its own key index, sealed under `K_user_wrap`, so that
later substitution is detected. A client MUST NOT wrap to an `SK.pub` that
it has not taken from an attested session.

## Key slots

A slot holds `VMK` under one unlock method:

```json
{
  "v": "openagents.vault-slot.v1",
  "requires": [],
  "vault": "<id>",
  "slot": "<id>",
  "method": "passkey-prf",
  "params": { },
  "nonce": "<12 B>",
  "vmk": "<48 B>",
  "label": "Chris's iPhone",
  "created_at": 0
}
```

The wrapping key is `HKDF-SHA256(ikm=<method secret>, salt=slot id bytes,
info="openagents.vault.v1/slot\0" ‖ method)`. `vmk` is AES-256-GCM of `VMK`
under it, with associated data `JCS(slot without nonce and vmk)`.

| `method` | `params` | method secret |
| --- | --- | --- |
| `passkey-prf` | `{rp_id, credential_id, prf_salt}` | the WebAuthn PRF output for `prf_salt`; `prf_salt` is 32 random bytes per slot |
| `device` | `{platform, key_ref}` | 32 random bytes kept in the iOS/macOS Keychain (this device only) or the Android Keystore; `key_ref` names the entry, never the value |
| `nostr` | `{pubkey}` | the NIP-44 v2 conversation key of `(person's key, person's pubkey)`. Unwrap by NIP-07 `nip44.decrypt` or NIP-46 `nip44_decrypt` of a 32-byte value that was encrypted to self at slot creation. That value is the method secret. |
| `recovery` | `{log_n, salt}` | `scrypt(NFKC(24 BIP-39 words), salt, N=2^log_n, r=8, p=1, 32)`; `log_n` is at least 20 |

Slots are stored by the service, and copied between the person's devices as
self-addressed `3188` artifacts. A service that holds all slots holds no
method secret, so it cannot open any of them.

## Key index

The key index is a per-person object of tier `user` or `sealed`. It holds
each object's wraps, the system key certificates, and the person's NIP-ATT
approval set. Deleting an object rewrites the index without it, under a new
index epoch:

- **`sealed` tier:** the index is wrapped to a per-person KMS key version,
  and the old version is scheduled for destruction.
- **`user` tier:** the index key is
  `HKDF(VMK, info="openagents.vault.v1/index\0" ‖ be32(epoch))`, and the
  old epoch's index is deleted.

An object whose wraps appear only in a destroyed index is unreadable.

## Opening sealed objects

1. **Session.** The client fetches an ATT endpoint (`30203`) for workload
   `vault`, verifies it per [NIP-ATT](NIP-ATT.md#client-verification), and
   requires level `tee-cloud`. The endpoint's `hpke` key is the session
   key.
2. **Request.** The client sends one HPKE `mode_base` message to the
   endpoint's `hpke` key, with `info = "openagents.vault.v1/request\0" ‖
   endpoint event id`. The plaintext is:

```json
{
  "v": "openagents.vault-request.v1",
  "requires": [],
  "vault": "<id>",
  "user_share": "<S_user, 32 B>",
  "lease": { },
  "reply_to": "<client X25519 public key, 32 B>",
  "nonce": "<16 B>"
}
```

3. **Lease** (inside the request, signed by the client's device-slot key or
   the person's Nostr key):

```json
{
  "v": "openagents.vault-lease.v1",
  "requires": [],
  "lease": "<id>",
  "run": "<id>",
  "objects": ["<object id>"],
  "purpose": "answer",
  "route": "private",
  "effects": { "read": true, "write_derived": false, "export": false },
  "expires_at": 0,
  "max_uses": 1,
  "issued_by": "<slot id>",
  "issued_at": 0
}
```

- `route` is `private` (an attested model, NIP-ATT level `tee-cloud` or
  `tee`), `fast` (a named outside model, declared in the workload's
  release as egress), or `device` (plaintext returned to the client only).
- `expires_at` is at most 86400 seconds after `issued_at`. Clients SHOULD
  default to 900 seconds for one turn.
- `write_derived` lets the run store new objects (answers, extracted text).
  The client gets them back sealed and wrapped the same way.
- `export` returns plaintext to `reply_to`, and nothing else.

4. **Inside the workload.** It verifies the lease signature against a slot
   key the person enrolled. Those keys are listed in the key index and the
   client sends them along. It then:

   - unseals `SK.priv` through the KMS;
   - opens each named `DEK` with `SK.priv` and `S_user`;
   - does the work;
   - seals every reply to `reply_to` with HPKE `mode_base`.

   It records `lease`, `run`, the object IDs, `route` and the result
   (`granted`, `refused:<reason>`, `released`), and never content.
5. **Release.** When the run ends, the lease expires, or a revocation
   arrives (`{v: "openagents.vault-revoke.v1", lease}` signed like the
   lease), the workload zeroes `S_user`, `SK.priv` and every `DEK` for that
   lease, and refuses further use.

## Validation

- Refuse an object whose magic, header, chunk count or final marker is
  wrong, or whose wrap's associated data does not match `core_digest`.
- Refuse a `sealed` wrap to a system key with no verified certificate in the
  index.
- Refuse a lease that is expired, already used `max_uses` times, signed by
  an unknown slot key, or wider than the request it rides in.
- Refuse `route: fast` unless the workload's release declares that egress
  and the person's project allows Fast.

## Privacy and disclosure

| Tier | Who can open stored objects | Metadata visible to the storage operator |
| --- | --- | --- |
| `user` | the person's devices | object count, sizes, times, `media` if set |
| `sealed` | the attested workload, while the person's client supplies `S_user`; the person's devices if a `user` wrap is present | the same, plus lease records (IDs, route, times) |
| `operator` | anyone with decrypt on the KMS key | the same, plus KMS use logs |

The plaintext an attested workload sends to a `fast` route leaves the TEE,
and that provider's terms apply. A client MUST say so before the first use
in a project.

## Security considerations

- **Client code.** A web client is code served by the operator. Pin its
  digest (Subresource Integrity, and the same NIP-ATT release log) and
  prefer native clients for `user` and `sealed`.
- **Loss.** Losing every slot and the recovery code loses `user`- and
  `sealed`-tier data. There is no recovery path by design.
- **Device copy.** A `user` wrap on a `sealed` object means a stolen, unlocked
  device alone opens it.
- **Passkey PRF** support differs by platform. A vault MUST have a second
  slot (`recovery`, at least) before it accepts data.
- **Plaintext digests** are never stored or logged, because the digest of a
  templated statement can be guessed.

## Implementation status

Nothing is implemented. The [vault milestones](../../docs/security/sensitive-data-vault.md#recommendation-and-milestones)
V1–V4 build it. Test vectors for the content scheme, the three wraps and each
slot method land with the first implementation (V1 gate), in
`fixtures/nips/vault/`.

## Conformance

Fixtures must cover:

- each wrap and slot method;
- a truncated stream, an extended stream and a reordered chunk;
- a wrap moved between objects (the associated data mismatches);
- a `sealed` wrap opened with `SK.priv` but the wrong `S_user`, and with
  `S_user` but the wrong `SK.priv`;
- a lease that is expired, reused or too wide;
- a revoked lease;
- an object whose only wrap sits in a destroyed index.

Advertise `nip-vault-v1` in NIP-11 `supported_extensions` only for a relay
that applies the `3188` privacy rules to vault artifacts. Keep this draft
name out of numeric `supported_nips`.
