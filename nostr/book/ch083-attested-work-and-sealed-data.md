# 83. Attested work and sealed data

Some work must run where the operator cannot read it, and some data must
sit where the storage operator holds only ciphertext. NIP-ATT gives a
client a way to check, before it sends anything, that a remote program is a
publicly listed build; NIP-VAULT gives a person's files a format that opens
only through one of their own key slots. This chapter reads both from the
client's side: it checks an attested endpoint against a release head under
a fixed clock and refuses one that does not match, and it opens a sealed
vault object through a key slot and refuses every other way in.

## What this chapter teaches

- NIP-ATT's three public records: a release (kind 3202) names one image, a
  release head (30202) lists the releases admitted for a workload and from
  when, and an attested endpoint (30203) is one running instance's key with
  evidence that binds the key to a release.
- Admission under a notice delay, an emergency release, retirement and
  rollback, worked out by hand.
- The binding, `SHA-256("openagents.att.v1" 0 || endpoint || release id)`,
  and how a Confidential Space token's claims must match the release.
- Why the level stays `open`: the client reads the token's claims but does
  not yet verify the token's signature to Google's root.
- NIP-VAULT tier `user`: a key slot holds the vault master key (VMK), the
  VMK derives `K_user_wrap`, a user wrap holds the object's data key (DEK),
  and the DEK opens AES-256-GCM chunks with a final-chunk marker.
- The `nostr` slot method, whose secret is NIP-44 sealed to the person's own
  key ([chapter 20](ch020-nip-44-encrypted-payloads.md)).
- The same attested endpoint key can later serve as one co-signer in a
  NIP-KEYS wallet, with its signing policy pinned by the release ([chapter
  104](ch104-agent-treasuries-and-a-network-of-keys.md)); that chapter
  builds it.

## Where the code is

| File | What it holds |
| --- | --- |
| `coax/lib/nostr/att.coax` | `#authority`. The release, head and endpoint readers, `at_admitted`, `at_generation`, `at_binding`, the token claim reader and `at_verify` |
| `coax/lib/nostr/vault.coax` | `#authority`. Slots (`vt_slot_read`, `vt_slot_open`, `vt_nostr_secret`), `vt_user_keys`, `vt_object_read` and `vt_open` |
| `coax/lib/crypto/aes_gcm.coax` | The AES-256 forward cipher and AES-256-GCM, new in this chapter; `aes.coax` stays decrypt-only |
| `coax/tests/nostr/att_fixtures.coax`, `coax/tests/nostr/vault_fixtures.coax` | Hand-made records and helpers for the two units' checks |
| `coax/tests/nostr/att_vault_reference.py` | The independent Python writer of every record the example reads (tooling only) |
| `nostr/examples/attested-vault.coax` | The example |
| `nostr/examples/checks/attested-vault.coax` | Its checks against the Python reference |

The caller verifies each event first (`nostr_event_verify_vartime` from
[chapter 21](ch021-events-ids-and-signatures.md)) and passes its signer, tags
and unescaped content. The readers decide only from those.

## Attested workloads

### Levels

