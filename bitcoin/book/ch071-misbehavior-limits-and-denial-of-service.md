# 71. Misbehavior, limits and denial of service

Anyone can connect to a Bitcoin node and send it anything. A node that
did whatever its peers asked would run out of memory, disk or time, so
Bitcoin Core bounds every message, punishes peers that send what an
honest node never would, and stops reading from a peer that sends faster
than it can work. None of this needs an identity: a peer is judged only
by what it sends.

This chapter builds those defenses as Core's procedures: the size limit
of each message, what happens to a peer that misbehaves, which invalid
blocks count against the peer that sent them, the filter that remembers
discouraged addresses, the work threshold that keeps a node from
storing worthless headers, the flood control that bounds each peer's
memory, and the limits on transaction announcements.

## What this chapter teaches

- The limit of each message, and which limits disconnect a peer and
  which mark it as misbehaving.
- `Misbehaving` and `MaybeDiscourageAndDisconnect`: there is no ban
  score; one offense discourages and disconnects, unless the peer has
  the noban permission or was added by hand.
- `MaybePunishNodeForBlock`: which validation failures are the sender's
  fault.
- MurmurHash3 and `CRollingBloomFilter`, the fixed-size filter that holds
  discouraged addresses and each peer's known transactions and
  addresses.
- `BanMan`: discouragement against bans that expire.
- The anti-DoS work threshold for headers, and the memory bound of a
  headers presync.
- Flood control: why a peer costs at most about 15 MB.
- The limits on transaction announcements per peer.

## Where Bitcoin uses these defenses

| Where | What it uses | Chapter |
| --- | --- | --- |
| The handshake | the connection type and permissions of each peer | 68 |
| Connections | discouraged and banned addresses at accept time | 70 |
| Header sync | the anti-DoS work threshold and presync | 72 |
| Compact blocks | blocks that may be relayed before full validation | 73 |
| Tor and I2P | peers that arrive through localhost | 75 |
| Transaction relay | announcement limits and request delays | 77 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [dos.coax](../../coax/lib/bitcoin/dos.coax) | `ds_check` | each message's size limit |
| | `ds_punish` | `MaybeDiscourageAndDisconnect` |
| | `ds_punish_block` | `MaybePunishNodeForBlock` |
| | `ds_murmur3` | `MurmurHash3` |
| | `ds_exp`, `ds_rb_size`, `ds_rb_init`, `ds_rb_reset`, `ds_rb_spot`, `ds_rb_insert`, `ds_rb_contains` | `CRollingBloomFilter` |
| | `ds_banman_init`, `ds_discourage`, `ds_is_discouraged`, `ds_ban`, `ds_is_banned` | `BanMan` |
| | `ds_getheaders_due`, `ds_threshold`, `ds_headers_route`, `ds_presync_bound` | headers from a peer |
| | `ds_read`, `ds_take`, `ds_push`, `ds_sent`, `ds_note`, `ds_recv_bound`, `ds_send_bound` | flood control |
| | `ds_tx_delay` | `AddTxAnnouncement`'s limits |
| [connman.coax](../../coax/lib/bitcoin/connman.coax) | `cn_ln` | the natural logarithm (chapter 70) |
| [addrman.coax](../../coax/lib/bitcoin/addrman.coax) | `am_addr_bytes` | `GetAddrBytes` (chapter 69) |
| [pow.coax](../../coax/lib/bitcoin/pow.coax) | `pow_work_vartime` | `GetBlockProof` |

Everything here is pure. The caller says what arrived and when; the
procedures say what Core would do. Peers here are simulated.

## Message limits

A message is read before it is parsed, so its frame is checked first: a
v1 payload over 4,000,000 bytes, or a v2 message whose contents are
longer than a type and that payload, disconnects the peer at once.
After parsing, each message type has its own limit:

<!-- source: coax/lib/bitcoin/dos.coax limits -->
```jai
// What Core does with a message by its size: nothing, disconnect, or
// Misbehaving (which then discourages and disconnects). n is the payload
// or contents length for a frame, else the count of entries, hashes or
// bytes; extra is filterload's number of hash functions.
DS_OK :: 0; DS_DISCONNECT :: 1; DS_MISBEHAVING :: 2;
DS_FRAME :: 0; DS_FRAME_V2 :: 1; DS_INV :: 2; DS_GETDATA :: 3; DS_ADDR :: 4; DS_ADDRV2 :: 5; DS_HEADERS :: 6;
DS_GETHEADERS :: 7; DS_GETBLOCKS :: 8; DS_FILTERLOAD :: 9; DS_FILTERADD :: 10;
ds_check :: (kind: int, n: int, extra: int) -> int {
  if kind == DS_FRAME { return ifx n > DS_MAX_SIZE || n > DS_MAX_MESSAGE then DS_DISCONNECT else DS_OK; }
  if kind == DS_FRAME_V2 { return ifx n > DS_V2_MAX_CONTENTS then DS_DISCONNECT else DS_OK; }
  if kind == DS_INV || kind == DS_GETDATA { return ifx n > DS_MAX_INV then DS_MISBEHAVING else DS_OK; }
  if kind == DS_ADDR || kind == DS_ADDRV2 { return ifx n > DS_MAX_ADDR then DS_MISBEHAVING else DS_OK; }
  if kind == DS_HEADERS { return ifx n > DS_MAX_HEADERS then DS_MISBEHAVING else DS_OK; }
  if kind == DS_GETHEADERS || kind == DS_GETBLOCKS { return ifx n > DS_MAX_LOCATOR then DS_DISCONNECT else DS_OK; }
  if kind == DS_FILTERLOAD { return ifx n > DS_MAX_FILTER || extra > DS_MAX_HASH_FUNCS then DS_MISBEHAVING else DS_OK; }
  if kind == DS_FILTERADD { return ifx n > DS_MAX_ELEMENT then DS_MISBEHAVING else DS_OK; }
  return DS_OK;
}
```

