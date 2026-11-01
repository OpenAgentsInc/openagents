# 104. Agent treasuries and a network of keys

> **Status: partly implemented.** The pure contracts (NIP-KEYS step 1) and
> the signer's checks from step 2 run, with a worked example, a refusal
> and independent checks. Producing signatures, the 3188 transport, the
> signet round trip and recovery with an independent wallet tool are not
> built, so the chapter's first two "Done means" lines don't hold yet. See
> [Not built](#not-built).

An agent that earns and spends bitcoin needs somewhere safe to keep it. One
hot key on the agent, or one wallet host, is a single point of failure. This
chapter builds the checking half of the alternative: a wallet whose coins
move only when enough independent key agents sign, described by records
that every party can rebuild and check alone.

Part XII, Nostr in OpenAgents. Result: NIP-KEYS wallet, enrollment,
independence and signing-request bodies read under closed schemas; a
wallet admitted by recomputing its BIP-380 descriptor and applying the
four composition rules; the wallet's P2WSH scripts derived from public keys
alone; and a signer that recomputes every output, the change and the fee
from the PSBT before it would sign, with nonce idempotency, recovery
timelocks, retired wallets, the attested co-signer's delay and the
hot-balance ceiling.

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
- A signer that never trusts the request: it reads a PSBT v0, checks every
  input is the wallet's, compares every output with the request's
  statement, derives change from the descriptor and recomputes the fee.
- Idempotency by `(agent, requester, nonce)` with the request digest.
- What is not built, and why: signatures, BIP-322, Taproot, PSBT v2,
  miniscript spends, the relay round trip and signet.

## Where the code is

| File | What |
| --- | --- |
| [coax/lib/nostr/nipkeys.coax](../../coax/lib/nostr/nipkeys.coax) | NIP-KEYS: the checksum, the readers, xpub decoding and public derivation, admission, scripts, the PSBT reader and the signer's checks. `#authority` |
| [coax/tests/nostr/keys_fixtures.coax](../../coax/tests/nostr/keys_fixtures.coax) | The treasury's records, written by an independent Python reference |
| [nostr/examples/agent-treasury.coax](../examples/agent-treasury.coax) | The example |
| [nostr/examples/checks/agent-treasury.coax](../examples/checks/agent-treasury.coax) | Its independent checks |

The file reuses `json.coax` for parsing, `nipa3.coax` (chapter 73) to decode
segwit addresses, `bip32.coax` and `secp_mult.coax` (chapters 10 and 46) for
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
ky_polymod :: (c:u64,v:u64)->u64 {
  top:=c>>35;r:=((c&0x7ffffffff)<<5)^v;
  for i:0..<KY_GEN.count {if ((top>>cast(u64)i)&1)!=0 {r^=KY_GEN[i];}}
  return r;
}
// The eight checksum characters of a descriptor without its "#...", into
// out; false on a character outside BIP-380's input set.
ky_checksum :: (s:string,out:[]u8)->bool {
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
ky_descriptor :: (w:KyWallet,e:[]KyEnrollment,out:[]u8)->int {
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
ky_path :: (j:[]Json,p:[]KyPath)->KyCode {
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
ky_wallet_read :: (content:string)->KyWallet {
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
breaks the wallet. The `possession` field is a BIP-322 signature; this
build reads it and doesn't verify it.

<!-- source: coax/lib/nostr/nipkeys.coax enrollment -->
```jai
ky_enrollment_read :: (signer:string,content:string)->KyEnrollment {
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
    else if f==6 {if t!=JSON_STRING || s.count==0 {return ky_efail(KyCode.field);}}   // BIP-322: read, not verified
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

Admission binds the records together, then checks the rules:

<!-- source: coax/lib/nostr/nipkeys.coax admit -->
```jai
// Admission of one wallet revision at now (unix seconds) and height (the
// chain tip). e and ind are the enrollments in the wallet's order and each
// one's independence declaration. sov: the holder is a NIP-SOV agent.
// min_lock: the holder policy's shortest recovery timelock (KY_MIN_LOCK
// unless the policy says otherwise). proven stays false: possession proofs
// (BIP-322) are read and not verified in this build.
ky_admit :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,sov:bool,min_lock:int,now:int,height:int)->KyAdmit {
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
ky_operator :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,i:int)->string {
  if e[i].agent!=w.holder {return ind[i].op;}
  for k:0..<w.nenroll {if e[k].agent==w.holder {return ind[k].op;}}
  return "";
}
ky_compose :: (w:KyWallet,e:[]KyEnrollment,ind:[]KyIndependence,sov:bool,min_lock:int,height:int)->KyCode {
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
since nothing here is secret:

<!-- source: coax/lib/nostr/nipkeys.coax xpub -->
```jai
// A base58check extended public key: 78 bytes, version, depth, parent
// fingerprint, child number, chain code and a compressed public key.
ky_ext_key :: (s:string)->KyExtKey {
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
ky_child :: (parent:KyExtKey,index:int)->KyExtKey {
  r:KyExtKey;p:=parent;
  if !p.ok || index<0 || index>=BIP32_HARDENED {return r;}
  data:[37]u8;i:[64]u8;
  for k:0..<33 {data[k]=p.key[k];}
  for k:0..<4 {data[33+k]=cast(u8)((index>>(8*(3-k)))&255);}
  if !hmac_sha512(p.chain[..],data[..],i[..]) {return r;}
  il:=sc_load(i[0..<32]);a:=sg_decode_public(p.key[..]);
  if !il.ok || !a.ok {return r;}                          // IL >= n, or a key off the curve
  s:=ecmult_vartime(il.value,a.value,sc_small(1));        // IL·G + K: public data, variable time
  e:=sg_encode_public(s.value);
  if e.length!=33 {return r;}                             // infinity encodes as one byte
  for k:0..<33 {r.key[k]=e.bytes[k];}
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
ky_script :: (w:KyWallet,branch:[]KyExtKey,index:int)->KyScript {
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

## The signer

A signing request states every output (`address`, `amount_sat`, `change`),
the fee and a nonce, and names the wallet revision and the PSBT by digest:

<!-- source: coax/lib/nostr/nipkeys.coax request -->
```jai
ky_request_read :: (content:string)->KyRequest {
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
ky_psbt_read :: (b:[]u8)->KyPsbt {
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
ky_sign_check :: (w:KyWallet,wallet_digest:[32]u8,all:KyScripts,r:KyRequest,psbt:[]u8,ceiling:int,now:int)->KyCode {
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
ky_consume :: (l:[]KyLedger,agent:string,r:KyRequest,code:KyCode)->KyRetry {
  for i:0..<l[0].count {
    x:=l[0].e[i];
    if x.agent==agent && x.requester==r.requester && x.nonce==r.nonce {
      for k:0..<32 {if x.digest[k]!=r.digest[k] {return KyRetry.{code=KyCode.nonce};}}
      return KyRetry.{code=x.code,retry=true};
    }
  }
  if l[0].count==KY_LEDGER {return KyRetry.{code=KyCode.nonce};}
  l[0].e[l[0].count]=KyEntry.{agent=agent,requester=r.requester,nonce=r.nonce,digest=r.digest,code=code};
  l[0].count+=1;
  return KyRetry.{code=code};
}
```

A signature already returned can't be recalled, so a key agent records the
request before releasing its partial signature. The exact retry returns the
retained outcome; the same nonce with different bytes refuses.

### Timelocks, rotation, the co-signer and the hot ceiling

`ky_path_open` refuses a recovery spend before `older(n)` confirmations or
the `after` height; `ky_retired` refuses every spend of a retired wallet
except its rotation's sweeps; `ky_sweep_signer` leaves a compromised signer
out of a sweep when the others meet the threshold; `ky_delay` holds a spend
above the co-signer's `delay_above_sat` until `delay_seconds` after receipt
and cancels it on a primary refusal; `ky_topup` refuses a `hot-topup` that
would take the hot Lightning balance over `hot_ceiling_msat`.

## The example

The example is
[nostr/examples/agent-treasury.coax](../examples/agent-treasury.coax). It
reads the three enrollments and the wallet, recomputes the descriptor,
admits the wallet, tries the two broken declarations, derives receive 0
and change 0, runs the request through all three signers, then the PSBT
with a different payee, five broken requests, the ledger and the timelock,
rotation, delay and hot-ceiling checks.

<!-- source: nostr/examples/agent-treasury.coax example -->
```jai
KY_WORDS :: [43]string.["ok","json","version","field","wallet","holder","revision","network","descriptor","checksum","family","paths",
  "threshold","timelock","primary","enrollment","key_order","xpub","recipient","times","attestation","operator","sovereign","recovery",
  "attested_only","weakening","approval","psbt","inputs","outputs","amount","address","change","fee","ceiling","nonce","deadline","locked",
  "retired","compromised","delay","hot","not_built"];
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
  a:=ky_admit(w,p.e[..],p.ind[..],true,KY_MIN_LOCK,KY_T0,KY_H0);
  print("admit: %, possession proven: %\n",kw(a.code),a.proven);

  print("composition rule 1 and 2, by declaration\n");
  q:=p;q.ind[2].op="keyco-llc";
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
  print("partial signatures: not built\n");
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
admit: ok, possession proven: false
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
partial signatures: not built
```

The refusal to look at is the PSBT whose first output pays an address the
request doesn't name: `outputs` from every signer.

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
rotation, delay and hot-ceiling functions at their boundaries.

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

NIP-KEYS has no worked examples or JSON fixtures of its own, so there are no
published record IDs to compare with; the record bytes and their digests
come from the Python reference.

## Not built

- **Signatures.** No partial signature is produced: `wsh` spends need ECDSA
  with BIP-143 sighashes, `tr` spends BIP-341 sighashes. Without them the
  first "Done means" line (three key agents on signet sign a 2-of-3 spend
  from requests carried over a Coax relay) doesn't hold. The second half of
  that line holds in the checks: a PSBT whose outputs differ from its
  request is refused by every signer.
- **Recovery with an independent wallet tool.** The scripts are derived
  from the descriptor alone and match the Python reference, but no
  coordinator outage was recovered on signet.
- **BIP-322 possession proofs** are read and not verified: `ky_admit`
  returns `proven: false`, and nothing here moves coins.
- **Miniscript spends.** Wallets with a recovery path are admitted (their
  descriptor is recomputed), but the signer refuses their spends as
  `not_built`; three or more paths refuse at admission.
- **Taproot** `tr(...)`, **PSBT v2**, MuSig2 and FROST.
- **Receipts, liveness, rotation and agency-terms bodies**, the 3188
  transport, the coordinator, and MKT settlement. The checks they decide
  (retired wallets, compromised sweeps, the delay, the hot ceiling) are
  pure functions here.
- Showing the holder the attributes two keys in a quorum share
  (jurisdictions, vendors, hosts, affiliations).

Adding `NIP-KEYS.md` changed the OpenAgents NIP folder digest pinned in
`coax/tests/nostr/profiles.py` (now `fb7563e7…`, pinned `2fb1f51f…`). The
owner reviews the NIP before it is re-pinned; this chapter doesn't re-pin
it.

## Summary

- A NIP-KEYS wallet is admitted only if its BIP-380 descriptor is the one
  its paths and enrollments describe, and only if no operator, and no
  sovereign holder, can meet a threshold alone.
- A signer derives the wallet's scripts from xpubs, reads the PSBT, and
  refuses any input that isn't the wallet's, any output, amount or change
  that differs from the request, and any fee that differs or exceeds its
  ceiling.
- A retry returns the retained outcome; a reused nonce with changed bytes
  refuses.
- Signing and the signet round trip are the next step.

## Dependencies

- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)
- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md)
- [79. Agents, skills and automation](ch079-agents-skills-and-automation.md)
- [83. Attested work and sealed data](ch083-attested-work-and-sealed-data.md)
- [73. Payment targets](ch073-payment-targets.md), for segwit addresses

## Sources

- [OpenAgents NIP-KEYS](../nips/openagents/NIP-KEYS.md)
- [OpenAgents NIP-SOV](../nips/openagents/NIP-SOV.md)
- [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md)
- [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki),
  [BIP-174](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki),
  [BIP-380](https://github.com/bitcoin/bips/blob/master/bip-0380.mediawiki)
  and [BIP-383](https://github.com/bitcoin/bips/blob/master/bip-0383.mediawiki).
- Research: [A network of keys](../../docs/research/2026-11-01-network-of-keys.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