NIP-ATT gives every endpoint exactly one level, computed by the client from
evidence and never taken from a claim: `open` (no evidence), `hardened`
(Apple App Attest), `tee` (a verified TDX or SEV-SNP report on hardware the
provider holds) and `tee-cloud` (as `tee`, and the evidence proves the
machine is in a named cloud's data center). Data whose policy requires a
level never goes to a lower one, and the client never falls back silently.

### The release and the head

A release is signed by the workload's publisher. Its content names the
image (`image.digest`, `sha256:` and 64 hex digits), the platforms it is
admitted on, the source and the time it was published. Its `x` tag is the
SHA-256 of the content and its `w` tag the workload slug. `at_release_read`
refuses an unknown version, an unknown or repeated field, a publisher other
than the signer, a malformed digest, and anything this chapter does not
build: a platform other than `gcp-confidential-space`, and a GPU.

The head is the publisher's addressable list (`d` = the workload) of
admitted releases, each with `effective_at` and `retire_at`, plus a
`generation` that only grows and an optional `emergency` release with a
reason. A release is admitted at the reader's time `now` when it is listed,
`effective_at <= now`, `retire_at` is null or later than `now`, and
`effective_at` is at least `notice_seconds` after the release's
`published_at` (one day for the emergency release):

<!-- source: coax/lib/nostr/att.coax admitted -->
```jai
// NIP-ATT: admitted at now when listed, effective_at <= now, retire_at
// null or later than now, and effective_at at least the notice delay after
// the release's published_at (one day for the head's emergency release).
at_admitted :: (h:AtHead,release:string,published_at:int,now:int)->AtCode {
  for i:0..<h.count {
    a:=h.admitted[i];
    if a.release==release {
      if a.effective_at>now {return AtCode.early;}
      if a.retire_at>=0 && a.retire_at<=now {return AtCode.retired;}
      notice:=ifx h.emergency==release then AT_EMERGENCY else h.notice;
      if a.effective_at<published_at+notice {return AtCode.notice;}
      return AtCode.ok;
    }
  }
  return AtCode.listed;
}
// held is the highest generation this reader has seen for the workload.
at_generation :: (h:AtHead,held:int)->AtCode {return ifx h.generation<held then AtCode.generation else AtCode.ok;}
```

### Worked example: admission, by hand

The example's clock is T0 = 1792000000. Key 1 published the release at
`published_at` = T0 - 700000 = 1791300000. The head (generation 7,
`notice_seconds` 604800, seven days) lists it with `effective_at` =
1791904800 and `retire_at` null.

- Notice: 1791904800 - 1791300000 = 604800, exactly the notice delay, so
  the release passes; one second later in `published_at` and it would be
  `notice`.
- Effective: 1791904800 = T0 - 95200, so at T0 it is admitted, at
  T0 - 95200 it is admitted (the bound is inclusive), and at T0 - 95201 it
  is `early`.
- Rollback: a reader that already holds generation 8 refuses generation 7.
- A release published at T0 - 100000 (release 2, another image) is in no
  head at all: `listed`.

### The endpoint and its binding

The endpoint is signed by a key generated inside the TEE, the same key that
would receive sealed jobs; its content must name that key as `endpoint`. It
names the release, a `binding`, its evidence, and `issued_at` and
`valid_until`, at most 3600 seconds apart, with the `expiration` tag equal
to `valid_until`. The binding ties the key to the release:

<!-- source: coax/lib/nostr/att.coax binding -->
```jai
// SHA-256("openagents.att.v1" || 0 || endpoint (32 bytes) || release id
// (32 bytes)), as 64 lowercase hex digits. hpke is null here, so it adds no
// bytes.
at_binding :: (endpoint:string,release:string,out:[]u8)->bool {
  if !at_hex64(endpoint) || !at_hex64(release) {return false;}
  msg:[82]u8;for i:0..<AT_DOMAIN.count {msg[i]=AT_DOMAIN[i];}   // msg[17] stays 0
  for i:0..<32 {msg[18+i]=cast(u8)(at_nibble(endpoint[2*i])*16+at_nibble(endpoint[2*i+1]));msg[50+i]=cast(u8)(at_nibble(release[2*i])*16+at_nibble(release[2*i+1]));}
  digest:[32]u8;status:=sha256_hash_checked(msg[..],digest[..],false);
  hex:="0123456789abcdef";
  for i:0..<32 {out[2*i]=hex[cast(int)digest[i]>>4];out[2*i+1]=hex[cast(int)digest[i]&15];}
  return status==SHA_OK;
}
```

### Worked example: the binding, by hand

The message is 82 bytes: the 17 bytes of `openagents.att.v1`, one zero
byte, the 32 bytes of key 3 (`f9308a01...e036f9`), and the 32 bytes of the
release id (`97263cdd...060d504a`). Its SHA-256 is
`5312236c3e5543abaa57fae6c8e37fcad8e22b9965e06311b9c1de1b6fd74035`, which is
the endpoint's `binding` and the one `eat_nonce` value in its token. The
Python reference computes the same digest with `hashlib`. Key 4 cannot
reuse key 3's token: its own binding differs, so the token's nonce does not
match.

### The token and the chain

The evidence is one Confidential Space token, `header.payload.signature` in
base64url. `at_claims` reads the payload's `iss`, `aud`, `exp`, `hwmodel`,
`dbgstat`, `eat_nonce`, `submods.container.image_digest` and
`submods.confidential_space.support_attributes`, skipping other claims.
`at_verify` runs NIP-ATT's client verification in order and returns the
first refusal:

<!-- source: coax/lib/nostr/att.coax verify -->
```jai
// NIP-ATT client verification, steps 1, 2, 4, 5 and 6, for verified events:
// release (its id and reading), head (its signer and reading, and the
// highest generation held), endpoint (its reading; its signer is checked by
// at_endpoint_read), at the reader's clock now. Step 3, the token's
// signature to Google's root, is not built, so the level is open.
at_verify :: (release_id:string,r:AtRelease,head_signer:string,h:AtHead,held:int,e:AtEndpoint,now:int)->AtVerdict {
  if e.code!=AtCode.ok {return AtVerdict.{code=e.code};}
  if r.code!=AtCode.ok {return AtVerdict.{code=r.code};}
  if h.code!=AtCode.ok {return AtVerdict.{code=h.code};}
  if head_signer!=r.publisher {return AtVerdict.{code=AtCode.head};}
  if h.workload!=r.workload {return AtVerdict.{code=AtCode.workload};}
  address:[160]u8;n:=json_raw(address[..],0,"30202:");n=json_raw(address[..],n,r.publisher);n=json_raw(address[..],n,":");n=json_raw(address[..],n,r.workload);
  if e.head!=cast(string)address[0..<n] {return AtVerdict.{code=AtCode.head};}
  if e.release!=release_id {return AtVerdict.{code=AtCode.release};}
  code:=at_generation(h,held);if code!=AtCode.ok {return AtVerdict.{code=code};}
  code=at_admitted(h,release_id,r.published_at,now);if code!=AtCode.ok {return AtVerdict.{code=code};}
  if now>e.valid_until {return AtVerdict.{code=AtCode.expired};}
  bound:[64]u8;
  if !at_binding(e.endpoint,e.release,bound[..]) || e.binding!=cast(string)bound[..] {return AtVerdict.{code=AtCode.binding};}
  buf:[AT_PAYLOAD]u8;payload:=at_payload(e.token,buf[..]);
  c:=at_claims(payload,e.binding);
  if !c.ok {return AtVerdict.{code=AtCode.token};}
  if !c.iss {return AtVerdict.{code=AtCode.issuer};}
  if !c.aud {return AtVerdict.{code=AtCode.audience};}
  if !c.dbg {return AtVerdict.{code=AtCode.debug};}
  if c.hwmodel!=r.hwmodel {return AtVerdict.{code=AtCode.hwmodel};}
  if !c.stable || r.support!="STABLE" {return AtVerdict.{code=AtCode.support};}
  if c.image!=r.image {return AtVerdict.{code=AtCode.measurement};}
  if !c.nonce {return AtVerdict.{code=AtCode.nonce};}
  if c.exp<e.valid_until {return AtVerdict.{code=AtCode.window};}
  return AtVerdict.{code=AtCode.ok,level=AtLevel.open};
}
// Whether data whose policy requires level required may go to this
// endpoint. Never falls back to a lower level.
at_may_send :: (v:AtVerdict,required:AtLevel)->bool {
  return v.code==AtCode.ok && cast(int)v.level>=cast(int)required;
}
```

Step 3 of NIP-ATT, verifying the token's RS256 signature and certificate
chain to Google's attestation root with issuer
`https://confidentialcomputing.googleapis.com`, is not built. So even an
endpoint whose every claim matches gets level `open`, and `at_may_send`
refuses it for data that needs `tee` or `tee-cloud`. A client that took
the claims at face value would be trusting whoever wrote the token.

## Sealed personal data

### Keys and slots

A vault's master key VMK is 32 random bytes on the person's first device.
A key slot holds it under one unlock method. The slot's wrapping key is
HKDF-SHA256 with the method secret as input key material, the 32 bytes of
the slot id as salt, and `openagents.vault.v1/slot`, a zero byte and the
method name as info. The slot's `vmk` is AES-256-GCM of the VMK under that
key, with the slot's own JCS bytes, minus `nonce` and `vmk`, as associated
data, so the label, method and parameters cannot be changed without the
tag failing:

<!-- source: coax/lib/nostr/vault.coax slot_open -->
```jai
// The slot's wrapping key is HKDF-SHA256(ikm = the method secret, salt =
// the 32 bytes of the slot id, info = "openagents.vault.v1/slot" 0 method);
// vmk is AES-256-GCM of the VMK under it with the slot's associated data.
// A wrong secret, or any changed byte of the slot, fails the tag.
vt_slot_open :: (text:string,secret:[]u8,vmk:[]u8)->VtCode {
  kdf:[1]VtKdf;return vt_slot_open_work(text,secret,vmk,kdf[..]);
}
vt_slot_open_work :: (text:string,secret:[]u8,vmk:[]u8,kdf:[]VtKdf)->VtCode {
  aad:[VT_AAD_MAX]u8;s:=vt_slot_read(text,aad[..]);
  if s.code!=VtCode.ok {return s.code;}
  if secret.count<32 || secret.count>VT_SECRET_MAX {return VtCode.secret;}
  if vmk.count<32 {return VtCode.room;}
  nonce:[12]u8;sealed:[48]u8;
  if !vt_b64(s.nonce,nonce[..],12) || !vt_b64(s.vmk,sealed[..],48) {return VtCode.field;}
  salt:[32]u8;vt_bytes(s.slot,salt[..]);
  info:[64]u8;n:=json_raw(info[..],0,"openagents.vault.v1/slot");n=json_raw(info[..],n+1,s.method);  // info[24] stays 0
  ok:=hkdf_extract_checked(salt[..],secret,kdf[0].prk[..],false)==SHA_OK && hkdf_expand_checked(kdf[0].prk[..],info[0..<n],kdf[0].key[..],32,false)==SHA_OK
    && aes256_gcm_open(kdf[0].key[..],nonce[..],aad[0..<s.aad],sealed[..],vmk)==32;
  kdf[0]=VtKdf.{};
  return ifx ok then VtCode.ok else VtCode.slot;
}
```

Two methods are built. `device`: the secret is 32 bytes the host keeps in
its keychain. `nostr`: the secret is 32 bytes, and the slot's
`params.sealed` is NIP-44 v2 of their 64 hex digits, encrypted by the
person's key to its own public key; only that key opens it.

From the VMK come `K_user_wrap` and the public `user_key_id`:

<!-- source: coax/lib/nostr/vault.coax user_keys -->
```jai
// K_user_wrap = HKDF-SHA256(ikm = VMK, salt = empty, info =
// "openagents.vault.v1/user-wrap"); user_key_id = the first 16 bytes of
// SHA-256("openagents.vault.v1/key-id" 0 K_user_wrap), as 32 hex digits.
vt_user_keys :: (vmk:[]u8,kwrap:[]u8,key_id:[]u8)->bool {
  kdf:[1]VtKdf;return vt_user_keys_work(vmk,kwrap,key_id,kdf[..]);
}
vt_user_keys_work :: (vmk:[]u8,kwrap:[]u8,key_id:[]u8,kdf:[]VtKdf)->bool {
  info:[29]u8;i0:=json_raw(info[..],0,"openagents.vault.v1/user-wrap");
  ok:=hkdf_extract_checked(vmk[0..<0],vmk,kdf[0].prk[..],false)==SHA_OK && hkdf_expand_checked(kdf[0].prk[..],info[0..<i0],kwrap,32,false)==SHA_OK;
  msg:[59]u8;n:=json_raw(msg[..],0,"openagents.vault.v1/key-id");   // msg[26] stays 0
  for i:0..<32 {msg[n+1+i]=kwrap[i];}
  digest:[32]u8;ok=ok && sha256_hash_checked(msg[..],digest[..],false)==SHA_OK;
  hex:="0123456789abcdef";
  for i:0..<16 {key_id[2*i]=hex[cast(int)digest[i]>>4];key_id[2*i+1]=hex[cast(int)digest[i]&15];}
  kdf[0]=VtKdf.{};
  return ok;
}
```

### Objects

An object is `OAVAULT1`, a big-endian 32-bit header length, the header
(exactly its JCS form), then the chunks. `core_digest` is the SHA-256 of
the header's `core` member and binds every chunk and every wrap. A user
wrap is AES-256-GCM of the DEK under `K_user_wrap` with associated data
`openagents.vault.v1/wrap-user`, a zero byte and `core_digest`. Chunk `i`
is AES-256-GCM under the DEK with nonce `nonce_prefix (7) || be32(i) ||
last (1)` and associated data `openagents.vault.v1/chunk`, a zero byte and
`core_digest`. The sizes alone tell the reader where chunks end:

<!-- source: coax/lib/nostr/vault.coax spans -->
```jai
// Every chunk but the last is chunk_bytes + 16 bytes; the last is 16 to
// chunk_bytes + 16, and more than 16 unless it is the only chunk.
vt_spans_ok :: (n:int,chunk_bytes:int,chunks:int)->bool {
  last:=n-(chunks-1)*(chunk_bytes+16);
  return last>=16 && last<=chunk_bytes+16 && (chunks==1 || last>16);
}
```

`vt_open` walks the chain and refuses at the first link that fails,
leaving nothing in the output:

<!-- source: coax/lib/nostr/vault.coax open -->
```jai
// Slot -> VMK -> K_user_wrap -> the user wrap whose key is user_key_id ->
// DEK -> every chunk. Nothing is left in out unless every link holds.
vt_open :: (blob:[]u8,slot:string,secret:[]u8,out:[]u8)->VtResult {
  work:[1]VtWork;return vt_open_work(blob,slot,secret,out,work[..]);
}
vt_open_work :: (blob:[]u8,slot:string,secret:[]u8,out:[]u8,work:[]VtWork)->VtResult {
  o:=vt_object_read(blob);if o.code!=VtCode.ok {return VtResult.{code=o.code};}
  aad:[VT_AAD_MAX]u8;s:=vt_slot_read(slot,aad[..]);if s.code!=VtCode.ok {return VtResult.{code=s.code};}
  if s.vault!=o.vault {return VtResult.{code=VtCode.vault};}
  if out.count<blob.count-o.body-16*o.chunks {return VtResult.{code=VtCode.room};}
  code:=vt_slot_open_work(slot,secret,work[0].vmk[..],work[0].kdf[..]);
  if code==VtCode.ok {code=vt_unwrap(o,work);}
  cad:[58]u8;n:=json_raw(cad[..],0,"openagents.vault.v1/chunk");for i:0..<32 {cad[n+1+i]=o.digest[i];}
  at:=o.body;written:=0;i:=0;
  while code==VtCode.ok && i<o.chunks {
    last:=i==o.chunks-1;size:=ifx last then blob.count-at else o.chunk_bytes+16;
    nonce:[12]u8;vt_chunk_nonce(o.prefix,i,last,nonce[..]);
    got:=aes256_gcm_open(work[0].dek[..],nonce[..],cad[..],blob[at..<at+size],out[written..<out.count]);
    if got==-1 {code=VtCode.chunk;} else {written+=got;at+=size;i+=1;}
  }
  work[0]=VtWork.{};
  if code!=VtCode.ok {for k:0..<written {out[k]=0;}return VtResult.{code=code};}
  return VtResult.{code=VtCode.ok,written=written};
}
// VMK -> K_user_wrap and user_key_id -> the DEK from the wrap with that key.
vt_unwrap :: (o:VtObject,work:[]VtWork)->VtCode {
  id:[32]u8;
  if !vt_user_keys_work(work[0].vmk[..],work[0].kwrap[..],id[..],work[0].kdf[..]) {return VtCode.wrap;}
  wad:[62]u8;n:=json_raw(wad[..],0,"openagents.vault.v1/wrap-user");for i:0..<32 {wad[n+1+i]=o.digest[i];}
  wraps:=o.wraps;
  for w:wraps[0..<o.count] {
    nonce:[12]u8;sealed:[48]u8;
    if w.key==cast(string)id[..] && vt_b64(w.nonce,nonce[..],12) && vt_b64(w.dek,sealed[..],48)
      && aes256_gcm_open(work[0].kwrap[..],nonce[..],wad[..],sealed[..],work[0].dek[..])==32 {return VtCode.ok;}
  }
  return VtCode.wrap;
}
// Chunk i's nonce: nonce_prefix (7 bytes) || be32(i) || 1 for the last chunk, else 0.
vt_chunk_nonce :: (prefix:[7]u8,i:int,last:bool,nonce:[]u8) {
  for k:0..<7 {nonce[k]=prefix[k];}
  for k:0..<4 {nonce[7+k]=cast(u8)((i>>(8*(3-k)))&255);}
  nonce[11]=ifx last then cast(u8)1 else cast(u8)0;
}
```

Each step keeps its key material (the HKDF pseudorandom key, the wrapping
key, the VMK, `K_user_wrap` and the DEK) in a work value the caller
passes in, and sets it to zero before every return after it is written.
The public procedures (`vt_slot_open`, `vt_user_keys`, `vt_nostr_secret`
and `vt_open`) hold that value on their own stack; the checks call the
`*_work` forms and read the value afterwards, so a wipe that was left out
fails a check. As in `aes.coax` and `nip44-key.coax`, these are plain
stores: erasure the C compiler cannot drop is the host's contract
([coax/lib/crypto/zeroize.coax](../../coax/lib/crypto/zeroize.coax)).
`vt_open` checks the output has room before it opens the slot, and a
method secret is 32 to 1024 bytes (`VT_SECRET_MAX`), so neither HKDF
step can refuse.

### Worked example: the object, by hand

The plaintext is 1060 bytes and `chunk_bytes` is 1024, so there are two
chunks: chunk 0 holds 1024 bytes and is 1024 + 16 = 1040 bytes of body;
chunk 1 holds 36 bytes and is 52 bytes, with `last` = 1 in its nonce. The
body is 1092 bytes; with the 12 bytes of magic and length and the 516-byte
header the object is 1620 bytes. Dropping the last chunk leaves 1040 bytes
for two chunks: the last would be 0 bytes, below the 16-byte tag, so the
reader refuses `chunks` before it decrypts anything. The slot's associated
data is 534 bytes: the 634-byte slot without its `nonce` member (26 bytes
and a comma, 16 base64 digits for 12 bytes) and its `vmk` member (72 bytes
and a comma, 64 base64 digits for the 32-byte VMK and its 16-byte tag). `core_digest` is
`f8664f07...31aa5946`, and `user_key_id` is
`027fdf6029d13db20b2462c5424fcc73`, the first 16 bytes of
SHA-256(`openagents.vault.v1/key-id` 0 `K_user_wrap`).

## What is not built

- NIP-ATT: the token's signature and certificate chain (client step 3), so
  no endpoint gets a level above `open`; raw TDX quotes, SEV-SNP reports,
  NVIDIA evidence and App Attest; the `tdx`, `sev-snp` and
  `apple-app-attest` platforms and GPU releases (refused); HPKE keys
  (refused); transparency entries and Rekor; independent rebuilds (3203);
  personal approval sets; sealed jobs, HPKE carriage, sealed answers and
  sealed-job receipts.
- NIP-VAULT: the `passkey-prf`, `recovery` (scrypt) and `pairing` slot
  methods (refused); tiers `sealed` (HPKE mode_psk to an attested system
  key, which needs X25519) and `operator` (KMS) (refused); system key
  certificates; the key index and its epochs; the service API; leases and
  revocation; the enough-slots rule.
- Hardware evidence is the vendors' trust chain; nothing here verifies it.

## The example

[nostr/examples/attested-vault.coax](../examples/attested-vault.coax) reads eight events and one vault
object written by `coax/tests/nostr/att_vault_reference.py`. Keys are the
scalars 1 (publisher), 2 (the vault's owner), 3 (the endpoint key) and 4
(the operator), test keys only; the token is a test token not signed by
Google; the vault secrets are hashes of fixed phrases. It verifies each
event, reads it, checks the endpoint at T0, refuses seven endpoints that do
not match, opens the object with the nostr slot, and refuses six other ways
in.

<!-- source: nostr/examples/attested-vault.coax example -->
```jai
T0 :: 1792000000;   // the reader's clock
PUBLISHER :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";   // scalar 1
PERSON :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";      // scalar 2
ENDPOINT_KEY :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"; // scalar 3

AT_WORDS :: [34]string.["ok","json","version","field","tags","marker","digest","workload","publisher","image","platform","gpu","times","head",
  "generation","listed","early","retired","notice","release","endpoint","hpke","binding","evidence","token","issuer","audience","measurement",
  "hwmodel","support","debug","nonce","expired","window"];
VT_WORDS :: [19]string.["ok","json","canonical","version","field","vault","method","params","label","secret","slot","magic","header","tier",
  "content","wrap","chunks","chunk","room"];
NE_WORDS :: [9]string.["ok","json","shape","limit","hex","output","alias","id","signature"];
LEVEL_WORDS :: [4]string.["open","hardened","tee","tee-cloud"];

to_hex :: (bytes:[]u8,out:[]u8)->string {r:=hex_encode_checked(cast(string)bytes,out,false);return cast(string)out[0..<r.written];}
// A verified event: error is NE_OK only when the id recomputes and the
// BIP-340 signature verifies under pubkey.
Opened :: struct {error:int;id:string;signer:string;kind:int;tags:string;content:string;}
Bufs :: struct {id:[64]u8;signer:[64]u8;content:[4096]u8;}
open_event :: (event:string,b:[]Bufs)->Opened {
  o:Opened;scratch:[16384]u8;e:=nostr_event_verify_vartime(event,scratch[..],false);o.error=e.error;
  if e.error!=NE_OK {return o;}
  o.id=to_hex(e.id[..],b[0].id[..]);o.signer=to_hex(e.public_key[..],b[0].signer[..]);o.kind=e.kind;
  o.tags=event[e.tags_from..<e.tags_to];
  n:=json_unescape(event[e.content_from..<e.content_to],b[0].content[..]);#assert n>=0;o.content=cast(string)b[0].content[0..<n];
  return o;
}
// The lowercase SHA-256 of text, as the x tag carries it.
digest_hex :: (text:string,out:[]u8)->string {
  b:[4096]u8;for i:0..<text.count {b[i]=text[i];}
  d:[32]u8;#assert sha256_hash_checked(b[0..<text.count],d[..],false)==SHA_OK;return to_hex(d[..],out);
}
// An endpoint event checked against the release it names and the head.
check :: (event:string,release_id:string,r:AtRelease,head_signer:string,h:AtHead,held:int,now:int)->AtVerdict {
  b:[1]Bufs;o:=open_event(event,b[..]);
  if o.error!=NE_OK {return AtVerdict.{code=AtCode.endpoint};}
  return at_verify(release_id,r,head_signer,h,held,at_endpoint_read(o.signer,o.tags,o.content),now);
}
// A copy of text with the byte at i replaced by c.
altered :: (text:string,i:int,c:u8,out:[]u8)->string {for k:0..<text.count {out[k]=text[k];}out[i]=c;return cast(string)out[0..<text.count];}
find :: (text:string,what:string)->int {for i:0..<text.count-what.count+1 {if text[i..<i+what.count]==what {return i;}}return -1;}
```

<!-- source: nostr/examples/attested-vault.coax main -->
```jai
main :: () {
  print("the release (key 1, kind 3202)\n");
  rb:[1]Bufs;orl:=open_event(RELEASE,rb[..]);x:[64]u8;
  print("verify: %, id %, signer is the publisher: %\n",NE_WORDS[orl.error],orl.id,orl.signer==PUBLISHER);
  print("content % bytes, x = sha256(content) = %\n",orl.content.count,digest_hex(orl.content,x[..]));
  r:=at_release_read(orl.signer,orl.tags,orl.content,digest_hex(orl.content,x[..]));
  print("read: %, workload %, image %\n",AT_WORDS[cast(int)r.code],r.workload,r.image);
  print("platform gcp-confidential-space, hwmodel %, support %, published_at T0 - %\n",r.hwmodel,r.support,T0-r.published_at);

  print("the release head (key 1, kind 30202)\n");
  hb:[1]Bufs;ohd:=open_event(HEAD,hb[..]);
  print("verify: %, id %\n",NE_WORDS[ohd.error],ohd.id);
  h:=at_head_read(ohd.signer,ohd.tags,ohd.content);
  a:=h.admitted[0];
  print("read: %, generation %, notice_seconds %, admitted % release, effective_at T0 - %, retire_at %\n",AT_WORDS[cast(int)h.code],
    h.generation,h.notice,h.count,T0-a.effective_at,ifx a.retire_at<0 then "null" else "set");
  print("effective_at - published_at = % = notice_seconds: %\n",a.effective_at-r.published_at,a.effective_at-r.published_at==h.notice);
  for at:[3]int.[0,-95200,-95201] {
    print("admitted at T0 % %: %\n",ifx at<0 then "-" else "+",ifx at<0 then -at else at,AT_WORDS[cast(int)at_admitted(h,orl.id,r.published_at,T0+at)]);
  }

  print("the endpoint (key 3, kind 30203)\n");
  eb:[1]Bufs;oep:=open_event(ENDPOINT,eb[..]);
  print("verify: %, id %, signer is the endpoint key: %\n",NE_WORDS[oep.error],oep.id,oep.signer==ENDPOINT_KEY);
  e:=at_endpoint_read(oep.signer,oep.tags,oep.content);
  print("read: %, release is the one above: %, issued_at T0 - %, valid_until T0 + %\n",AT_WORDS[cast(int)e.code],e.release==orl.id,T0-e.issued_at,e.valid_until-T0);
  bound:[64]u8;#assert at_binding(e.endpoint,e.release,bound[..]);
  print("binding = sha256(\"openagents.att.v1\" 0 endpoint release) = %, equal: %\n",cast(string)bound[..],cast(string)bound[..]==e.binding);
  payload:[AT_PAYLOAD]u8;claims:=at_payload(e.token,payload[..]);
  print("token claims (% bytes, signature not checked): %\n",claims.count,claims);
  v:=at_verify(orl.id,r,ohd.signer,h,0,e,T0);
  print("verify the chain at T0: %, level %\n",AT_WORDS[cast(int)v.code],LEVEL_WORDS[cast(int)v.level]);
  print("may send data that needs open: %, tee: %, tee-cloud: %\n",at_may_send(v,AtLevel.open),at_may_send(v,AtLevel.tee),at_may_send(v,AtLevel.tee_cloud));

  print("endpoints that do not match the head\n");
  r2b:[1]Bufs;or2:=open_event(RELEASE2,r2b[..]);x2:[64]u8;
  r2:=at_release_read(or2.signer,or2.tags,or2.content,digest_hex(or2.content,x2[..]));
  print("release 2 (published T0 - %, another image), read %, in the head: %\n",T0-r2.published_at,AT_WORDS[cast(int)r2.code],at_admitted(h,or2.id,r2.published_at,T0)==AtCode.ok);
  print("an endpoint running release 2: %\n",AT_WORDS[cast(int)check(EP_UNLISTED,or2.id,r2,ohd.signer,h,0,T0).code]);
  print("an endpoint whose token reports another image: %\n",AT_WORDS[cast(int)check(EP_WRONG_IMAGE,orl.id,r,ohd.signer,h,0,T0).code]);
  print("an endpoint whose token is a debug image, not STABLE: %\n",AT_WORDS[cast(int)check(EP_DEBUG,orl.id,r,ohd.signer,h,0,T0).code]);
  print("key 4 claiming key 3's token: %\n",AT_WORDS[cast(int)check(EP_OTHER_KEY,orl.id,r,ohd.signer,h,0,T0).code]);
  print("the good endpoint with generation 8 already held: %\n",AT_WORDS[cast(int)check(ENDPOINT,orl.id,r,ohd.signer,h,8,T0).code]);
  print("the good endpoint at T0 + 3541: %\n",AT_WORDS[cast(int)check(ENDPOINT,orl.id,r,ohd.signer,h,0,T0+3541).code]);
  print("the good endpoint with a head signed by key 3: %\n",AT_WORDS[cast(int)check(ENDPOINT,orl.id,r,ENDPOINT_KEY,h,0,T0).code]);
  out:[8192]u8;
  forged:=altered(ENDPOINT,ENDPOINT.count-3,ifx ENDPOINT[ENDPOINT.count-3]=='0' then cast(u8)'1' else cast(u8)'0',out[..]);
  fb:[1]Bufs;print("the endpoint with one signature digit changed: verify %\n",NE_WORDS[open_event(forged,fb[..]).error]);

  print("the vault object (tier user, 2 chunks)\n");
  blob:[2048]u8;br:=base64_decode_checked(OBJECT,blob[..],false);#assert br.error==NC_OK;object:=blob[0..<br.written];
  o:=vt_object_read(object);
  print("% bytes: magic OAVAULT1, header % bytes, body % bytes; read %, chunks % of %, wraps %\n",object.count,o.body-12,object.count-o.body,
    VT_WORDS[cast(int)o.code],o.chunks,o.chunk_bytes,o.count);
  cd:[64]u8;print("core_digest %\n",to_hex(o.digest[..],cd[..]));
  aad:[VT_AAD_MAX]u8;s:=vt_slot_read(SLOT_NOSTR,aad[..]);
  print("the nostr slot: read %, method %, associated data % bytes (the slot without nonce and vmk)\n",VT_WORDS[cast(int)s.code],s.method,s.aad);
  // key 2 opens its own NIP-44 seal: conversation key of key 2 with itself
  sec:[32]u8;sec[31]=2;pub:[32]u8;hb2:=hex_decode_checked(PERSON,pub[..],false);#assert hb2.error==NC_OK;
  work:[1]EcdhScratch;ck:[32]u8;#assert nip44_conversation_key_from_keys_checked(sec[..],pub[..],work[..],ck[..],false)==NIP44_OK;
  secret:[32]u8;print("NIP-44 opens the slot's sealed secret with key 2: %\n",vt_nostr_secret(s.sealed,ck[..],secret[..]));
  plain:[2048]u8;res:=vt_open(object,SLOT_NOSTR,secret[..],plain[..]);
  pd:[64]u8;
  print("open with the nostr slot: %, % bytes, sha256 %\n",VT_WORDS[cast(int)res.code],res.written,digest_hex(cast(string)plain[0..<res.written],pd[..]));
  first:=0;while plain[first]!='\n' {first+=1;}
  print("first line: %\n",cast(string)plain[0..<first]);

  print("refusals\n");
  print("the device slot with the nostr slot's secret: %\n",VT_WORDS[cast(int)vt_open(object,SLOT_DEVICE,secret[..],plain[..]).code]);
  t:[2048]u8;at:=find(SLOT_NOSTR,"Key 2 (test)");
  print("the nostr slot with its label changed to Key 3 (test): %\n",VT_WORDS[cast(int)vt_open(object,altered(SLOT_NOSTR,at+4,'3',t[..]),secret[..],plain[..]).code]);
  at=find(SLOT_NOSTR,"\"vault\":\"5c47");
  print("the nostr slot for vault 6c47...: %\n",VT_WORDS[cast(int)vt_open(object,altered(SLOT_NOSTR,at+9,'6',t[..]),secret[..],plain[..]).code]);
  print("the object without its last chunk: %\n",VT_WORDS[cast(int)vt_open(object[0..<o.body+1040],SLOT_NOSTR,secret[..],plain[..]).code]);
  object[o.body+100]^=1;
  print("one bit of chunk 0 flipped: %\n",VT_WORDS[cast(int)vt_open(object,SLOT_NOSTR,secret[..],plain[..]).code]);
  object[o.body+100]^=1;
  object[11]^=1;
  print("the header length one byte off: %\n",VT_WORDS[cast(int)vt_open(object,SLOT_NOSTR,secret[..],plain[..]).code]);
  object[11]^=1;
  for i:0..<32 {secret[i]=0;ck[i]=0;}
}
```

Its output, the same in debug and release
(`nostr/fixtures/attested-vault.out`):

```text
the release (key 1, kind 3202)
verify: ok, id 97263cdd56d438b8a5a2719dab4cc41d62ea7135522c8a838ebf5e82060d504a, signer is the publisher: true
content 806 bytes, x = sha256(content) = 0a4c0516008f3a460bd5a0c148f3625fb2918a4d893fd7fc64d1aa694544b2b8
read: ok, workload vault, image sha256:9497a089dc980f4251ab27f5e62c772cfabd0b02b9a8219b4539f86111f288f4
platform gcp-confidential-space, hwmodel GCP_INTEL_TDX, support STABLE, published_at T0 - 700000
the release head (key 1, kind 30202)
verify: ok, id e6d0a5d59831d5464c76d5e1039f2235eb46d83732772c42061f3c9acecbe3db
read: ok, generation 7, notice_seconds 604800, admitted 1 release, effective_at T0 - 95200, retire_at null
effective_at - published_at = 604800 = notice_seconds: true
admitted at T0 + 0: ok
admitted at T0 - 95200: ok
admitted at T0 - 95201: early
the endpoint (key 3, kind 30203)
verify: ok, id 2ea538e2569a137ca38a7e9a6096dbe992454ef6eeaa508ba22f36e663a156cd, signer is the endpoint key: true
read: ok, release is the one above: true, issued_at T0 - 60, valid_until T0 + 3540
binding = sha256("openagents.att.v1" 0 endpoint release) = 5312236c3e5543abaa57fae6c8e37fcad8e22b9965e06311b9c1de1b6fd74035, equal: true
token claims (471 bytes, signature not checked): {"aud":"openagents.att.v1","dbgstat":"disabled-since-boot","eat_nonce":["5312236c3e5543abaa57fae6c8e37fcad8e22b9965e06311b9c1de1b6fd74035"],"exp":1792003600,"hwmodel":"GCP_INTEL_TDX","iat":1791999940,"iss":"https://confidentialcomputing.googleapis.com","submods":{"confidential_space":{"support_attributes":["LATEST","STABLE","USABLE"]},"container":{"image_digest":"sha256:9497a089dc980f4251ab27f5e62c772cfabd0b02b9a8219b4539f86111f288f4"}},"swname":"CONFIDENTIAL_SPACE"}
verify the chain at T0: ok, level open
may send data that needs open: true, tee: false, tee-cloud: false
endpoints that do not match the head
release 2 (published T0 - 100000, another image), read ok, in the head: false
an endpoint running release 2: listed
an endpoint whose token reports another image: measurement
an endpoint whose token is a debug image, not STABLE: debug
key 4 claiming key 3's token: nonce
the good endpoint with generation 8 already held: generation
the good endpoint at T0 + 3541: expired
the good endpoint with a head signed by key 3: head
the endpoint with one signature digit changed: verify signature
the vault object (tier user, 2 chunks)
1620 bytes: magic OAVAULT1, header 516 bytes, body 1092 bytes; read ok, chunks 2 of 1024, wraps 1
core_digest f8664f07d3a40f465d7c05f2b5ce3b355f72d5658ee6673e311e6eb831aa5946
the nostr slot: read ok, method nostr, associated data 534 bytes (the slot without nonce and vmk)
NIP-44 opens the slot's sealed secret with key 2: true
open with the nostr slot: ok, 1060 bytes, sha256 590d186ab08b0b0c09666f8b3a192d9a2866e0a581e0994752453a58e1e283bb
first line: Statement for the chapter 83 vault: opening balance 21000 sat, closing balance 34000 sat. Test data only.
refusals
the device slot with the nostr slot's secret: slot
the nostr slot with its label changed to Key 3 (test): slot
the nostr slot for vault 6c47...: vault
the object without its last chunk: chunks
one bit of chunk 0 flipped: chunk
the header length one byte off: canonical
```

The refusals read in order: release 2 is in no head (`listed`); a token
reporting another image (`measurement`); a debug token without `STABLE`
(`debug`); key 4 presenting key 3's token (`nonce`); generation 8 already
held (`generation`); one second past `valid_until` (`expired`); a head
signed by key 3, not the release's publisher (`head`); one signature digit
(`signature`). For the vault: the device slot with the nostr slot's secret
and the nostr slot with its label edited both fail the slot's tag (`slot`);
a slot for vault `6c47...` (`vault`); the object without its last chunk
(`chunks`); one flipped bit of chunk 0 (`chunk`); and a header length one
byte off, which takes one byte of the body into a header that is then not
its own JCS form (`canonical`).

## How the code is checked

### Checks in the example

`nostr/examples/checks/attested-vault.coax` compares the Coax results with
the Python reference, which shares no code with Coax:

<!-- source: nostr/examples/checks/attested-vault.coax check -->
```jai
// From coax/tests/nostr/att_vault_reference.py, written from NIP-ATT,
// NIP-VAULT, FIPS-197, SP 800-38D, RFC 5869, NIP-01, NIP-44 and BIP-340
// with hashlib, hmac and integers; it shares no code with Coax. It signed
// every event (aux 0, and its own verifier accepts each), and it printed
// each id, each content's SHA-256 and length, the binding, user_key_id, the
// object's core_digest, and the opened plaintext's length and SHA-256. Its
// AES-256-GCM reproduces GCM spec test case 16 before it writes anything.
PY_EVENTS :: [8]string.[RELEASE,RELEASE2,HEAD,ENDPOINT,EP_UNLISTED,EP_WRONG_IMAGE,EP_DEBUG,EP_OTHER_KEY];
PY_ID :: [8]string.[
  "97263cdd56d438b8a5a2719dab4cc41d62ea7135522c8a838ebf5e82060d504a",
  "9f726a76dec4d66d197f6b0ed5483516df6567accc3643ffa6cf1f72044179fd",
  "e6d0a5d59831d5464c76d5e1039f2235eb46d83732772c42061f3c9acecbe3db",
  "2ea538e2569a137ca38a7e9a6096dbe992454ef6eeaa508ba22f36e663a156cd",
  "df8b864fecd981bbf26d4a29ae2a4909993a48add0792999726e21dcb72273e1",
  "4a46691c32e300b55d6c351917473f33c35551592a6d540f14484c7e5b213ed6",
  "a3ee6a8e52c15fed3c38b8b96b56ef9069cc903d4dcd552d5482e844313b7739",
  "6c714a51b029fc2b382c0f7f146caccc941a1cab86b0d87364aa4731c0750218"];
PY_X :: [8]string.[
  "0a4c0516008f3a460bd5a0c148f3625fb2918a4d893fd7fc64d1aa694544b2b8",
  "982fd7bc4c1e03a41f6e08a192157bcd3459b535010d485b0d02b427bcfdfa1e",
  "f184ce2bcf146aef6d11b97267403f581931fdd2489661582f7f8c00a65a4ce5",
  "4514553b38bbdd955af43c43e2c42f7a26f98da474fef7b6ff3c959a5e2ce2d7",
  "cb90ba726c944a3eb769929fa1270ddb51d328f42ee2d976d05d35eb84a3ffbe",
  "f507095ce82dc71cab3466525917a75bd4cc0fbd98c16f2204f92f0224974361",
  "5fa4169f1c4d27c220fca60e21b37ebd74de684630c886b96aefccde6a32b2a0",
  "23129bbdf5cac5de32deedd94064733a502910bb15732d0bcc366bc3cf25bb53"];
PY_LEN :: [8]int.[806,806,254,1238,1238,1238,1210,1238];
PY_BINDING :: "5312236c3e5543abaa57fae6c8e37fcad8e22b9965e06311b9c1de1b6fd74035";
PY_KEY_ID :: "027fdf6029d13db20b2462c5424fcc73";
PY_CORE_DIGEST :: "f8664f07d3a40f465d7c05f2b5ce3b355f72d5658ee6673e311e6eb831aa5946";
PY_PLAIN_SHA :: "590d186ab08b0b0c09666f8b3a192d9a2866e0a581e0994752453a58e1e283bb";

// Read an event without its signature: the id must recompute.
read_only :: (event:string,b:[]Bufs)->Opened {
  o:Opened;scratch:[16384]u8;e:=nostr_event_read(event,scratch[..],false);o.error=e.error;
  if e.error!=NE_OK {return o;}
  o.id=to_hex(e.id[..],b[0].id[..]);o.signer=to_hex(e.public_key[..],b[0].signer[..]);o.kind=e.kind;
  o.tags=event[e.tags_from..<e.tags_to];
  n:=json_unescape(event[e.content_from..<e.content_to],b[0].content[..]);o.content=cast(string)b[0].content[0..<n];
  return o;
}

#test "every id and content digest matches the Python reference" #steps 200000000 {
  for i:0..<8 {
    b:[1]Bufs;o:=read_only(PY_EVENTS[i],b[..]);#assert o.error==NE_OK && o.id==PY_ID[i] && o.content.count==PY_LEN[i];
    x:[64]u8;#assert digest_hex(o.content,x[..])==PY_X[i];
  }
}
#test "the chain by hand: notice delay, binding, window" #steps 400000000 {
  b:[1]Bufs;orl:=read_only(RELEASE,b[..]);x:[64]u8;r:=at_release_read(orl.signer,orl.tags,orl.content,digest_hex(orl.content,x[..]));
  hb:[1]Bufs;ohd:=read_only(HEAD,hb[..]);h:=at_head_read(ohd.signer,ohd.tags,ohd.content);
  // published 700000 s before T0; effective exactly notice_seconds = 604800 later, 95200 s before T0
  #assert r.published_at==T0-700000 && h.admitted[0].effective_at==T0-95200 && h.admitted[0].effective_at-r.published_at==604800;
  #assert at_admitted(h,orl.id,r.published_at,T0-95200)==AtCode.ok && at_admitted(h,orl.id,r.published_at,T0-95201)==AtCode.early;
  #assert at_admitted(h,orl.id,r.published_at+1,T0)==AtCode.notice;
  eb:[1]Bufs;oep:=read_only(ENDPOINT,eb[..]);e:=at_endpoint_read(oep.signer,oep.tags,oep.content);
  bound:[64]u8;#assert at_binding(ENDPOINT_KEY,orl.id,bound[..]) && cast(string)bound[..]==PY_BINDING && e.binding==PY_BINDING;
  // issued 60 s before T0, valid for 3600 s, the token expiring 60 s after that
  #assert e.issued_at==T0-60 && e.valid_until==T0+3540 && e.valid_until-e.issued_at==AT_MAX_WINDOW;
  #assert at_verify(orl.id,r,ohd.signer,h,7,e,T0+3540).code==AtCode.ok && at_verify(orl.id,r,ohd.signer,h,7,e,T0+3541).code==AtCode.expired;
  // Done means: an endpoint whose attestation does not match a release head is refused
  r2b:[1]Bufs;or2:=read_only(RELEASE2,r2b[..]);x2:[64]u8;r2:=at_release_read(or2.signer,or2.tags,or2.content,digest_hex(or2.content,x2[..]));
  ub:[1]Bufs;ou:=read_only(EP_UNLISTED,ub[..]);
  #assert at_verify(or2.id,r2,ohd.signer,h,0,at_endpoint_read(ou.signer,ou.tags,ou.content),T0).code==AtCode.listed;
  wb:[1]Bufs;ow:=read_only(EP_WRONG_IMAGE,wb[..]);
  #assert at_verify(orl.id,r,ohd.signer,h,0,at_endpoint_read(ow.signer,ow.tags,ow.content),T0).code==AtCode.measurement;
}
#test "NIP-ATT's own examples are templates, not vectors: their placeholders refuse" {
  head:="{\"v\":\"openagents.att-head.v1\",\"requires\":[],\"workload\":\"vault\",\"generation\":7,\"notice_seconds\":604800,\"admitted\":[{\"release\":\"<event id of a 3202>\",\"effective_at\":0,\"retire_at\":null}],\"emergency\":null}";
  #assert at_head_read(PUBLISHER,"[[\"d\",\"vault\"],[\"t\",\"oa:att-head:v1\"]]",head).code==AtCode.head;
}
#test "the vault object by hand: core digest, user_key_id, two chunks, the plaintext" #steps 2000000000 {
  blob:[2048]u8;br:=base64_decode_checked(OBJECT,blob[..],false);object:=blob[0..<br.written];
  o:=vt_object_read(object);cd:[64]u8;
  #assert o.code==VtCode.ok && to_hex(o.digest[..],cd[..])==PY_CORE_DIGEST && object.count==1620 && o.body==12+516;
  // 1060 bytes in chunks of 1024: 1024 + 16 and 36 + 16 bytes of body
  #assert object.count-o.body==1040+52 && vt_spans_ok(1092,1024,2) && !vt_spans_ok(1040,1024,2);
  aad:[VT_AAD_MAX]u8;s:=vt_slot_read(SLOT_NOSTR,aad[..]);#assert s.aad==534;
  secret:[32]u8;sec:[32]u8;sec[31]=2;pub:[32]u8;hb2:=hex_decode_checked(PERSON,pub[..],false);
  work:[1]EcdhScratch;ck:[32]u8;#assert nip44_conversation_key_from_keys_checked(sec[..],pub[..],work[..],ck[..],false)==NIP44_OK;
  #assert vt_nostr_secret(s.sealed,ck[..],secret[..]);
  vmk:[32]u8;#assert vt_slot_open(SLOT_NOSTR,secret[..],vmk[..])==VtCode.ok;
  k:[32]u8;id:[32]u8;#assert vt_user_keys(vmk[..],k[..],id[..]) && cast(string)id[..]==PY_KEY_ID && o.wraps[0].key==PY_KEY_ID;
  plain:[2048]u8;r:=vt_open(object,SLOT_NOSTR,secret[..],plain[..]);pd:[64]u8;
  #assert r.code==VtCode.ok && r.written==1060 && digest_hex(cast(string)plain[0..<1060],pd[..])==PY_PLAIN_SHA;
  // Done means: a vault object opens only with a valid key slot
  #assert vt_open(object,SLOT_DEVICE,secret[..],plain[..]).code==VtCode.slot;
}
```

NIP-ATT's and NIP-VAULT's own examples are templates (`<64-hex pubkey>`,
`<event id of a 3202>`, times of 0), not vectors; the checks show the head
template's placeholder release refuses. There are no published ids to
compare.

### The units' own checks

- `aes_gcm.coax` (35 checks with the units it loads): FIPS-197 appendix
  C.3 (the forward cipher, round by round), GCM spec test cases 13 to 16
  for AES-256, refusal of a changed byte, key, nonce or associated data
  with nothing written, the size edges, CTR for texts of 0 to 48 bytes
  against each block's pad, the big-endian counter and length bytes, and
  the expanded key zero again after seal, open and a refused open.
- `att.coax` (77 checks): every reader refusal, the admission edges, the
  binding, every claim, token shapes and the chain.
- `vault.coax` (192 checks with the units it loads): slot fields and
  canonical form, every platform, secret lengths, slot opening, the nostr
  secret, the user keys, header refusals and trailing members, chunk spans
  for every size, chunk nonces for indexes 256, 65536 and above, the
  chapter's two-chunk object (a bad last chunk leaves nothing of the
  first), the chain with each broken link, and the key material zero again
  after each step.

Run on 2026-11-01 with the cached compiler
`bcefdc7fa553`: `att.coax` 77/77 checks and root-only mutation 1066/1066
killed; `vault.coax` 192/192 checks and root-only mutation 730/730 killed;
`aes_gcm.coax` 35/35 checks and root-only mutation 239/239 killed; the
example in debug and release, with the same output; the checks file
375/375.

## Summary

- A release names an image; a head admits releases under a notice delay,
  with a generation that never goes back; an endpoint's key is bound to a
  release by `SHA-256("openagents.att.v1" 0 || key || release)`.
- An endpoint whose release is not admitted, whose token names another
  image, a debug build, another binding, an expired window or a head from
  another key is refused. One whose claims all match still gets level
  `open` until the token's signature is verified to Google's root.
- A vault object opens only through a key slot: the slot's secret unwraps
  the VMK, the VMK derives `K_user_wrap`, a user wrap gives the DEK, and the
  DEK opens each chunk; changing a slot's label, the wrong secret, another
  vault, a missing chunk or one flipped bit each refuses.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [82. Pylons, environments and
  deploys](ch082-pylons-environments-and-deploys.md)

## Sources

- [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md)
- [OpenAgents NIP-VAULT](../nips/openagents/NIP-VAULT.md)
- FIPS-197 (AES) and NIST SP 800-38D (GCM), with the GCM specification's
  test cases.
- RFC 5869 (HKDF) and RFC 8785 (JCS).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