```text
message limits
  v1 payload of 4000000 bytes: ok
  v1 payload of 4000001 bytes: disconnect (oversized message)
  v2 contents of 4000014 bytes: disconnect (oversized message)
  inv with 50000 entries: ok
  inv with 50001 entries: misbehaving (inv message size = 50001)
  getdata with 50001 entries: misbehaving (getdata message size = 50001)
  addr with 1001 addresses: misbehaving (addr message size = 1001)
  addrv2 with 1001 addresses: misbehaving (addrv2 message size = 1001)
  headers with 2000 headers: ok
  headers with 2001 headers: misbehaving (headers message size = 2001)
  getheaders with a 102-hash locator: disconnect (getheaders locator size 102 > 101)
  getblocks with a 101-hash locator: ok
  filterload of 36001 bytes: misbehaving (too-large bloom filter)
  filterload with 51 hash functions: misbehaving (too-large bloom filter)
  filteradd of 521 bytes: misbehaving (bad filteradd message)
```

The reasons in parentheses are the strings Core logs. A locator larger
than 101 hashes disconnects without discouraging: a locator that long
costs the node work to answer, but an old client might send one.

## What misbehaving costs

Older versions of Core added points to a ban score and banned a peer at
100. Core now has no score: `Misbehaving` marks the peer, and the next
pass of `MaybeDiscourageAndDisconnect` decides what that costs.

<!-- source: coax/lib/bitcoin/dos.coax punish -->
```jai
// MaybeDiscourageAndDisconnect, once Misbehaving has marked a peer.
// Noban and manual peers are kept: the operator chose them. A private
// broadcast connection, and a peer that arrived through localhost (a Tor
// or I2P proxy, where the address says nothing about who it is), are
// disconnected but their address is not discouraged. Everyone else is
// both.
DS_KEEP :: 0; DS_DROP :: 1; DS_DISCOURAGE :: 2;
ds_punish :: (noban: bool, manual: bool, private_broadcast: bool, local: bool) -> int {
  if noban || manual { return DS_KEEP; }
  if private_broadcast || local { return DS_DROP; }
  return DS_DISCOURAGE;
}
// MaybePunishNodeForBlock: is the peer that sent a block that failed
// validation Misbehaving? Data that contradicts its own proof of work
// is, except through a compact block, which may be relayed before full
// validation. A block already known invalid counts only from an outbound
// peer. A block on an invalid or unknown parent is. A block from the
// future, or on a chain with too little work, is not.
DS_BLOCK_UNSET :: 0; DS_BLOCK_CONSENSUS :: 1; DS_BLOCK_CACHED_INVALID :: 2; DS_BLOCK_INVALID_HEADER :: 3;
DS_BLOCK_MUTATED :: 4; DS_BLOCK_MISSING_PREV :: 5; DS_BLOCK_INVALID_PREV :: 6; DS_BLOCK_TIME_FUTURE :: 7;
DS_BLOCK_HEADER_LOW_WORK :: 8;
ds_punish_block :: (result: int, via_compact: bool, inbound: bool) -> bool {
  if result == DS_BLOCK_CONSENSUS || result == DS_BLOCK_MUTATED { return !via_compact; }
  if result == DS_BLOCK_CACHED_INVALID { return !via_compact && !inbound; }
  return result == DS_BLOCK_INVALID_HEADER || result == DS_BLOCK_INVALID_PREV || result == DS_BLOCK_MISSING_PREV;
}
```

```text
what misbehaving costs
  inbound or outbound peer: discouraged and disconnected
  noban: kept; manual: kept; private broadcast: disconnected; localhost: disconnected, not discouraged
  after 3 discouragements: 160.176.192.1 True, the onion address True, 160.176.255.1 False
  banned 160.176.255.1 for a day: True now, False after 24 hours
invalid blocks
  BLOCK_CONSENSUS: misbehaving; compact not punished
  BLOCK_CACHED_INVALID: outbound misbehaving; inbound not punished
  BLOCK_INVALID_HEADER: misbehaving
  BLOCK_MUTATED: misbehaving; compact not punished
  BLOCK_MISSING_PREV: misbehaving
  BLOCK_INVALID_PREV: misbehaving
  BLOCK_TIME_FUTURE: not punished
  BLOCK_HEADER_LOW_WORK: not punished
```

A peer behind a Tor or I2P proxy arrives from localhost, so discouraging
its address would discourage every peer that comes the same way. Core
disconnects it and discourages nothing.

## MurmurHash3

Core's bloom filters hash with MurmurHash3, a fast non-cryptographic
hash. The filter seeds it with a random tweak, so an attacker can't
choose entries that collide.

<!-- source: coax/lib/bitcoin/dos.coax murmur -->
```jai
// MurmurHash3 (x86, 32 bits), as Core's hash.cpp: 4-byte little-endian
// blocks mixed into h1, the tail, then the length and a final mix.
ds_murmur3 :: (seed: u32, data: [] u8) -> u32 {
  h1 := seed; c1: u32 = 0xcc9e2d51; c2: u32 = 0x1b873593;
  n := data.count / 4;
  for i: 0..<n {
    k1 := cast(u32) data[4*i] | (cast(u32) data[4*i+1] << 8) | (cast(u32) data[4*i+2] << 16) | (cast(u32) data[4*i+3] << 24);
    k1 = mul_wrap(rotl(mul_wrap(k1, c1), 15), c2);
    h1 = add_wrap(mul_wrap(rotl(h1 ^ k1, 13), 5), 0xe6546b64);
  }
  t := data.count & 3; k1: u32 = 0;
  if t >= 3 { k1 ^= cast(u32) data[4*n+2] << 16; }
  if t >= 2 { k1 ^= cast(u32) data[4*n+1] << 8; }
  if t >= 1 { k1 ^= cast(u32) data[4*n]; h1 ^= mul_wrap(rotl(mul_wrap(k1, c1), 15), c2); }
  h1 ^= cast,trunc(u32) data.count;
  h1 ^= h1 >> 16; h1 = mul_wrap(h1, 0x85ebca6b);
  h1 ^= h1 >> 13; h1 = mul_wrap(h1, 0xc2b2ae35);
  h1 ^= h1 >> 16;
  return h1;
}
```

## Rolling bloom filters

A discouraged address must be remembered, but an attacker controls how
many addresses it uses. A set would grow without limit; a
`CRollingBloomFilter` has a fixed size. It remembers at least its last
`n` entries, with a small false-positive rate, and forgets older ones.

