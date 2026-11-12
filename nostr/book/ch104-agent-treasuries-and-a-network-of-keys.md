# 104. Agent treasuries and a network of keys

> **Status: partly implemented.** The pure contracts (NIP-KEYS step 1) and
> the signing loop (step 2) run: admission with every enrollment's BIP-322
> possession proof verified, the signer's checks, ECDSA partial
> signatures over BIP-143 digests, the PSBT with them, and the coordinator's
> 2-of-3 transaction, with worked examples, refusals and independent checks
> (BIP-143's and BIP-341's published vectors, libsecp256k1, a Python
> reference). Requests and receipts travel as 3188 artifacts over a Coax
> relay between three key agents and a coordinator; Bitcoin Core accepts and
> confirms the spend in regtest and on a private signet, and recovers the
> coins with its own descriptor wallet
> ([Over the network](#over-the-network)). One part of "Done means" is
> open: the spend has not been broadcast on the public signet, whose coins
> come only from a captcha-gated faucet. See [Not built](#not-built).

The chapter qualifies checking contracts on native CPU. It records no
external treasury consumer and no Wasm qualification. Those claims stay
separate from the public signet broadcast that the
[completion route](COMPLETION.md#5-finish-treasury-signing-and-independent-recovery)
still asks for.
The OpenAgents NIP-KEYS source-pin review remains pending; this chapter does
not approve a replacement digest.

An agent that earns and spends bitcoin needs somewhere safe to keep it. One
hot key on the agent, or one wallet host, is a single point of failure. This
chapter builds the alternative up to the network: a wallet whose coins
move only when enough independent key agents sign, described by records
that every party can rebuild and check alone, and the signatures that move
them.

Part VIII, Nostr in OpenAgents. Result: NIP-KEYS wallet, enrollment,
independence and signing-request bodies read under closed schemas; a
wallet admitted by recomputing its BIP-380 descriptor, applying the
four composition rules and verifying each enrollment's BIP-322 possession
proof; the wallet's P2WSH scripts derived from public keys
alone; and a signer that recomputes every output, the change and the fee
from the PSBT before it would sign, with nonce idempotency, recovery
timelocks, retired wallets, the attested co-signer's delay and the
hot-balance ceiling; then ECDSA signatures with RFC 6979 nonces over
BIP-143 digests, a PSBT carrying them, and the coordinator's final 2-of-3
transaction. BIP-341 Taproot key-path digests and signatures are built and
checked, for the `tr` wallets that come next.

## What this chapter teaches

- Why NIP-SOV's guardian quorum is a rule the host enforces, and why a
  multisig wallet moves that quorum into Bitcoin script.
- The BIP-380 descriptor checksum, and a wallet descriptor recomputed from
  the wallet's paths and enrollments with key origins and `<0;1>/*`
  multipath keys.
- The composition rules: no one operator meets a threshold, a sovereign
  agent never holds a primary-path majority, recovery paths are later and
  don't depend only on an attested key, and a weakening revision needs the
  primary quorum's approval.
- BIP-32 public derivation (`CKDpub`) and the `wsh(sortedmulti(...))`
  witness script, so a signer derives the wallet's addresses from xpubs.
- A possession proof: a BIP-322 signature by the key at `xpub/0/0` over the
  digest of the wallet, the agent and the xpub, so nobody enrolls a key
  they only saw, and a proof made for one wallet fails in another.
- A signer that never trusts the request: it reads a PSBT v0, checks every
  input is the wallet's, compares every output with the request's
  statement, derives change from the descriptor and recomputes the fee.
- Idempotency by `(agent, requester, nonce)` with the request digest.
- ECDSA with RFC 6979's deterministic nonce and low-S, under Bitcoin chapter 3's
  constant-time rules, and BIP-66's DER.
- The BIP-143 and BIP-341 signature digests, and the Taproot key tweak.
- A key agent's partial signatures (BIP-174 `PSBT_IN_PARTIAL_SIG`) and the
  coordinator's witness: an empty item, a threshold of signatures in key
  order, the witness script.
- What is not built, and why: `tr` wallets, PSBT v2, miniscript spends
  and a broadcast on the public signet.

## Where the code is

| File | What |
| --- | --- |
| [coax/lib/nostr/nipkeys.coax](../../coax/lib/nostr/nipkeys.coax) | NIP-KEYS: the checksum, the readers, xpub decoding and public derivation, admission, scripts, the PSBT reader and the signer's checks. `#authority` |
| [coax/lib/nostr/nipkeys_possess.coax](../../coax/lib/nostr/nipkeys_possess.coax) | Possession proofs, the rules: the message, the challenge, which answer counts, and admission with the answers. `#authority` |
| [coax/lib/nostr/nipkeys_proof.coax](../../coax/lib/nostr/nipkeys_proof.coax) | Possession proofs verified: the rules joined to BIP-322's verifier, with no decisions of its own |
| [coax/lib/bitcoin/bip322.coax](../../coax/lib/bitcoin/bip322.coax) | BIP-322 (Bitcoin chapter 96): the verifier possession proofs go through, and the signer a key agent makes one with. `#authority` |
| [coax/lib/nostr/nipkeys_sign.coax](../../coax/lib/nostr/nipkeys_sign.coax) | The key agent's signatures, its PSBT, and the coordinator's transaction. `#authority` |
| [coax/lib/crypto/ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | ECDSA: RFC 6979, low-S, DER. `#authority` |
| [coax/lib/bitcoin/sighash.coax](../../coax/lib/bitcoin/sighash.coax) | The transaction reader, the BIP-143 and BIP-341 digests, the Taproot key path. `#authority` |
| [coax/tests/nostr/keys_fixtures.coax](../../coax/tests/nostr/keys_fixtures.coax) | The treasury's records, written by an independent Python reference |
| [coax/tests/nostr/keys_sign_fixtures.coax](../../coax/tests/nostr/keys_sign_fixtures.coax) | The treasury's signed spend, written by [keys_sign_reference.py](../../coax/tests/nostr/keys_sign_reference.py) |
| [coax/tests/bitcoin/sighash_vectors.coax](../../coax/tests/bitcoin/sighash_vectors.coax) | BIP-143's examples and BIP-341's wallet vectors, copied by [make_sighash_vectors.py](../../coax/tests/bitcoin/make_sighash_vectors.py) |
| [coax/lib/nostr/nipkeys_carry.coax](../../coax/lib/nostr/nipkeys_carry.coax) | KEYS records sealed into, and opened from, 3188 private artifacts. `#authority` |
| [nostr/tools/treasury.coax](../tools/treasury.coax) | The key agent and the coordinator as a command-line tool over a relay |
| [coax/tests/nostr/treasury_regtest.py](../../coax/tests/nostr/treasury_regtest.py) | The network run against Bitcoin Core, in regtest or on a private signet |
| [nostr/examples/agent-treasury.coax](../examples/agent-treasury.coax) | The example |
| [nostr/examples/checks/agent-treasury.coax](../examples/checks/agent-treasury.coax) | Its independent checks |

The file reuses `json.coax` for parsing, `nipa3.coax` (chapter 73) to decode
segwit addresses, `bip32.coax` and `secp_mult.coax` (Bitcoin chapter 16 and chapter 46) for
the curve, and `sha256.coax` and `sha512.coax` for hashing.

## From an approval rule to a script

[NIP-SOV](../nips/openagents/NIP-SOV.md) lets guardians stop an agent: a
quorum approves an exact action, and the host enforces it. A host that
ignores the quorum, or a wallet host that fails, still loses the coins.
NIP-KEYS moves the quorum into the coins: a spend needs *m* signatures from
*n* keys, and Bitcoin script checks them. The host can't skip that check.

The parties are the **holder** (whose coins they are), the **key agents**
(each holds one key and signs under its own policy) and a **coordinator**
(stores the wallet description and routes requests, holds no key, and is
never the only copy). Every record travels as a signed, recipient-encrypted
`3188` artifact; KEYS allocates no event kinds. This chapter starts after
the transport: the caller has verified each artifact and passes its signer
and decrypted body.

This version supports `wsh(sortedmulti(...))` and, for a primary path with
one recovery path, `wsh(or_d(multi(...),and_v(v:older(n),multi(...))))`.
Taproot `tr(...)` refuses as `not_built`; MuSig2 and FROST are out of scope
in NIP-KEYS v1 and refuse as an unknown family.

## The descriptor and its checksum

A BIP-380 descriptor ends in `#` and eight characters. The checksum is a
BCH code over the descriptor's characters: each character's position in a
95-character input set gives a 5-bit symbol (`pos & 31`), and every three
characters also contribute a group symbol from their high bits (`pos >> 5`).

<!-- source: coax/lib/nostr/nipkeys.coax checksum -->
```jai
KY_INPUT :: "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ ";
KY_CHECK :: "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
KY_GEN :: [5] u64.[0xf5dee51989, 0xa9fdca3312, 0x1bab10e32d, 0x3706b1677a, 0x644d626ffd];
ky_polymod :: (c:u64,v:u64)->u64 #effects(all) {
  top:=c>>35;r:=((c&0x7ffffffff)<<5)^v;
  for i:0..<KY_GEN.count {if ((top>>cast(u64)i)&1)!=0 {r^=KY_GEN[i];}}
  return r;
}
// The eight checksum characters of a descriptor without its "#...", into
// out; false on a character outside BIP-380's input set.
ky_checksum :: (s:string,out:[]u8)->bool #effects(all) {
  c:u64=1;cls:u64=0;count:=0;
  for ch:s {
    pos:=-1;for k:0..<KY_INPUT.count {if KY_INPUT[k]==ch {pos=k;}}
    if pos<0 {return false;}
    c=ky_polymod(c,cast(u64)(pos&31));cls=cls*3+cast(u64)(pos>>5);count+=1;
    if count==3 {c=ky_polymod(c,cls);cls=0;count=0;}
  }
  if count>0 {c=ky_polymod(c,cls);}
  for k:0..<8 {c=ky_polymod(c,0);}
  c^=1;
  for j:0..<8 {out[j]=KY_CHECK[cast(int)((c>>cast(u64)(5*(7-j)))&31)];}
  return true;
}
```

The wallet doesn't get to choose its descriptor. Admission rebuilds it from
the `paths` and the enrollments, in enrollment order, each key as the
enrollment's `[fingerprint/path]xpub` followed by `/<0;1>/*` (receive and
change):

<!-- source: coax/lib/nostr/nipkeys.coax descriptor -->
```jai
// The descriptor the paths and enrollments describe, with its checksum,
// into out; its length, or -1. One path: wsh(sortedmulti(...)). A primary
// path and one timelocked path: wsh(or_d(multi(primary),
// and_v(v:older(n)|v:after(h),multi(recovery)))). Every enrollment is in
// exactly one path, and every primary key precedes every recovery key.
ky_descriptor :: (w:KyWallet,e:[]KyEnrollment,out:[]u8)->int #effects(all) {
  if w.npaths>2 {return -1;}
  p:=ky_primary(w);a:int;
  if w.npaths==1 {
    a=ky_put(out,0,"wsh(");a=ky_multi(out,a,"sortedmulti",w,e,w.paths[0]);a=ky_put(out,a,")");
  } else {
    r:=w.paths[1-p];
    a=ky_put(out,0,"wsh(or_d(");a=ky_multi(out,a,"multi",w,e,w.paths[p]);
    a=ky_put(out,a,ifx r.older>0 then ",and_v(v:older(" else ",and_v(v:after(");
    a=ky_put_int(out,a,r.older+r.after);a=ky_put(out,a,"),");
    a=ky_multi(out,a,"multi",w,e,r);a=ky_put(out,a,")))");
  }
  if a==-1 || a+9>out.count {return -1;}
  if !ky_checksum(cast(string)out[0..<a],out[a+1..<a+9]) {return -1;}
  out[a]='#';return a+9;
}
```

### Worked example: the treasury's descriptor

The treasury has three enrollments, in this order:

| Agent | Class | Origin | Operator |
| --- | --- | --- | --- |
| alice (the holder, a SOV agent) | software | `[78c16713/48h/1h/0h/2h]` | alice-agent |
| keyco | software | `[669198ad/48h/1h/0h/2h]` | keyco-llc |
| enclave | attested | `[66d2046a/48h/1h/0h/2h]` | enclave-co |

One path, `primary`, threshold 2, no timelock, naming all three
enrollments. The rule gives, by hand:

```text
wsh(sortedmulti(2,
  [78c16713/48h/1h/0h/2h]tpubDFBC6m...6DqemFm3Gd/<0;1>/*,
  [669198ad/48h/1h/0h/2h]tpubDEY9Ma...RrXyTAbv8X/<0;1>/*,
  [66d2046a/48h/1h/0h/2h]tpubDFNVBL...bJRSwJ8vg/<0;1>/*))
```

without the line breaks, then `#660mkcnk`, which an independent Python
implementation of the polymod computed. The program prints the same 457
bytes, and the wallet's `descriptor` field equals them.

## The records

Every body has `v`, `requires: []`, optional inert `meta`, and exactly the
fields NIP-KEYS lists; an unknown version, required feature or field
refuses before any effect. The readers are closed in the same way as
chapter 83's: each field once, every required field present.

A wallet path is `{name, threshold, keys, timelock}`, the keys being
enrollment ArtifactRefs and the timelock `null`, `{older: blocks}` or
`{after: height}`:

<!-- source: coax/lib/nostr/nipkeys.coax path -->
```jai
// One spending path: name, threshold, keys (enrollment ArtifactRefs) and
// timelock (null, {older: blocks} or {after: height}).
ky_path :: (j:[]Json,p:[]KyPath)->KyCode #effects(all) {
  names:=[4]string.["name","threshold","keys","timelock"];
  seen:=0;
  while true {
    t:=json_next(j);if t==JSON_OBJECT_END {break;}if t!=JSON_STRING {return KyCode.paths;}
    f:=ky_field(json_text(j),names[..]);t=json_next(j);
    if f<0 || (seen&(1<<f))!=0 {return KyCode.paths;}seen|=1<<f;
    if f==0 {if t!=JSON_STRING || json_text(j).count==0 {return KyCode.paths;}p[0].name=json_text(j);}
    else if f==1 {p[0].threshold=ky_uint(j,t);}
    else if f==2 {p[0].count=ky_refs(j,t,p[0].keys[..]);if p[0].count<1 {return KyCode.paths;}}
    else if t!=JSON_NULL {
      if t!=JSON_OBJECT || json_next(j)!=JSON_STRING {return KyCode.timelock;}
      older:=json_is(json_text(j),"older");
      if !older && !json_is(json_text(j),"after") {return KyCode.timelock;}
      v:=ky_uint(j,json_next(j));
      if v<1 || json_next(j)!=JSON_OBJECT_END {return KyCode.timelock;}
      if older {p[0].older=v;} else {p[0].after=v;}
    }
  }
  if seen!=15 {return KyCode.paths;}
  if p[0].threshold<1 || p[0].threshold>p[0].count {return KyCode.threshold;}
  return KyCode.ok;
}
```

The wallet reader keeps each ArtifactRef as its 32-byte digest, checks the
revision lineage (revision 0 has `previous: null`, later revisions name the
predecessor), the network (`mainnet`, `testnet4` or `signet`), the
checksum, one to eight paths with distinct names, and exactly one path with
no timelock: the primary path.

<!-- source: coax/lib/nostr/nipkeys.coax wallet -->
```jai
ky_wallet_read :: (content:string)->KyWallet #effects(all) {
  w:=[1]KyWallet.[KyWallet.{}];names:=KY_WALLET_FIELDS;
  j:[1]Json;json_start(j[..],content);if json_next(j[..])!=JSON_OBJECT {return ky_wfail(KyCode.json);}
  seen:=0;
  while true {
    t:=json_next(j[..]);if t==JSON_OBJECT_END {break;}if t!=JSON_STRING {return ky_wfail(KyCode.json);}
    f:=ky_field(json_text(j[..]),names[..]);t=json_next(j[..]);
    if f<0 || (seen&(1<<f))!=0 {return ky_wfail(KyCode.field);}seen|=1<<f;
    s:=json_text(j[..]);
    if f==0 {if t!=JSON_STRING || !json_is(s,"openagents.keys-wallet.v1") {return ky_wfail(KyCode.version);}}
    else if f==1 {if !ky_requires_empty(j[..],t) {return ky_wfail(KyCode.version);}}
    else if f==2 {if t!=JSON_OBJECT || !json_skip(j[..]) {return ky_wfail(KyCode.field);}}
    else if f==3 {if t!=JSON_STRING || !ky_hex64(s) {return ky_wfail(KyCode.wallet);}w[0].wallet=s;}
    else if f==4 {if t!=JSON_STRING || !ky_hex64(s) {return ky_wfail(KyCode.holder);}w[0].holder=s;}
    else if f==5 {w[0].revision=ky_uint(j[..],t);if w[0].revision<0 {return ky_wfail(KyCode.revision);}}
    else if f==6 {
      if t==JSON_OBJECT {d:=ky_ref(j[..]);if d.count==0 {return ky_wfail(KyCode.revision);}w[0].previous=ky_bytes(d[7..<71]);w[0].has_previous=true;}
      else if t!=JSON_NULL {return ky_wfail(KyCode.revision);}
    }
    else if f==7 {
      if t!=JSON_STRING || !(json_is(s,"mainnet") || json_is(s,"testnet4") || json_is(s,"signet")) {return ky_wfail(KyCode.network);}
      w[0].network=s;
    }
    else if f==8 {if t!=JSON_STRING || !ky_checksum_ok(s) {return ky_wfail(KyCode.checksum);}w[0].descriptor=s;}
    else if f==9 {
      if t!=JSON_ARRAY {return ky_wfail(KyCode.paths);}
      v:=json_next(j[..]);
      while v!=JSON_ARRAY_END {
        if w[0].npaths==KY_PATHS {return ky_wfail(KyCode.paths);}   // a non-object never reads as a path
        c:=ky_path(j[..],w[0].paths[w[0].npaths..<w[0].npaths+1]);
        if c!=KyCode.ok {return KyWallet.{code=c};}
        w[0].npaths+=1;v=json_next(j[..]);
      }
    }
    else if f==10 {w[0].nenroll=ky_refs(j[..],t,w[0].enrollments[..]);if w[0].nenroll<1 {return ky_wfail(KyCode.enrollment);}}
    else if f==11 {w[0].nrecip=ky_keys(j[..],t,w[0].recipients[..]);if w[0].nrecip<1 {return ky_wfail(KyCode.recipient);}}
    else if f==12 {w[0].starts_at=ky_uint(j[..],t);}
    else {w[0].expires_at=ky_uint(j[..],t);}
  }
  if json_next(j[..])!=JSON_END {return ky_wfail(KyCode.json);}
  if (seen|4)!=16383 {return ky_wfail(KyCode.field);}
  if (w[0].revision==0)==w[0].has_previous {return ky_wfail(KyCode.revision);}
  if w[0].starts_at<0 || w[0].expires_at<=w[0].starts_at {return ky_wfail(KyCode.times);}
  if w[0].npaths<1 {return ky_wfail(KyCode.paths);}
  primary:=0;
  for i:0..<w[0].npaths {
    if w[0].paths[i].older==0 && w[0].paths[i].after==0 {primary+=1;}
    for k:0..<i {if w[0].paths[k].name==w[0].paths[i].name {return ky_wfail(KyCode.paths);}}
  }
  if primary!=1 {return ky_wfail(KyCode.primary);}
  return w[0];
}
```

An enrollment is signed by its agent and offers one key to one wallet. Its
digest is what the wallet's ArtifactRef names, so changing any byte of it
breaks the wallet. The `possession` field is a BIP-322 signature. The
reader keeps it, and admission verifies it
([Possession](#possession-proving-the-key-is-held)).

<!-- source: coax/lib/nostr/nipkeys.coax enrollment -->
```jai
ky_enrollment_read :: (signer:string,content:string)->KyEnrollment #effects(all) {
  e:KyEnrollment;cls:string;att:=false;names:=KY_ENR_FIELDS;
  if !ky_sha(content,e.digest[..]) {return ky_efail(KyCode.json);}
  j:[1]Json;json_start(j[..],content);if json_next(j[..])!=JSON_OBJECT {return ky_efail(KyCode.json);}
  seen:=0;
  while true {
    t:=json_next(j[..]);if t==JSON_OBJECT_END {break;}if t!=JSON_STRING {return ky_efail(KyCode.json);}
    f:=ky_field(json_text(j[..]),names[..]);t=json_next(j[..]);
    if f<0 || (seen&(1<<f))!=0 {return ky_efail(KyCode.field);}seen|=1<<f;
    s:=json_text(j[..]);
    if f==0 {if t!=JSON_STRING || !json_is(s,"openagents.keys-enrollment.v1") {return ky_efail(KyCode.version);}}
    else if f==1 {if !ky_requires_empty(j[..],t) {return ky_efail(KyCode.version);}}
    else if f==2 {if t!=JSON_OBJECT || !json_skip(j[..]) {return ky_efail(KyCode.field);}}
    else if f==3 {if t!=JSON_STRING || s!=signer || !ky_hex64(s) {return ky_efail(KyCode.enrollment);}e.agent=s;}
    else if f==4 {if t!=JSON_STRING || !ky_hex64(s) {return ky_efail(KyCode.wallet);}e.wallet=s;}
    else if f==5 {
      if t!=JSON_STRING {return ky_efail(KyCode.xpub);}
      e.key=ky_origin(s);if !ky_ext_key(e.key).ok {return ky_efail(KyCode.xpub);}e.xpub=s;
    }
    else if f==6 {if t!=JSON_STRING || s.count==0 {return ky_efail(KyCode.field);}e.possession=s;}   // BIP-322: verified by ky_possession
    else if f==7 {
      if t!=JSON_STRING || !(json_is(s,"software") || json_is(s,"hardware") || json_is(s,"attested") || json_is(s,"institutional")) {return ky_efail(KyCode.field);}
      cls=s;
    }
    else if f==8 {
      if t==JSON_OBJECT {if ky_ref(j[..]).count==0 {return ky_efail(KyCode.attestation);}att=true;}
      else if t!=JSON_NULL {return ky_efail(KyCode.attestation);}
    }
    else if f==9 {if t!=JSON_OBJECT || ky_ref(j[..]).count==0 {return ky_efail(KyCode.field);}}
    else if f==10 {if t!=JSON_OBJECT {return ky_efail(KyCode.field);}e.independence=ky_ref(j[..]);if e.independence.count==0 {return ky_efail(KyCode.field);}}
    else {e.expires_at=ky_uint(j[..],t);if e.expires_at<0 {return ky_efail(KyCode.times);}}
  }
  if json_next(j[..])!=JSON_END {return ky_efail(KyCode.json);}
  if (seen|4)!=4095 {return ky_efail(KyCode.field);}
  e.attested=json_is(cls,"attested");
  if e.attested!=att {return ky_efail(KyCode.attestation);}
  return e;
}
```

The independence declaration is a signed claim, not proof: `operator`,
jurisdictions, custody, affiliations and insurance. Admission uses the
operator; the rest is shown to the holder, never refused.

## Admission and the composition rules

Admission binds the records together, then checks the rules. `ky_admit`
is every rule except the possession proofs, which need the wallet's keys
derived first and come in
[Possession](#possession-proving-the-key-is-held):

<!-- source: coax/lib/nostr/nipkeys.coax admit -->
```jai
// Admission of one wallet revision at now (unix seconds) and height (the
// chain tip). e and ind are the enrollments in the wallet's order and each
// one's independence declaration. sov: the holder is a NIP-SOV agent.
// min_lock: the holder policy's shortest recovery timelock (KY_MIN_LOCK
// unless the policy says otherwise). proven stays false here: this is
// every rule but the possession proofs, which need chapter 96's BIP-322
// verifier. ky_admit_proven (nipkeys_possess.coax) runs this and then them.
ky_admit :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,sov:bool,min_lock:int,now:int,height:int)->KyAdmit #effects(all) {
  r:KyAdmit;r.code=w.code;
  if w.code!=KyCode.ok {return r;}
  r.code=KyCode.enrollment;
  if e.count!=w.nenroll || ind.count!=w.nenroll {return r;}
  version:=ifx json_is(w.network,"mainnet") then KY_XPUB else KY_TPUB;
  for i:0..<w.nenroll {
    r.code=e[i].code;if r.code!=KyCode.ok {return r;}
    r.code=ind[i].code;if r.code!=KyCode.ok {return r;}
    r.code=KyCode.enrollment;
    if !ky_eq32(w.enrollments[i],e[i].digest) || e[i].wallet!=w.wallet {return r;}
    if !ky_digest_is(e[i].independence,ind[i].digest) || ind[i].agent!=e[i].agent {return r;}
    for k:0..<i {if e[k].agent==e[i].agent && e[k].agent!=w.holder {return r;}}
    r.code=KyCode.times;if now>e[i].expires_at {return r;}
    r.code=KyCode.recipient;if !ky_recipient(w,e[i].agent) {return r;}
    r.code=KyCode.network;if ky_ext_key(e[i].key).version!=version {return r;}
  }
  r.code=ky_layout(w);if r.code!=KyCode.ok {return r;}
  r.code=KyCode.family;
  if w.descriptor.count<4 || (w.descriptor[0..<4]!="wsh(") {
    if w.descriptor.count>3 && w.descriptor[0..<3]=="tr(" {r.code=KyCode.not_built;}
    return r;
  }
  if w.npaths>2 {r.code=KyCode.not_built;return r;}
  d:[4096]u8;n:=ky_descriptor(w,e,d[..]);
  r.code=KyCode.descriptor;if n==-1 || cast(string)d[0..<n]!=w.descriptor {return r;}
  r.code=ky_compose(w,e,ind,sov,min_lock,height);
  return r;
}
```

1. **No one party reaches a threshold alone.** For every path, no set of
   keys whose declarations name one operator meets the threshold. The
   holder's keys count as one operator: each takes the operator of the
   holder's first enrollment.
2. **A sovereign agent never holds a primary-path majority.** Because the
   holder's keys are one operator, rule 1 already counts them; when the
   overweight group is the holder's on the primary path of a SOV holder,
   the refusal is `sovereign` instead of `operator`.
3. **Recovery paths are later and wider.** A timelocked path needs
   `older` of at least 4,320 blocks (or the holder policy's shorter value),
   or an `after` height at least that far past the tip, and its keys
   without the attested ones must still meet its threshold. That is this
   chapter's reading of "must not depend solely on an attested co-signer".
4. **No silent weakening.** A successor that lowers a threshold, shortens or
   drops a timelock, removes a path or removes a key agent needs approvals
   from a primary quorum of the previous revision (`ky_successor`). The POL
   approvals themselves are verified by the caller.

<!-- source: coax/lib/nostr/nipkeys.coax compose -->
```jai
ky_operator :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,i:int)->string #effects(all) {
  if e[i].agent!=w.holder {return ind[i].op;}
  for k:0..<w.nenroll {if e[k].agent==w.holder {return ind[k].op;}}
  return "";
}
ky_compose :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,sov:bool,min_lock:int,height:int)->KyCode #effects(all) {
  pi:=ky_primary(w);
  for p:0..<w.npaths {
    path:=w.paths[p];plain:=0;
    for a:0..<path.count {
      ia:=ky_find(w,path.keys[a]);same:=0;
      for b:0..<path.count {if ky_operator(w,e,ind,ky_find(w,path.keys[b]))==ky_operator(w,e,ind,ia) {same+=1;}}
      if same>=path.threshold {               // rule 1, and rule 2 when it is the holder's group
        h:=ky_holder(w,e);held:=h>=0 && ky_operator(w,e,ind,ia)==ky_operator(w,e,ind,h);
        return ifx sov && p==pi && held then KyCode.sovereign else KyCode.operator;
      }
      if !e[ia].attested {plain+=1;}
    }
    if p!=pi {                                // rule 3
      if path.older>0 && path.older<min_lock {return KyCode.recovery;}
      if path.after>0 && path.after<height+min_lock {return KyCode.recovery;}
      if plain<path.threshold {return KyCode.attested_only;}
    }
  }
  return KyCode.ok;
}
```

### Worked example: one treasury policy by hand

The primary path is 2-of-3 over alice, keyco and enclave.

| Check | Count | Threshold | Result |
| --- | --- | --- | --- |
| Rule 1, alice-agent | 1 (alice) | 2 | 1 < 2, holds |
| Rule 1, keyco-llc | 1 (keyco) | 2 | holds |
| Rule 1, enclave-co | 1 (enclave) | 2 | holds |
| Rule 2, alice is a SOV agent | 1 runtime key | 2 | 1 < 2, holds |
| Rule 3 | no timelocked path | | holds |

Admission is `ok`. Now the broken declarations the example tries:

- enclave declares `keyco-llc`: keyco-llc has 2 keys, 2 ≥ 2, so keyco's
  operator could spend alone. Refused: `operator`.
- enclave declares `alice-agent`: the holder's group has 2 keys on the
  primary path of a SOV holder, so the agent could spend alone. Refused:
  `sovereign`.

A recovery path that lets alice alone spend after 4,320 blocks is refused
by rule 1 (`operator`: 1 ≥ 1); with alice and keyco, 2-of-2, after 4,320
blocks it holds; at 4,319 blocks it is `recovery`; with alice and enclave
it is `attested_only`, since only one non-attested key remains for a
threshold of 2. The checks in the file run each of these.

## Addresses from public keys

A signer must know the wallet's scripts to recognise its inputs and change.
BIP-32 public derivation gets a child public key from an xpub: with
`I = HMAC-SHA512(chain, K || index)`, the child key is `I_L·G + K` and its
chain code `I_R`. The code computes `I_L·G + K` as one variable-time sum,
since nothing here is secret. It reads and writes the 33-byte key with
[Bitcoin chapter 17](../../bitcoin/book/ch017-point-encodings-compressed-uncompressed-and-x-only.md)'s codec:

<!-- source: coax/lib/nostr/nipkeys.coax xpub -->
```jai
// A base58check extended public key: 78 bytes, version, depth, parent
// fingerprint, child number, chain code and a compressed public key.
ky_ext_key :: (s:string)->KyExtKey #effects(all) {
  r:KyExtKey;b:[82]u8;
  for c:s {
    d:=-1;for k:0..<58 {if KY_B58[k]==c {d=k;}}
    if d<0 {return r;}
    carry:=d;
    for k:0..<82 {carry+=cast(int)b[81-k]*58;b[81-k]=cast(u8)(carry&255);carry=carry>>8;}
    if carry!=0 {return r;}
  }
  h:[32]u8;h2:[32]u8;
  if sha256_hash_checked(b[0..<78],h[..],false)!=SHA_OK || sha256_hash_checked(h[..],h2[..],false)!=SHA_OK {return r;}
  for k:0..<4 {if h2[k]!=b[78+k] {return r;}}
  r.version=(cast(int)b[0]<<24)|(cast(int)b[1]<<16)|(cast(int)b[2]<<8)|cast(int)b[3];
  for k:0..<32 {r.chain[k]=b[13+k];}
  for k:0..<33 {r.key[k]=b[45+k];}
  r.ok=(r.version==KY_TPUB || r.version==KY_XPUB) && (r.key[0]==2 || r.key[0]==3);   // the point is decoded when used
  return r;
}
// The public child at a normal index: K + IL·G, chain code IR.
ky_child :: (parent:KyExtKey,index:int)->KyExtKey #effects(all) {
  r:KyExtKey;p:=parent;
  if !p.ok || index<0 || index>=BIP32_HARDENED {return r;}
  data:[37]u8;i:[64]u8;
  for k:0..<33 {data[k]=p.key[k];}
  for k:0..<4 {data[33+k]=cast(u8)((index>>(8*(3-k)))&255);}
  if !hmac_sha512(p.chain[..],data[..],i[..]) {return r;}
  il:=sc_load(i[0..<32]);a:=se_decode(p.key[..]);
  if !il.ok || !a.ok {return r;}                          // IL >= n, or a key off the curve
  s:=ecmult_vartime(il.value,a.value,sc_small(1));        // IL·G + K: public data, variable time
  if se_encode(s.value,SE_COMPRESSED,r.key[..])!=SE_COMPRESSED {return r;}   // infinity has no encoding
  for k:0..<32 {r.chain[k]=i[32+k];}
  r.version=p.version;r.ok=true;return r;
}
```

The witness script of `sortedmulti(2, A, B, C)` at an index is
`OP_2 <33-byte keys in byte order> OP_3 OP_CHECKMULTISIG`, and the P2WSH
scriptPubKey is `0x00 0x20` followed by the script's SHA-256:

<!-- source: coax/lib/nostr/nipkeys.coax scripts -->
```jai
// The P2WSH scriptPubKey of a one-path wallet at one index, from each
// key's branch key (account/0 for receive, account/1 for change):
// 0x00 0x20 SHA-256(k <sorted child keys> n CHECKMULTISIG).
ky_script :: (w:KyWallet,branch:[]KyExtKey,index:int)->KyScript #effects(all) {
  r:KyScript;p:=w.paths[0];keys:[KY_KEYS][33]u8;
  for i:0..<w.nenroll {
    c:=ky_child(branch[i],index);
    if !c.ok {return r;}
    // insertion sort, lexicographic on the 33 bytes
    at:=i;
    while at>0 && ky_less(c.key,keys[at-1]) {keys[at]=keys[at-1];at-=1;}
    keys[at]=c.key;
  }
  ws:[3+34*KY_KEYS]u8;
  ws[0]=cast(u8)(0x50+p.threshold);n:=1;
  for i:0..<w.nenroll {ws[n]=0x21;for k:0..<33 {ws[n+1+k]=keys[i][k];}n+=34;}
  ws[n]=cast(u8)(0x50+w.nenroll);ws[n+1]=0xae;n+=2;
  r.b[1]=0x20;                                            // r.b[0] is OP_0
  h:[32]u8;#assert sha256_hash_checked(ws[0..<n],h[..],false)==SHA_OK;
  for k:0..<32 {r.b[2+k]=h[k];}
  r.n=34;return r;
}
```

The treasury's receive 0 is
`tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2` and its
change 0 is
`tb1qzzxa9gpdee47e9k7wp8h8ql9h8y5aray38h6z2unnhjhw74jqz0sr0utuy`. These
come from the descriptor alone, which is the property that lets a key agent
recover the coins without the coordinator.

## Possession: proving the key is held

An xpub is public. Without a proof, anyone who has seen keyco's xpub could
enroll it under their own name, and a wallet would count a key whose
"agent" can't sign. So an enrollment carries `possession`: a BIP-322
signature by the key at the xpub's first receive derivation over the digest
of `{wallet, agent, xpub}`. The digest names the wallet, so a proof made
for one wallet fails in every other.

BIP-322 (Bitcoin chapter 96) turns "sign this message" into a spend. A
virtual transaction `to_spend` pays 0 to a challenge script and commits to
the message's tagged hash; a second one, `to_sign`, spends it. The
signature is `to_sign`'s witness, and verifying it means running the
script. Nothing here is broadcast.

NIP-KEYS names the signer and what is signed, and leaves the bytes open.
This build fixes them, and the NIP should say the same before another
implementation enrolls a key:

- **The message** is the text `sha256:` and 64 lowercase hex digits: the
  SHA-256 of the canonical JSON `{"agent":A,"wallet":W,"xpub":X}`, names in
  order and no spaces, `X` the whole field with its origin. This is the form
  every ArtifactRef digest in these records has.
- **The key** is the xpub's public child at 0, then 0: the key this agent
  contributes to the wallet's receive 0.
- **The challenge** is that key's P2WPKH script, `0x00 0x14 HASH160(key)`,
  and the signature is BIP-322's simple form: `smp` and the Base64 of the
  witness stack, a signature and the public key.

<!-- source: coax/lib/nostr/nipkeys_possess.coax message -->
```jai
// What the key signs: "sha256:" and the hex SHA-256 of the canonical JSON
// {"agent":A,"wallet":W,"xpub":X}, written into out (at least
// KY_PROOF_TEXT bytes). "" when out is too short or the JSON is longer
// than KY_PROOF_JSON.
ky_possession_message :: (e:KyEnrollment,out:[]u8)->string #effects(all) {
  j:[KY_PROOF_JSON]u8;d:[32]u8;hx:="0123456789abcdef";
  n:=json_write_string(j[..],json_raw(j[..],0,"{\"agent\":"),e.agent);
  n=json_write_string(j[..],json_raw(j[..],n,",\"wallet\":"),e.wallet);
  n=json_write_string(j[..],json_raw(j[..],n,",\"xpub\":"),e.xpub);
  n=json_raw(j[..],n,"}");
  if n==-1 || out.count<KY_PROOF_TEXT {return "";}
  #assert ky_sha(cast(string)j[0..<n],d[..]) && json_raw(out,0,"sha256:")==7;
  for k:0..<32 {out[7+2*k]=hx[cast(int)(d[k]>>4)];out[8+2*k]=hx[cast(int)(d[k]&15)];}
  return cast(string)out[0..<KY_PROOF_TEXT];
}
```

<!-- source: coax/lib/nostr/nipkeys_possess.coax script -->
```jai
// The challenge: the P2WPKH scriptPubKey of the key at the enrollment's
// xpub/0/0, the wallet's first receive derivation for this key. n is 0
// when the enrollment has no extended public key.
ky_possession_script :: (e:KyEnrollment)->KyScript #effects(all) {
  r:KyScript;
  k:=ky_child(ky_child(ky_ext_key(e.key),0),0);
  if !k.ok {return r;}
  h:[20]u8;#assert hash160_checked(k.key[..],h[..],false)==RMD_OK;
  r.b[1]=20;for i:0..<20 {r.b[2+i]=h[i];}                 // r.b[0] is OP_0
  r.n=22;return r;
}
```

Verification is chapter 96's `bip322_verify_vartime`, not a second copy of
it. That verifier takes a mainnet address. BIP-322 signs the script, not
the address text, so `ky_possession_address` writes the script's `bc1`
form whatever the wallet's network, and the same signature verifies for
the `tb1` form on signet. Only `valid` counts. BIP-322 also has an
`inconclusive` answer, for scripts a verifier can't judge, and here that
is a refusal.

<!-- source: coax/lib/nostr/nipkeys_possess.coax address -->
```jai
// The challenge as the address chapter 96's signer and verifier take: the
// script's mainnet form (42 characters, bc1q...), whatever the wallet's
// network. "" when there is no script or out is too short.
ky_possession_address :: (e:KyEnrollment,out:[]u8)->string #effects(all) {
  s:=ky_possession_script(e);
  if s.n==0 {return "";}
  n:=segwit_encode_vartime(SW_MAINNET,0,s.b[2..<22],out);
  return ifx n==KY_PROOF_ADDRESS then cast(string)out[0..<n] else "";
}
```

<!-- source: coax/lib/nostr/nipkeys_possess.coax holds -->
```jai
// Does BIP-322's answer for an enrollment's possession field prove its
// key? Only "valid" does, and only for a challenge and a message that
// exist: an inconclusive answer is a refusal.
ky_proof_holds :: (address:string,message:string,state:int)->bool #effects(all) {
  return address.count>0 && message.count>0 && state==KY_PROOF_VALID;
}
```

The rules above are in `nipkeys_possess.coax`, which is `#authority` and
doesn't run BIP-322. `nipkeys_proof.coax` joins them to the verifier and
decides nothing itself:

<!-- source: coax/lib/nostr/nipkeys_proof.coax possession -->
```jai
// Does the enrollment's possession field prove its key? Chapter 96's
// verification of the field as a signature of the message under the
// challenge; ky_proof_holds judges the answer.
ky_possession :: (e:KyEnrollment,x:[]B322Work)->bool #effects(all) {
  m:[KY_PROOF_TEXT]u8;a:[KY_PROOF_ADDRESS]u8;
  address:=ky_possession_address(e,a[..]);message:=ky_possession_message(e,m[..]);
  return ky_proof_holds(address,message,bip322_verify_vartime(address,e.possession,message,x).state);
}
```

`ky_admit` applies every admission rule except this one and leaves `proven`
false. `ky_admit_proven` is the whole admission: `ky_admit`'s refusals
first, as they are, then each enrollment's proof, and `proven` only when
all of it holds. A bad proof refuses as `possession`. `ky_admit_proofs`
holds that rule: one answer for each of the wallet's enrollments, all of
them yes.

<!-- source: coax/lib/nostr/nipkeys_possess.coax admit_proofs -->
```jai
// How many proofs ky_admit's result asks for: none after a refusal, else
// one for each enrollment. More enrollments than a wallet has keys get
// none, and ky_admit_proofs then refuses.
ky_proofs_wanted :: (a:KyAdmit,e:[]KyEnrollment)->int #effects(all) {
  if a.code!=KyCode.ok || e.count>KY_KEYS {return 0;}
  return e.count;
}
// NIP-KEYS admission, whole: ky_admit's result a, then one answer for each
// of the wallet's enrollments, in order. A refusal of ky_admit stays as it
// is. proven is true only when all of it holds.
ky_admit_proofs :: (a:KyAdmit,w:KyWallet,proven:[]bool)->KyAdmit #effects(all) {
  r:=a;r.proven=false;
  if r.code!=KyCode.ok {return r;}
  r.code=KyCode.possession;
  if proven.count!=w.nenroll {return r;}
  for i:0..<proven.count {
    if !proven[i] {return r;}
  }
  r.code=KyCode.ok;r.proven=true;return r;
}
```

<!-- source: coax/lib/nostr/nipkeys_proof.coax admit_proven -->
```jai
// NIP-KEYS admission, whole: every rule of ky_admit, then every
// enrollment's possession proof. ky_admit_proofs judges the answers.
ky_admit_proven :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,sov:bool,min_lock:int,now:int,height:int,x:[]B322Work)->KyAdmit #effects(all) {
  a:=ky_admit(w,e,ind,sov,min_lock,now,height);
  proven:[KY_KEYS]bool;n:=ky_proofs_wanted(a,e);
  for i:0..<n {proven[i]=ky_possession(e[i],x);}
  return ky_admit_proofs(a,w,proven[0..<n]);
}
```

A key agent makes its proof with chapter 96's signer,
`bip322_sign_vartime`, given the address and message above and its private
key at `account/0/0`. ECDSA here takes its nonce from RFC 6979, so the
same key and message always give the same proof.

### Worked example: alice's proof by hand

alice's enrollment has agent `79be667e...16f81798`, wallet `7777...7777`
and xpub `[78c16713/48h/1h/0h/2h]tpubDFBC6...emFm3Gd`.

1. The JSON is
   `{"agent":"79be667e...","wallet":"7777...","xpub":"[78c16713/48h/1h/0h/2h]tpubDFBC6...emFm3Gd"}`,
   296 bytes. Its SHA-256 is `963a6edb...db1123a0`, so the message is
   `sha256:963a6edba8240b48b5924d671b6fe9b0ad0696c10e513539c9662c45db1123a0`,
   71 characters.
2. The key at `xpub/0/0` is
   `027ae41a7f786c8d6e8397a1a1b1f9099315fd201fa8afe680cadb206bacc39f74`.
   It is the first key in receive 0's witness script, and the key alice
   signs the spend's first input with later in this chapter.
3. HASH160 of that key is `72369fbd9ef1c382297e34679259e782b224578a`, so
   the challenge is `0014 72369fbd...b224578a`, 22 bytes. As an address it
   is `tb1qwgmfl0v778pcy2t7x3neyk08s2ezg4u2zm4cyu` on signet and
   `bc1qwgmfl0v778pcy2t7x3neyk08s2ezg4u2gawtl0` in the form the verifier
   takes.
4. The message's tagged hash, SHA-256 with the tag
   `BIP0322-signed-message`, is `5c013660...e6338bc8`. `to_spend` is 116
   bytes: version 0, one input from the all-zero outpoint with index
   `ffffffff` and the scriptSig `00 20 5c013660...e6338bc8`, one output of 0
   to the challenge, lock time 0. Its txid is
   `a0dba5f17a5ee7327e0a3a7a6ac3663f9aa0ee9ea8065095ce2df3d371f18016`.
5. `to_sign` spends output 0 of that txid and pays 0 to `OP_RETURN`. Its
   BIP-143 digest is signed with the key from step 2. The witness is two
   items: a 72-byte signature ending in hash type `01`, and the 33-byte
   key. Written as a stack it is 108 bytes,
   `02 48 3045022100c4526b2b...520ee2d901 21 027ae41a...acc39f74`.
6. Base64 of those 108 bytes after `smp` is the field, 147 characters:
   `smpAkgwRQIhAMRSayunavRoLmgt...IGusw590`.

To verify, a reader repeats steps 1 to 4 from the enrollment alone, puts
the witness on `to_sign`, and runs the P2WPKH script: the key must hash to
the challenge and the signature must verify.

Change one thing and it fails. The same key's proof for wallet
`7878...7878` signs a different message, so in this wallet the signature
doesn't verify. alice's proof in keyco's enrollment carries alice's key,
which doesn't hash to keyco's challenge. Both are in the example.

## The signer

A signing request states every output (`address`, `amount_sat`, `change`),
the fee and a nonce, and names the wallet revision and the PSBT by digest:

<!-- source: coax/lib/nostr/nipkeys.coax request -->
```jai
ky_request_read :: (content:string)->KyRequest #effects(all) {
  r:=[1]KyRequest.[KyRequest.{}];names:=KY_REQ_FIELDS;
  if !ky_sha(content,r[0].digest[..]) {return ky_rfail(KyCode.json);}
  j:[1]Json;json_start(j[..],content);if json_next(j[..])!=JSON_OBJECT {return ky_rfail(KyCode.json);}
  seen:=0;
  while true {
    t:=json_next(j[..]);if t==JSON_OBJECT_END {break;}if t!=JSON_STRING {return ky_rfail(KyCode.json);}
    f:=ky_field(json_text(j[..]),names[..]);t=json_next(j[..]);
    if f<0 || (seen&(1<<f))!=0 {return ky_rfail(KyCode.field);}seen|=1<<f;
    s:=json_text(j[..]);
    if f==0 {if t!=JSON_STRING || !json_is(s,"openagents.keys-sign-request.v1") {return ky_rfail(KyCode.version);}}
    else if f==1 {if !ky_requires_empty(j[..],t) {return ky_rfail(KyCode.version);}}
    else if f==3 {if t!=JSON_OBJECT {return ky_rfail(KyCode.wallet);}r[0].wallet=ky_ref(j[..]);if r[0].wallet.count==0 {return ky_rfail(KyCode.wallet);}}
    else if f==4 {if t!=JSON_STRING || !ky_hex64(s) {return ky_rfail(KyCode.field);}r[0].requester=s;}
    else if f==5 {if t!=JSON_OBJECT {return ky_rfail(KyCode.psbt);}r[0].psbt=ky_ref(j[..]);if r[0].psbt.count==0 {return ky_rfail(KyCode.psbt);}}
    else if f==6 {
      if t!=JSON_ARRAY {return ky_rfail(KyCode.outputs);}
      v:=json_next(j[..]);
      while v!=JSON_ARRAY_END {
        n:=r[0].nout;
        if n==KY_IO || !ky_stated(j[..],r[0].out[n..<n+1]) {return ky_rfail(KyCode.outputs);}   // a non-object never reads as one
        r[0].nout+=1;v=json_next(j[..]);
      }
    }
    else if f==7 {r[0].fee=ky_uint(j[..],t);if r[0].fee<0 || r[0].fee>KY_MAX_SAT {return ky_rfail(KyCode.fee);}}
    else if f==8 {if t!=JSON_STRING || !at_slug_ky(s) {return ky_rfail(KyCode.field);}}
    else if f==9 || f==10 || f==2 {if t!=JSON_NULL && (t!=JSON_OBJECT || !json_skip(j[..])) {return ky_rfail(KyCode.field);}}
    else if f==11 {if t!=JSON_STRING || !ky_hex64(s) {return ky_rfail(KyCode.nonce);}r[0].nonce=s;}
    else {r[0].deadline=ky_uint(j[..],t);if r[0].deadline<0 {return ky_rfail(KyCode.deadline);}}
  }
  if json_next(j[..])!=JSON_END {return ky_rfail(KyCode.json);}
  if (seen|4)!=8191 {return ky_rfail(KyCode.field);}
  if r[0].nout<1 {return ky_rfail(KyCode.outputs);}
  return r[0];
}
```

The PSBT v0 reader takes the unsigned transaction from the global map,
requires every scriptSig empty, and requires a witness UTXO for every input,
which is what lets the fee be computed. PSBT v2 (global version 2) refuses
as `not_built`:

<!-- source: coax/lib/nostr/nipkeys.coax psbt -->
```jai
ky_psbt_read :: (b:[]u8)->KyPsbt #effects(all) {
  r:=[1]KyPsbt.[KyPsbt.{}];bad:=KyPsbt.{code=KyCode.psbt};
  for k:0..<KY_MAGIC.count {if ky_at(b,k)!=cast(int)KY_MAGIC[k] {return bad;}}
  g:=ky_map(b,5);
  if !g.found[0] || g.dup {return bad;}
  if g.other!=0 {return KyPsbt.{code=KyCode.not_built};}       // PSBT v2 (BIP-370)
  // The unsigned transaction: version, inputs with empty scriptSigs, outputs, locktime.
  t:=g.at[0];end:=t+g.n[0];
  ni:=ky_var(b,t+4);
  if ni.value<1 || ni.value>KY_IO {return bad;}   // a malformed size reads as 0
  i:=ni.next;
  for k:0..<ni.value {
    if ky_at(b,i+36)!=0 {return bad;}                      // past the end, the outputs fail to read
    i+=41;
  }
  no:=ky_var(b,i);
  if no.value<1 || no.value>KY_IO {return bad;}
  i=no.next;
  for k:0..<no.value {i=ky_txout(b,i,end,r[0].out_value[k..<k+1],r[0].out_script[k..<k+1]);if i==-1 {return bad;}}
  if i+4!=end {return bad;}
  r[0].nin=ni.value;r[0].nout=no.value;
  // One map per input with its witness UTXO, then one per output.
  at:=g.end;
  for k:0..<r[0].nin {
    m:=ky_map(b,at);
    if !m.ok || m.dup {return bad;}
    if !m.found[1] {return KyPsbt.{code=KyCode.inputs};}
    u:=m.at[1];e:=u+m.n[1];if ky_txout(b,u,e,r[0].in_value[k..<k+1],r[0].in_script[k..<k+1])!=e {return bad;}
    at=m.end;
  }
  for k:0..<r[0].nout {m:=ky_map(b,at);if !m.ok {return bad;}at=m.end;}
  if at!=b.count {return bad;}
  return r[0];
}
```

The signer never trusts `outputs` or `fee_sat`:

<!-- source: coax/lib/nostr/nipkeys.coax sign -->
```jai
// The signer's checks before it signs one request. wallet_digest is the
// admitted wallet revision's digest, all its scripts (ky_scripts, derived
// once at admission), psbt the bytes the request names, ceiling the
// signing policy's fee ceiling in satoshis.
ky_sign_check :: (w:KyWallet,wallet_digest:[32]u8,all:KyScripts,r:KyRequest,psbt:[]u8,ceiling:int,now:int)->KyCode #effects(all) {
  if r.code!=KyCode.ok {return r.code;}
  if !ky_digest_is(r.wallet,wallet_digest) {return KyCode.wallet;}
  if now>=r.deadline {return KyCode.deadline;}
  if now>=w.expires_at {return KyCode.times;}
  d:[32]u8;
  #assert sha256_hash_checked(psbt,d[..],false)==SHA_OK;
  if !ky_digest_is(r.psbt,d) {return KyCode.psbt;}
  t:=ky_psbt_read(psbt);
  if t.code!=KyCode.ok {return t.code;}
  if w.npaths!=1 {return KyCode.not_built;}               // miniscript spends
  sum_in:=0;
  for k:0..<t.nin {
    if !ky_is_ours(all,t.in_script[k],-1) {return KyCode.inputs;}
    sum_in+=t.in_value[k];
  }
  if r.nout!=t.nout {return KyCode.outputs;}
  sum_out:=0;
  for k:0..<t.nout {
    s:=ky_address(w,r.out[k].address);
    if s.n==0 {return KyCode.address;}
    if !ky_same(s,t.out_script[k]) {return KyCode.outputs;}
    if r.out[k].amount!=t.out_value[k] {return KyCode.amount;}
    if r.out[k].change && !ky_is_ours(all,s,1) {return KyCode.change;}
    sum_out+=t.out_value[k];
  }
  if sum_in-sum_out!=r.fee {return KyCode.fee;}             // the request's fee is never negative
  if r.fee>ceiling {return KyCode.ceiling;}
  return KyCode.ok;
}
```

### Worked example: the fee and the outputs by hand

The PSBT spends receive 0 (60,000 sat) and receive 1 (40,000 sat), and pays
70,000 sat to `tb1qqypqxpq9qcrsszg2pvxq6rs0zqg3yyc5r7fxez` and 29,000 sat to
change 0. By hand:

```text
inputs   60000 + 40000 = 100000   both are wallet scripts at receive 0, 1
outputs  70000 + 29000 =  99000
fee      100000 - 99000 =  1000   equals fee_sat 1000
output 1 0014 0102...14 = the P2WPKH the request names, 70000 = 70000
output 2 0020 108dd2...9f = change 0, derived, change: true
ceilings alice 2000, keyco 1000, enclave 1500: 1000 is within each
```

Each of the three signers returns `ok`. The second PSBT is the same except
that output 1 pays `tb1qz5tpwxqergd3c8g7ruszzg3rysjjvfeg5mag35`: its
script `0014 1516...28` is not the one the request names, so every signer
refuses with `outputs`.

### Idempotency

<!-- source: coax/lib/nostr/nipkeys.coax ledger -->
```jai
// (agent, requester, nonce) with the request digest. A new triple is
// recorded with its outcome before any partial signature leaves; an exact
// retry returns the retained outcome; a changed request with a used nonce
// refuses. A full ledger refuses new requests.
ky_consume :: (l:[]KyLedger,agent:string,r:KyRequest,code:KyCode)->KyRetry #effects(all) {
  if agent.count>KY_TEXT || r.requester.count>KY_TEXT || r.nonce.count>KY_TEXT {return KyRetry.{code=KyCode.field};}
  a:=kept64(agent);q:=kept64(r.requester);n:=kept64(r.nonce);
  for i:0..<l[0].count {
    x:=l[0].e[i];
    if kept_same(x.agent,a) && kept_same(x.requester,q) && kept_same(x.nonce,n) {
      for k:0..<32 {if x.digest[k]!=r.digest[k] {return KyRetry.{code=KyCode.nonce};}}
      return KyRetry.{code=x.code,retry=true};
    }
  }
  if l[0].count==KY_LEDGER {return KyRetry.{code=KyCode.nonce};}
  l[0].e[l[0].count]=KyEntry.{agent=a,requester=q,nonce=n,digest=r.digest,code=code};
  l[0].count+=1;
  return KyRetry.{code=code};
}
```

A signature already returned can't be recalled, so a key agent records the
request before releasing its partial signature. The exact retry returns the
retained outcome; the same nonce with different bytes refuses.

The ledger keeps its own copy of each agent, requester and nonce in a
`Kept(8)`, the standard library's owned text of up to 64 bytes
([`coax/lib/kept.coax`](../../coax/lib/kept.coax)). A string argument only
borrows the caller's bytes. A signer that reads each request into one buffer
would otherwise find its retained nonce rewritten by the next request: the
next request would refuse as `nonce`, and the old nonce, no longer on file,
could be signed again with other outputs. Anything longer than 64 bytes
refuses as `field`.

### Timelocks, rotation, the co-signer and the hot ceiling

`ky_path_open` refuses a recovery spend before `older(n)` confirmations or
the `after` height; `ky_retired` refuses every spend of a retired wallet
except its rotation's sweeps; `ky_sweep_signer` leaves a compromised signer
out of a sweep when the others meet the threshold; `ky_delay` holds a spend
above the co-signer's `delay_above_sat` until `delay_seconds` after receipt
and cancels it on a primary refusal; `ky_topup` refuses a `hot-topup` that
would take the hot Lightning balance over `hot_ceiling_msat`.

[Bitcoin chapter 122](../../bitcoin/book/ch122-agent-treasuries.md)
builds the on-chain side of such a treasury: a 2-of-3 with a recovery
path as P2WSH and as Taproot with a MuSig2 key path, and the refill
rule an agent applies to a treasury PSBT before it signs.

## Signatures

A signer that accepted a request still has to sign. A `wsh` input is a
segwit version 0 spend: its signature is ECDSA over the BIP-143 digest.
A `tr` key-path input is a version 1 spend: a BIP-340 Schnorr signature
over the BIP-341 digest, by a key tweaked with the output's script tree.
The generic Bitcoin code is in two files: the signer in
`coax/lib/crypto/ecdsa_sign.coax` and the digests, the transaction reader
and the Taproot key path in `coax/lib/bitcoin/sighash.coax`. Both are
`#authority`.

### ECDSA with a deterministic nonce

ECDSA signs a 32-byte hash *z* with key *d* by choosing a nonce *k*,
taking *r* = x(*k*G) mod *n* and *s* = *k*⁻¹(*z* + *rd*) mod *n*. A nonce
that repeats, or that an attacker can bias, gives the key away, so the
nonce is not random: RFC 6979 derives it from the key and the hash with
HMAC-SHA256. V starts as 32 bytes of 0x01 and K as 32 bytes of 0x00; two
rounds mix in `x || h` (the key, and the hash reduced mod *n*); each later
V = HMAC_K(V) is a candidate, kept when it lies in [1, *n*−1].
[Bitcoin chapter 20](../../bitcoin/book/ch020-sign-with-ecdsa.md) teaches
this signer in full, with its vectors and its secret-flow probe.

<!-- source: coax/lib/crypto/ecdsa_sign.coax rfc6979 -->
```jai
// RFC 6979 section 3.2 for SHA-256 and secp256k1's 256-bit order: the
// first two candidates for key and hash, into w.first and w.second.
// bits2octets(h) is h mod n. data is empty or 32 bytes of extra data
// (section 3.6). buf holds V || tag || key || h || data.
ec_candidates :: (key: [] u8, hash: [] u8, w: [] EcdsaWork) #effects(all) { ec_seed(key, hash, hash[0..<0], w); }
ec_seed :: (key: [] u8, hash: [] u8, data: [] u8, w: [] EcdsaWork) #effects(all) {
  z := sc_reduce_bytes(hash);                      // the message hash is public
  for i: 0..<32 { w[0].hv[i] = 1; w[0].hk[i] = 0; w[0].buf[33+i] = key[i]; }   // b, c: V = 01..01, K = 00..00
  #assert ss_write(z.value, w[0].buf[65..<97]);
  for i: 0..<data.count { w[0].buf[97+i] = data[i]; }   // public: a counter
  n := 97 + data.count;
  ec_step(w, 0, n);                                // d, e: K = HMAC_K(V || 00 || key || h || data), V = HMAC_K(V)
  ec_step(w, 1, n);                                // f, g: K = HMAC_K(V || 01 || key || h || data), V = HMAC_K(V)
  ec_next(w);                                      // h: V = HMAC_K(V), the first candidate
  for i: 0..<32 { w[0].first[i] = w[0].hv[i]; }
  ec_step(w, 0, 33);                               // h, retry: K = HMAC_K(V || 00), V = HMAC_K(V)
  ec_next(w);                                      // V = HMAC_K(V), the second candidate
  for i: 0..<32 { w[0].second[i] = w[0].hv[i]; }
}
```

The key and the nonce are secrets, so Bitcoin chapter 3's rules apply. RFC 6979
loops until a candidate is valid; a loop whose length depends on the key
leaks a little about it. This signer always computes two candidates and
picks the first valid one with a mask (`ec_pick`). A first candidate
outside the range has probability below 2⁻¹²⁷, both below 2⁻²⁵⁴; then it
refuses instead of looping. *k*G is the comb from Bitcoin chapter 16 (`sp_base`),
the inverse is a fixed power of *k* (Bitcoin chapter 14), and BIP-146's low-S rule
replaces *s* by *n* − *s* with a mask when 2*s* ≥ *n*:

<!-- source: coax/lib/crypto/ecdsa_sign.coax ecdsa -->
```jai
// The signature of a 32-byte message hash under a 32-byte key, with RFC
// 6979's nonce (with data as extra data when it isn't empty), into out:
// r || s (32 bytes each, s in the low half), then the recovery ID when out
// has 65 bytes. The caller checks the views.
ec_sign :: (key: [] u8, hash: [] u8, data: [] u8, w: [] EcdsaWork, out: [] u8) -> bool #effects(all) {
  #assert ss_key(key, w[0].key[..], false);              // d, or 1 with valid = 0
  ec_seed(key, hash, data, w);
  ec_pick(w);                                            // k
  #assert sp_base(w[0].k, w[0].point[..]) && sp_x(w[0].point[..], w[0].x[..]);
  w[0].r = sc_reduce_bytes(w[0].x[..]).value;            // r = x(kG) mod n
  z := sc_reduce_bytes(hash).value;                      // z = h mod n
  w[0].kinv = sc_inverse(w[0].k).value;
  w[0].s = sc_mul(w[0].kinv, sc_add(z, sc_mul(w[0].r, w[0].key[0].safe)));   // s = (z + r*d) / k
  w[0].high = ec_high(w[0].s);
  w[0].s = sc_select(w[0].high, sc_negate(w[0].s), w[0].s);                // the low half
  valid := w[0].key[0].valid & w[0].valid & w[0].point[0].finite & ec_nonzero(w[0].r) & ec_nonzero(w[0].s);
  if !ec_declassify(valid) { return ec_clear(w); }      // the one fact the signer reveals
  for i: 0..<8 {
    #assert cb_store_be32(out, 28-4*i, w[0].r.limbs[i]) && cb_store_be32(out, 60-4*i, w[0].s.limbs[i]);
  }
  if out.count == 65 { out[64] = ec_recid(w); }         // a public length decides
  return !ec_clear(w);
}
// The low-S ECDSA signature r || s (32 bytes each). Views are stable and
// disjoint (overlap is the host's truthful observation). out is written
// only when the result is true; the workspace is cleared either way.
ecdsa_sign :: (key: [] u8, hash: [] u8, w: [] EcdsaWork, out: [] u8, overlap: bool) -> bool #export_c #effects(all) {
  if !ec_views(key, hash, w, out, overlap, 64) { return false; }
  return ec_sign(key, hash, hash[0..<0], w, out);
}
```

`ecdsa_der` writes BIP-66's DER: `30 len 02 len r 02 len s`, each integer
without leading zeros and with a `00` in front when its top bit is set.

### Worked example: RFC 6979 by hand

Key 1 and the hash SHA-256("Satoshi Nakamoto") =
`a0dc65ff…7704883e`. The two rounds and the first candidate give
*k* = `8f8a276c…74f15d15`, in range, so it is the nonce (Python's `hmac`
module gives the same value, and the second candidate `f15fb763…d771045e`).
*k*G has x = `934b1ea1…210ee3d8` = *r*. Then *s* = *k*⁻¹(*z* + *r*) mod *n*
is above *n*/2, so the signature carries *n* − *s* = `2442ce9d…2aafd9e5`.
libsecp256k1 signs the same key and hash to the same 64 bytes.

### The BIP-143 digest

Before segwit, a signature hashed a copy of the whole transaction for each
input, and signing cost grew with the square of the inputs. BIP-143 hashes
the shared parts once: all outpoints (`hashPrevouts`), all sequences
(`hashSequence`) and all outputs (`hashOutputs`), each a double SHA-256.
The input's own outpoint, script code, amount and sequence follow. Signing
the amount is what lets a hardware signer, or a key agent, trust the fee
it computed: a lie about an input's value changes the digest.

<!-- source: coax/lib/bitcoin/sighash.coax bip143 -->
```jai
// The BIP-143 signature hash of input index, spending amount satoshis
// under script code (for P2WSH the witness script, for P2WPKH the P2PKH
// script of its key hash; at most BT_TX bytes), with hash type hashtype,
// into out.
bt_sighash_v0 :: (b:[]u8,tp:BtTx,index:int,code:[]u8,amount:int,hashtype:int,out:[]u8)->bool #effects(all) {t:=tp;
  if !t.ok || index<0 || index>=t.nin || amount<0 || code.count>BT_TX || out.count!=32 {return false;}
  anyone:=(hashtype&BT_ANYONE)!=0;base:=hashtype&0x1f;
  m:[BT_TX+160]u8;hp:[32]u8;hs:[32]u8;ho:[32]u8;           // the hashes BIP-143 zeroes stay zero
  if !anyone {bt_sha2(m[0..<bt_prevouts(t,m[..])],hp[..]);}
  if !anyone && base!=BT_SINGLE && base!=BT_NONE {bt_sha2(m[0..<bt_sequences(t,m[..])],hs[..]);}
  if base!=BT_SINGLE && base!=BT_NONE {bt_sha2(m[0..<bt_outputs(b,t,m[..])],ho[..]);}
  else if base==BT_SINGLE && index<t.nout {bt_sha2(b[t.out_at[index]..<t.out_at[index]+t.out_n[index]],ho[..]);}
  n:=bt_put(m[..],0,t.version[..]);n=bt_put(m[..],n,hp[..]);n=bt_put(m[..],n,hs[..]);
  n=bt_put(m[..],n,t.inputs[index].prev[..]);
  n=bt_var_put(m[..],n,code.count);n=bt_put(m[..],n,code);
  n=bt_le(m[..],n,amount,8);n=bt_put(m[..],n,t.inputs[index].seq[..]);
  n=bt_put(m[..],n,ho[..]);n=bt_put(m[..],n,t.locktime[..]);n=bt_le(m[..],n,hashtype,4);
  bt_sha2(m[0..<n],out);
  return true;
}
```

### Worked example: the treasury's first input

The PSBT's transaction is version 2, two inputs
(`1111…11:0` and `2222…22:1`, sequence `fdffffff`), two outputs, locktime
0. For input 0, SIGHASH_ALL:

```text
nVersion      02000000
hashPrevouts  7b3c2ebd5b19afa039feca8f9dd091e9091a054cef8430c8fcd2cc5a32f51fcd
hashSequence  957879fdce4d8ab885e32ff307d54e75884da52522cc53d3c4fdb60edb69a098
outpoint      1111111111111111111111111111111111111111111111111111111111111111 00000000
scriptCode    69 52 21 027ae41a… 21 0360ab78… 21 03e47364… 53 ae   (the witness script)
amount        60ea000000000000   (60,000 sat)
nSequence     fdffffff
hashOutputs   98652593c9090007703fe698b8f97d2ad0edc6dd8bcbd6f2892d77b9013e87c0
nLockTime     00000000
nHashType     01000000
```

Its double SHA-256 is `c5806db5…6c45c3be`. Python computes the same bytes
from BIP-143's text, and the program computes them with
`bt_sighash_v0`.

### The Taproot key path

A Taproot output commits to a key Q = P + *t*G, where P is the internal
key and *t* = H_TapTweak(P ‖ merkle root) (BIP-341). To spend by the key
path, the signer tweaks its secret the same way: it negates *d* when P's
y is odd (BIP-340 keys are x-only), adds *t*, and signs the BIP-341 digest
with Bitcoin chapter 24's `schnorr_sign_step`. The digest is a tagged hash over
the transaction's parts, and unlike BIP-143 it commits to every input's
amount and scriptPubKey, so a signer can't be lied to about any of them.

<!-- source: coax/lib/bitcoin/sighash.coax tweak -->
```jai
// The internal key P = dG (public, into w.internal), then the tweak hash
// TapTweak(P || merkle) into th.
bt_internal :: (key:[]u8,merkle:[]u8,w:[]BtTapWork,th:[]u8) #effects(all) {
  #assert ss_key(key,w[0].key[..],false) && sp_base(w[0].key[0].safe,w[0].point[..]) && sp_x(w[0].point[..],w[0].internal[..]);
  m:[64]u8;n:=bt_put(m[..],0,w[0].internal[..]);
  bt_tagged(BH_TAPTWEAK,m[0..<bt_put(m[..],n,merkle)],th);
}
// The tweaked key d' + t into w.tweaked, where d' is d or n - d so that P
// has an even y and t is the tweak hash th; false when t >= n. valid is
// the masked result: d in [1, n-1] and d' + t not zero.
bt_tweak :: (th:[]u8,w:[]BtTapWork)->bool #effects(all) {
  t:=sc_load(th);                                       // public: from the public key
  tweaked:=sc_add(ss_even(w[0].key[0].safe,w[0].point[0].odd&1),t.value);
  #assert ss_write(tweaked,w[0].tweaked[..]);
  w[0].valid=w[0].key[0].valid & w[0].point[0].finite & ec_nonzero(tweaked);
  return t.ok;
}
```

The wallets this chapter admits are `wsh`, so the signer doesn't use the
Taproot key path yet; `tr(...)` descriptors still refuse as `not_built`.
The key path is checked against BIP-341's published wallet vectors.

### The key agent signs, and the coordinator combines

`ky_sign` runs `ky_sign_check`, checks that the account key belongs to the
agent's enrollment (its public key and chain code are the xpub's), records
the outcome in the ledger, and only then signs each input. Each input's
script is found among the wallet's, its witness script (derived from the
xpubs at admission, `ky_witnesses`, which reads each xpub and derives
each branch key once) must hash to the input's program, and
the child key comes from BIP-32 private derivation on the comb
(`ky_child_priv`):

<!-- source: coax/lib/nostr/nipkeys_sign.coax ky_sign -->
```jai
// One key agent's answer to one request: the checks (ky_sign_check), the
// account key against its enrollment me, the outcome recorded in the
// ledger before any signature exists, then one SIGHASH_ALL ECDSA signature
// per input. An exact retry signs again, to the same bytes (RFC 6979).
// account is the agent's account private key and chain code (64 bytes);
// wit the wallet's witness scripts (ky_witnesses).
ky_sign :: (w:KyWallet,wallet_digest:[32]u8,all:KyScripts,wit:KyWitnesses,e:[]KyEnrollment,me:int,r:KyRequest,psbt:[]u8,ceiling:int,now:int,l:[]KyLedger,account:[]u8,work:[]KySignWork)->KySigned #effects(all) {
  code:=ky_sign_check(w,wallet_digest,all,r,psbt,ceiling,now);
  if !ky_account_is(e,me,account,work) {return ky_refusal(work,KyCode.xpub,false);}
  c:=ky_consume(l,e[me].agent,r,code);
  if c.code!=KyCode.ok {return ky_refusal(work,c.code,c.retry);}
  t:=ky_psbt_read(psbt);u:=ky_unsigned(psbt);tx:=bt_tx_read(u);x:=wit;
  out:=[1]KySigned.[KySigned.{code=KyCode.ok,retry=c.retry,nin=t.nin}];
  bt_sha(psbt,out[0].psbt[..]);
  for k:0..<t.nin {
    slot:=ky_slot(all,t.in_script[k]);         // ky_sign_check found every input among them
    ws:=x.ws[slot][0..<x.n[slot]];h:[32]u8;
    if !ky_ws_is(ws,t.in_script[k]) {return ky_refusal(work,KyCode.inputs,false);}
    #assert bt_sighash_v0(u,tx,k,ws,t.in_value[k],BT_ALL,h[..]);
    #assert ky_slot_key(account,slot,work);
    #assert ecdsa_sign(work[0].k2[..],h[..],work[0].ec[..],work[0].sig[..],false) && ky_pub(work[0].k2[..],work);
    d:=ecdsa_der(work[0].sig[..],out[0].p[k].sig[..]);
    out[0].p[k].sig[d]=cast(u8)BT_ALL;out[0].p[k].n=d+1;out[0].p[k].pub=work[0].pub;
  }
  ky_sign_clear(work);
  return out[0];
}
```

The result goes into the agent's receipt as a PSBT with
`PSBT_IN_PARTIAL_SIG` entries (`ky_psbt_signed`). An exact retry signs
again; RFC 6979 makes the bytes identical, so a retry can't produce a
second, different signature. The coordinator takes a threshold of
signatures for the PSBT (matched by its digest) and writes each input's
witness: an empty item (CHECKMULTISIG pops one extra), the signatures in
the witness script's key order, then the witness script:

<!-- source: coax/lib/nostr/nipkeys_sign.coax combine -->
```jai
// The coordinator: the final transaction from the signers' results for
// this PSBT (matched by digest), with the wallet's scripts and witness
// scripts. Each input's witness is 0, then a threshold of signatures in
// the witness script's key order, then the witness script. Into out; the
// length, or -1 when an input has fewer than a threshold of signatures.
ky_combine :: (w:KyWallet,all:KyScripts,wit:KyWitnesses,psbt:[]u8,s:[]KySigned,out:[]u8)->int #effects(all) {
  t:=ky_psbt_read(psbt);x:=wit;
  if t.code!=KyCode.ok {return -1;}
  d:[32]u8;bt_sha(psbt,d[..]);u:=ky_unsigned(psbt);
  buf:[2048]u8;wn:=0;need:=w.paths[0].threshold;
  for k:0..<t.nin {
    slot:=ky_slot(all,t.in_script[k]);if slot<0 {return -1;}
    ws:=x.ws[slot][0..<x.n[slot]];
    if !ky_ws_is(ws,t.in_script[k]) {return -1;}
    buf[wn]=cast(u8)(need+2);wn+=2;got:=0;               // the item count, then an empty item (buf is zeroed)
    for j:0..<w.nenroll {
      used:=false;
      for xi:0..<s.count {
        y:=s[xi];same:=y.code==KyCode.ok && ky_eq32(y.psbt,d) && got<need && !used;
        for b:0..<33 {same=same && y.p[k].pub[b]==ws[2+34*j+b];}
        if same {buf[wn]=cast(u8)y.p[k].n;wn=bt_put(buf[..],wn+1,y.p[k].sig[0..<y.p[k].n]);got+=1;used=true;}
      }
    }
    if got<need {return -1;}
    buf[wn]=cast(u8)ws.count;wn=bt_put(buf[..],wn+1,ws);
  }
  if out.count<u.count+2+wn {return -1;}
  m:=bt_put(out,0,u[0..<4]);out[m]=0;out[m+1]=1;
  m=bt_put(out,m+2,u[4..<u.count-4]);m=bt_put(out,m,buf[0..<wn]);
  return bt_put(out,m,u[u.count-4..<u.count]);
}
```

## Over the network

Every record travels as a kind 3188 private artifact
(contracts.md, "Private artifact envelope"). `nipkeys_carry.coax` seals a
KEYS record the way NIP-AGENT's carriage seals a definition: the record goes
inline in the artifact envelope, NIP-44 v2 encrypts it from the sender's key
to the recipient's, and the event has exactly one `p`, one `h` mailbox and
`t: oa:artifact:v1`. `kc_open` checks the id and signature, the recipient,
the decryption and the envelope's schema, in that order, before it hands
back the record. A PSBT is binary, so it doesn't ride inline: the request's
`psbt` ArtifactRef names its SHA-256, and the receiver fetches the bytes from
the artifact store and checks the digest before reading them.

`nostr/tools/treasury.coax` is the network half:

```shell
treasury agent alice --relay ws://127.0.0.1:PORT --dir STORE --now T --count 2
treasury coordinator --relay ws://127.0.0.1:PORT --dir STORE --request REQUEST.json --now T --out TX.hex
```

A key agent authenticates with NIP-42 (the relay shows a 3188 only to its
recipient and takes writes only from an authenticated connection), subscribes
to 3188s for its key, opens each request, refuses one whose signer isn't the
request's `requester`, and runs `ky_sign`. It stores its signed PSBT by
digest and seals a `openagents.keys-sign-receipt.v1` back to the requester:
`signed` with the signed PSBT's ArtifactRef, or `refused` with the code.
It reads on one connection and publishes on another, so waiting for an `OK`
never swallows a request.

The coordinator seals the request to every key agent and reads receipts for
this request only (the subscription also returns stored receipts for older
ones). It reads each signed PSBT back with `ky_signed_read`: one partial
signature per input, and a rebuild from the original PSBT that must give the
same bytes, so a signer can't change anything but its own signatures. Then
`ky_combine` builds the transaction.

[treasury_regtest.py](../../coax/tests/nostr/treasury_regtest.py) runs it
against Bitcoin Core 32.1 in regtest, on 127.0.0.1:

```text
coordinator, request 1 (the PSBT the request states):
  receipt from alice: signed, its PSBT reads back
  receipt from keyco: signed, its PSBT reads back
  receipt from enclave: signed, its PSBT reads back
coordinator, request 2 (a PSBT that pays someone the request does not name):
  receipt from alice: refused (outputs)
  receipt from keyco: refused (outputs)
  receipt from enclave: refused (outputs)
  fewer than 2 signatures: no transaction
```

Core's `testmempoolaccept` accepts the first transaction, and a block
confirms it with the request's outputs. Bitcoin Core, not our code, runs the
2-of-3 witness script.

Then the coordinator is gone. A new Core descriptor wallet takes the
descriptor from the wallet record (any key agent's copy), with alice's and
keyco's account keys in place of their xpubs, and rescans. It finds 79,000
sat: a new 50,000 sat coin at receive 2 and the spend's 29,000 sat change.
Its `sendall` signs both with Core's own signer, and a block confirms it.
Core shares no code with the Coax signer, so the descriptor alone is enough
to recover the coins. The run's record is
[2026-11-05-nostr-ch104-treasury-network](../../docs/verification/2026-11-05-nostr-ch104-treasury-network/README.md).

That run is from 2026-11-05, before the enrollments carried real
possession proofs. The proofs change each enrollment's digest, and so the
wallet record's digest and the request's wallet reference. On 2026-11-12
the run was repeated with the new records and Bitcoin Core 31.1, twice,
and passed both times
([the record](../../docs/verification/2026-11-12-nostr-ch104-possession-and-private-signet/README.md)).

The second time was on a private signet: `CHAIN=signet` starts Core with
`-signet -signetchallenge=51`. Signet (BIP-325) is a chain whose blocks
must satisfy a challenge script, and `51` is `OP_TRUE`, so this node signs
its own blocks and needs no faucet. Core then works with the treasury's own
`tb1` addresses, the ones the wallet record's `network: signet` means,
where regtest needed them rewritten as `bcrt1`. The key agents sign, the
PSBT with another payee is refused, Core confirms the spend, and its wallet
recovers 79,000 sat from the descriptor, as in regtest. The script rules
are the same on both chains, so this adds the addresses and nothing more.
It is not the public signet: the node has no peers and nothing reaches
anyone else.

## The example

The example is
[nostr/examples/agent-treasury.coax](../examples/agent-treasury.coax). It
reads the three enrollments and the wallet, recomputes the descriptor,
verifies each possession proof and admits the wallet, makes alice's proof
again from her account key, tries two bad proofs and the two broken
declarations, derives receive 0
and change 0, runs the request through all three signers, then the PSBT
with a different payee, five broken requests, the ledger and the timelock,
rotation, delay and hot-ceiling checks. Then each agent signs the spend
with its account key (test keys from `keys_sign_fixtures.coax`), the
coordinator combines alice's and keyco's signatures, and alice is asked to
sign the PSBT with the other payee.

<!-- source: nostr/examples/agent-treasury.coax example -->
```jai
KY_WORDS :: [44]string.["ok","json","version","field","wallet","holder","revision","network","descriptor","checksum","family","paths",
  "threshold","timelock","primary","enrollment","key_order","xpub","recipient","times","attestation","operator","sovereign","recovery",
  "attested_only","weakening","approval","psbt","inputs","outputs","amount","address","change","fee","ceiling","nonce","deadline","locked",
  "retired","compromised","delay","hot","not_built","possession"];
NAMES :: [3]string.["alice","keyco","enclave"];
CEILING :: [3]int.[2000,1000,1500];   // each signer's fee ceiling, satoshis
kw :: (c:KyCode)->string {return KY_WORDS[cast(int)c];}
to_hex :: (b:[]u8,out:[]u8)->string {
  hx:="0123456789abcdef";
  for i:0..<b.count {out[2*i]=hx[cast(int)(b[i]>>4)];out[2*i+1]=hx[cast(int)(b[i]&15)];}
  return cast(string)out[0..<2*b.count];
}
// The PSBT bytes from hex.
psbt_bytes :: (h:string,out:[]u8)->[]u8 {return ky_unhex(h,out);}
// One request checked by all three signers.
sign_all :: (w:KyWallet,wd:[32]u8,sc:KyScripts,request:string,psbt:[]u8,now:int) {
  r:=ky_request_read(request);
  for i:0..<3 {print("  %: %\n",NAMES[i],kw(ky_sign_check(w,wd,sc,r,psbt,CEILING[i],now)));}
}
```

<!-- source: nostr/examples/agent-treasury.coax main -->
```jai
main :: () {
  print("enrollments\n");
  p:=ky_parties();
  for i:0..<3 {
    e:=p.e[i];d:=p.ind[i];
    print("%: read %, class %, origin %, operator % (read %)\n",NAMES[i],kw(e.code),ifx e.attested then "attested" else "software",
      e.xpub[0..<23],d.op,kw(d.code));
  }
  print("the wallet\n");
  w:=ky_wallet_read(KY_WALLET);
  print("read %, network %, revision %, % path: % needs % of % keys, timelock none\n",kw(w.code),w.network,w.revision,w.npaths,
    w.paths[0].name,w.paths[0].threshold,w.paths[0].count);
  d:[4096]u8;n:=ky_descriptor(w,p.e[..],d[..]);
  print("recomputed descriptor (% bytes) ends %\n",n,cast(string)d[n-40..<n]);
  print("equal to the wallet's: %\n",cast(string)d[0..<n]==w.descriptor);

  print("possession proofs (BIP-322, the key at xpub/0/0)\n");
  bx:[1]B322Work;mb:[KY_PROOF_TEXT]u8;ab:[90]u8;hp:[68]u8;
  for i:0..<3 {
    print("%: message %...\n",NAMES[i],ky_possession_message(p.e[i],mb[..])[0..<23]);
    ch:=ky_possession_script(p.e[i]);
    print("  challenge %, proof % characters: %\n",to_hex(ch.b[0..<22],hp[..]),p.e[i].possession.count,ky_possession(p.e[i],bx[..]));
  }
  a:=ky_admit_proven(w,p.e[..],p.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0,bx[..]);
  print("admit: %, possession proven: %\n",kw(a.code),a.proven);
  // alice makes her proof again: the key at account/0/0, chapter 96's signer.
  pa:=account_of(0);pw:[1]KySignWork;made:[256]u8;none32:[32]u8;
  ok:=ky_slot_key(pa[..],0,pw[..]);
  mn:=bip322_sign_vartime(ky_possession_address(p.e[0],ab[..]),ky_possession_message(p.e[0],mb[..]),pw[0].k2[..],made[0..<0],B322_SIMPLE,0,0,0,none32[..],bx[..],made[..]);
  ky_sign_clear(pw[..]);
  print("alice signs hers again from her account key: equal to the enrollment's: %\n",ok && mn>0 && cast(string)made[0..<mn]==p.e[0].possession);
  q:=p;q.e[1].possession=p.e[0].possession;
  print("keyco's enrollment with alice's proof: %\n",kw(ky_admit_proven(w,q.e[..],q.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0,bx[..]).code));
  bad:=ky_enrollment_read(KY_AGENT1,KY_ENR1_BAD_PROOF);
  print("keyco's enrollment with the proof its key made for wallet %...: read %, proven %\n",KY_OTHER_WALLET_ID[0..<8],kw(bad.code),ky_possession(bad,bx[..]));
  bad.wallet=KY_OTHER_WALLET_ID;
  print("  the same proof read against wallet %...: %\n",KY_OTHER_WALLET_ID[0..<8],ky_possession(bad,bx[..]));

  print("composition rule 1 and 2, by declaration\n");
  q=p;q.ind[2].op="keyco-llc";
  print("enclave declares keyco-llc: %\n",kw(ky_admit(w,q.e[..],q.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0).code));
  q=p;q.ind[2].op="alice-agent";
  print("enclave declares alice-agent, holder a SOV agent: %\n",kw(ky_admit(w,q.e[..],q.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0).code));
  q=p;q.e[1].xpub=p.e[2].xpub;
  print("keyco's xpub replaced by enclave's after its digest was checked: %\n",kw(ky_admit(w,q.e[..],q.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0).code));

  print("scripts from the descriptor alone\n");
  sc:=ky_scripts(w,p.e[..]);h:[68]u8;
  print("receive 0: %\n",to_hex(sc.s[0].b[0..<34],h[..]));
  print("  is %: %\n",KY_RECEIVE0,ky_same(sc.s[0],ky_address(w,KY_RECEIVE0)));
  print("change 0:  %\n",to_hex(sc.s[KY_GAP].b[0..<34],h[..]));
  print("  is %: %\n",KY_CHANGE0,ky_same(sc.s[KY_GAP],ky_address(w,KY_CHANGE0)));

  print("the signing request\n");
  pb:[512]u8;ps:=psbt_bytes(KY_PSBT,pb[..]);
  t:=ky_psbt_read(ps);
  print("PSBT % bytes: read %, % inputs (% + % sat), % outputs (% + % sat)\n",ps.count,kw(t.code),t.nin,t.in_value[0],t.in_value[1],
    t.nout,t.out_value[0],t.out_value[1]);
  print("fee = % - % = % sat\n",t.in_value[0]+t.in_value[1],t.out_value[0]+t.out_value[1],t.in_value[0]+t.in_value[1]-t.out_value[0]-t.out_value[1]);
  r:=ky_request_read(KY_REQUEST);
  print("request: read %, % outputs stated, fee_sat %, change on output 2: %\n",kw(r.code),r.nout,r.fee,r.out[1].change);
  wd:=ky_bytes(KY_WALLET_DIGEST);
  sign_all(w,wd,sc,KY_REQUEST,ps,KY_T0);

  print("a PSBT whose first output pays %\n",KY_EVIL_ADDRESS);
  eb:[512]u8;evil:=psbt_bytes(KY_PSBT_EVIL,eb[..]);
  sign_all(w,wd,sc,KY_REQUEST_EVIL,evil,KY_T0);

  print("broken requests (enclave's ceiling, 1500 sat)\n");
  c:[2048]u8;
  x:=ky_request_read(ky_case(c[..],KY_REQUEST,"\"change\":false","\"change\":true"));
  print("the payment marked as change: %\n",kw(ky_sign_check(w,wd,sc,x,ps,1500,KY_T0)));
  x=ky_request_read(ky_case(c[..],KY_REQUEST,"\"amount_sat\":29000","\"amount_sat\":29500"));
  print("change stated as 29500: %\n",kw(ky_sign_check(w,wd,sc,x,ps,1500,KY_T0)));
  x=ky_request_read(ky_case(c[..],KY_REQUEST,"\"fee_sat\":1000","\"fee_sat\":500"));
  print("fee stated as 500: %\n",kw(ky_sign_check(w,wd,sc,x,ps,1500,KY_T0)));
  print("a ceiling of 999: %\n",kw(ky_sign_check(w,wd,sc,r,ps,999,KY_T0)));
  print("at the deadline: %\n",kw(ky_sign_check(w,wd,sc,r,ps,1500,r.deadline)));

  print("idempotency (keyco's ledger)\n");
  l:=[1]KyLedger.[KyLedger.{}];
  print("first: %\n",kw(ky_consume(l[..],KY_AGENT1,r,KyCode.ok).code));
  y:=ky_consume(l[..],KY_AGENT1,r,KyCode.ok);
  print("exact retry: %, retained: %\n",kw(y.code),y.retry);
  x=ky_request_read(ky_case(c[..],KY_REQUEST,"\"amount_sat\":70000","\"amount_sat\":71000"));
  print("same nonce, changed bytes: %\n",kw(ky_consume(l[..],KY_AGENT1,x,KyCode.ok).code));

  print("recovery, rotation, the co-signer and the hot balance\n");
  rec:KyPath;rec.older=KY_MIN_LOCK;
  print("older(4320), coin confirmed at %, tip %: %\n",KY_H0,KY_H0+4318,kw(ky_path_open(rec,KY_H0,KY_H0+4318)));
  print("older(4320), coin confirmed at %, tip %: %\n",KY_H0,KY_H0+4319,kw(ky_path_open(rec,KY_H0,KY_H0+4319)));
  none:[1]string;
  print("a retired wallet, a request that is not its sweep: %\n",kw(ky_retired(true,r.digest,none[0..<0])));
  print("a compromised signer of a 2-of-3 sweep, one compromised: %\n",kw(ky_sweep_signer(w.paths[0],1,true)));
  print("1,000,001 sat over a 1,000,000 delay threshold, 3600 s after receipt of 86400: %\n",kw(ky_delay(1000001,1000000,86400,0,3600,false)));
  print("a 600,000 sat top-up onto 500,000,000 msat under a 1,000,000,000 msat ceiling: %\n",kw(ky_topup(500000000,600000,1000000000)));

  print("signatures (BIP-143 SIGHASH_ALL, RFC 6979 ECDSA, low-S)\n");
  wit:=ky_witnesses(w,p.e[..]);
  print("witness script at receive 0: % bytes, 2-of-3 CHECKMULTISIG\n",wit.n[0]);
  signed:[3]KySigned;
  for i:0..<3 {
    acct:=account_of(i);
    led:[1]KyLedger;work:[1]KySignWork;
    signed[i]=ky_sign(w,wd,sc,wit,p.e[..],i,r,ps,CEILING[i],KY_T0,led[..],acct[..],work[..]);
    s:=signed[i];
    print("%: %, % signatures; input 1 key %...\n",NAMES[i],kw(s.code),s.nin,to_hex(s.p[0].pub[0..<8],h[..]));
    print("  sig %... (% bytes, hash type %)\n",to_hex(s.p[0].sig[0..<12],h[..]),s.p[0].n,s.p[0].sig[s.p[0].n-1]);
  }
  print("each equal to libsecp256k1's: %\n",same_partials(signed[..]));
  tx:[2048]u8;big:[4096]u8;
  print("alice alone: %\n",ky_combine(w,sc,wit,ps,signed[0..<1],tx[..]));
  m:=ky_combine(w,sc,wit,ps,signed[0..<2],tx[..]);
  print("alice and keyco: a % byte transaction, equal to Python's: %\n",m,to_hex(tx[0..<m],big[..])==KY_FINAL_TX);
  id:[32]u8;rev:[32]u8;bt_sha2(ky_unsigned(ps),id[..]);for k:0..<32 {rev[k]=id[31-k];}
  print("txid % (Python's: %)\n",to_hex(rev[..],h[..]),to_hex(rev[..],h[..])==KY_TXID);
  acct0:=account_of(0);work0:[1]KySignWork;
  ex:=ky_sign(w,wd,sc,wit,p.e[..],0,ky_request_read(KY_REQUEST_EVIL),evil,CEILING[0],KY_T0,l[..],acct0[..],work0[..]);
  print("alice asked to sign the PSBT paying %: %, % signatures\n",KY_EVIL_ADDRESS[0..<12],kw(ex.code),ex.nin);
}
// Agent a's account key and chain code (test fixtures).
account_of :: (a:int)->[64]u8 {r:[64]u8;k:=ky_unhex(KY_ACCOUNT_KEY[a],r[0..<32]);c:=ky_unhex(KY_ACCOUNT_CHAIN[a],r[32..<64]);return r;}
same_partials :: (s:[]KySigned)->bool {
  h:[160]u8;
  for a:0..<3 {for k:0..<2 {if to_hex(s[a].p[k].sig[0..<s[a].p[k].n],h[..])!=KY_PARTIAL[2*a+k] {return false;}}}
  return true;
}
```

Its output, which the book's checks compare in debug and release:

```text
enrollments
alice: read ok, class software, origin [78c16713/48h/1h/0h/2h], operator alice-agent (read ok)
keyco: read ok, class software, origin [669198ad/48h/1h/0h/2h], operator keyco-llc (read ok)
enclave: read ok, class attested, origin [66d2046a/48h/1h/0h/2h], operator enclave-co (read ok)
the wallet
read ok, network signet, revision 0, 1 path: primary needs 2 of 3 keys, timelock none
recomputed descriptor (457 bytes) ends PmBUisT1sfsHbJRSwJ8vg/<0;1>/*))#660mkcnk
equal to the wallet's: true
possession proofs (BIP-322, the key at xpub/0/0)
alice: message sha256:963a6edba8240b48...
  challenge 001472369fbd9ef1c382297e34679259e782b224578a, proof 147 characters: true
keyco: message sha256:8e038bb18c14c3ef...
  challenge 001447c3077aae94bb866ba6e743a75c2661b203b6c9, proof 147 characters: true
enclave: message sha256:427f822bfcf1fd53...
  challenge 0014b231386756fd222528db030b9e5afe6d67c2cde7, proof 147 characters: true
admit: ok, possession proven: true
alice signs hers again from her account key: equal to the enrollment's: true
keyco's enrollment with alice's proof: possession
keyco's enrollment with the proof its key made for wallet 78787878...: read ok, proven false
  the same proof read against wallet 78787878...: true
composition rule 1 and 2, by declaration
enclave declares keyco-llc: operator
enclave declares alice-agent, holder a SOV agent: sovereign
keyco's xpub replaced by enclave's after its digest was checked: descriptor
scripts from the descriptor alone
receive 0: 0020187ef2ba1a2f4237094fa0f25106d3af890c8b8e7170d4f78ac27b3e776d2f6f
  is tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2: true
change 0:  0020108dd2a02dce6bec96de704f7383e5b9c94e8fa489efa12b939de5777ab2009f
  is tb1qzzxa9gpdee47e9k7wp8h8ql9h8y5aray38h6z2unnhjhw74jqz0sr0utuy: true
the signing request
PSBT 271 bytes: read ok, 2 inputs (60000 + 40000 sat), 2 outputs (70000 + 29000 sat)
fee = 100000 - 99000 = 1000 sat
request: read ok, 2 outputs stated, fee_sat 1000, change on output 2: true
  alice: ok
  keyco: ok
  enclave: ok
a PSBT whose first output pays tb1qz5tpwxqergd3c8g7ruszzg3rysjjvfeg5mag35
  alice: outputs
  keyco: outputs
  enclave: outputs
broken requests (enclave's ceiling, 1500 sat)
the payment marked as change: change
change stated as 29500: amount
fee stated as 500: fee
a ceiling of 999: ceiling
at the deadline: deadline
idempotency (keyco's ledger)
first: ok
exact retry: ok, retained: true
same nonce, changed bytes: nonce
recovery, rotation, the co-signer and the hot balance
older(4320), coin confirmed at 900000, tip 904318: locked
older(4320), coin confirmed at 900000, tip 904319: ok
a retired wallet, a request that is not its sweep: retired
a compromised signer of a 2-of-3 sweep, one compromised: compromised
1,000,001 sat over a 1,000,000 delay threshold, 3600 s after receipt of 86400: delay
a 600,000 sat top-up onto 500,000,000 msat under a 1,000,000,000 msat ceiling: hot
signatures (BIP-143 SIGHASH_ALL, RFC 6979 ECDSA, low-S)
witness script at receive 0: 105 bytes, 2-of-3 CHECKMULTISIG
alice: ok, 2 signatures; input 1 key 027ae41a7f786c8d...
  sig 3045022100d49ed3818ca822... (72 bytes, hash type 1)
keyco: ok, 2 signatures; input 1 key 0360ab78dedf6908...
  sig 304502210090458259581594... (72 bytes, hash type 1)
enclave: ok, 2 signatures; input 1 key 03e47364df38f1c8...
  sig 304402202f487509db2e3d6a... (71 bytes, hash type 1)
each equal to libsecp256k1's: true
alice alone: -1
alice and keyco: a 675 byte transaction, equal to Python's: true
txid 101d9f23d34871c476143aa4d9f64cfd2ea71cada47c93ed4c654c0d58361347 (Python's: true)
alice asked to sign the PSBT paying tb1qz5tpwxqe: outputs, 0 signatures
```

Two refusals to look at. The first is `possession`: keyco's enrollment
carrying alice's proof is refused at admission, and the proof keyco's own
key made for wallet `7878...` proves nothing in this wallet, though it
verifies against the wallet it names. The second is the PSBT whose first
output pays an address the request doesn't name: `outputs` from every
signer, and no signature when alice is asked to sign it. The signatures that follow are the ones
libsecp256k1 makes for the same keys and digests, and alice's and keyco's
combine into the 675-byte transaction Python built; alice alone can't.

## How the code is checked

### Checks in the library

`coax/lib/nostr/nipkeys.coax` is `#authority`: every procedure is exercised
by a `#test`, and `coax mutate --root-only` must leave no survivors. Eight
shards hit the 14-minute limit, so the run is split 40 ways
(`coax mutate coax/lib/nostr/nipkeys.coax --root-only --shard I/40`), each
shard about a minute with `--jobs 8`. On 2026-11-01 the first pass over all
40 shards, on the file as this chapter first shipped, left 179 of 2054
mutants alive. They were killed with sharper checks or by removing the code
they showed had no effect (the PSBT map reader now reports `ok` and `found`
flags instead of `-1` positions, and redundant length guards are gone),
and no mutant was marked equivalent. The second pass, on the file as it now
stands, ran all 40 shards (1 to 40) and merged their records: 1963 mutants,
1963 killed, 0 survived, with 273 checks passing. Its
checks cover every reader's refusals field by field, the descriptor for one
path and for a recovery path (against a Python checksum), every composition
rule at its boundary, rule 4's successor approvals, the PSBT reader's
malformed inputs, every signer refusal, the ledger, and the timelock,
rotation, delay and hot-ceiling functions at their boundaries. On
2026-11-03 the ledger began keeping its own copies of the agent,
requester and nonce (#12692). A new check reads requests into one buffer
and the agent into another, reuses both, and asserts that the first
triple's exact retry still returns its outcome. Before the fix that
request refused as `nonce` and the old triple was gone. The file then
passed 299 checks, and all 20 root-only shards merged to 2006 mutants,
2006 killed, 0 survived, none marked equivalent. `nipkeys_sign.coax`
passed its 503 checks and the example's checks file passed 506. Later that
day the copies moved to the standard library's `Kept(8)` (#12696): the file
passed 345 checks (its own, `kept.coax`'s 6 and the loaded units'), all 20
root-only shards merged to 1970 mutants, 1970 killed, 0 survived, none
marked equivalent, and `kept.coax`'s own run killed all 173.
`nipkeys_sign.coax` passed 509 checks and the example's checks file 512.
On 2026-11-12 the reader began keeping `possession`, `KyCode` gained
`possession`, and the fixtures' enrollments took real proofs, which changed
every digest the checks quote. The file passed 375 checks, and 8 root-only
shards merged to 1947 mutants, 1947 killed, 0 survived, none marked
equivalent. `nipkeys_sign.coax` (525 checks) and `nipkeys_carry.coax`
(456 checks) were not edited and pass with the new fixtures; their
mutation runs were not repeated.

`coax/lib/nostr/nipkeys_possess.coax` holds the possession rules and is
`#authority`. It passed 394 checks (its own five and `nipkeys.coax`'s), and
on 2026-11-12 six root-only shards merged to 84 mutants, 84 killed, 0
survived, none marked equivalent. The pass before left 6 of 81 alive, each
pointing at code with no effect: a second length test on the message
buffer, a slice bound the hash never reached, an address length compared
with 0 where only 42 or an error can come back, and a cap on the number of
proofs that returned the same count either way. The buffer test is an
assertion now, the hash goes through 20 bytes of its own, the length is
compared with 42, and more enrollments than a wallet has keys ask for no
proofs. Its checks hold the message against Python's for all three agents
and at both buffer boundaries, the script against Python's key, the address
against Python's bech32, BIP-322's three answers, and admission with one
answer missing, one too many, and each one false.

`coax/lib/nostr/nipkeys_proof.coax` joins those rules to BIP-322's verifier
and is not `#authority`: it has no rule of its own, and a mutation run that
loads `bip322.coax` did not finish a sixth of its mutants in 15 minutes
here (Bitcoin chapter 96 records that `bip322.coax` itself has had no
mutation run). Its 639 checks, the loaded units' included, run the real
verifier: every agent's proof, the proof for another wallet, another key's
proof, another agent's name, one changed character, and the whole
admission.

The signing files are `#authority` too. On 2026-11-01 root-only mutation
left zero survivors in `coax/lib/crypto/ecdsa_sign.coax` (224 mutants,
153 checks; rerun after the shared test helpers changed, 224 of 224
again, in 2 shards) and `coax/lib/bitcoin/sighash.coax` (406 mutants, 287
checks). `coax/lib/nostr/nipkeys_sign.coax` (331 mutants, 415 checks with
nipkeys.coax's) ran in 8 shards
(`coax mutate coax/lib/nostr/nipkeys_sign.coax --root-only --run-steps 250000000 --shard I/8 --jobs 4`,
11 to 12 minutes each on a loaded machine) and its merged records report
331 mutants, 331 killed, 0 survived. Getting there took three changes to
how the checks run, none weaker. The file allocates strings, so every
mutant runs the checks in order until one fails: the checks are now in
order of cost, with the xpub derivation (about 1.1 billion steps) last,
and `ky_witnesses` reads each xpub and derives each branch key once
(18 public derivations, not 36). The PSBT and combine checks take the
signers' results from the libsecp256k1 fixtures (`ks_signed`) instead of
signing again, and "each agent signs" checks that `ky_sign` returns those
bytes, its PSBT digest included. The pass before the last left two
survivors: the `inputs` refusal's retry flag wasn't checked (it is now),
and `ky_ws_is` hashed into a view whose end could move by one byte
without effect (it hashes into 32 bytes and copies them now).
The first runs left survivors
that pointed at code with no effect: RFC 6979's second candidate wasn't
checked (it now is, against Python), status results of hashes that can't
fail were chained into `ok` flags (they are assertions now), and the
transaction reader had guards the final length check already made; no
mutant was marked equivalent. The checks hold each function at its
boundaries: compact sizes in all three forms, 16 and 17 inputs and outputs,
a 4,096-byte transaction and one byte more, every BIP-341 hash type from
-1 to 256, DER lengths and sign bytes, the low-S boundary at *n*/2, and
every refusal of the key agent (another agent's key, a changed chain code,
a witness script that doesn't hash to the input's program, a PSBT that
isn't the one signed).

The key and nonce code follows Bitcoin chapter 3's rules by construction (masks,
fixed loops, the comb); the source audit's profile doesn't yet accept
arrays and loops, so it was not run on these files.

### Independent checks

The checks in
[nostr/examples/checks/agent-treasury.coax](../examples/checks/agent-treasury.coax)
compare the code with values it didn't produce:

- BIP-380's published checksum `raw(deadbeef)#89f8spxm`;
- BIP-32 test vector 1: the public child of `m/0H`'s xpub at index 1 is
  `03501e45...cd711c`;
- the wallet's four scripts (receive 0, 1 and change 0, 1), the enrollment
  and wallet digests, the descriptors' checksums and the PSBT digest, all
  written by an independent Python reference that uses Python integers for
  the curve and `hashlib` for hashing and shares no code with Coax.
- BIP-143's published examples (native P2WPKH, P2SH-P2WPKH, and the
  P2SH-P2WSH 6-of-6 with all six hash types): every sighash, and every
  signature from the published private keys byte for byte (the published
  signatures are RFC 6979 with low-S, so a correct signer reproduces them);
- BIP-341's `keyPathSpending` wallet vectors: the five intermediate
  hashes, all seven inputs' sighashes, internal keys, tweaks, tweaked keys
  and witness signatures;
- libsecp256k1 v0.8.0, the pinned oracle of Bitcoin chapter 13:
  `coax/tests/bitcoin/ecdsa_sign_libsecp.py` sends it 1,200 Coax ECDSA
  signatures (release) and 200 more (checked build under ASan/UBSan), and
  every one parses as DER, is low-S, verifies, and equals libsecp256k1's own
  `secp256k1_ecdsa_sign` byte for byte (0 failures, 2026-11-01);
- the treasury's spend from `keys_sign_reference.py`: Python derives the
  account and child keys, the witness scripts and the BIP-143 digests on
  its own, libsecp256k1 signs, and Python assembles each signer's PSBT and
  the final transaction and verifies every signature over its own digest
  and in the witness script's key order. The Coax signatures, PSBTs and
  transaction are equal to these bytes.
- the possession proofs from `keys_reference.py`: Python derives each
  agent's private key at `account/0/0`, builds the message and the P2WPKH
  challenge, and signs and verifies with
  [bip322_reference.py](../../coax/tests/bitcoin/bip322_reference.py), a
  pure-Python BIP-322 with its own script interpreter. Coax verifies those
  proofs, and Coax's signer, given the same keys, writes the same 147
  characters;
- BIP-322's published P2WPKH signature of `Hello World`, which passes
  through the same verifier.

RFC 6979 itself has no secp256k1 vectors (its appendix covers the NIST and
binary curves), so the nonce is checked against an independent Python
HMAC-DRBG and through the libsecp256k1 equality above.

<!-- source: nostr/examples/checks/agent-treasury.coax check -->
```jai
// BIP-380's published checksum, and two descriptors whose checksums an
// independent Python implementation of BIP-380's polymod computed.
#test "descriptor checksums: BIP-380's vector and Python's" {
  cs:[8]u8;
  #assert ky_checksum("raw(deadbeef)",cs[..]) && cast(string)cs[..]=="89f8spxm";
  w:=ky_wallet_read(KY_WALLET);
  #assert w.descriptor[w.descriptor.count-9..<w.descriptor.count]=="#660mkcnk" && ky_checksum_ok(w.descriptor);
  #assert KY_DESC2[KY_DESC2.count-9..<KY_DESC2.count]=="#slnp8lul" && ky_checksum_ok(KY_DESC2);
}
// BIP-32 test vector 1, chain m/0H/1: the public child of m/0H's xpub.
#test "BIP-32 vector 1: m/0H/1 from m/0H's xpub" #steps 500000000 {
  c:=ky_child(ky_ext_key(KY_BIP32_M0H),1);
  #assert c.ok && a3_hex_is(c.key[..],"03501e454bf00751f24b1b489aa925215d66af2234e3891c3b21a52bedb3cd711c");
}
// The wallet's four scripts from Python's pure-integer BIP-32 and
// hashlib, and the enrollment, wallet and PSBT digests it wrote.
#test "the wallet's scripts and digests match Python" #steps 2000000000 {
  w:=ky_wallet_read(KY_WALLET);p:=ky_parties();
  sc:=ky_scripts(w,p.e[..]);
  for i:0..<4 {#assert ky_same(sc.s[i],ky_spk(KY_SPK[i]));}
  for i:0..<3 {#assert ky_eq32(p.e[i].digest,w.enrollments[i]);}
  d:[32]u8;#assert ky_sha(KY_WALLET,d[..]) && a3_hex_is(d[..],KY_WALLET_DIGEST);
  b:[512]u8;ps:=ky_unhex(KY_PSBT,b[..]);
  #assert sha256_hash_checked(ps,d[..],false)==SHA_OK && a3_hex_is(d[..],"4d98f570ee1e7e274be0736b532e9978fcd92f86743a514ac9a4c480a3ca0812");
}
```

The possession proofs are checked in both directions, and the verifier
against a vector neither side wrote:

<!-- source: nostr/examples/checks/agent-treasury.coax possession_check -->
```jai
// Possession proofs in both directions. Python (keys_reference.py with
// bip322_reference.py, pure integers) signed the proofs in the fixtures;
// Coax verifies them. Then Coax signs: each agent's key at account/0/0,
// derived from its account key, through chapter 96's BIP-322 signer, gives
// the bytes Python wrote. The verifier is anchored outside both: BIP-322's
// published P2WPKH signature of "Hello World" passes through it too.
#test "possession: Python's proofs verify, Coax signs the same bytes, BIP-322's vector passes" #steps 20000000000 {
  x:[1]B322Work;p:=ky_parties();
  for i:0..<3 {
    m:[KY_PROOF_TEXT]u8;a:[90]u8;out:[256]u8;aux:[32]u8;work:[1]KySignWork;acct:=account_of(i);
    #assert ky_possession(p.e[i],x[..]);
    #assert ky_possession_message(p.e[i],m[..])==KY_PROOF_MESSAGE[i];
    #assert ky_slot_key(acct[..],0,work[..]);
    n:=bip322_sign_vartime(ky_possession_address(p.e[i],a[..]),KY_PROOF_MESSAGE[i],work[0].k2[..],out[0..<0],B322_SIMPLE,0,0,0,aux[..],x[..],out[..]);
    #assert n>0 && cast(string)out[0..<n]==p.e[i].possession;
  }
  v:=B322_CASES[2];
  #assert v.source=="basic simple" && v.message=="Hello World" && v.state==B322_VALID && b322_case_passes(v,x[..]);
  // The conformance fixture NIP-KEYS asks for: an enrollment with a bad
  // possession proof reads, and admission of a wallet that names it stops
  // at the proof.
  bad:=ky_enrollment_read(KY_AGENT1,KY_ENR1_BAD_PROOF);
  #assert bad.code==KyCode.ok && !ky_possession(bad,x[..]);
  w:=ky_wallet_read(KY_WALLET);q:=p;q.e[1].possession=bad.possession;
  r:=ky_admit_proven(w,q.e[..],q.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0,x[..]);
  #assert r.code==KyCode.possession && !r.proven;
}
```

NIP-KEYS has no worked examples or JSON fixtures of its own, so there are no
published record IDs to compare with; the record bytes and their digests
come from the Python reference.

## Not built

- **A broadcast on the public signet.** The spend and the recovery run in
  regtest and on a private signet, where Bitcoin Core runs the script. The
  public signet needs signet coins, and the public faucets hand them out
  only behind a captcha or a GitHub sign-in. The same tools broadcast
  through a public signet node once a faucet pays the treasury's receive 0,
  `tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2`.
- **Miniscript spends.** Wallets with a recovery path are admitted (their
  descriptor is recomputed), but the signer refuses their spends as
  `not_built`; three or more paths refuse at admission.
- **`tr(...)` wallets.** The BIP-341 digest, the key tweak and key-path
  signatures are built and match BIP-341's vectors, but admission still
  refuses `tr` descriptors and the signer signs only `wsh` inputs.
- **PSBT v2**, MuSig2 and FROST.
- **Liveness, rotation and agency-terms bodies**, a long-running
  coordinator service (the tool runs one request at a time), and MKT settlement. The checks they decide
  (retired wallets, compromised sweeps, the delay, the hot ceiling) are
  pure functions here.
- Showing the holder the attributes two keys in a quorum share
  (jurisdictions, vendors, hosts, affiliations).

NIP-KEYS says a possession proof is a BIP-322 signature "by the key at its
first receive derivation over the digest of `{wallet, agent, xpub}`" and
stops there. The message text, the JSON it hashes and the P2WPKH challenge
in [Possession](#possession-proving-the-key-is-held) are this chapter's
reading, and the NIP doesn't state them yet.

Adding `NIP-KEYS.md` changed the OpenAgents NIP folder digest pinned in
`coax/lib/nostr/profiles.coax` and `profiles.json` (`2fb1f51f…`), and other
NIPs in the folder have changed since, so the pin test in
`coax/tests/nostr/profiles.py` fails. The owner reviews the NIP before it
is re-pinned; this chapter doesn't re-pin it.

## Summary

- A NIP-KEYS wallet is admitted only if its BIP-380 descriptor is the one
  its paths and enrollments describe, and only if no operator, and no
  sovereign holder, can meet a threshold alone.
- Every enrollment proves its key: a BIP-322 signature by the key at
  `xpub/0/0` over the digest of the wallet, the agent and the xpub. A proof
  from another key, or for another wallet, refuses as `possession`.
- A signer derives the wallet's scripts from xpubs, reads the PSBT, and
  refuses any input that isn't the wallet's, any output, amount or change
  that differs from the request, and any fee that differs or exceeds its
  ceiling.
- A retry returns the retained outcome; a reused nonce with changed bytes
  refuses.
- A key agent signs each input with ECDSA over its BIP-143 digest, its
  nonce from RFC 6979 so a retry signs the same bytes, and records the
  outcome before the signature exists; a coordinator needs a threshold of
  signatures for the same PSBT to build the transaction.
- Requests and receipts travel as 3188 artifacts over a Coax relay; Bitcoin
  Core confirms the spend, and recovers the coins from the descriptor with
  its own wallet, in regtest and on a private signet. A broadcast on the
  public signet is the step left.

## Dependencies

- [Bitcoin chapter 24. Sign with Schnorr](../../bitcoin/book/ch024-sign-with-schnorr.md)
- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md)
- [79. Agents, skills and automation](ch079-agents-skills-and-automation.md)
- [83. Attested work and sealed data](ch083-attested-work-and-sealed-data.md)
- [73. Payment targets](ch073-payment-targets.md), for segwit addresses

## Sources

- [OpenAgents NIP-KEYS](../nips/openagents/NIP-KEYS.md)
- [OpenAgents NIP-SOV](../nips/openagents/NIP-SOV.md)
- [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md)
- [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki),
  [BIP-66](https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki),
  [BIP-143](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki),
  [BIP-146](https://github.com/bitcoin/bips/blob/master/bip-0146.mediawiki),
  [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  [RFC 6979](https://www.rfc-editor.org/rfc/rfc6979),
  [BIP-174](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki),
  [BIP-380](https://github.com/bitcoin/bips/blob/master/bip-0380.mediawiki)
  and [BIP-383](https://github.com/bitcoin/bips/blob/master/bip-0383.mediawiki).
- Research: [A network of keys](../../docs/research/2026-11-01-network-of-keys.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