<!-- source: coax/lib/bitcoin/dos.coax rolling -->
```jai
// CRollingBloomFilter: remembers at least the last n entries, with a
// false-positive rate near fp. Three generations of n/2 entries share
// the bits: each position has two bits naming the generation that set
// it (0 is unset). Starting a fourth generation wipes the oldest, so the
// memory stays fixed however many entries go in.
// e^x, for the sizes below: x = k ln 2 + r with |r| at most ln 2 / 2,
// then the series for e^r, doubled or halved k times.
ds_exp :: (x: f64) -> f64 {
  k := 0; r := x;
  while r > 0.34657359027997264 { r -= CN_LN2; k += 1; }
  while r < -0.34657359027997264 { r += CN_LN2; k -= 1; }
  term := 1.0; sum := 1.0;
  for i: 1..<24 { term *= r / cast(f64) i; sum += term; }
  while k > 0 { sum *= 2.0; k -= 1; }
  while k < 0 { sum *= 0.5; k += 1; }
  return sum;
}
// The constructor's sizes: hash functions log(fp)/log(0.5) rounded (1 to
// 50), n/2 rounded up a generation, and enough bits that three
// generations stay at fp; two 64-bit words per 64 positions.
RbSize :: struct { hash_funcs: int; per_generation: int; words: int; }
ds_rb_size :: (n: int, fp: f64) -> RbSize {
  log_fp := cn_ln(fp);
  k := cast(int) (log_fp / cn_ln(0.5) + 0.5);
  if k < 1 { k = 1; } if k > 50 { k = 50; }
  per := (n + 1) / 2;
  v := -1.0 * cast(f64) k * cast(f64) (per * 3) / cn_ln(1.0 - ds_exp(log_fp / cast(f64) k));
  bits := cast(int) v;
  if cast(f64) bits < v { bits += 1; }
  return RbSize.{hash_funcs=k, per_generation=per, words=((bits + 63) / 64) * 2};
}
RbFilter :: struct { size: RbSize; this_generation: int; generation: int; tweak: u32; data: [] u64; }
// A filter in the caller's words (at least size.words); Core draws the
// tweak at random.
ds_rb_init :: (f: [] RbFilter, n: int, fp: f64, tweak: u32, data: [] u64) -> bool {
  s := ds_rb_size(n, fp);
  if data.count < s.words { return false; }
  f[0] = RbFilter.{size=s, data=data[0..<s.words]};
  ds_rb_reset(f, tweak);
  return true;
}
ds_rb_reset :: (f: [] RbFilter, tweak: u32) {
  f[0].tweak = tweak; f[0].this_generation = 0; f[0].generation = 1;
  for i: 0..<f[0].data.count { f[0].data[i] = 0; }
}
// Hash n of key: MurmurHash3 seeded n * 0xFBA4C795 + tweak. Its low 6
// bits pick the bit, FastRange32 of the whole picks the word pair.
RbSpot :: struct { bit: u64; pos: int; }
ds_rb_spot :: (f: [] RbFilter, n: int, key: [] u8) -> RbSpot {
  h := ds_murmur3(add_wrap(mul_wrap(cast,trunc(u32) n, 0xfba4c795), f[0].tweak), key);
  return RbSpot.{bit=cast(u64) (h & 0x3f), pos=cast(int) ((cast(u64) h * cast(u64) f[0].data.count) >> 32)};
}
ds_rb_insert :: (f: [] RbFilter, key: [] u8) {
  if f[0].this_generation == f[0].size.per_generation {
    f[0].this_generation = 0;
    f[0].generation += 1;
    if f[0].generation == 4 { f[0].generation = 1; }
    m1: u64 = ifx (f[0].generation & 1) == 1 then 0xffffffffffffffff else 0;
    m2: u64 = ifx (f[0].generation >> 1) == 1 then 0xffffffffffffffff else 0;
    p := 0;
    while p < f[0].data.count {
      p1 := f[0].data[p]; p2 := f[0].data[p + 1];
      mask := (p1 ^ m1) | (p2 ^ m2);
      f[0].data[p] = p1 & mask; f[0].data[p + 1] = p2 & mask;
      p += 2;
    }
  }
  f[0].this_generation += 1;
  g1 := cast(u64) (f[0].generation & 1); g2 := cast(u64) (f[0].generation >> 1);
  for n: 0..<f[0].size.hash_funcs {
    s := ds_rb_spot(f, n, key);
    a := (s.pos - (s.pos & 1)); b := s.pos | 1;
    one := cast(u64) 1 << s.bit;
    f[0].data[a] = (f[0].data[a] & (one ^ 0xffffffffffffffff)) | (g1 << s.bit);
    f[0].data[b] = (f[0].data[b] & (one ^ 0xffffffffffffffff)) | (g2 << s.bit);
  }
}
ds_rb_contains :: (f: [] RbFilter, key: [] u8) -> bool {
  for n: 0..<f[0].size.hash_funcs {
    s := ds_rb_spot(f, n, key);
    if (((f[0].data[(s.pos - (s.pos & 1))] | f[0].data[s.pos | 1]) >> s.bit) & 1) == 0 { return false; }
  }
  return true;
}
```

```text
MurmurHash3 and rolling bloom filters
  MurmurHash3(0xfba4c795, "") = 6a396f08
  MurmurHash3(0xffffffff, "") = 81f16f39
  MurmurHash3(0xfba4c795, "00") = ea3f0b17
  MurmurHash3(0x00000000, "001122334455667788") = b4698def
  (50000, 1e-06) discouraged addresses: 20 hash functions, 25000 a generation, 539168 bytes
  (50000, 1e-06) known transactions, per peer: 20 hash functions, 25000 a generation, 539168 bytes
  (5000, 0.001) known addresses, per peer: 10 hash functions, 2500 a generation, 26960 bytes
  (100, 0.01) Core's test filter: 7 hash functions, 50 a generation, 368 bytes
  (100, 0.01) after 399 inserts: last 100 remembered 100, false positives in 10000 others 74
  after reset, rolling through them: every one of the last 100 remembered True
  100000 discouraged: of the last 50000, 5000 of 5000 sampled remembered; of the first 10000, 0 of 1000; 0 of 20000 others match
```

The rate the filter is built for is 1%; 74 of 10,000 is close to it.
After 100,000 discouragements the oldest are forgotten, so an attacker
who discourages many addresses of its own only pushes out older entries;
it can't make the filter larger.

## Discouraged and banned

<!-- source: coax/lib/bitcoin/dos.coax banman -->
```jai
// BanMan: discouraged addresses go into a (50000, 0.000001) rolling
// filter, keyed by GetAddrBytes, so a discouraged address is forgotten
// after enough others; banned addresses (setban, or -banscore in older
// versions) are kept with an expiry. Discouraged peers can still
// connect while inbound slots are free (chapter 70); banned ones can't.
DS_DISCOURAGE_N :: 50000; DS_DISCOURAGE_FP :: 0.000001;
DsBan :: struct { addr: AmAddr; until: int; }
DsBanMan :: struct { discouraged: [1] RbFilter; bans: [] DsBan; n_bans: int; }
ds_banman_init :: (b: [] DsBanMan, tweak: u32, words: [] u64, bans: [] DsBan) -> bool {
  b[0] = DsBanMan.{bans=bans};
  return ds_rb_init(b[0].discouraged[..], DS_DISCOURAGE_N, DS_DISCOURAGE_FP, tweak, words);
}
ds_discourage :: (b: [] DsBanMan, a: AmAddr) {
  k: [32] u8;
  ds_rb_insert(b[0].discouraged[..], am_addr_bytes(a, k[..]));
}
ds_is_discouraged :: (b: [] DsBanMan, a: AmAddr) -> bool {
  k: [32] u8;
  return ds_rb_contains(b[0].discouraged[..], am_addr_bytes(a, k[..]));
}
ds_ban :: (b: [] DsBanMan, a: AmAddr, until: int) -> bool {
  for i: 0..<b[0].n_bans { if am_same_ip(b[0].bans[i].addr, a) { b[0].bans[i].until = until; return true; } }
  if b[0].n_bans >= b[0].bans.count { return false; }
  b[0].bans[b[0].n_bans] = DsBan.{addr=a, until=until}; b[0].n_bans += 1;
  return true;
}
// IsBanned: until the ban's time has come.
ds_is_banned :: (b: [] DsBanMan, a: AmAddr, now: int) -> bool {
  for i: 0..<b[0].n_bans { if am_same_ip(b[0].bans[i].addr, a) && now < b[0].bans[i].until { return true; } }
  return false;
}
```

A discouraged address may still connect while inbound slots are free,
and is the first evicted when they are full (chapter 70). A banned
address can't connect at all until its ban expires.

## Headers

A peer can send headers that cost almost nothing to make, such as a long
chain at minimum difficulty. Core stores headers only once their chain
has enough work. Below that threshold, a full message starts a presync
that keeps only a commitment bit per period headers; the headers are
downloaded again and stored only if the chain turns out to have enough
work.

<!-- source: coax/lib/bitcoin/dos.coax headers -->
```jai
// Headers that don't connect cost the peer nothing: they may announce a
// block on a chain we haven't seen. Core answers with getheaders, at
// most once every 2 minutes per peer. last is 0 before the first.
ds_getheaders_due :: (last: int, now: int) -> bool { return now - last > DS_HEADERS_RESPONSE; }
// GetAntiDoSWorkThreshold: the minimum chain work, or the tip's work
// less 144 blocks' worth if that is more. A chain of headers below it
// isn't stored.
ds_threshold :: (tip_work: U256, tip_bits: u32, min_work: U256) -> U256 {
  back := u256_mul32(pow_work_vartime(tip_bits), 144);
  if u256_compare_vartime(back, tip_work) > 0 { back = tip_work; }
  near := u256_sub(tip_work, back);
  return ifx u256_compare_vartime(near, min_work) > 0 then near else min_work;
}
// TryLowWorkHeadersSync: a headers message whose chain, from its start,
// stays below the threshold is not stored. A full message (2,000)
// starts a presync, which only keeps commitments; a shorter one is
// ignored, since the peer has nothing more to send.
DS_ACCEPTED :: 0; DS_PRESYNC :: 1; DS_IGNORED :: 2;
ds_headers_route :: (start_work: U256, bits: u32, count: int, threshold: U256) -> int {
  total := u256_add(start_work, u256_mul32(pow_work_vartime(bits), cast,trunc(u32) count));
  if u256_compare_vartime(total, threshold) < 0 { return ifx count == DS_MAX_HEADERS then DS_PRESYNC else DS_IGNORED; }
  return DS_ACCEPTED;
}
// HeadersSyncState's memory bound: one commitment bit per period
// headers, for as many headers as could exist at 6 a second since the
// chain start's median time past (plus 2 hours), then a redownload
// buffer of compressed headers.
DsPresync :: struct { commitments: int; bytes: int; }
ds_presync_bound :: (now: int, start_mtp: int, period: int, buffer: int) -> DsPresync {
  c := 6 * (now - start_mtp + DS_MAX_FUTURE) / period;
  return DsPresync.{commitments=c, bytes=(c + 7) / 8 + buffer * DS_COMPRESSED_HEADER};
}
```

```text
headers
  unconnecting headers at 0 30 120 121 200 242 s: getheaders sent at 0 121 242
  mainnet at genesis: threshold 000000000000000000000000000000000000000145ec036acc5ba740052af1a0
  10000 blocks past the minimum: threshold is the tip less 144 blocks: True
  2000 genesis-difficulty headers from genesis: presync; 1999: ignored
  regtest, 1 header from genesis: accepted
  presync from genesis on 2026-11-03: at most 5201830 commitment bits (650229 bytes) and 15400 headers of 48 bytes: 1389429 bytes
```

A presync of mainnet from genesis costs at most about 1.4 MB, whatever
the peer sends.

## Floods

<!-- source: coax/lib/bitcoin/dos.coax flood -->
```jai
// Flood control for one peer's stream of messages of one size (with the
// 24-byte header). The network thread reads at most 64 KiB at a time,
// completes messages and queues them; when the queue passes 5 MB it
// stops reading the socket (fPauseRecv). The message thread takes one at
// a time, but none while the send buffer passes 1 MB (fPauseSend),
// since the peer isn't reading our answers. Core counts each message's
// memory with its allocation overhead; this counts bytes.
DsBuf :: struct { msg: int; partial: int; queued: int; send: int; pause_recv: bool; pause_send: bool; peak: int; }
ds_note :: (b: [] DsBuf) {
  m := b[0].partial + b[0].queued * b[0].msg + b[0].send;
  if m > b[0].peak { b[0].peak = m; }
}
ds_read :: (b: [] DsBuf, n: int) -> int {
  if b[0].pause_recv { return 0; }
  b[0].partial += n;
  done := b[0].partial / b[0].msg; b[0].partial %= b[0].msg;
  b[0].queued += done;
  b[0].pause_recv = b[0].queued * b[0].msg > DS_RECV_FLOOD;
  ds_note(b);
  return done;
}
ds_take :: (b: [] DsBuf) -> bool {
  if b[0].pause_send || b[0].queued == 0 { return false; }
  b[0].queued -= 1;
  b[0].pause_recv = b[0].queued * b[0].msg > DS_RECV_FLOOD;
  return true;
}
ds_push :: (b: [] DsBuf, n: int) {
  b[0].send += n; b[0].pause_send = b[0].send > DS_SEND_MAX;
  ds_note(b);
}
ds_sent :: (b: [] DsBuf, n: int) {
  b[0].send -= ifx n < b[0].send then n else b[0].send;
  b[0].pause_send = b[0].send > DS_SEND_MAX;
}
// The bound that follows. Reading stops once the queue passes 5 MB, so
// the last read finds at most 5 MB queued, a partial message under 4 MB
// and 64 KiB more. Processing stops once the send buffer passes 1 MB,
// so the last answer finds at most 1 MB waiting and adds one message.
ds_recv_bound :: () -> int { return DS_RECV_FLOOD + DS_MAX_MESSAGE + 24 + DS_READ_CHUNK; }
ds_send_bound :: () -> int { return DS_SEND_MAX + DS_MAX_MESSAGE + 24; }
```

## Transaction announcements

<!-- source: coax/lib/bitcoin/dos.coax tx -->
```jai
// AddTxAnnouncement's limits: a peer without the relay permission gets
// at most 5,000 announcements tracked; the rest are dropped (-1). The
// request waits 2 s for a peer we didn't choose, 2 s for a txid while
// wtxid peers exist, and 2 s when the peer has 100 requests in flight.
ds_tx_delay :: (tracked: int, in_flight: int, preferred: bool, wtxid: bool, wtxid_peers: int, relay: bool) -> int {
  if !relay && tracked >= DS_TX_ANNOUNCEMENTS { return -1; }
  d := 0;
  if !preferred { d += DS_TX_DELAY; }
  if !wtxid && wtxid_peers > 0 { d += DS_TX_DELAY; }
  if !relay && in_flight >= DS_TX_IN_FLIGHT { d += DS_TX_DELAY; }
  return d;
}
```

```text
floods
  4 MB messages at 1 MiB a tick, one processed every 8 ticks: peak 8064256 bytes, reading paused 246 of 400 ticks; receive bound 9065560
  getdata for 1 MB blocks from a peer that never reads: 1000024 bytes waiting to send, peak 6046235, reading paused 46 of 50 ticks
  per peer: receive 9065560 + send 5000024 + filters 566128 = 14631712 bytes
  10 inv messages of 50000 wtxids: 5000 announcements tracked, 495000 dropped; with the relay permission: True
  request delay: outbound 0 s, inbound 2 s, inbound txid with wtxid peers 4 s, inbound with 100 in flight 4 s
```

A peer that sends faster than the node works fills at most 5 MB of
queue before the node stops reading from it. A peer that asks for
blocks and never reads the answers fills at most about 5 MB of send
buffer before the node stops answering it. With its two filters, a peer
costs at most about 15 MB, and 125 peers about 1.8 GB.

## The example

[bitcoin/examples/dos.coax](../examples/dos.coax) prints every
transcript in this chapter:

<!-- source: bitcoin/examples/dos.coax example -->
```jai
#load "../../coax/lib/bitcoin/dos.coax";
#import "Basic";

NOW :: 1793664000;             // 2026-11-03
GENESIS_TIME :: 1231006505;
MIN_WORK_MAIN :: "000000000000000000000000000000000000000145ec036acc5ba740052af1a0";

yes :: (b: bool) -> string { return ifx b then "True" else "False"; }
hex_digit :: (x: int) -> string { return "0123456789abcdef"[x..<x + 1]; }
hex32 :: (x: u32) { s := 28; while s >= 0 { print("%", hex_digit(cast(int) ((x >> cast(u32) s) & 15))); s -= 4; } }
hex256 :: (a: U256) {
  b: [32] u8; ok := u256_to_hash_vartime(a, b[..]);
  i := 31; while i >= 0 { print("%", hex_digit(cast(int) b[i] >> 4)); print("%", hex_digit(cast(int) b[i] & 15)); i -= 1; }
}
unhex :: (s: string, out: [] u8) -> [] u8 {
  n := s.count / 2;
  for i: 0..<n { out[i] = cast,trunc(u8) (nib(s[2*i]) * 16 + nib(s[2*i+1])); }
  return out[0..<n];
}
nib :: (c: u8) -> int { return ifx c >= 'a' then cast(int) (c - 'a') + 10 else cast(int) (c - '0'); }
// 32 bytes of key: four draws, little-endian.
key32 :: (rng: [] AmRng, out: [] u8) -> [] u8 {
  for i: 0..<4 { ok := cb_store_le64(out, 8 * i, am_rand64(rng)); }
  return out[0..<32];
}
addr10 :: (first: int, i: int) -> AmAddr { return am_v4(first, i >> 16, (i >> 8) & 255, i & 255, 8333); }

limit :: (label: string, kind: int, name: string, n: int, extra: int) {
  print("  %: ", label);
  v := ds_check(kind, n, extra);
  if v == DS_OK { print("ok\n"); return; }
  if v == DS_DISCONNECT {
    if kind == DS_GETHEADERS || kind == DS_GETBLOCKS { print("disconnect (% locator size % > %)\n", name, n, DS_MAX_LOCATOR); }
    else { print("disconnect (oversized message)\n"); }
    return;
  }
  if kind == DS_FILTERLOAD { print("misbehaving (too-large bloom filter)\n"); }
  else if kind == DS_FILTERADD { print("misbehaving (bad filteradd message)\n"); }
  else { print("misbehaving (% message size = %)\n", name, n); }
}
limits :: () {
  print("message limits\n");
  limit("v1 payload of 4000000 bytes", DS_FRAME, "", 4000000, 0);
  limit("v1 payload of 4000001 bytes", DS_FRAME, "", 4000001, 0);
  limit("v2 contents of 4000014 bytes", DS_FRAME_V2, "", 4000014, 0);
  limit("inv with 50000 entries", DS_INV, "inv", 50000, 0);
  limit("inv with 50001 entries", DS_INV, "inv", 50001, 0);
  limit("getdata with 50001 entries", DS_GETDATA, "getdata", 50001, 0);
  limit("addr with 1001 addresses", DS_ADDR, "addr", 1001, 0);
  limit("addrv2 with 1001 addresses", DS_ADDRV2, "addrv2", 1001, 0);
  limit("headers with 2000 headers", DS_HEADERS, "headers", 2000, 0);
  limit("headers with 2001 headers", DS_HEADERS, "headers", 2001, 0);
  limit("getheaders with a 102-hash locator", DS_GETHEADERS, "getheaders", 102, 0);
  limit("getblocks with a 101-hash locator", DS_GETBLOCKS, "getblocks", 101, 0);
  limit("filterload of 36001 bytes", DS_FILTERLOAD, "filterload", 36001, 10);
  limit("filterload with 51 hash functions", DS_FILTERLOAD, "filterload", 100, 51);
  limit("filteradd of 521 bytes", DS_FILTERADD, "filteradd", 521, 0);
}

verdict :: (v: int, local: bool) -> string {
  if v == DS_KEEP { return "kept"; }
  if v == DS_DISCOURAGE { return "discouraged and disconnected"; }
  return ifx local then "disconnected, not discouraged" else "disconnected";
}
costs :: () {
  print("what misbehaving costs\n");
  print("  inbound or outbound peer: %\n", verdict(ds_punish(false, false, false, false), false));
  print("  noban: %; manual: %; private broadcast: %; localhost: %\n",
    verdict(ds_punish(true, false, false, false), false), verdict(ds_punish(false, true, false, false), false),
    verdict(ds_punish(false, false, true, false), false), verdict(ds_punish(false, false, false, true), true));
  words: [67396] u64; bans: [4] DsBan; b: [1] DsBanMan;
  ok := ds_banman_init(b[..], 0x2b7e1516, words[..], bans[..]);
  a0 := am_v4(160, 176, 192, 1, 8333); a1 := am_v4(160, 176, 192, 2, 8333); other := am_v4(160, 176, 255, 1, 8333);
  raw: [32] u8; tor := am_bytes(AM_NET_ONION, unhex("79bcc625184b05194975c28b66b66b0469f7f6556fb1ac3189a79b40dda32f1f", raw[..]), 8333);
  ds_discourage(b[..], a0); ds_discourage(b[..], a1); ds_discourage(b[..], tor);
  print("  after 3 discouragements: 160.176.192.1 %, the onion address %, 160.176.255.1 %\n",
    yes(ds_is_discouraged(b[..], a0)), yes(ds_is_discouraged(b[..], tor)), yes(ds_is_discouraged(b[..], other)));
  ok = ds_ban(b[..], other, NOW + 86400);
  print("  banned 160.176.255.1 for a day: % now, % after 24 hours\n",
    yes(ds_is_banned(b[..], other, NOW)), yes(ds_is_banned(b[..], other, NOW + 86400)));
}

punished :: (r: int, compact: bool, inbound: bool) -> string { return ifx ds_punish_block(r, compact, inbound) then "misbehaving" else "not punished"; }
blocks :: () {
  print("invalid blocks\n");
  names := [8] string.["BLOCK_CONSENSUS", "BLOCK_CACHED_INVALID", "BLOCK_INVALID_HEADER", "BLOCK_MUTATED",
                       "BLOCK_MISSING_PREV", "BLOCK_INVALID_PREV", "BLOCK_TIME_FUTURE", "BLOCK_HEADER_LOW_WORK"];
  for i: 0..<8 {
    r := i + 1;
    print("  %: ", names[i]);
    if r == DS_BLOCK_CONSENSUS || r == DS_BLOCK_MUTATED { print("%; compact %\n", punished(r, false, true), punished(r, true, true)); }
    else if r == DS_BLOCK_CACHED_INVALID { print("outbound %; inbound %\n", punished(r, false, false), punished(r, false, true)); }
    else { print("%\n", punished(r, false, true)); }
  }
}

murmur :: (seed: u32, data: string) {
  b: [16] u8;
  print("  MurmurHash3(0x"); hex32(seed); print(", \"%\") = ", data); hex32(ds_murmur3(seed, unhex(data, b[..]))); print("\n");
}
size :: (label: string, n: int, fp: f64, fp_text: string) {
  s := ds_rb_size(n, fp);
  print("  (%, %) %: % hash functions, % a generation, % bytes\n", n, fp_text, label, s.hash_funcs, s.per_generation, s.words * 8);
}
filters :: () {
  print("MurmurHash3 and rolling bloom filters\n");
  murmur(0xfba4c795, ""); murmur(0xffffffff, ""); murmur(0xfba4c795, "00"); murmur(0, "001122334455667788");
  size("discouraged addresses", 50000, 0.000001, "1e-06");
  size("known transactions, per peer", 50000, 0.000001, "1e-06");
  size("known addresses, per peer", 5000, 0.001, "0.001");
  size("Core's test filter", 100, 0.01, "0.01");
  seed: [32] u8; for i: 0..<32 { seed[i] = 7; }
  rng := [1] AmRng.[am_rng(seed[..])];
  data: [399] [32] u8;
  for i: 0..<399 { k := key32(rng[..], data[i][..]); }
  words: [46] u64; f: [1] RbFilter;
  ok := ds_rb_init(f[..], 100, 0.01, 0x9e3779b9, words[..]);
  for i: 0..<399 { ds_rb_insert(f[..], data[i][..]); }
  last := 0; for i: 299..<399 { if ds_rb_contains(f[..], data[i][..]) { last += 1; } }
  fp := 0; k: [32] u8;
  for i: 0..<10000 { if ds_rb_contains(f[..], key32(rng[..], k[..])) { fp += 1; } }
  print("  (100, 0.01) after 399 inserts: last 100 remembered %, false positives in 10000 others %\n", last, fp);
  ds_rb_reset(f[..], 0x9e3779b9);
  rolling := true;
  for i: 0..<399 {
    if i >= 100 && !ds_rb_contains(f[..], data[i - 100][..]) { rolling = false; }
    ds_rb_insert(f[..], data[i][..]);
    rolling = rolling && ds_rb_contains(f[..], data[i][..]);
  }
  print("  after reset, rolling through them: every one of the last 100 remembered %\n", yes(rolling));
  big: [67396] u64; bans: [1] DsBan; b: [1] DsBanMan;
  ok = ds_banman_init(b[..], 0x243f6a88, big[..], bans[..]);
  for i: 0..<100000 { ds_discourage(b[..], addr10(10, i)); }
  kept := 0; i := 50000; while i < 100000 { if ds_is_discouraged(b[..], addr10(10, i)) { kept += 1; } i += 10; }
  old := 0; i = 0; while i < 10000 { if ds_is_discouraged(b[..], addr10(10, i)) { old += 1; } i += 10; }
  fresh := 0; for j: 0..<20000 { if ds_is_discouraged(b[..], addr10(11, j)) { fresh += 1; } }
  print("  100000 discouraged: of the last 50000, % of 5000 sampled remembered; of the first 10000, % of 1000; % of 20000 others match\n", kept, old, fresh);
}

route_name :: (r: int) -> string {
  if r == DS_PRESYNC { return "presync"; }
  return ifx r == DS_IGNORED then "ignored" else "accepted";
}
headers :: () {
  print("headers\n");
  times := [6] int.[0, 30, 120, 121, 200, 242];
  print("  unconnecting headers at 0 30 120 121 200 242 s: getheaders sent at");
  last := 0;
  for i: 0..<6 { if ds_getheaders_due(last, NOW + times[i]) { last = NOW + times[i]; print(" %", times[i]); } }
  print("\n");
  min_work := pw_number(MIN_WORK_MAIN);
  g := pow_work_vartime(0x1d00ffff);
  t := ds_threshold(g, 0x1d00ffff, min_work);
  print("  mainnet at genesis: threshold "); hex256(t); print("\n");
  tip := u256_add(min_work, u256_mul32(pow_work_vartime(0x17023a04), 10000));
  back := u256_sub(tip, u256_mul32(pow_work_vartime(0x17023a04), 144));
  print("  10000 blocks past the minimum: threshold is the tip less 144 blocks: %\n",
    yes(u256_compare_vartime(ds_threshold(tip, 0x17023a04, min_work), back) == 0));
  print("  2000 genesis-difficulty headers from genesis: %; 1999: %\n",
    route_name(ds_headers_route(g, 0x1d00ffff, 2000, t)), route_name(ds_headers_route(g, 0x1d00ffff, 1999, t)));
  r := pow_work_vartime(0x207fffff);
  print("  regtest, 1 header from genesis: %\n", route_name(ds_headers_route(r, 0x207fffff, 1, ds_threshold(r, 0x207fffff, U256.{}))));
  p := ds_presync_bound(NOW, GENESIS_TIME, 649, 15400);
  print("  presync from genesis on 2026-11-03: at most % commitment bits (% bytes) and 15400 headers of % bytes: % bytes\n",
    p.commitments, (p.commitments + 7) / 8, DS_COMPRESSED_HEADER, p.bytes);
}

floods :: () {
  print("floods\n");
  b := [1] DsBuf.[DsBuf.{msg=DS_MAX_MESSAGE}];
  paused := 0;
  for tick: 0..<400 {
    for r: 0..<16 { done := ds_read(b[..], DS_READ_CHUNK); }
    if b[0].pause_recv { paused += 1; }
    if tick % 8 == 7 { took := ds_take(b[..]); }
  }
  print("  4 MB messages at 1 MiB a tick, one processed every 8 ticks: peak % bytes, reading paused % of 400 ticks; receive bound %\n",
    b[0].peak, paused, ds_recv_bound());
  g := [1] DsBuf.[DsBuf.{msg=24 + 37}];
  paused = 0;
  for tick: 0..<50 {
    for r: 0..<16 { done := ds_read(g[..], DS_READ_CHUNK); }
    if g[0].pause_recv { paused += 1; }
    for i: 0..<100 { if ds_take(g[..]) { ds_push(g[..], 1000024); } }
  }
  print("  getdata for 1 MB blocks from a peer that never reads: % bytes waiting to send, peak %, reading paused % of 50 ticks\n",
    g[0].send, g[0].peak, paused);
  filter_bytes := (ds_rb_size(50000, 0.000001).words + ds_rb_size(5000, 0.001).words) * 8;
  print("  per peer: receive % + send % + filters % = % bytes\n", ds_recv_bound(), ds_send_bound(), filter_bytes,
    ds_recv_bound() + ds_send_bound() + filter_bytes);
  tracked := 0; dropped := 0;
  for i: 0..<500000 { if ds_tx_delay(tracked, 0, false, true, 0, false) < 0 { dropped += 1; } else { tracked += 1; } }
  print("  10 inv messages of 50000 wtxids: % announcements tracked, % dropped; with the relay permission: %\n",
    tracked, dropped, yes(ds_tx_delay(5000, 0, false, true, 0, true) >= 0));
  print("  request delay: outbound % s, inbound % s, inbound txid with wtxid peers % s, inbound with 100 in flight % s\n",
    ds_tx_delay(0, 0, true, true, 1, false), ds_tx_delay(0, 0, false, true, 1, false),
    ds_tx_delay(0, 0, false, false, 1, false), ds_tx_delay(0, 100, false, true, 1, false));
}

main :: () { limits(); costs(); blocks(); filters(); headers(); floods(); }
```

```shell
coax/bin/coax run bitcoin/examples/dos.coax
```

Its output, 55 lines, is
[bitcoin/fixtures/dos.out](../fixtures/dos.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/dos.coax](../examples/checks/dos.coax), which
loads the example. A rolling filter remembers its last 100 entries
whatever its tweak, flood control holds the receive bound for any
message size and processing pace, and each limit accepts its maximum and
refuses one more:

<!-- source: bitcoin/examples/checks/dos.coax check -->
```jai
// Whatever the tweak, a rolling filter remembers the last n entries.
#forall "a rolling filter never forgets its last 100" k: 0..<8 #steps 4000000000 {
  rng := [1] AmRng.[seeded_rng(cast,trunc(u8) (k + 1))];
  words: [46] u64; f: [1] RbFilter; data: [300] [32] u8;
  #assert ds_rb_init(f[..], 100, 0.01, cast,trunc(u32) am_rand64(rng[..]), words[..]);
  for i: 0..<300 {
    d := key32(rng[..], data[i][..]);
    ds_rb_insert(f[..], data[i][..]);
    j := ifx i >= 99 then i - 99 else 0;
    while j <= i { #assert ds_rb_contains(f[..], data[j][..]); j += 1; }
  }
}
// Whatever the message size and the pace of processing, a peer's memory
// stays under the receive bound.
#forall "flood control holds the bound" k: 0..<12 #steps 4000000000 {
  b := [1] DsBuf.[DsBuf.{msg=24 + 1 + k * 333331}];
  for tick: 0..<120 {
    for r: 0..<16 { done := ds_read(b[..], DS_READ_CHUNK); }
    if tick % (k + 1) == 0 { took := ds_take(b[..]); }
  }
  #assert b[0].peak <= ds_recv_bound();
}
// No limit lets an oversized message through, and none refuses one at
// the limit.
#forall "each limit is exact" kind: 0..<11 {
  at := 0;
  if kind == DS_FRAME { at = DS_MAX_MESSAGE; } else if kind == DS_FRAME_V2 { at = DS_V2_MAX_CONTENTS; }
  else if kind == DS_INV || kind == DS_GETDATA { at = DS_MAX_INV; } else if kind == DS_ADDR || kind == DS_ADDRV2 { at = DS_MAX_ADDR; }
  else if kind == DS_HEADERS { at = DS_MAX_HEADERS; } else if kind == DS_GETHEADERS || kind == DS_GETBLOCKS { at = DS_MAX_LOCATOR; }
  else if kind == DS_FILTERLOAD { at = DS_MAX_FILTER; } else { at = DS_MAX_ELEMENT; }
  #assert ds_check(kind, at, 0) == DS_OK && ds_check(kind, at + 1, 0) != DS_OK;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/dos.coax
```

`coax test` runs 322 checks, including the loaded files' own checks, in
about 2 minutes on 2026-11-03.

### The library's own checks

`dos.coax` is an `#authority` file. Its checks port Bitcoin Core's
`src/test/hash_tests.cpp` (all 14 `murmurhash3` vectors),
`src/test/bloom_tests.cpp` (`rolling_bloom`) and
`src/test/denialofservice_tests.cpp` (`peer_discouragement` and
`DoS_bantime`). Core's `rolling_bloom` draws its tweak and entries from
its test generator, so its exact false-positive counts differ here; the
guarantees it checks, that the last 100 entries are always remembered
and that a reset forgets them, hold. The checks also cover each message
limit, `MaybePunishNodeForBlock`, the getheaders interval, the work
threshold, presync's bound, flood control and the announcement limits.

```shell
coax/bin/coax test coax/lib/bitcoin/dos.coax
```

`coax test` runs 319 checks, including those of the files it loads, in
about 2 minutes on 2026-11-03. Mutation testing: not run (owner speed
order).

### Bitcoin Core's code

[dos.py](../../coax/tests/bitcoin/dos.py) computes every line the
example prints. It transliterates `MurmurHash3` from the pinned
`hash.cpp`, `CRollingBloomFilter` from `common/bloom.cpp` with
`FastRange32` from `util/fastrange.h`, `BanMan` from `banman.cpp`, the
message limits, `MaybeDiscourageAndDisconnect`,
`MaybePunishNodeForBlock`, `GetAntiDoSWorkThreshold` and
`TryLowWorkHeadersSync` from `net_processing.cpp`, the presync bound
from `headerssync.cpp`, flood control from `net.cpp`, and the
announcement limits from `node/txdownloadman_impl.cpp`. It reads the
constants from the pinned sources, checks Core's 14 MurmurHash3
vectors, and compares its transcript with the fixture:

```shell
python3 -I coax/tests/bitcoin/dos.py --coax coax/bin/coax
```

On 2026-11-03 all 55 lines agree, and the example's output equals them.

## What this chapter leaves out

- Sockets and the network thread: chapter 89.
- `setban`, `listbanned` and `banlist.json`.
- The full header sync state machine: chapter 72.
- Compact block reconstruction: chapter 73.
- Transaction requests and orphans: chapter 77.
- Core's count of each message's allocation overhead; flood control
  here counts bytes.

## Summary

- Every message has a size limit. Oversized frames and locators
  disconnect; other oversized messages are misbehavior.
- One offense discourages and disconnects a peer, unless it has noban,
  was added by hand, or came through localhost.
- Discouraged addresses live in a fixed-size rolling bloom filter, so an
  attacker can't grow it.
- Headers with too little work aren't stored, and a presync from
  genesis costs at most about 1.4 MB.
- Flood control bounds a peer to about 15 MB, and a peer's tracked
  transaction announcements to 5,000.

## Dependencies

- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md)
- [70. Connections: outbound, block-relay-only, feelers and eviction](ch070-connections-outbound-block-relay-only-feelers-and-eviction.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `banman.cpp`, `banman.h`,
  `common/bloom.cpp`, `common/bloom.h`, `hash.cpp`, `headerssync.cpp`,
  `headerssync.h`, `net_processing.h`, `node/txdownloadman.h`,
  `node/txdownloadman_impl.cpp`, `net_permissions.h`,
  `util/fastrange.h`, `test/denialofservice_tests.cpp` and
  `test/bloom_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/banman.cpp)
  by this chapter; `net.cpp`, `net.h`, `net_processing.cpp`,
  `chainparams.cpp`, `chain.h`, `serialize.h`, `script.h` and
  `test/hash_tests.cpp`, pinned there by earlier chapters.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
