# 69. Addresses and the address manager

A node needs peers, and to find peers it needs addresses. It learns them
from DNS seeds when it first starts, and after that from its peers, which
send `addr` and `addrv2` messages. It keeps what it learns in an address
manager, saves the manager to `peers.dat` when it stops, and picks
addresses from it whenever it opens a connection.

Addresses are also an attack surface. Someone who fills a node's address
manager with addresses they control can choose all of the node's peers.
That is an eclipse attack: the victim sees only the attacker's chain and
the attacker's transactions. Bitcoin Core's address manager is built to
make this expensive. It sorts addresses into buckets by keyed hashes of
network groups, so that one source, or one block of addresses, can only
fill a small, unpredictable part of the tables.

This chapter builds that address manager as Core's `AddrMan`, with the
same buckets, positions, eviction rules and random choices, and reads and
writes `peers.dat` in Core's own format. It then follows addresses
through the network: how a node accepts an `addr` message, which
addresses it relays and to whom, how it answers `getaddr`, and when it
asks DNS seeds.

## What this chapter teaches

- How a network group is formed: a /16 for IPv4, a /32 for IPv6, four
  bits for Tor and I2P, and the IPv4 address inside 6to4 and Teredo.
- How the new table's 1,024 buckets and the tried table's 256 buckets
  are chosen by keyed SHA-256d, and why one source group reaches at most
  64 new buckets.
- What happens when an address is added, seen again, connected to, or
  fails, and when an address moves from new to tried.
- How test-before-evict protects a working tried address from a newcomer
  that hashes to its slot.
- How `Select` picks an address: tried or new with even odds, then a
  random bucket and slot, weighted by recent failures.
- `peers.dat`: Core's format 4, its checksum, and what a reader does with
  a damaged file.
- The rules for incoming `addr` messages: at most 1,000 addresses, a
  token bucket of 0.1 addresses a second, and relay of small, fresh
  messages to two peers chosen for a day.
- The `getaddr` answer, its 23% limit and its cache, and the timing of
  DNS seeds.

## Where Bitcoin uses addresses

| Where | What it uses | Chapter |
| --- | --- | --- |
| The handshake | `sendaddrv2`, which picks `addr` or `addrv2` for the connection | 68 |
| Outbound connections | `Select`, `Attempt`, `Good`, and feelers that test collisions | 70 |
| Denial of service | the `addr` limits and the misbehavior they cause | 71 |
| Tor and I2P | addresses that only `addrv2` carries, and reachable networks | 75 |
| The node process | `peers.dat` in the data directory, and DNS seed lookups | 89 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [addrman.coax](../../coax/lib/bitcoin/addrman.coax) | `am_group`, `am_routable`, `am_network` | `CNetAddr` classification and `NetGroupManager::GetGroup` |
| | `am_tried_bucket`, `am_new_bucket`, `am_position` | bucket and slot of an address |
| | `am_rng`, `am_randrange`, `am_randbits` | `FastRandomContext` (ChaCha20) |
| | `am_init`, `am_add`, `am_good`, `am_attempt`, `am_connected`, `am_set_services` | `AddrMan`'s updates |
| | `am_select`, `am_get_addr` | `Select` and `GetAddr` |
| | `am_resolve_collisions`, `am_select_tried_collision` | test-before-evict |
| | `am_find_entry`, `am_size`, `am_check` | where an entry is, counts, and `CheckAddrman` |
| | `am_write_vartime`, `am_read_vartime` | `peers.dat` |
| | `am_wire_write_vartime`, `am_from_wire` | `addr` and `addrv2` entries |
| | `am_process_addrs`, `am_relay_targets` | `ProcessAddrs` and `RelayAddress` |
| | `am_getaddr_reply`, `am_seed_plan` | the `getaddr` answer and DNS seed timing |
| [p2p.coax](../../coax/lib/bitcoin/p2p.coax) | `p2p_addr_read_vartime` | the `addr` and `addrv2` messages (chapter 65) |
| [handshake.coax](../../coax/lib/bitcoin/handshake.coax) | `hs_has_desirable` | the service flags a node wants (chapter 68) |
| [siphash.coax](../../coax/lib/crypto/siphash.coax) | `siphash24` | SipHash-2-4 (chapter 10) |

Everything here is pure. The caller passes the time, the memory for the
tables, and the bytes of files and messages. Nothing opens a socket,
reads a file or asks DNS. Addresses are public, so the procedures run in
variable time; the bucket key is secret, but it only goes into SHA-256,
which runs in constant time.

The limits come from Core's sources:

<!-- source: coax/lib/bitcoin/addrman.coax constants -->
```jai
AM_TRIED_BUCKETS :: 256;          // ADDRMAN_TRIED_BUCKET_COUNT (1 << 8)
AM_NEW_BUCKETS :: 1024;           // ADDRMAN_NEW_BUCKET_COUNT (1 << 10)
AM_BUCKET_SIZE :: 64;             // ADDRMAN_BUCKET_SIZE (1 << 6)
AM_TRIED_PER_GROUP :: 8;          // ADDRMAN_TRIED_BUCKETS_PER_GROUP
AM_NEW_PER_SOURCE_GROUP :: 64;    // ADDRMAN_NEW_BUCKETS_PER_SOURCE_GROUP
AM_NEW_PER_ADDRESS :: 8;          // ADDRMAN_NEW_BUCKETS_PER_ADDRESS
AM_HORIZON :: 30 * 86400;         // ADDRMAN_HORIZON: older addresses are terrible
AM_RETRIES :: 3;                  // ADDRMAN_RETRIES: attempts before a never-good address is terrible
AM_MAX_FAILURES :: 10;            // ADDRMAN_MAX_FAILURES ...
AM_MIN_FAIL :: 7 * 86400;         // ... in ADDRMAN_MIN_FAIL without a success
AM_REPLACEMENT :: 4 * 3600;       // ADDRMAN_REPLACEMENT: a tried entry this fresh is kept
AM_COLLISIONS :: 10;              // ADDRMAN_SET_TRIED_COLLISION_SIZE
AM_TEST_WINDOW :: 40 * 60;        // ADDRMAN_TEST_WINDOW: then evict untested
AM_TIME_INIT :: 100000000;        // CAddress::TIME_INIT: an address with no time
AM_MAX_ADDR :: 1000;              // MAX_ADDR_TO_SEND
AM_MAX_PCT :: 23;                 // MAX_PCT_ADDR_TO_SEND
AM_ADDR_RATE :: 0.1;              // MAX_ADDR_RATE_PER_SECOND
AM_RELAY_LIMIT :: 10;             // only messages of at most 10 addresses are relayed
AM_ROTATE :: 86400;               // ROTATE_ADDR_RELAY_DEST_INTERVAL
AM_RANDOMIZER_RELAY :: cast(u64) 0x3cac0035b5866b90; // RANDOMIZER_ID_ADDRESS_RELAY
AM_CACHE_LIFE :: 21 * 3600;       // the getaddr cache lives 21 hours ...
AM_CACHE_JITTER :: 6 * 3600;      // ... plus up to 6 more, drawn in microseconds
AM_SEEDS_AT_ONCE :: 3;            // DNSSEEDS_TO_QUERY_AT_ONCE
AM_SEEDS_FEW :: 11;               // DNSSEEDS_DELAY_FEW_PEERS, seconds
AM_SEEDS_MANY :: 300;             // DNSSEEDS_DELAY_MANY_PEERS
AM_SEEDS_MANY_PEERS :: 1000;      // DNSSEEDS_DELAY_PEER_THRESHOLD
AM_SEED_OUTBOUND :: 2;            // SEED_OUTBOUND_CONNECTION_THRESHOLD
```

## Addresses and networks

Core keeps a `CNetAddr` as a network and its bytes: 4 for IPv4, 16 for
IPv6 and CJDNS, 32 for Tor v3 and I2P. A `CService` adds a port. Here
both are one struct, with port 0 for a source address.

<!-- source: coax/lib/bitcoin/addrman.coax networks -->
```jai
// Core's enum Network, which GetGroup writes as a group's first byte. BIP-155
// numbers the same networks differently on the wire.
AM_NET_UNROUTABLE :: 0; AM_NET_IPV4 :: 1; AM_NET_IPV6 :: 2; AM_NET_ONION :: 3;
AM_NET_I2P :: 4; AM_NET_CJDNS :: 5; AM_NET_INTERNAL :: 6;
// Prefixes, padded to 16 bytes; am_has compares the first n.
AM_V4_PREFIX :: [16] u8.[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 0, 0];
AM_TORV2_PREFIX :: [16] u8.[0xfd, 0x87, 0xd8, 0x7e, 0xeb, 0x43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
AM_INTERNAL_PREFIX :: [16] u8.[0xfd, 0x6b, 0x88, 0xc0, 0x87, 0x24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
// A CService: a network, its address bytes and a port. A source address
// (a CNetAddr) is the same with port 0.
AmAddr :: struct { net: int; n: int; ip: [32] u8; port: int; }
```

The wire has two forms. `addr` (version 1) carries 16 bytes, so IPv4 is
mapped into IPv6 as `::ffff:a.b.c.d`. `addrv2` (BIP-155) carries a
network ID and the address's own length, which is the only way to send
a Tor v3, I2P or CJDNS address. Reading the 16-byte form undoes the
mapping; a Tor v2 address, no longer valid, becomes `::`:

<!-- source: coax/lib/bitcoin/addrman.coax legacy -->
```jai
// SetLegacyIPv6: the 16-byte form of addr v1 and of old peers.dat files.
// IPv4 travels mapped into IPv6; a Tor v2 address is no longer valid and
// reads as "::", which is invalid; an internal name keeps its 10 bytes.
am_legacy :: (b16: [] u8, port: int) -> AmAddr {
  x := am_bytes(AM_NET_IPV6, b16[0..<16], port);
  if am_has(x.ip, AM_V4_PREFIX, 12) { return am_bytes(AM_NET_IPV4, b16[12..<16], port); }
  if am_has(x.ip, AM_TORV2_PREFIX, 6) { x = am_none(); x.port = port; return x; }
  if am_has(x.ip, AM_INTERNAL_PREFIX, 6) { return am_bytes(AM_NET_INTERNAL, b16[6..<16], port); }
  return x;
}
// UnserializeV2Stream: a BIP-155 network ID and its bytes. An unknown ID,
// or IPv4 or Tor v2 inside an IPv6 entry, reads as "::".
am_bip155 :: (id: int, raw: [] u8, port: int) -> AmAddr {
  none := am_none(); none.port = port;
  if id == P2P_NET_IPV4 && raw.count == 4 { return am_bytes(AM_NET_IPV4, raw, port); }
  if id == P2P_NET_TORV3 && raw.count == 32 { return am_bytes(AM_NET_ONION, raw, port); }
  if id == P2P_NET_I2P && raw.count == 32 { return am_bytes(AM_NET_I2P, raw, port); }
  if id == P2P_NET_CJDNS && raw.count == 16 { return am_bytes(AM_NET_CJDNS, raw, port); }
  if id != P2P_NET_IPV6 || raw.count != 16 { return none; }
  x := am_bytes(AM_NET_IPV6, raw, port);
  if am_has(x.ip, AM_INTERNAL_PREFIX, 6) { return am_bytes(AM_NET_INTERNAL, raw[6..<16], port); }
  if am_has(x.ip, AM_V4_PREFIX, 12) || am_has(x.ip, AM_TORV2_PREFIX, 6) { return none; }
  return x;
}
```

An address is routable unless it is local, private (RFC 1918 and
friends), reserved for documentation, or otherwise not reachable from
the internet. The address manager ignores unroutable addresses.

## Network groups

An attacker can't easily get addresses in many different /16 blocks of
IPv4, or many /32 blocks of IPv6. Core's `GetGroup` turns an address into
those leading bits, with its network first. Addresses that tunnel IPv4
(6to4, Teredo, NAT64) are grouped by the IPv4 address inside. Tor and I2P
addresses are hashes of keys, which anyone can make, so their group is
only 4 bits: they all fall into 16 groups.

<!-- source: coax/lib/bitcoin/addrman.coax group -->
```jai
// GetAddrBytes: the 16-byte v1 form where one exists, else the raw bytes.
am_addr_bytes :: (a: AmAddr, out: [] u8) -> [] u8 {
  if a.net == AM_NET_IPV4 { for i: 0..<12 { out[i] = AM_V4_PREFIX[i]; } for i: 0..<4 { out[12+i] = a.ip[i]; } return out[0..<16]; }
  if a.net == AM_NET_INTERNAL { for i: 0..<6 { out[i] = AM_INTERNAL_PREFIX[i]; } for i: 0..<10 { out[6+i] = a.ip[i]; } return out[0..<16]; }
  for i: 0..<a.n { out[i] = a.ip[i]; }
  return out[0..<a.n];
}
// CService::GetKey: the address bytes and the port, big-endian.
am_key :: (a: AmAddr, out: [] u8) -> [] u8 {
  b := am_addr_bytes(a, out);
  out[b.count] = cast,trunc(u8) (a.port >> 8); out[b.count+1] = cast,trunc(u8) a.port;
  return out[0..<b.count+2];
}
// NetGroupManager::GetGroup with no asmap: the network class, then the
// leading bits that one operator can't easily vary. IPv4 keeps its /16,
// IPv6 its /32 (he.net's /36), Tor and I2P 4 bits, CJDNS 12; every local
// or unroutable address shares one group.
am_group :: (a: AmAddr, out: [] u8) -> [] u8 {
  out[0] = cast(u8) am_net_class(a);
  start := 0; bits := 0;
  if am_local(a) {
  } else if a.net == AM_NET_INTERNAL { start = 6; bits = 80;
  } else if !am_routable(a) {
  } else if am_linked_ipv4(a) >= 0 {
    v := am_linked_ipv4(a);
    out[1] = cast,trunc(u8) (v >> 24); out[2] = cast,trunc(u8) (v >> 16);
    return out[0..<3];
  } else if a.net == AM_NET_ONION || a.net == AM_NET_I2P { bits = 4;
  } else if a.net == AM_NET_CJDNS { bits = 12;
  } else if am_henet(a) { bits = 36;
  } else { bits = 32; }
  raw: [32] u8; b := am_addr_bytes(a, raw[..]);
  n := bits / 8;
  for i: 0..<n { out[1+i] = b[start+i]; }
  if bits % 8 == 0 { return out[0..<1+n]; }
  out[1+n] = b[start+n] | cast,trunc(u8) ((1 << (8 - bits % 8)) - 1);
  return out[0..<2+n];
}
```

Core can also group by autonomous system, with an asmap file that maps
address ranges to the networks that own them. Without one, it uses the
prefixes above; this chapter does the same.

The example prints these groups:

```text
groups
  250.1.1.1: 01fa01
  10.0.0.1: 00 (unroutable)
  127.0.0.1: 00 (unroutable)
  2001:470:1234::1 (he.net): 02200104701f
  2a01:e::1: 022a01000e
  2002:fa01:0101::1 (6to4): 01fa01
  2001:0::0506:fefe (Teredo): 01faf9
  ::ffff:250.1.1.1 (v1 form): 01fa01
  fc00::1 (CJDNS): 05fc0f
  I2P: 04af
  Tor v3: 03ff
```

The 6to4 address has the same group as `250.1.1.1`. Teredo stores its
IPv4 address with every bit flipped, so `05 06 fe fe` is `250.249.1.1`.
Hurricane Electric (`2001:470::/32`) hands out /48s freely, so its group
is 36 bits. A group's last partial byte is padded with ones.

## Two tables of buckets

The address manager has two tables:

- The new table, 1,024 buckets of 64 slots, holds addresses the node has
  heard of but not connected to. An address can be in up to 8 new
  buckets.
- The tried table, 256 buckets of 64 slots, holds addresses the node has
  connected to and completed a handshake with. An address is in one
  tried bucket.

The bucket comes from SHA-256d keyed with a 256-bit secret, `nKey`, that
each node draws once and keeps in `peers.dat`. Core's `GetCheapHash`
takes the first 8 bytes of the digest:

<!-- source: coax/lib/bitcoin/addrman.coax buckets -->
```jai
// GetCheapHash: the first 8 bytes of SHA-256d, little-endian. Core's
// HashWriter writes a vector with its CompactSize length first.
am_cheap :: (data: [] u8) -> u64 {
  d: [32] u8; r := sha256d_checked(data, d[..], false);
  return cb_load_le64(d[..], 0).value;
}
am_put_vec :: (out: [] u8, at: int, b: [] u8) -> int {
  at2 := tx_compact_write_vartime(out, at, b.count);
  for i: 0..<b.count { out[at2+i] = b[i]; }
  return at2 + b.count;
}
am_put_key :: (out: [] u8, key: [] u8) -> int { for i: 0..<32 { out[i] = key[i]; } return 32; }
// The tried bucket: the address picks one of 8 slots for its group, and
// the group and slot pick the bucket. One /16 reaches at most 8 buckets.
am_tried_bucket :: (key: [] u8, a: AmAddr) -> int {
  buf: [128] u8; k: [40] u8; g: [24] u8;
  at := am_put_vec(buf[..], am_put_key(buf[..], key), am_key(a, k[..]));
  h1 := am_cheap(buf[0..<at]);
  at = am_put_vec(buf[..], am_put_key(buf[..], key), am_group(a, g[..]));
  stored := cb_store_le64(buf[..], at, h1 % AM_TRIED_PER_GROUP);
  return cast(int) (am_cheap(buf[0..<at+8]) % AM_TRIED_BUCKETS);
}
// The new bucket: the address's group and the source's group pick one of
// 64 slots for the source group, and the source group and slot pick the
// bucket. One source group reaches at most 64 of the 1,024 buckets.
am_new_bucket :: (key: [] u8, a: AmAddr, src: AmAddr) -> int {
  buf: [128] u8; g: [24] u8; s: [24] u8;
  sg := am_group(src, s[..]);
  at := am_put_vec(buf[..], am_put_vec(buf[..], am_put_key(buf[..], key), am_group(a, g[..])), sg);
  h1 := am_cheap(buf[0..<at]);
  at = am_put_vec(buf[..], am_put_key(buf[..], key), sg);
  stored := cb_store_le64(buf[..], at, h1 % AM_NEW_PER_SOURCE_GROUP);
  return cast(int) (am_cheap(buf[0..<at+8]) % AM_NEW_BUCKETS);
}
// GetBucketPosition: the slot inside a bucket, from 'N' (new) or 'K'
// (tried), the bucket and the address with its port.
am_position :: (key: [] u8, fnew: bool, bucket: int, a: AmAddr) -> int {
  buf: [96] u8; k: [40] u8;
  at := am_put_key(buf[..], key);
  buf[at] = ifx fnew then cast(u8) 'N' else cast(u8) 'K';
  stored := cb_store_le32(buf[..], at + 1, cast,trunc(u32) bucket);
  at = am_put_vec(buf[..], at + 5, am_key(a, k[..]));
  return cast(int) (am_cheap(buf[0..<at]) % AM_BUCKET_SIZE);
}
```

A tried bucket comes in two steps. The address with its port picks one
of 8 numbers, and the address's group with that number picks the bucket.
So all the addresses of one /16 share at most 8 tried buckets.

A new bucket depends on the source: the peer that sent the address. The
address's group and the source's group pick one of 64 numbers, and the
source's group with that number picks the bucket. So one source group
can put addresses in at most 64 of the 1,024 new buckets, however many
addresses it sends. The slot in a bucket hashes the address with its
port, the bucket, and a letter: `N` for new, `K` for tried.

Core's unit tests key these hashes with SHA-256d of the 32-bit number 1,
and expect bucket 40 and bucket 786:

```text
buckets with SHA-256d(1) as the key
  250.1.1.1:8333 tried bucket 40, with key 2: 191, port 9999: 113
  250.1.2.1:8333 from itself: new bucket 786, with key 2: 852, port 9999: 786
  tried buckets for 255 addresses in one /16: 8; in 255 /16s: 160
  new buckets for 1,020 addresses from one source group: 63; one address from 255 source groups: 221; one /16 from itself: 1
```

Another key gives another bucket, which is the point: an attacker who
doesn't know the key can't aim at a bucket. The port moves a tried
bucket but not a new one.

## Randomness

Core's `FastRandomContext` is ChaCha20's keystream: the seed is the key,
the nonce is zero, and blocks are read 8 bytes at a time. `randrange(n)`
draws as many bits as `n - 1` needs and tries again when the draw is too
large. Core's tests use seed 0 and the key 1 (one byte 1, then zeros);
so does this chapter, so a run is the same every time.

<!-- source: coax/lib/bitcoin/addrman.coax rng -->
```jai
// FastRandomContext: ChaCha20 with the seed as key, nonce 0, blocks from
// counter 0, read 8 bytes at a time; RandomMixin keeps leftover bits for
// small requests. Seed 0 is Core's deterministic mode.
AmRng :: struct { key: [32] u8; block: u32; buf: [64] u8; left: int; bits: u64; nbits: int; }
am_rng :: (seed: [] u8) -> AmRng {
  r: AmRng;
  for i: 0..<32 { r.key[i] = seed[i]; }
  return r;
}
am_rand64 :: (r: [] AmRng) -> u64 {
  if r[0].left == 0 {
    nonce: [12] u8;
    c := chacha20_block_checked(r[0].key[..], nonce[..], r[0].block, r[0].buf[..], false);
    r[0].block += 1; r[0].left = 64;
  }
  v := cb_load_le64(r[0].buf[..], 64 - r[0].left).value;
  r[0].left -= 8;
  return v;
}
am_randbits :: (r: [] AmRng, bits: int) -> u64 {
  if bits == 64 { return am_rand64(r); }
  ret: u64;
  if bits <= r[0].nbits {
    ret = r[0].bits; r[0].bits >>= cast(u64) bits; r[0].nbits -= bits;
  } else {
    gen := am_rand64(r);
    ret = (gen << cast(u64) r[0].nbits) | r[0].bits;
    r[0].bits = gen >> cast(u64) (bits - r[0].nbits);
    r[0].nbits = 64 + r[0].nbits - bits;
  }
  return ret & ((cast(u64) 1 << cast(u64) bits) - 1);
}
// randrange: draw bit_width(range - 1) bits until the value is below range.
am_randrange :: (r: [] AmRng, range: int) -> int {
  maxval := cast(u64) (range - 1);
  bits := 0;
  while (maxval >> cast(u64) bits) != 0 { bits += 1; }
  while true {
    v := am_randbits(r, bits);
    if v <= maxval { return cast(int) v; }
  }
  return 0;
}
am_randbool :: (r: [] AmRng) -> bool { return am_randbits(r, 1) == 1; }
```

## What an entry knows

Each address has an `AddrInfo`: the address, the peer it came from, the
services it claims, `nTime` (the last time anyone said it was online),
the last try and the last success, and how many attempts failed since.
Two rules use this:

<!-- source: coax/lib/bitcoin/addrman.coax entry -->
```jai
// AddrInfo: an address with what the node learned about it. time is the
// address's nTime, the last time someone said it was online.
AmInfo :: struct {
  addr: AmAddr; source: AmAddr; services: u64; time: int;
  last_try: int; last_count_attempt: int; last_success: int; attempts: int;
  refs: int;          // how many new buckets hold it (nRefCount), 0 when tried
  tried: bool;        // in the tried table (fInTried)
  pos: int;           // its place in the random order (nRandomPos)
  id: int;            // Core's nId: creation order
  used: bool; next_free: int;
}
// IsTerrible: not worth keeping or sharing. Never within a minute of a try.
am_terrible :: (x: AmInfo, now: int) -> bool {
  if now - x.last_try <= 60 { return false; }
  if x.time > now + 600 { return true; }                 // "came in a flying DeLorean"
  if now - x.time > AM_HORIZON { return true; }           // not seen in 30 days
  if x.last_success == 0 && x.attempts >= AM_RETRIES { return true; }
  if now - x.last_success > AM_MIN_FAIL && x.attempts >= AM_MAX_FAILURES { return true; }
  return false;
}
// GetChance: how readily Select takes it. A try in the last 10 minutes
// divides by 100; each failure since the last success multiplies by 0.66,
// at most 8 times.
am_chance :: (x: AmInfo, now: int) -> f64 {
  chance := 1.0;
  if now - x.last_try < 600 { chance *= 0.01; }
  p := 1.0;
  for i: 0..<(ifx x.attempts < 8 then x.attempts else 8) { p *= 0.66; }
  return chance * p;
}
```

A terrible address isn't shared with peers and is the first to go when
its slot is wanted. `GetChance` makes `Select` avoid an address that was
tried in the last 10 minutes or that keeps failing.

The address manager itself is the two tables, the entries, a random
order of the entries (for `GetAddr` and serialization), and per-network
counts. The caller owns all the memory:

<!-- source: coax/lib/bitcoin/addrman.coax state -->
```jai
// The address manager. The caller owns the memory: new_t holds 1,024 x 64
// slots, tried_t 256 x 64, info the entries, random as many positions, and
// index (a power of two, at least twice info.count) finds an entry by its
// address. Slots hold an entry's index into info, or -1.
AddrMan :: struct {
  key: [32] u8;
  new_t: [] int; tried_t: [] int; info: [] AmInfo; random: [] int; index: [] int;
  n_random: int; n_new: int; n_tried: int; next_id: int; free: int;
  last_good: int = 1;
  collisions: [10] int; collision_ids: [10] int; n_collisions: int; // waiting for test-before-evict, by id
  counts_new: [7] int; counts_tried: [7] int; // m_network_counts
  rng: [] AmRng;   // the manager's FastRandomContext, in the caller's memory
}
// A fresh manager with bucket key key. Core's deterministic test mode uses
// key 1 (one byte 1, then zeros) and an RNG of seed 0.
am_init :: (m: [] AddrMan, key: [] u8, rng: [] AmRng, new_t: [] int, tried_t: [] int, info: [] AmInfo, random: [] int, index: [] int) -> bool {
  if key.count != 32 || new_t.count != AM_NEW_BUCKETS * AM_BUCKET_SIZE || tried_t.count != AM_TRIED_BUCKETS * AM_BUCKET_SIZE { return false; }
  if random.count < info.count || index.count < 2 * info.count || (index.count & (index.count - 1)) != 0 { return false; }
  m[0] = AddrMan.{new_t=new_t, tried_t=tried_t, info=info, random=random, index=index, rng=rng};
  for i: 0..<32 { m[0].key[i] = key[i]; }
  for i: 0..<new_t.count { new_t[i] = -1; }
  for i: 0..<tried_t.count { tried_t[i] = -1; }
  for i: 0..<index.count { index[i] = -1; }
  for i: 0..<info.count { info[i] = AmInfo.{next_free=ifx i + 1 < info.count then i + 1 else -1}; }
  m[0].free = ifx info.count > 0 then 0 else -1;
  return true;
}
```

## Adding addresses

`Add` puts each address in the new table, in the bucket its source
picks. An address announced by someone else is made 2 hours older (the
time penalty) so that relayed addresses rank below ones a peer announced
for itself.

<!-- source: coax/lib/bitcoin/addrman.coax add -->
```jai
// One address in an addr message, or from a DNS seed.
AmRecord :: struct { addr: AmAddr; services: u64; time: int; }
// AddSingle: put an address in the new table, from source src, with its
// time made penalty seconds older (unless the source announced itself).
// A known address only gains services and a fresher time, and joins
// another new bucket with probability 1 in 2^refs. A slot held by a
// terrible entry, or by one that is in other buckets too, is taken over.
am_add_one :: (m: [] AddrMan, r: AmRecord, src: AmAddr, penalty_in: int, now: int) -> bool {
  a := r.addr;
  if !am_routable(a) { return false; }
  penalty := ifx am_same_ip(a, src) then 0 else penalty_in;
  e := am_find(m, a);
  if e >= 0 {
    online := now - r.time < 86400;
    interval := ifx online then 3600 else 86400;
    if m[0].info[e].time < r.time - interval - penalty { m[0].info[e].time = ifx r.time - penalty > 0 then r.time - penalty else 0; }
    m[0].info[e].services |= r.services;
    if r.time <= m[0].info[e].time { return false; }
    if m[0].info[e].tried { return false; }
    refs := m[0].info[e].refs;
    if refs == AM_NEW_PER_ADDRESS { return false; }
    if refs > 0 && am_randrange(m[0].rng, 1 << refs) != 0 { return false; }
  } else {
    e = am_create(m, a, r.services, r.time, src);
    if e < 0 { return false; }
    m[0].info[e].time = ifx r.time - penalty > 0 then r.time - penalty else 0;
  }
  key := m[0].key;
  ub := am_new_bucket(key[..], a, src);
  up := am_position(key[..], true, ub, a);
  slot := ub * AM_BUCKET_SIZE + up;
  held := m[0].new_t[slot];
  insert := held < 0;
  if held != e {
    if !insert {
      old := m[0].info[held];
      if am_terrible(old, now) || (old.refs > 1 && m[0].info[e].refs == 0) { insert = true; }
    }
    if insert {
      am_clear_new(m, ub, up);
      m[0].info[e].refs += 1;
      m[0].new_t[slot] = e;
    } else if m[0].info[e].refs == 0 {
      am_delete(m, e);
    }
  }
  return insert;
}
// Add: each record in turn; the number that went into a bucket.
am_add :: (m: [] AddrMan, records: [] AmRecord, src: AmAddr, penalty: int, now: int) -> int {
  added := 0;
  for r: records { if am_add_one(m, r, src, penalty, now) { added += 1; } }
  return added;
}
```

An address already known only gains: services are combined, and `nTime`
moves forward if the new time is newer by more than an hour (a day for
addresses not seen in the last day). It then joins one more new bucket,
but with probability 1 in 2, 4, 8 and so on for each bucket it is
already in, up to 8. An address many peers tell us about gets more
buckets, but slowly.

When the slot is taken, the newcomer wins only if the old entry is
terrible, or is in other buckets too. Otherwise the newcomer is dropped.
The new table therefore fills up and stays full of addresses the node
has no reason to forget. The example adds `250.1.1.1` to `.24` from one
source:

```text
new table collisions (deterministic key 1)
  sizes after adding 250.1.1.1 to .24: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 22 23
  check: 0
```

`250.1.1.23` lands on a slot that an earlier address holds, and is
dropped. Core's `addrman_new_collisions` test expects the same.

The same address from many sources:

```text
multiplicity
  19 sources, same time: 1; 399 sources, newer each time: 8; size 1
```

With the same time the address gains nothing, so it stays in one
bucket. With a newer time each time, it reaches the limit of 8.

## Good, attempts and connections

`Good` says the node completed a handshake with the address. The entry
leaves all its new buckets and goes to its tried slot. If another entry
holds that slot, the old one goes back to the new table.

<!-- source: coax/lib/bitcoin/addrman.coax maketried -->
```jai
// MakeTried: take an entry out of every new bucket and put it in its tried
// slot. An entry already there goes back to the new table, to the slot its
// own source picks, deleting what was there.
am_make_tried :: (m: [] AddrMan, e: int) {
  key := m[0].key;
  a := m[0].info[e].addr;
  start := am_new_bucket(key[..], a, m[0].info[e].source);
  for n: 0..<AM_NEW_BUCKETS {
    b := (start + n) % AM_NEW_BUCKETS;
    p := am_position(key[..], true, b, a);
    if m[0].new_t[b * AM_BUCKET_SIZE + p] == e {
      m[0].new_t[b * AM_BUCKET_SIZE + p] = -1;
      m[0].info[e].refs -= 1;
      if m[0].info[e].refs == 0 { break; }
    }
  }
  m[0].n_new -= 1; am_count(m, a, false, -1);
  kb := am_tried_bucket(key[..], a);
  kp := am_position(key[..], false, kb, a);
  slot := kb * AM_BUCKET_SIZE + kp;
  if m[0].tried_t[slot] >= 0 {
    old := m[0].tried_t[slot];
    m[0].info[old].tried = false;
    m[0].tried_t[slot] = -1;
    m[0].n_tried -= 1; am_count(m, m[0].info[old].addr, true, -1);
    ub := am_new_bucket(key[..], m[0].info[old].addr, m[0].info[old].source);
    up := am_position(key[..], true, ub, m[0].info[old].addr);
    am_clear_new(m, ub, up);
    m[0].info[old].refs = 1;
    m[0].new_t[ub * AM_BUCKET_SIZE + up] = old;
    m[0].n_new += 1; am_count(m, m[0].info[old].addr, false, 1);
  }
  m[0].tried_t[slot] = e;
  m[0].n_tried += 1;
  m[0].info[e].tried = true;
  am_count(m, a, true, 1);
}
```

Moving the old entry out at once would let a newcomer push out an
address that still works. So `Good` with test-before-evict, the default,
doesn't move a colliding entry. It records it in a set of at most 10
collisions, and the node tests the entry in the way: chapter 70's feeler
connections do this.

<!-- source: coax/lib/bitcoin/addrman.coax good -->
```jai
// Good_: a connection to this address finished its handshake. It moves
// to its tried slot, unless that slot is taken and test_before_evict is
// set: then the move waits in the collision set (at most 10) until
// ResolveCollisions learns whether the entry already there still answers.
am_good_ :: (m: [] AddrMan, a: AmAddr, test_before_evict: bool, time: int) -> bool {
  m[0].last_good = time;
  e := am_find(m, a);
  if e < 0 { return false; }
  m[0].info[e].last_success = time; m[0].info[e].last_try = time; m[0].info[e].attempts = 0;
  if m[0].info[e].tried || m[0].info[e].refs <= 0 { return false; }
  key := m[0].key;
  tb := am_tried_bucket(key[..], a);
  tp := am_position(key[..], false, tb, a);
  if test_before_evict && m[0].tried_t[tb * AM_BUCKET_SIZE + tp] >= 0 {
    if m[0].n_collisions < AM_COLLISIONS { am_collision_add(m, e); }
    return false;
  }
  am_make_tried(m, e);
  return true;
}
am_good :: (m: [] AddrMan, a: AmAddr, time: int) -> bool { return am_good_(m, a, true, time); }
// m_tried_collisions is a std::set of ids: kept here in id order, as
// entries, each checked against its id in case the entry was deleted.
am_collision_add :: (m: [] AddrMan, e: int) {
  id := m[0].info[e].id;
  at := 0;
  while at < m[0].n_collisions {
    if m[0].collision_ids[at] == id { return; }
    if m[0].collision_ids[at] > id { break; }
    at += 1;
  }
  j := m[0].n_collisions;
  while j > at { m[0].collisions[j] = m[0].collisions[j-1]; m[0].collision_ids[j] = m[0].collision_ids[j-1]; j -= 1; }
  m[0].collisions[at] = e; m[0].collision_ids[at] = id;
  m[0].n_collisions += 1;
}
am_collision_live :: (m: [] AddrMan, at: int) -> bool {
  e := m[0].collisions[at];
  return m[0].info[e].used && m[0].info[e].id == m[0].collision_ids[at];
}
am_collision_erase :: (m: [] AddrMan, at: int) {
  for j: at..<m[0].n_collisions - 1 { m[0].collisions[j] = m[0].collisions[j+1]; m[0].collision_ids[j] = m[0].collision_ids[j+1]; }
  m[0].n_collisions -= 1;
}
// Attempt_: we tried to connect. A failure counts once per success of
// any address (m_last_good), so an outage doesn't mark every address bad.
am_attempt :: (m: [] AddrMan, a: AmAddr, count_failure: bool, time: int) {
  e := am_find(m, a);
  if e < 0 { return; }
  m[0].info[e].last_try = time;
  if count_failure && m[0].info[e].last_count_attempt < m[0].last_good {
    m[0].info[e].last_count_attempt = time;
    m[0].info[e].attempts += 1;
  }
}
// Connected_: we are connected now; nTime moves at most every 20 minutes.
am_connected :: (m: [] AddrMan, a: AmAddr, time: int) {
  e := am_find(m, a);
  if e >= 0 && time - m[0].info[e].time > 1200 { m[0].info[e].time = time; }
}
// SetServices_: what the peer's version said it offers.
am_set_services :: (m: [] AddrMan, a: AmAddr, services: u64) {
  e := am_find(m, a);
  if e >= 0 { m[0].info[e].services = services; }
}
```

`Attempt` counts a failure at most once per success of any address, so
a node that loses its internet connection doesn't mark every address as
failing. `Connected` refreshes `nTime` at most every 20 minutes, so that
the times a node shares don't reveal its current connections.

## Test-before-evict

`ResolveCollisions` decides each waiting collision:

<!-- source: coax/lib/bitcoin/addrman.coax collisions -->
```jai
// ResolveCollisions_: for each waiting entry, by id: keep the tried entry
// if it succeeded in the last 4 hours; replace it if a try in the last 4
// hours (but more than a minute ago) failed; replace it anyway once the
// waiting entry's own success is 40 minutes old.
am_resolve_collisions :: (m: [] AddrMan, now: int) {
  at := 0;
  while at < m[0].n_collisions {
    erase := false;
    if !am_collision_live(m, at) { erase = true; }
    else {
      e := m[0].collisions[at];
      a := m[0].info[e].addr;
      key := m[0].key;
      tb := am_tried_bucket(key[..], a);
      tp := am_position(key[..], false, tb, a);
      if !am_valid(a) { erase = true; }
      else {
        old := m[0].info[m[0].tried_t[tb * AM_BUCKET_SIZE + tp]];
        if now - old.last_success < AM_REPLACEMENT { erase = true; }
        else if now - old.last_try < AM_REPLACEMENT {
          if now - old.last_try > 60 { g := am_good_(m, a, false, now); erase = true; }
        } else if now - m[0].info[e].last_success > AM_TEST_WINDOW {
          g := am_good_(m, a, false, now); erase = true;
        }
      }
    }
    if erase { am_collision_erase(m, at); } else { at += 1; }
  }
}
// SelectTriedCollision_: a random waiting entry's rival in the tried
// table, for a feeler to test (chapter 70); -1 if none.
am_select_tried_collision :: (m: [] AddrMan) -> int {
  if m[0].n_collisions == 0 { return -1; }
  at := am_randrange(m[0].rng, m[0].n_collisions);
  if !am_collision_live(m, at) { am_collision_erase(m, at); return -1; }
  a := m[0].info[m[0].collisions[at]].addr;
  key := m[0].key;
  tb := am_tried_bucket(key[..], a);
  return m[0].tried_t[tb * AM_BUCKET_SIZE + am_position(key[..], false, tb, a)];
}
```

- The tried entry succeeded in the last 4 hours: it stays and the
  newcomer waits in the new table.
- The tried entry was tried in the last 4 hours (but more than a minute
  ago) and failed: the newcomer replaces it.
- Nobody tested the tried entry for 40 minutes after the newcomer's
  success: the newcomer replaces it.

The example reproduces Core's `addrman_tried_collisions`,
`addrman_noevict` and `addrman_evictionworks`:

```text
tried table collisions
  Good for 250.1.1.1 to .37: 1111111111111111111111111111111111101
  collision with: 250.1.1.19:0
  after ResolveCollisions, 19 still recent: [::]:0
  250.1.1.36 stays in new: tried=False, multiplicity 1
test-before-evict
  Good(250.1.1.36): False, colliding with 250.1.1.19:0
  19 last succeeded at 1 s and was tried 61 s ago: 36 tried=True, 19 tried=False
  Good(250.1.1.19): False, colliding with 250.1.1.36:0
  4 hours later, untested: 19 tried=True, 36 tried=False, check 0
```

The 36th address hashes to the slot of the 19th. Since 19 succeeded
just now, the collision resolves in its favor. In the second run, 19's
last success is at time 1 and a try 61 seconds ago didn't succeed, so 36
replaces it. Then 19 collides with 36, nobody tests 36 for 4 hours, and
19 gets its slot back.

## Select

`Select` picks an address to connect to:

<!-- source: coax/lib/bitcoin/addrman.coax select -->
```jai
// Select_: an address to connect to, or -1. Tried or new with even odds
// (unless one is empty or new_only), then a random bucket and a random
// entry in it, accepted with probability GetChance times a factor that
// grows 1.2 times with every refusal. nets is a mask of networks
// (1 << AM_NET_*), 0 for any.
am_in_nets :: (nets: int, net: int) -> bool { return nets == 0 || ((nets >> net) & 1) == 1; }
am_select :: (m: [] AddrMan, new_only: bool, nets: int, now: int) -> int {
  if m[0].n_random == 0 { return -1; }
  new_count := m[0].n_new; tried_count := m[0].n_tried;
  if nets != 0 {
    new_count = 0; tried_count = 0;
    for n: 0..<7 { if am_in_nets(nets, n) { new_count += m[0].counts_new[n]; tried_count += m[0].counts_tried[n]; } }
  }
  if new_only && new_count == 0 { return -1; }
  if new_count + tried_count == 0 { return -1; }
  search_tried := false;
  if new_only || tried_count == 0 { search_tried = false; }
  else if new_count == 0 { search_tried = true; }
  else { search_tried = am_randbool(m[0].rng); }
  buckets := ifx search_tried then AM_TRIED_BUCKETS else AM_NEW_BUCKETS;
  factor := 1.0;
  while true {
    b := am_randrange(m[0].rng, buckets);
    copied: [64] int; n := 0;
    for i: 0..<AM_BUCKET_SIZE {
      e := ifx search_tried then m[0].tried_t[b * AM_BUCKET_SIZE + i] else m[0].new_t[b * AM_BUCKET_SIZE + i];
      if e >= 0 { copied[n] = e; n += 1; }
    }
    e := -1;
    while n > 0 {
      p := am_randrange(m[0].rng, n);
      e = copied[p];
      if nets == 0 || am_in_nets(nets, am_network(m[0].info[e].addr)) { break; }
      n -= 1; copied[p] = copied[n];
    }
    if n == 0 { continue; }
    draw := cast(f64) am_randbits(m[0].rng, 30);
    if draw < factor * am_chance(m[0].info[e], now) * 1073741824.0 { return e; }
    factor *= 1.2;
  }
  return -1;
}
```

It picks the tried or the new table with even odds (or the only one with
entries for the networks asked for), then a random bucket, then a random
entry in it. It accepts the entry with probability `GetChance`, and each
refusal makes the next acceptance 1.2 times likelier, so the loop ends.
The bucket is chosen before the entry, so a full bucket of an attacker's
addresses is no more likely than a bucket with one honest address.

```text
select
  3 new, 4 tried
  any: 250.3.3.3:9999 250.3.3.3:9999 250.3.1.1:8333 250.4.6.6:8333 250.3.2.2:9999 250.4.4.4:8333 250.3.3.3:9999 250.4.4.4:8333 250.4.5.5:7777 250.1.1.1:8333 250.1.1.1:8333 250.3.2.2:9999
  new only: 250.3.3.3:9999 250.3.2.2:9999 250.3.3.3:9999 250.3.1.1:8333 250.3.3.3:9999 250.3.2.2:9999
  I2P only: net4:a0ce38ce2224d2cecaf9929388f73379259c0c27e0debdbd7ca4cd085b55e25a:0; CJDNS only: [::]:0
  chance of 250.1.1.1 after a failed attempt 30 s ago: 1 in 152
```

A failure 30 seconds ago multiplies the chance by 0.01 for the recent try
and by 0.66 for the failure: 1 in 152.

The rules for terrible addresses, with the time of the example as now:

```text
terrible
  tried 30 s ago: False
  11 minutes in the future: True
  31 days old: True
  3 attempts, never a success: True
  10 failures in 8 days: True
  9 failures in 8 days: False
  fresh: False
```

## An eclipse attempt

One source group sends 20,000 addresses from 200 /16s. The buckets keep
most of them out:

```text
eclipse: one source group sends 20,000 addresses
  kept 3945 of 20000 in 62 of 1024 new buckets
  1,000 honest addresses from 1,000 groups: 910 kept; of 400 selections 38 are the attacker's, check 0
```

The attacker's addresses fill 62 buckets, all their source group can
reach. Then 1,000 honest addresses from many groups arrive, and 910 of
them find a free slot. Most of the new table's buckets now hold honest
addresses, and since `Select` picks a bucket first, fewer than 1 in 10
selections is the attacker's, though the attacker's addresses are 4 in 5
of the table's entries.

## Sharing addresses: GetAddr

`GetAddr` returns up to a maximum number of addresses and at most a
percentage of the table, in random order, leaving out terrible ones:

<!-- source: coax/lib/bitcoin/addrman.coax getaddr -->
```jai
// GetAddr_: up to max addresses (0: no limit), at most pct percent of the
// table (0: no limit), in random order, of class net (-1: any), leaving
// out terrible ones when filtered. Entries go to out; returns how many.
am_get_addr :: (m: [] AddrMan, max: int, pct: int, net: int, filtered: bool, now: int, out: [] int) -> int {
  want := m[0].n_random;
  if pct != 0 { want = (ifx pct < 100 then pct else 100) * want / 100; }
  if max != 0 && max < want { want = max; }
  if want > out.count { want = out.count; }
  got := 0;
  for n: 0..<m[0].n_random {
    if got >= want { break; }
    am_swap(m, n, am_randrange(m[0].rng, m[0].n_random - n) + n);
    e := m[0].random[n];
    if net >= 0 && am_net_class(m[0].info[e].addr) != net { continue; }
    if filtered && am_terrible(m[0].info[e], now) { continue; }
    out[got] = e; got += 1;
  }
  return got;
}
```

Core's `addrman_getaddr` test, with its numbers:

```text
getaddr
  five addresses: 5 returned, 23% gives 1
  size 2006 (1762 new, 244 tried), 23% gives 461, check 0
  first five: 213.5.1.23:0 165.6.1.23:0 32.7.1.23:0 5.1.1.23:0 138.2.1.23:0
```

## peers.dat

At shutdown, and every 15 minutes, Core writes the address manager to
`peers.dat`. This chapter writes Core's format, so Core can read our
file and we can read Core's:

- the network's 4-byte magic;
- the format (4, `V4_MULTIPORT`) and the lowest format a reader must
  understand to read it, plus 32 (36);
- `nKey`, the new and tried counts, and 1,024 XOR 2^30;
- each new entry, then each tried entry: a version word, `nTime`,
  services as a CompactSize, the address in BIP-155 form, the port, the
  source, the last success and the attempts;
- for each new bucket, its count and the indexes of its entries among
  the new entries;
- the asmap version, 32 zero bytes without an asmap;
- SHA-256d of everything before it.

Core walks a hash map for the entries, so its order isn't specified; we
write them in the random order.

<!-- source: coax/lib/bitcoin/addrman.coax write -->
```jai
// peers.dat as Core writes it: the network's magic, AddrMan::Serialize in
// format 4 (V4_MULTIPORT, lowest compatible 4), and SHA-256d of both. The
// new entries come first, then the tried ones, then each new bucket's
// slots as indexes into the new entries, then the asmap version: 32 zero
// bytes without an asmap. Core walks an unordered_map for the entries, so
// its order is unspecified; this walks the random order. Each entry is a
// CAddress in its disk form (a format word, time, services, then BIP-155
// network, address and port), its source, its last success and attempts.
// scratch holds info.count ints. Returns the size, or -1 if out is short.
AM_DISK_V2 :: cast(u32) (220000 | (1 << 29));  // DISK_VERSION_INIT | DISK_VERSION_ADDRV2
am_put_net :: (out: [] u8, at: int, a: AmAddr) -> int {
  out[at] = cast(u8) am_bip155_id(a.net);
  raw: [32] u8;
  b := am_addr_bytes(a, raw[..]);
  if a.net == AM_NET_IPV4 { b = raw[12..<16]; }
  return am_put_vec(out, at + 1, b);
}
am_put_entry :: (out: [] u8, at_in: int, x: AmInfo) -> int {
  at := at_in;
  stored := cb_store_le32(out, at, AM_DISK_V2);
  stored = cb_store_le32(out, at + 4, cast,trunc(u32) x.time);
  at = p2p_compact64_write_vartime(out, at + 8, x.services);
  at = am_put_net(out, at, x.addr);
  out[at] = cast,trunc(u8) (x.addr.port >> 8); out[at+1] = cast,trunc(u8) x.addr.port;
  at = am_put_net(out, at + 2, x.source);
  stored = cb_store_le64(out, at, cast(u64) x.last_success);
  stored = cb_store_le32(out, at + 8, cast,trunc(u32) x.attempts);
  return at + 12;
}
AM_ENTRY_MAX :: 99;
am_write_vartime :: (m: [] AddrMan, magic: [4] u8, scratch: [] int, out: [] u8) -> int {
  need := 4 + 2 + 32 + 12 + AM_ENTRY_MAX * m[0].n_random + 4 * AM_NEW_BUCKETS + 4 * AM_NEW_PER_ADDRESS * m[0].n_new + 32 + 32;
  if out.count < need { return -1; }
  for i: 0..<4 { out[i] = magic[i]; }
  out[4] = 4; out[5] = 32 + 4;
  for i: 0..<32 { out[6+i] = m[0].key[i]; }
  stored := cb_store_le32(out, 38, cast,trunc(u32) m[0].n_new);
  stored = cb_store_le32(out, 42, cast,trunc(u32) m[0].n_tried);
  stored = cb_store_le32(out, 46, cast(u32) (AM_NEW_BUCKETS ^ (1 << 30)));
  at := 50; k := 0;
  for p: 0..<m[0].n_random {
    e := m[0].random[p];
    if m[0].info[e].refs > 0 { scratch[e] = k; k += 1; at = am_put_entry(out, at, m[0].info[e]); }
  }
  for p: 0..<m[0].n_random {
    e := m[0].random[p];
    if m[0].info[e].tried { at = am_put_entry(out, at, m[0].info[e]); }
  }
  for b: 0..<AM_NEW_BUCKETS {
    count := 0;
    for i: 0..<AM_BUCKET_SIZE { if m[0].new_t[b * AM_BUCKET_SIZE + i] >= 0 { count += 1; } }
    stored = cb_store_le32(out, at, cast(u32) count); at += 4;
    for i: 0..<AM_BUCKET_SIZE {
      e := m[0].new_t[b * AM_BUCKET_SIZE + i];
      if e >= 0 { stored = cb_store_le32(out, at, cast(u32) scratch[e]); at += 4; }
    }
  }
  for i: 0..<32 { out[at+i] = 0; }
  at += 32;
  r := sha256d_checked(out[0..<at], out[at..<at+32], false);
  return at + 32;
}
```

Reading checks the magic and the checksum, refuses a format newer than
it understands, and then puts each new entry back in the buckets the
file lists. If the bucket count or the asmap differ, an entry goes to the
bucket its own source picks instead. A tried entry whose slot is taken
is lost, and a new entry left in no bucket is deleted. Finally
`CheckAddrman` must pass.

<!-- source: coax/lib/bitcoin/addrman.coax read -->
```jai
// AddrManImpl::Unserialize behind DeserializeDB's magic and checksum, into
// a manager fresh from am_init. Formats 0 to 4 read; each entry is in the
// 16-byte v1 form before format 3. New entries take ids 0..nNew-1 in file
// order; a tried entry that is invalid or lands on a taken slot is lost.
// Each new entry is then put back in the buckets the file lists, while
// the bucket count and asmap version match ours, or else in the bucket of
// its own source; one left in no bucket is deleted. Finally CheckAddrman
// must pass.
AmReader :: struct { b: [] u8; at: int; ok: bool; }
am_take :: (r: [] AmReader, n: int) -> int {
  if !r[0].ok || n < 0 || r[0].b.count - r[0].at < n { r[0].ok = false; return 0; }
  at := r[0].at; r[0].at += n; return at;
}
am_le :: (r: [] AmReader, n: int) -> int { at := am_take(r, n); return ifx r[0].ok then tx_le_vartime(r[0].b, at, n) else 0; }
am_i32 :: (r: [] AmReader) -> int { v := am_le(r, 4); return ifx v >= 0x80000000 then v - 0x100000000 else v; }
am_compact :: (r: [] AmReader) -> u64 {
  if !r[0].ok { return 0; }
  c := p2p_compact64_read_vartime(r[0].b, r[0].at);
  if c.code != P2P_OK { r[0].ok = false; return 0; }
  r[0].at = c.next; return c.value;
}
am_read_net :: (r: [] AmReader, v2: bool) -> AmAddr {
  if !v2 { at := am_take(r, 16); return ifx r[0].ok then am_legacy(r[0].b[at..<at+16], 0) else am_none(); }
  id := am_le(r, 1);
  n := cast(int) am_compact(r);
  if n > P2P_MAX_ADDRV2 { r[0].ok = false; }
  known := id == P2P_NET_IPV4 || id == P2P_NET_IPV6 || id == P2P_NET_TORV3 || id == P2P_NET_I2P || id == P2P_NET_CJDNS;
  if known && n != P2P_V2_LENGTHS[id] { r[0].ok = false; }
  at := am_take(r, n);
  if !r[0].ok { return am_none(); }
  return am_bip155(id, r[0].b[at..<at+n], 0);
}
am_read_entry :: (r: [] AmReader, format: int) -> AmInfo {
  x := AmInfo.{used=true, next_free=-1};
  stored := am_le(r, 4) & 0xfff80000;  // the low bits (DISK_VERSION_IGNORE_MASK) are ignored
  v2 := false;
  if stored == (1 << 29) && format >= 3 { v2 = true; }
  else if stored != 0 { r[0].ok = false; }
  x.time = am_le(r, 4);
  x.services = ifx v2 then am_compact(r) else cast(u64) am_le(r, 8);
  x.addr = am_read_net(r, v2);
  at := am_take(r, 2);
  if r[0].ok { x.addr.port = (cast(int) r[0].b[at] << 8) | cast(int) r[0].b[at+1]; }
  x.source = am_read_net(r, v2);
  x.last_success = am_le(r, 8);
  x.attempts = am_i32(r);
  return x;
}
am_read_vartime :: (m: [] AddrMan, data: [] u8, magic: [4] u8, scratch: [] int) -> int {
  if data.count < 36 { return AM_CORRUPT; }
  for i: 0..<4 { if data[i] != magic[i] { return AM_BAD_MAGIC; } }
  d: [32] u8;
  h := sha256d_checked(data[0..<data.count-32], d[..], false);
  for i: 0..<32 { if d[i] != data[data.count-32+i] { return AM_BAD_CHECKSUM; } }
  rd: [1] AmReader; rd[0] = AmReader.{b=data[4..<data.count-32], ok=true};
  r := rd[..];
  format := am_le(r, 1);
  compat := am_le(r, 1);
  if !rd[0].ok || compat < 32 { return AM_CORRUPT; }
  if compat - 32 > 4 { return AM_BAD_VERSION; }
  at := am_take(r, 32);
  if !rd[0].ok { return AM_CORRUPT; }
  for i: 0..<32 { m[0].key[i] = rd[0].b[at+i]; }
  n_new := am_i32(r); n_tried := am_i32(r); buckets := am_i32(r);
  if format >= 1 { buckets ^= 1 << 30; }
  if !rd[0].ok || n_new < 0 || n_new > AM_NEW_BUCKETS * AM_BUCKET_SIZE || n_tried < 0 || n_tried > AM_TRIED_BUCKETS * AM_BUCKET_SIZE { return AM_CORRUPT; }
  if n_new + n_tried > m[0].info.count { return AM_FULL; }
  key := m[0].key;
  for n: 0..<n_new {
    x := am_read_entry(r, format);
    if !rd[0].ok { return AM_CORRUPT; }
    x.id = n; x.pos = n;
    m[0].info[n] = x; m[0].random[n] = n;
    am_index_put(m, n);
    am_count(m, x.addr, false, 1);
  }
  m[0].n_random = n_new; m[0].n_new = n_new; m[0].next_id = n_new;
  for n: 0..<n_tried {
    x := am_read_entry(r, format);
    if !rd[0].ok { return AM_CORRUPT; }
    kb := am_tried_bucket(key[..], x.addr);
    slot := kb * AM_BUCKET_SIZE + am_position(key[..], false, kb, x.addr);
    if am_valid(x.addr) && m[0].tried_t[slot] < 0 {
      e := m[0].n_random;
      x.id = m[0].next_id; x.pos = e; x.tried = true;
      m[0].info[e] = x; m[0].random[e] = e;
      am_index_put(m, e);
      m[0].tried_t[slot] = e;
      m[0].n_random += 1; m[0].next_id += 1; m[0].n_tried += 1;
      am_count(m, x.addr, true, 1);
    }
  }
  // The bucket lists, read twice: the asmap version after them decides.
  lists := rd[0].at;
  for b: 0..<buckets { k := am_i32(r); for i: 0..<k { e := am_i32(r); } if !rd[0].ok { return AM_CORRUPT; } }
  restore := buckets == AM_NEW_BUCKETS;
  if format >= 2 {
    v := am_take(r, 32);
    if !rd[0].ok { return AM_CORRUPT; }
    for i: 0..<32 { if rd[0].b[v+i] != 0 { restore = false; } }
  }
  rd[0].at = lists;
  for b: 0..<buckets {
    k := am_i32(r);
    for i: 0..<k {
      e := am_i32(r);
      if e < 0 || e >= n_new || !am_valid(m[0].info[e].addr) || m[0].info[e].refs >= AM_NEW_PER_ADDRESS { continue; }
      a := m[0].info[e].addr;
      bucket := b;
      p := am_position(key[..], true, bucket, a);
      if !restore || m[0].new_t[bucket * AM_BUCKET_SIZE + p] >= 0 {
        bucket = am_new_bucket(key[..], a, m[0].info[e].source);
        p = am_position(key[..], true, bucket, a);
        if m[0].new_t[bucket * AM_BUCKET_SIZE + p] >= 0 { continue; }
      }
      m[0].new_t[bucket * AM_BUCKET_SIZE + p] = e;
      m[0].info[e].refs += 1;
    }
  }
  // Free the slots no entry took, then delete new entries in no bucket.
  m[0].free = -1;
  e := m[0].info.count - 1;
  while e >= m[0].n_random { m[0].info[e] = AmInfo.{next_free=m[0].free}; m[0].free = e; e -= 1; }
  for n: 0..<n_new { if m[0].info[n].refs == 0 { am_delete(m, n); } }
  if am_check(m, scratch) != 0 { return AM_CORRUPT; }
  return AM_OK;
}
```

```text
peers.dat
  4583 bytes, SHA-256 2e2826f2ca723f6744a50b9ef353c085940c39d49c951fb6656c324c939be244
  read back: 7 new, 2 tried, same places: True
  a flipped byte: refused
  the last byte missing: refused
  another network: refused
  20 entries claimed, one present: refused (end of data)
  four entries, two made invalid: 2 remain
```

The last two lines are Core's `load_addrman_corrupted` and
`remove_invalid` tests. A Core node would refuse the damaged file and
start with an empty address manager.

## Receiving addr

Each entry of an `addr` or `addrv2` message (chapter 65) becomes a
record. A peer that didn't send `sendaddrv2` in the handshake only gets
addresses the 16-byte form can carry:

<!-- source: coax/lib/bitcoin/addrman.coax wire -->
```jai
// IsAddrCompatible: a peer that didn't send sendaddrv2 only gets
// addresses the 16-byte v1 form can carry.
am_addr_compatible :: (wants_v2: bool, a: AmAddr) -> bool {
  return wants_v2 || a.net == AM_NET_IPV4 || a.net == AM_NET_IPV6 || a.net == AM_NET_INTERNAL;
}
// An addr (v1) or addrv2 payload from records, as PushAddress queues them
// and chapter 65's p2p_addr_read_vartime reads them. Returns the size, or
// -1 for more than 1,000 records, a record v1 can't carry, or a short out.
am_wire_write_vartime :: (records: [] AmRecord, v2: bool, out: [] u8) -> int {
  if records.count > AM_MAX_ADDR || out.count < 9 + records.count * 64 { return -1; }
  at := tx_compact_write_vartime(out, 0, records.count);
  raw: [32] u8;
  for r: records {
    if !am_addr_compatible(v2, r.addr) { return -1; }
    stored := cb_store_le32(out, at, cast,trunc(u32) r.time); at += 4;
    if v2 {
      at = p2p_compact64_write_vartime(out, at, r.services);
      at = am_put_net(out, at, r.addr);
    } else {
      stored = cb_store_le64(out, at, r.services);
      b := am_addr_bytes(r.addr, raw[..]);
      for i: 0..<16 { out[at+8+i] = b[i]; }
      at += 24;
    }
    out[at] = cast,trunc(u8) (r.addr.port >> 8); out[at+1] = cast,trunc(u8) r.addr.port; at += 2;
  }
  return at;
}
```

`ProcessAddrs` decides what happens to the records:

<!-- source: coax/lib/bitcoin/addrman.coax relay -->
```jai
// What a node keeps per peer for address relay (net_processing.cpp Peer).
AmPeer :: struct { tokens: f64 = 1.0; stamp: int; getaddr_recvd: bool; }
AmProcessed :: struct { code: int; processed: int; limited: int; added: int; relay: int; }
// MayHaveUsefulAddressDB, else HasAllDesirableServiceFlags (chapter 68).
am_useful :: (services: u64, tip_depth: int) -> bool {
  if (services & (HS_NODE_NETWORK | HS_NODE_NETWORK_LIMITED)) != 0 { return true; }
  return hs_has_desirable(services, tip_depth);
}
// ProcessAddrs: an addr or addrv2 message's records from a peer at src.
// More than 1,000 is misbehaviour. The token bucket refills at 0.1 per
// second up to 1,000 (our getaddr adds 1,000 more, outside this call);
// each address spends one, and without tokens the rest are dropped unless
// the peer has the addr permission. Core shuffles first, so the dropped
// ones are random: here a Fisher-Yates shuffle with rng, whose draws are
// not libstdc++'s std::shuffle. Addresses without useful services are
// skipped; a time before 1973 or more than 10 minutes ahead becomes five
// days ago. A message of at most 10 addresses has its fresh routable ones
// relayed (into relay_out); reachable ones (a mask of networks) go to the
// address manager, two hours older.
am_process_addrs :: (m: [] AddrMan, peer: [] AmPeer, records: [] AmRecord, src: AmAddr, now: int, rng: [] AmRng,
                     permission: bool, tip_depth: int, reachable: int, relay_out: [] AmAddr) -> AmProcessed {
  if records.count > AM_MAX_ADDR { return AmProcessed.{code=AM_MISBEHAVING}; }
  if peer[0].tokens < cast(f64) AM_MAX_ADDR {
    elapsed := ifx now > peer[0].stamp then now - peer[0].stamp else 0;
    t := peer[0].tokens + cast(f64) elapsed * AM_ADDR_RATE;
    peer[0].tokens = ifx t < cast(f64) AM_MAX_ADDR then t else cast(f64) AM_MAX_ADDR;
  }
  peer[0].stamp = now;
  i := records.count - 1;
  while i > 0 {
    j := am_randrange(rng, i + 1);
    tmp := records[i]; records[i] = records[j]; records[j] = tmp;
    i -= 1;
  }
  res := AmProcessed.{};
  for k: 0..<records.count {
    if peer[0].tokens < 1.0 {
      if !permission { res.limited += 1; continue; }
    } else { peer[0].tokens -= 1.0; }
    r := records[k];
    if !am_useful(r.services, tip_depth) { continue; }
    if r.time <= AM_TIME_INIT || r.time > now + 600 { r.time = now - 5 * 86400; }
    res.processed += 1;
    if r.time > now - 600 && records.count <= AM_RELAY_LIMIT && am_routable(r.addr) && res.relay < relay_out.count {
      relay_out[res.relay] = r.addr; res.relay += 1;
    }
    if am_in_nets(reachable, am_network(r.addr)) && reachable != 0 && am_add_one(m, r, src, 2 * 3600, now) { res.added += 1; }
  }
  return res;
}
// CServiceHash(0, 0): SipHash of the network (as a 64-bit word), the port
// and the address bytes.
am_service_hash :: (a: AmAddr) -> u64 {
  buf: [48] u8;
  stored := cb_store_le64(buf[..], 0, cast(u64) a.net);
  stored = cb_store_le64(buf[..], 8, cast(u64) a.port);
  for i: 0..<a.n { buf[16+i] = a.ip[i]; }
  return siphash24(0, 0, buf[0..<16+a.n]);
}
// RelayAddress: the one or two peers an address goes to, best first. The
// node's randomizer (seed0, seed1) hashes the address and a 24-hour
// window, so for a day the same address goes to the same peers; each
// eligible peer's id gets a score and the highest win. A reachable
// address goes to two; an unreachable one to one or two, by a bit of the
// hash. Returns how many ids it wrote into out.
am_relay_targets :: (seed0: u64, seed1: u64, a: AmAddr, reachable: bool, now: int, ids: [] int, eligible: [] bool, out: [] int) -> int {
  buf: [32] u8;
  h := am_service_hash(a);
  stored := cb_store_le64(buf[..], 0, AM_RANDOMIZER_RELAY);
  stored = cb_store_le64(buf[..], 8, h);
  stored = cb_store_le64(buf[..], 16, (cast(u64) now + h) / cast(u64) AM_ROTATE);
  count := ifx reachable || (siphash24(seed0, seed1, buf[0..<24]) & 1) == 1 then 2 else 1;
  best: [2] u64; who: [2] int;
  for p: 0..<ids.count {
    if !eligible[p] { continue; }
    stored = cb_store_le64(buf[..], 24, cast(u64) ids[p]);
    k := siphash24(seed0, seed1, buf[0..<32]);
    for i: 0..<count {
      if k > best[i] {
        if i == 0 && count == 2 { best[1] = best[0]; who[1] = who[0]; }
        best[i] = k; who[i] = ids[p];
        break;
      }
    }
  }
  n := 0;
  for i: 0..<count { if best[i] != 0 && n < out.count { out[n] = who[i]; n += 1; } }
  return n;
}
```

- More than 1,000 addresses is misbehavior (chapter 71).
- Each peer has a token bucket: it starts with 1 token, gains 0.1 a
  second up to 1,000, and gains 1,000 when we send it `getaddr`. Each
  address spends a token. Without tokens, the rest of the message is
  dropped, unless the peer has the `addr` permission. Core shuffles the
  message first, so a peer can't choose which addresses survive.
- An address without `NODE_NETWORK` or `NODE_NETWORK_LIMITED`, and
  without the services the node wants, is skipped.
- A time before 1973 or more than 10 minutes ahead becomes five days
  ago.
- If the message has at most 10 addresses, each one less than 10 minutes
  old and routable is relayed. Larger messages answer `getaddr` and are
  not relayed.
- Addresses on networks the node can reach go to the address manager,
  with the 2-hour penalty.

```text
addr relay
  1,001 addresses: misbehaving
  1,000 after our getaddr: 1000 processed, 0 rate-limited, 251 added, 0 to relay
  1,000 more 10 s later: 2 processed, 998 rate-limited
  1,000 from a peer with the addr permission: 1000 processed, 0 rate-limited
  10 fresh addresses an hour later: 10 processed, 10 to relay
  11 fresh addresses: 11 processed, 0 to relay, 0 rate-limited
  no services, an ancient time, a private address, Tor: 3 processed, 1 added; the ancient time became now - 122 h
  relay 250.1.1.1:8333 to: [8, 6], next day: [5, 4]
  unreachable Tor address to: [6, 1]
```

In the last case of the first block, the address without services is
skipped. The private address is processed but not added (it isn't
routable), and so is the Tor address (Tor isn't reachable here). The
ancient time becomes five days ago and then two hours older: 122 hours.

### Who gets a relayed address

`RelayAddress` sends an address to two peers. It hashes the address with
the node's secret randomizer and the current 24-hour period, and gives
each eligible peer a score from that hash and the peer's id. The two
highest scores win. For a day, the same address goes to the same peers,
so sending an address again doesn't spread it further; the next day the
choice changes. An address on a network the node can't reach goes to
one or two peers, by one bit of the hash, so its relay doesn't reveal
which networks the node reaches.

## Answering getaddr

<!-- source: coax/lib/bitcoin/addrman.coax reply -->
```jai
// The getaddr answer, one per inbound connection and none to outbound
// ones (a fingerprinting defence). Every requester on the same local
// address shares one cached answer: GetAddr(1000, 23%) taken when the
// cache is empty or stale, and kept 21 hours plus a random part of 6,
// drawn in microseconds. Times here are microseconds.
AM_R_OUTBOUND :: 1; AM_R_REPEATED :: 2;
AmCache :: struct { expires: int; n: int; }
AmReply :: struct { code: int; reason: int; n: int; }
am_getaddr_reply :: (m: [] AddrMan, peer: [] AmPeer, inbound: bool, cache: [] AmCache, records: [] AmRecord, now_us: int, rng: [] AmRng) -> AmReply {
  if !inbound { return AmReply.{code=AM_IGNORED, reason=AM_R_OUTBOUND}; }
  if peer[0].getaddr_recvd { return AmReply.{code=AM_IGNORED, reason=AM_R_REPEATED}; }
  peer[0].getaddr_recvd = true;
  if cache[0].expires < now_us {
    picked: [1000] int;
    n := am_get_addr(m, AM_MAX_ADDR, AM_MAX_PCT, -1, true, now_us / 1000000, picked[0..<(ifx records.count < 1000 then records.count else 1000)]);
    for i: 0..<n { x := m[0].info[picked[i]]; records[i] = AmRecord.{addr=x.addr, services=x.services, time=x.time}; }
    cache[0].n = n;
    cache[0].expires = now_us + AM_CACHE_LIFE * 1000000 + am_randrange(rng, AM_CACHE_JITTER * 1000000);
  }
  return AmReply.{code=AM_OK, n=cache[0].n};
}
```

A node answers `getaddr` only from inbound peers, and only once per
connection. An outbound peer asking is ignored, because an attacker
could otherwise link the node's answers on different networks. Every
answer for 21 to 27 hours is the same cached `GetAddr(1000, 23%)`, so
asking again from many connections reveals nothing new.

```text
getaddr replies
  inbound: 45 addresses, cache until +81813 s
  again on that connection: ignored: repeated
  from an outbound peer: ignored: outbound
  another inbound an hour later: same answer True
  after the cache expires: same answer False
```

## DNS seeds

A new node knows no addresses. It asks DNS seeds: hostnames run by
people Core trusts to return the addresses of working nodes. When it
already has addresses, it waits to see whether they work first.

<!-- source: coax/lib/bitcoin/addrman.coax seeds -->
```jai
// ThreadDNSAddressSeed's first decision. With no known address (or
// -forcednsseed) every seed is asked at once. Otherwise the node first
// tries what it knows: 11 seconds, or 5 minutes with 1,000 addresses or
// more, and then asks 3 seeds at a time, only while it has fewer than 2
// full outbound connections. The asking itself is a DNS lookup: chapter
// 89's node program does it, through a proxy in chapter 75.
AmSeedPlan :: struct { query: int; wait: int; }
am_seed_plan :: (size: int, force: bool, seeds: int) -> AmSeedPlan {
  if force || size == 0 { return AmSeedPlan.{query=seeds, wait=0}; }
  return AmSeedPlan.{query=AM_SEEDS_AT_ONCE, wait=ifx size >= AM_SEEDS_MANY_PEERS then AM_SEEDS_MANY else AM_SEEDS_FEW};
}
// After the wait: ask the next seeds only if outbound connections are
// still short.
am_seed_needed :: (full_outbound: int) -> bool { return full_outbound < AM_SEED_OUTBOUND; }
```

```text
DNS seeds
  0 known addresses: wait 0 s, then query 9 of 9
  500 known addresses: wait 11 s, then query 3 of 9
  5000 known addresses: wait 300 s, then query 3 of 9
  5000 known addresses, -forcednsseed: wait 0 s, then query 9 of 9
```

After the wait, the node asks the next three seeds only if it still has
fewer than two full outbound connections. The lookup itself is the node
program's job (chapter 89), through a proxy when one is set (chapter 75).

## The example

[bitcoin/examples/addrman.coax](../examples/addrman.coax) prints every
transcript in this chapter:

<!-- source: bitcoin/examples/addrman.coax example -->
```jai
#load "../../coax/lib/bitcoin/addrman.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

NOW :: 1700000000;
MAINNET_MAGIC :: [4] u8.[0xf9, 0xbe, 0xb4, 0xd9];
GOOD_SERVICES :: HS_NODE_NETWORK | HS_NODE_WITNESS;
IPV4_IPV6 :: (1 << AM_NET_IPV4) | (1 << AM_NET_IPV6);

// Decode hex into out and return the filled part.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
print_hex :: (bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
tf :: (b: bool) -> string { return ifx b then "True" else "False"; }
v4 :: (a: int, b: int, c: int, d: int) -> AmAddr { return am_v4(a, b, c, d, 0); }
// An IPv6 address as its 16 bytes, read the way addr v1 reads them.
v6 :: (text: string) -> AmAddr { raw: [16] u8; return am_legacy(decode(text, raw[..]), 0); }
one_record :: (a: AmAddr, services: u64, time: int) -> [1] AmRecord {
  return [1] AmRecord.[AmRecord.{addr=a, services=services, time=time}];
}
add1 :: (m: [] AddrMan, a: AmAddr, time: int, src: AmAddr) -> bool {
  r := one_record(a, 0, time);
  return am_add(m, r[..], src, 0, NOW) == 1;
}
show :: (a: AmAddr) {
  if a.net == AM_NET_IPV4 { print("%.%.%.%:%", a.ip[0], a.ip[1], a.ip[2], a.ip[3], a.port); return; }
  ip := a.ip; print("net%:", a.net); print_hex(ip[0..<a.n]); print(":%", a.port);
}
// An entry's address, or Core's empty CService for none.
show_entry :: (m: [] AddrMan, e: int) {
  if e < 0 { print("[::]:0"); } else { show(m[0].info[e].addr); }
}
// A manager with Core's deterministic key (1) and a seed-0 RNG.
start :: (m: [] AddrMan, rng: [] AmRng, new_t: [] int, tried_t: [] int, info: [] AmInfo, random: [] int, index: [] int) {
  zero: [32] u8; one: [32] u8; one[0] = 1;
  rng[0] = am_rng(zero[..]);
  #assert am_init(m, one[..], rng, new_t, tried_t, info, random, index);
}
// SHA-256d of a 32-bit little-endian number: Core's test keys.
test_key :: (n: u32, out: [] u8) -> [] u8 {
  b: [4] u8; stored := cb_store_le32(b[..], 0, n);
  #assert sha256d_checked(b[..], out, false) == SHA_OK;
  return out[0..<32];
}
// How many entries of a set of up-to-1,024 buckets are marked.
marked :: (seen: [] bool) -> int { n := 0; for s: seen { if s { n += 1; } } return n; }

groups :: () {
  print("groups\n");
  i2p: [32] u8; tor: [32] u8; cj: [16] u8; raw: [16] u8; g: [24] u8;
  tor[0] = 0xf2;
  labels := [11] string.["250.1.1.1", "10.0.0.1", "127.0.0.1", "2001:470:1234::1 (he.net)", "2a01:e::1",
    "2002:fa01:0101::1 (6to4)", "2001:0::0506:fefe (Teredo)", "::ffff:250.1.1.1 (v1 form)", "fc00::1 (CJDNS)", "I2P", "Tor v3"];
  addrs: [11] AmAddr;
  addrs[0] = v4(250, 1, 1, 1); addrs[1] = v4(10, 0, 0, 1); addrs[2] = v4(127, 0, 0, 1);
  addrs[3] = v6("20010470123400000000000000000001");
  addrs[4] = v6("2a01000e000000000000000000000001");
  addrs[5] = v6("2002fa01010100000000000000000001");
  addrs[6] = v6("2001000000000000000000000506fefe");
  addrs[7] = v6("00000000000000000000fffffa010101");
  addrs[8] = am_bytes(AM_NET_CJDNS, decode("fc000000000000000000000000000001", cj[..]), 0);
  addrs[9] = am_bytes(AM_NET_I2P, decode("a0ce38ce2224d2cecaf9929388f73379259c0c27e0debdbd7ca4cd085b55e25a", i2p[..]), 0);
  addrs[10] = am_bytes(AM_NET_ONION, tor[..], 0);
  for i: 0..<11 {
    print("  %: ", labels[i]); print_hex(am_group(addrs[i], g[..]));
    print("%\n", ifx am_routable(addrs[i]) then "" else " (unroutable)");
  }
}

buckets :: () {
  print("buckets with SHA-256d(1) as the key\n");
  b1: [32] u8; b2: [32] u8;
  k1 := test_key(1, b1[..]); k2 := test_key(2, b2[..]);
  a1 := am_v4(250, 1, 1, 1, 8333); a2 := am_v4(250, 1, 1, 1, 9999);
  print("  250.1.1.1:8333 tried bucket %, with key 2: %, port 9999: %\n", am_tried_bucket(k1, a1), am_tried_bucket(k2, a1), am_tried_bucket(k1, a2));
  b := am_v4(250, 1, 2, 1, 8333); s := v4(250, 1, 2, 1);
  print("  250.1.2.1:8333 from itself: new bucket %, with key 2: %, port 9999: %\n",
    am_new_bucket(k1, b, s), am_new_bucket(k2, b, s), am_new_bucket(k1, am_v4(250, 1, 2, 1, 9999), s));
  same16: [1024] bool; diff16: [1024] bool;
  for i: 0..<255 { same16[am_tried_bucket(k1, v4(250, 1, 1, i))] = true; diff16[am_tried_bucket(k1, v4(250, i, 1, 1))] = true; }
  print("  tried buckets for 255 addresses in one /16: %; in 255 /16s: %\n", marked(same16[..]), marked(diff16[..]));
  one_src: [1024] bool; many_src: [1024] bool; same_grp: [1024] bool;
  for j: 0..<4 * 255 { one_src[am_new_bucket(k1, v4(250 + j / 255, j % 256, 1, 1), v4(251, 4, 1, 1))] = true; }
  for p: 0..<255 { many_src[am_new_bucket(k1, v4(250, 1, 1, 1), v4(250, p, 1, 1))] = true; }
  for i: 0..<255 { same_grp[am_new_bucket(k1, v4(250, 1, 1, i), v4(250, 1, 1, i))] = true; }
  print("  new buckets for 1,020 addresses from one source group: %; one address from 255 source groups: %; one /16 from itself: %\n",
    marked(one_src[..]), marked(many_src[..]), marked(same_grp[..]));
}

collisions :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [64] AmInfo; random: [64] int; index: [128] int; rng: [1] AmRng; m: [1] AddrMan;
  scratch: [64] int;
  src := v4(252, 2, 2, 2);
  print("new table collisions (deterministic key 1)\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  print("  sizes after adding 250.1.1.1 to .24:");
  for i: 1..<25 { ok := add1(m[..], v4(250, 1, 1, i), AM_TIME_INIT, src); print(" %", am_size(m[..], -1, -1)); }
  print("\n  check: %\n", am_check(m[..], scratch[..]));

  print("tried table collisions\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  print("  Good for 250.1.1.1 to .37: ");
  for i: 1..<38 {
    a := v4(250, 1, 1, i);
    ok := add1(m[..], a, AM_TIME_INIT, src);
    print("%", ifx am_good(m[..], a, NOW) then "1" else "0");
  }
  print("\n  collision with: "); show_entry(m[..], am_select_tried_collision(m[..]));
  am_resolve_collisions(m[..], NOW);
  print("\n  after ResolveCollisions, 19 still recent: "); show_entry(m[..], am_select_tried_collision(m[..]));
  p := am_find_entry(m[..], v4(250, 1, 1, 36));
  print("\n  250.1.1.36 stays in new: tried=%, multiplicity %\n", tf(p.tried), p.multiplicity);

  print("test-before-evict\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for i: 1..<36 { a := v4(250, 1, 1, i); ok := add1(m[..], a, AM_TIME_INIT, src); g := am_good(m[..], a, NOW); }
  a36 := v4(250, 1, 1, 36); a19 := v4(250, 1, 1, 19);
  ok := add1(m[..], a36, AM_TIME_INIT, src);
  print("  Good(250.1.1.36): %, colliding with ", tf(am_good(m[..], a36, NOW))); show_entry(m[..], am_select_tried_collision(m[..]));
  g := am_good(m[..], a19, 1);
  am_attempt(m[..], a19, false, NOW - 61);
  am_resolve_collisions(m[..], NOW);
  print("\n  19 last succeeded at 1 s and was tried 61 s ago: 36 tried=%, 19 tried=%\n",
    tf(am_find_entry(m[..], a36).tried), tf(am_find_entry(m[..], a19).tried));
  print("  Good(250.1.1.19): %, colliding with ", tf(am_good(m[..], a19, NOW))); show_entry(m[..], am_select_tried_collision(m[..]));
  am_resolve_collisions(m[..], NOW + 4 * 3600);
  print("\n  4 hours later, untested: 19 tried=%, 36 tried=%, check %\n",
    tf(am_find_entry(m[..], a19).tried), tf(am_find_entry(m[..], a36).tried), am_check(m[..], scratch[..]));

  print("multiplicity\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  a := am_v4(253, 3, 3, 3, 8333);
  for i: 1..<20 { ok = add1(m[..], a, NOW, v4(i % 256, i >> 8, 1, 1)); }
  first := am_find_entry(m[..], a).multiplicity;
  for i: 1..<400 { r := one_record(a, 0, NOW + i); n := am_add(m[..], r[..], v4(i % 256, i >> 8, 1, 1), 0, NOW + 400); }
  print("  19 sources, same time: %; 399 sources, newer each time: %; size %\n", first, am_find_entry(m[..], a).multiplicity, am_size(m[..], -1, -1));
}

getaddr :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [4096] AmInfo; random: [4096] int; index: [8192] int; rng: [1] AmRng; m: [1] AddrMan;
  scratch: [4096] int; got: [2500] int;
  print("getaddr\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  first: [5] AmRecord;
  first[0] = AmRecord.{addr=am_v4(250, 250, 2, 1, 8333), time=NOW};
  first[1] = AmRecord.{addr=am_v4(250, 251, 2, 2, 9999), time=NOW};
  first[2] = AmRecord.{addr=am_v4(251, 252, 2, 3, 8333), time=NOW};
  first[3] = AmRecord.{addr=am_v4(252, 253, 3, 4, 8333), time=NOW};
  first[4] = AmRecord.{addr=am_v4(252, 254, 4, 5, 8333), time=NOW};
  from1 := [3] AmRecord.[first[0], first[2], first[4]];
  from2 := [2] AmRecord.[first[1], first[3]];
  n := am_add(m[..], from1[..], v4(250, 1, 2, 1), 0, NOW);
  n = am_add(m[..], from2[..], v4(250, 2, 3, 3), 0, NOW);
  all := am_get_addr(m[..], 0, 0, -1, true, NOW, got[..]);
  print("  five addresses: % returned, 23% gives %\n", all, "%", am_get_addr(m[..], 2500, 23, -1, true, NOW, got[..]));
  g := am_good(m[..], first[0].addr, NOW); g = am_good(m[..], first[1].addr, NOW);
  for i: 1..<8 * 256 {
    a := v4(i % 256, i >> 8, 1, 23);
    ok := add1(m[..], a, NOW, a);
    if i % 8 == 0 { g = am_good(m[..], a, NOW); }
  }
  k := am_get_addr(m[..], 2500, 23, -1, true, NOW, got[..]);
  print("  size % (% new, % tried), 23% gives %, check %\n", am_size(m[..], -1, -1), am_size(m[..], -1, 1), am_size(m[..], -1, 0), "%", k, am_check(m[..], scratch[..]));
  print("  first five:");
  for i: 0..<5 { print(" "); show_entry(m[..], got[i]); }
  print("\n");
}

select :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [64] AmInfo; random: [64] int; index: [128] int; rng: [1] AmRng; m: [1] AddrMan;
  print("select\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  adds := [7] AmAddr.[am_v4(250, 1, 1, 1, 8333), am_v4(250, 3, 1, 1, 8333), am_v4(250, 3, 2, 2, 9999), am_v4(250, 3, 3, 3, 9999),
                       am_v4(250, 4, 4, 4, 8333), am_v4(250, 4, 5, 5, 7777), am_v4(250, 4, 6, 6, 8333)];
  srcs := [7] AmAddr.[v4(252, 2, 2, 2), v4(250, 3, 1, 1), v4(250, 3, 1, 1), v4(250, 4, 1, 1), v4(250, 3, 2, 2), v4(250, 3, 2, 2), v4(250, 1, 1, 3)];
  for i: 0..<7 { ok := add1(m[..], adds[i], NOW, srcs[i]); }
  for i: [4] int.[0, 4, 5, 6] { g := am_good(m[..], adds[i], NOW - 3600); }
  print("  % new, % tried\n  any:", am_size(m[..], -1, 1), am_size(m[..], -1, 0));
  for i: 0..<12 { print(" "); show_entry(m[..], am_select(m[..], false, 0, NOW)); }
  print("\n  new only:");
  for i: 0..<6 { print(" "); show_entry(m[..], am_select(m[..], true, 0, NOW)); }
  raw: [32] u8;
  i2p := am_bytes(AM_NET_I2P, decode("a0ce38ce2224d2cecaf9929388f73379259c0c27e0debdbd7ca4cd085b55e25a", raw[..]), 0);
  ok := add1(m[..], i2p, NOW, v4(252, 2, 2, 2));
  print("\n  I2P only: "); show_entry(m[..], am_select(m[..], false, 1 << AM_NET_I2P, NOW));
  print("; CJDNS only: "); show_entry(m[..], am_select(m[..], false, 1 << AM_NET_CJDNS, NOW));
  am_attempt(m[..], adds[0], true, NOW - 30);
  c := am_chance(m[0].info[am_find(m[..], adds[0])], NOW);
  print("\n  chance of 250.1.1.1 after a failed attempt 30 s ago: 1 in %\n", cast(int) (1.0 / c + 0.5));
}

terrible :: () {
  print("terrible\n");
  labels := [7] string.["tried 30 s ago", "11 minutes in the future", "31 days old", "3 attempts, never a success",
    "10 failures in 8 days", "9 failures in 8 days", "fresh"];
  for k: 0..<7 {
    x := AmInfo.{addr=v4(250, 1, 1, 1), source=v4(252, 2, 2, 2), time=NOW};
    if k == 0 { x.last_try = NOW - 30; x.time = 0; }
    if k == 1 { x.time = NOW + 660; }
    if k == 2 { x.time = NOW - 31 * 86400; }
    if k == 3 { x.attempts = 3; }
    if k == 4 || k == 5 { x.attempts = 14 - k; x.last_success = NOW - 8 * 86400; }
    print("  %: %\n", labels[k], tf(am_terrible(x, NOW)));
  }
}

eclipse :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [8192] AmInfo; random: [8192] int; index: [16384] int; rng: [1] AmRng; m: [1] AddrMan;
  scratch: [8192] int;
  print("eclipse: one source group sends 20,000 addresses\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for i: 0..<20000 {
    ok := add1(m[..], am_v4(1 + i % 200, (i / 200) % 250, i % 7, 9 + i % 11, 8333), NOW, v4(66, 66, i % 250, 1 + i % 5));
  }
  used := 0;
  for b: 0..<AM_NEW_BUCKETS {
    any := false;
    for i: 0..<AM_BUCKET_SIZE { if new_t[b * AM_BUCKET_SIZE + i] >= 0 { any = true; } }
    if any { used += 1; }
  }
  print("  kept % of 20000 in % of 1024 new buckets\n", am_size(m[..], -1, -1), used);
  honest := 0;
  for i: 0..<1000 {
    if add1(m[..], am_v4(100 + i % 50, i / 50, 7, 7, 8333), NOW, v4(150 + i % 50, i / 50 + 30, 1, 1)) { honest += 1; }
  }
  theirs := 0;
  for i: 0..<400 { e := am_select(m[..], true, 0, NOW); if m[0].info[e].addr.ip[2] != 7 { theirs += 1; } }
  print("  1,000 honest addresses from 1,000 groups: % kept; of 400 selections % are the attacker's, check %\n", honest, theirs, am_check(m[..], scratch[..]));
}

// Read data into a fresh manager: the result code, and its size.
Back :: struct { code: int; size: int; }
read_back :: (data: [] u8, magic: [4] u8) -> Back {
  new_t: [65536] int; tried_t: [16384] int; info: [32] AmInfo; random: [32] int; index: [64] int; rng: [1] AmRng; scratch: [32] int;
  back: [1] AddrMan;
  start(back[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  code := am_read_vartime(back[..], data, magic, scratch[..]);
  return Back.{code=code, size=am_size(back[..], -1, -1)};
}
same_places :: (m: [] AddrMan, data: [] u8) -> bool {
  new_t: [65536] int; tried_t: [16384] int; info: [32] AmInfo; random: [32] int; index: [64] int; rng: [1] AmRng; scratch: [32] int;
  back: [1] AddrMan;
  start(back[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  #assert am_read_vartime(back[..], data, MAINNET_MAGIC, scratch[..]) == AM_OK;
  print("  read back: % new, % tried, ", am_size(back[..], -1, 1), am_size(back[..], -1, 0));
  same := true;
  for x: m[0].info {
    if !x.used { continue; }
    p := am_find_entry(m, x.addr); q := am_find_entry(back[..], x.addr);
    if p.found != q.found || p.tried != q.tried || p.multiplicity != q.multiplicity || p.bucket != q.bucket || p.position != q.position { same = false; }
  }
  return same;
}
peers_dat :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [32] AmInfo; random: [32] int; index: [64] int; rng: [1] AmRng; m: [1] AddrMan;
  scratch: [32] int; data: [8192] u8; digest: [32] u8; bad: [8192] u8;
  print("peers.dat\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for i: 1..<9 { r := one_record(am_v4(250, 7, i, 1, 8333), 9, NOW - i); n := am_add(m[..], r[..], v4(252, 5, 1, 1), 0, NOW); }
  g := am_good(m[..], am_v4(250, 7, 1, 1, 8333), NOW); g = am_good(m[..], am_v4(250, 7, 2, 1, 8333), NOW);
  raw: [32] u8;
  i2p := am_bytes(AM_NET_I2P, decode("a0ce38ce2224d2cecaf9929388f73379259c0c27e0debdbd7ca4cd085b55e25a", raw[..]), 0);
  r := one_record(i2p, 1024 | 8, NOW); n := am_add(m[..], r[..], v4(252, 5, 1, 1), 0, NOW);
  size := am_write_vartime(m[..], MAINNET_MAGIC, scratch[..], data[..]);
  #assert sha256_hash_checked(data[0..<size], digest[..], false) == SHA_OK;
  print("  % bytes, SHA-256 ", size); print_hex(digest[..]); print("\n");
  print("same places: %\n", tf(same_places(m[..], data[0..<size])));
  labels := [3] string.["a flipped byte", "the last byte missing", "another network"];
  for k: 0..<3 {
    for i: 0..<size { bad[i] = data[i]; }
    cut := size;
    magic := MAINNET_MAGIC;
    if k == 0 { bad[60] ^= 1; }
    if k == 1 { cut -= 1; }
    if k == 2 { magic = [4] u8.[0x0b, 0x11, 0x09, 0x07]; }
    print("  %: %\n", labels[k], ifx read_back(bad[0..<cut], magic).code == AM_OK then "read" else "refused");
  }
  // Core's load_addrman_corrupted: 10 new and 10 tried claimed, one entry present.
  body: [200] u8; at := 0;
  for i: 0..<4 { body[i] = MAINNET_MAGIC[i]; }
  body[4] = 1; body[5] = 32; body[6] = 1; at = 38;
  stored := cb_store_le32(body[..], at, 10); stored = cb_store_le32(body[..], at + 4, 10);
  stored = cb_store_le32(body[..], at + 8, cast(u32) (AM_NEW_BUCKETS ^ (1 << 30))); at += 12;
  stored = cb_store_le32(body[..], at + 4, cast(u32) AM_TIME_INIT); at += 16;
  for i: 0..<2 {
    for j: 0..<16 { body[at+j] = AM_V4_PREFIX[j]; }
    body[at+12] = 252; body[at+13] = cast(u8) (1 + i); body[at+14] = cast(u8) (1 + i); body[at+15] = cast(u8) (1 + i);
    at += 16;
    if i == 0 { body[at] = cast(u8) (7777 >> 8); body[at+1] = cast,trunc(u8) 7777; at += 2; }
  }
  at += 12;
  #assert sha256d_checked(body[0..<at], body[at..<at+32], false) == SHA_OK;
  code := read_back(body[0..<at+32], MAINNET_MAGIC).code;
  print("  20 entries claimed, one present: %\n", ifx code == AM_CORRUPT then "refused (end of data)" else "read");
  // Core's remove_invalid: two entries rewritten as 0.0.0.0 and 255.255.255.255 are dropped.
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  four: [4] AmRecord;
  four[0] = AmRecord.{addr=v4(5, 5, 5, 5), time=AM_TIME_INIT}; four[1] = AmRecord.{addr=v4(7, 7, 7, 7), time=AM_TIME_INIT};
  four[2] = AmRecord.{addr=v4(6, 6, 6, 6), time=AM_TIME_INIT}; four[3] = AmRecord.{addr=v4(8, 8, 8, 8), time=AM_TIME_INIT};
  n = am_add(m[..], four[..], am_none(), 0, NOW);
  g = am_good(m[..], v4(7, 7, 7, 7), NOW); g = am_good(m[..], v4(8, 8, 8, 8), NOW);
  size = am_write_vartime(m[..], MAINNET_MAGIC, scratch[..], data[..]) - 32;
  for k: 0..<2 {
    from := cast(u8) (6 + 2 * k); to := ifx k == 0 then cast(u8) 0 else cast(u8) 255;
    for i: 0..<size - 3 {
      if data[i] == from && data[i+1] == from && data[i+2] == from && data[i+3] == from {
        for j: 0..<4 { data[i+j] = to; }
        break;
      }
    }
  }
  #assert sha256d_checked(data[0..<size], data[size..<size+32], false) == SHA_OK;
  b := read_back(data[0..<size+32], MAINNET_MAGIC);
  print("  four entries, two made invalid: % remain\n", ifx b.code == AM_OK then b.size else -1);
}

// n records base.(i/250).(i%250).1:8333.
batch :: (n: int, base: int, time: int, out: [] AmRecord) -> [] AmRecord {
  for i: 0..<n { out[i] = AmRecord.{addr=am_v4(base, i / 250, i % 250, 1, 8333), services=GOOD_SERVICES, time=time}; }
  return out[0..<n];
}
relay :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [4096] AmInfo; random: [4096] int; index: [8192] int; rng: [1] AmRng; m: [1] AddrMan;
  records: [1001] AmRecord; out: [16] AmAddr; shuffle: [1] AmRng; seed: [32] u8;
  print("addr relay\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for i: 0..<32 { seed[i] = 7; }
  shuffle[0] = am_rng(seed[..]);
  peer := [1] AmPeer.[AmPeer.{stamp=NOW}];
  src := am_v4(99, 1, 1, 1, 8333);
  r := am_process_addrs(m[..], peer[..], batch(1001, 20, NOW, records[..]), src, NOW, shuffle[..], false, 0, IPV4_IPV6, out[..]);
  print("  1,001 addresses: %\n", ifx r.code == AM_MISBEHAVING then "misbehaving" else "ok");
  peer[0].tokens += 1000.0;
  r = am_process_addrs(m[..], peer[..], batch(1000, 21, NOW - 3600, records[..]), src, NOW + 1, shuffle[..], false, 0, IPV4_IPV6, out[..]);
  print("  1,000 after our getaddr: % processed, % rate-limited, % added, % to relay\n", r.processed, r.limited, r.added, r.relay);
  r = am_process_addrs(m[..], peer[..], batch(1000, 22, NOW - 3600, records[..]), src, NOW + 11, shuffle[..], false, 0, IPV4_IPV6, out[..]);
  print("  1,000 more 10 s later: % processed, % rate-limited\n", r.processed, r.limited);
  r = am_process_addrs(m[..], peer[..], batch(1000, 23, NOW - 3600, records[..]), src, NOW + 11, shuffle[..], true, 0, IPV4_IPV6, out[..]);
  print("  1,000 from a peer with the addr permission: % processed, % rate-limited\n", r.processed, r.limited);
  peer[0] = AmPeer.{stamp=NOW};
  r = am_process_addrs(m[..], peer[..], batch(10, 30, NOW + 3540, records[..]), src, NOW + 3600, shuffle[..], false, 0, IPV4_IPV6, out[..]);
  print("  10 fresh addresses an hour later: % processed, % to relay\n", r.processed, r.relay);
  r = am_process_addrs(m[..], peer[..], batch(11, 31, NOW + 3540, records[..]), src, NOW + 3600, shuffle[..], false, 0, IPV4_IPV6, out[..]);
  print("  11 fresh addresses: % processed, % to relay, % rate-limited\n", r.processed, r.relay, r.limited);
  peer[0] = AmPeer.{stamp=NOW - 100};
  zero: [32] u8;
  tor := am_bytes(AM_NET_ONION, zero[..], 9050);
  mixed := [4] AmRecord.[AmRecord.{addr=am_v4(40, 1, 1, 1, 8333), time=NOW},
                         AmRecord.{addr=am_v4(40, 1, 1, 2, 8333), services=HS_NODE_NETWORK_LIMITED, time=50},
                         AmRecord.{addr=am_v4(10, 1, 1, 3, 8333), services=HS_NODE_NETWORK, time=NOW},
                         AmRecord.{addr=tor, services=HS_NODE_NETWORK, time=NOW}];
  r = am_process_addrs(m[..], peer[..], mixed[..], src, NOW, shuffle[..], false, 0, IPV4_IPV6, out[..]);
  t := m[0].info[am_find(m[..], am_v4(40, 1, 1, 2, 8333))].time;
  print("  no services, an ancient time, a private address, Tor: % processed, % added; the ancient time became now - % h\n", r.processed, r.added, (NOW - t) / 3600);
  ids := [8] int.[1, 2, 3, 4, 5, 6, 7, 8];
  eligible := [8] bool.[true, true, false, true, true, true, true, true];
  to: [2] int;
  a := am_v4(250, 1, 1, 1, 8333);
  k := am_relay_targets(11, 22, a, true, NOW, ids[..], eligible[..], to[..]);
  print("  relay 250.1.1.1:8333 to: [%, %], ", to[0], to[1]);
  k = am_relay_targets(11, 22, a, true, NOW + 86400, ids[..], eligible[..], to[..]);
  print("next day: [%, %]\n", to[0], to[1]);
  k = am_relay_targets(11, 22, tor, false, NOW, ids[..], eligible[..], to[..]);
  print("  unreachable Tor address to:");
  for i: 0..<k { print("%", ifx i == 0 then " [" else ", "); print("%", to[i]); }
  print("]\n");
}

REASONS :: [3] string.["", "outbound", "repeated"];
replies :: () {
  new_t: [65536] int; tried_t: [16384] int; info: [256] AmInfo; random: [256] int; index: [512] int; rng: [1] AmRng; m: [1] AddrMan;
  records: [1000] AmRecord; first: [1000] AmRecord; seed: [32] u8; cr: [1] AmRng;
  print("getaddr replies\n");
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for i: 1..<200 { r := one_record(am_v4(250, i, 1, 1, 8333), 9, NOW); n := am_add(m[..], r[..], v4(251, i, 1, 1), 0, NOW); }
  cache: [1] AmCache;
  for i: 0..<32 { seed[i] = 9; }
  cr[0] = am_rng(seed[..]);
  p1 := [1] AmPeer.[AmPeer.{stamp=NOW}]; p2 := [1] AmPeer.[AmPeer.{stamp=NOW}]; p3 := [1] AmPeer.[AmPeer.{stamp=NOW}];
  us := NOW * 1000000;
  r1 := am_getaddr_reply(m[..], p1[..], true, cache[..], records[..], us, cr[..]);
  for i: 0..<r1.n { first[i] = records[i]; }
  print("  inbound: % addresses, cache until +% s\n", r1.n, (cache[0].expires - us) / 1000000);
  r := am_getaddr_reply(m[..], p1[..], true, cache[..], records[..], us, cr[..]);
  print("  again on that connection: ignored: %\n", REASONS[r.reason]);
  r = am_getaddr_reply(m[..], p3[..], false, cache[..], records[..], us, cr[..]);
  print("  from an outbound peer: ignored: %\n", REASONS[r.reason]);
  r = am_getaddr_reply(m[..], p2[..], true, cache[..], records[..], us + 3600 * 1000000, cr[..]);
  print("  another inbound an hour later: same answer %\n", tf(r.n == r1.n && same_records(records[0..<r.n], first[0..<r1.n])));
  p3[0] = AmPeer.{stamp=NOW};
  r = am_getaddr_reply(m[..], p3[..], true, cache[..], records[..], cache[0].expires + 1, cr[..]);
  print("  after the cache expires: same answer %\n", tf(r.n == r1.n && same_records(records[0..<r.n], first[0..<r1.n])));
}
same_records :: (a: [] AmRecord, b: [] AmRecord) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if !am_same(a[i].addr, b[i].addr) { return false; } }
  return true;
}

seeds :: () {
  print("DNS seeds\n");
  sizes := [4] int.[0, 500, 5000, 5000];
  for k: 0..<4 {
    p := am_seed_plan(sizes[k], k == 3, 9);
    print("  % known addresses%: wait % s, then query % of 9\n", sizes[k], ifx k == 3 then ", -forcednsseed" else "", p.wait, p.query);
  }
}

main :: () {
  groups();
  buckets();
  collisions();
  getaddr();
  select();
  terrible();
  eclipse();
  peers_dat();
  relay();
  replies();
  seeds();
}
```

```shell
coax/bin/coax run bitcoin/examples/addrman.coax
```

Its output, 82 lines, is
[bitcoin/fixtures/addrman.out](../fixtures/addrman.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/addrman.coax](../examples/checks/addrman.coax),
which loads the example. Every network's addresses round-trip through
`addrv2`; the token bucket never lets a peer get more processed than its
tokens allow, whatever the gaps between messages; `Select` keeps to the
networks asked for; and the `getaddr` cache lives 21 to 27 hours:

<!-- source: bitcoin/examples/checks/addrman.coax check -->
```jai
// Every address of every BIP-155 network survives addrv2 and comes back
// from the wire as itself; a v1 message refuses the ones it can't carry.
#forall "every network survives addrv2" k: 0..<60 #steps 400000000 {
  nets := [5] int.[AM_NET_IPV4, AM_NET_IPV6, AM_NET_ONION, AM_NET_I2P, AM_NET_CJDNS];
  sizes := [5] int.[4, 16, 32, 32, 16];
  net := nets[k % 5];
  a := AmAddr.{net=net, n=sizes[k % 5], port=k * 1009 % 65536};
  for i: 0..<a.n { a.ip[i] = cast,trunc(u8) (k * 31 + i * 17 + 1); }
  if net == AM_NET_CJDNS { a.ip[0] = 0xfc; }
  if net == AM_NET_IPV6 { a.ip[0] = 0x2a; }
  recs := [1] AmRecord.[AmRecord.{addr=a, services=cast(u64) k, time=NOW - k}];
  buf: [128] u8; got: [1] P2pAddress;
  n := am_wire_write_vartime(recs[..], true, buf[..]);
  l := p2p_addr_read_vartime(buf[0..<n], true, got[..]);
  #assert l.code == P2P_OK && l.count == 1 && am_same(am_from_wire(buf[0..<n], got[0], true), a);
  #assert (am_wire_write_vartime(recs[..], false, buf[..]) > 0) == (net == AM_NET_IPV4 || net == AM_NET_IPV6);
}
// The token bucket: whatever the gaps between messages, a peer without
// the addr permission never gets more addresses processed than one
// token to start, plus 0.1 a second, plus what our getaddr granted.
#forall "the token bucket bounds what a peer gets processed" k: 0..<40 #steps 400000000 {
  m: [1] AddrMan; rng: [1] AmRng; seed: [32] u8; seed[0] = cast,trunc(u8) k;
  rng[0] = am_rng(seed[..]);
  recs: [200] AmRecord; out: [16] AmAddr;
  for i: 0..<200 { recs[i] = AmRecord.{addr=am_v4(30, i, k, 1, 8333), services=GOOD_SERVICES, time=NOW}; }
  peer := [1] AmPeer.[AmPeer.{stamp=NOW}];
  now := NOW; total := 0;
  for step: 0..<6 {
    now += (k * 7 + step * 13) % 90;
    r := am_process_addrs(m[..], peer[..], recs[0..<10 + (k + step) % 190], am_v4(99, 1, 1, 1, 8333), now, rng[..], false, 0, 0, out[..]);
    total += r.processed;
    #assert peer[0].tokens >= 0.0 && peer[0].tokens <= cast(f64) AM_MAX_ADDR && r.added == 0;
  }
  #assert cast(f64) total <= 1.0 + cast(f64) (now - NOW) * AM_ADDR_RATE + 0.000001;
}
// Select with a network mask returns only that network's entries, or
// none when that network has none.
#forall "Select keeps to the networks asked for" k: 0..<12 #steps 4000000000 {
  new_t: [65536] int; tried_t: [16384] int; info: [64] AmInfo; random: [64] int; index: [128] int; rng: [1] AmRng; m: [1] AddrMan;
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  raw: [32] u8; raw[0] = cast,trunc(u8) k;
  i2p := am_bytes(AM_NET_I2P, raw[..], 0);
  ok := add1(m[..], am_v4(250, k, 1, 1, 8333), NOW, v4(252, 2, 2, 2));
  ok = add1(m[..], i2p, NOW, v4(252, 2, 2, 2));
  if k % 2 == 1 { g := am_good(m[..], i2p, NOW); }
  for n: [3] int.[AM_NET_IPV4, AM_NET_I2P, AM_NET_ONION] {
    e := am_select(m[..], false, 1 << n, NOW);
    #assert (e < 0) == (n == AM_NET_ONION) && (e < 0 || am_network(m[0].info[e].addr) == n);
  }
}
// The getaddr cache lives between 21 and 27 hours, and every inbound
// requester inside that window gets the same answer.
#forall "the getaddr cache lives 21 to 27 hours" k: 0..<8 #steps 4000000000 {
  new_t: [65536] int; tried_t: [16384] int; info: [64] AmInfo; random: [64] int; index: [128] int; rng: [1] AmRng; m: [1] AddrMan;
  start(m[..], rng[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for i: 1..<20 { ok := add1(m[..], am_v4(250, i, 1, 1, 8333), NOW, v4(251, i, 1, 1)); }
  seed: [32] u8; seed[1] = cast,trunc(u8) k; cr := [1] AmRng.[am_rng(seed[..])];
  cache: [1] AmCache; recs: [32] AmRecord;
  p := [1] AmPeer.[AmPeer.{}]; q := [1] AmPeer.[AmPeer.{}];
  us := NOW * 1000000;
  r := am_getaddr_reply(m[..], p[..], true, cache[..], recs[..], us, cr[..]);
  e := cache[0].expires;
  #assert r.n == 4 && e >= us + 21 * 3600 * 1000000 && e < us + 27 * 3600 * 1000000;
  r = am_getaddr_reply(m[..], q[..], true, cache[..], recs[..], e - 1, cr[..]);
  #assert r.n == 4 && cache[0].expires == e;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/addrman.coax
```

`coax test` runs 293 checks, including the loaded files' own checks, in
about 2 minutes on 2026-11-03.

### The library's own checks

`addrman.coax` is an `#authority` file. Its checks port the cases of
Bitcoin Core's `src/test/addrman_tests.cpp`: `addrman_simple`,
`addrman_terrible_many_failures`, `addrman_penalty_self_announcement`,
`addrman_ports`, `addrman_select_by_network`, `addrman_select_special`,
`addrman_new_collisions`, `addrman_new_multiplicity`,
`addrman_tried_collisions`, `addrman_getaddr`, `getaddr_unfiltered`,
the legacy bucket tests (buckets 40 and 786, 8 tried buckets for a /16,
160 for 255 /16s, at most 64 new buckets for a source group),
`addrman_serialization`, `remove_invalid`, `addrman_noevict`,
`addrman_evictionworks`, `load_addrman_corrupted`,
`addrman_update_address` and `addrman_size`. Core's tests run its
consistency check on every call, which draws from the same random
generator, so Core's sequences of random choices differ from ours; every
outcome checked here is decided by the hashes alone. The checks also
cover groups for every network, `addr` and `addrv2` for every BIP-155
network through chapter 65's reader, `ProcessAddrs`, `RelayAddress`, the
`getaddr` answer and the DNS seed plan.

```shell
coax/bin/coax test coax/lib/bitcoin/addrman.coax
```

`coax test` runs 286 checks, including those of the files it loads, in
about 2 minutes on 2026-11-03. Mutation testing: not run (owner speed
order).

### Bitcoin Core's code

[addrman.py](../../coax/tests/bitcoin/addrman.py) computes every line
the example prints. It transliterates `AddrManImpl` from the pinned
`addrman.cpp` and `addrman_impl.h`, `GetGroup` from `netgroup.cpp`,
`FastRandomContext` from `random.h` with the pinned `chacha20.py`, and
`ProcessAddrs`, `RelayAddress` and the `getaddr` cache from
`net_processing.cpp` and `net.cpp`. It reads the constants from the
pinned sources, and compares its transcript with the fixture:

```shell
python3 -I coax/tests/bitcoin/addrman.py --coax coax/bin/coax
```

On 2026-11-03 all 82 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- The asmap: grouping by autonomous system needs Core's asmap file and
  its interpreter.
- The DNS lookup itself, and fixed seeds compiled into the node:
  chapter 89, through chapter 75's proxy.
- The rolling bloom filter of addresses each peer already knows, which
  stops the node from sending a peer its own addresses back.
- Banned and discouraged addresses: chapter 71.
- `Select`'s other callers, feelers and anchors: chapter 70.

## Summary

- An address's group is its /16 (IPv4), /32 (IPv6) or 4 bits (Tor,
  I2P); tunnels group by the IPv4 inside.
- The new table has 1,024 buckets and the tried table 256, each of 64
  slots, picked by SHA-256d keyed with a secret. One source group reaches
  at most 64 new buckets; one /16 at most 8 tried buckets.
- An address enters the new table when heard of and the tried table
  after a handshake. A tried address that still works isn't evicted:
  the newcomer waits until a test shows the old one is gone.
- `Select` picks a table, a bucket and a slot, then accepts by chance,
  so an attacker's many addresses in a few buckets win few selections.
- `peers.dat` is Core's format 4, checked by SHA-256d.
- A node takes at most 1,000 addresses a message and 0.1 a second from
  each peer, relays small fresh messages to two peers fixed for a day,
  answers `getaddr` once per inbound connection from a cache, and asks
  DNS seeds only when its own addresses don't work.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [10. SipHash-2-4](ch010-siphash-2-4.md)
- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md)
- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md)

## Sources

- [BIP-155](https://github.com/bitcoin/bips/blob/master/bip-0155.mediawiki),
  pinned by chapter 65.
- Bitcoin Core at revision `ddea9196`: `addrman.h`, `addrman_impl.h`,
  `addrman.cpp`, `netgroup.h`, `netgroup.cpp`, `random.h` and
  `test/addrman_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/addrman.cpp)
  by this chapter; `net_processing.cpp` (`ProcessAddrs`,
  `RelayAddress`, the `getaddr` handler), `net.cpp` (`GetAddresses`'
  cache, `ThreadDNSAddressSeed`), `netaddress.h`, `netaddress.cpp`,
  `protocol.h` and `test_framework/crypto/chacha20.py`, pinned there by
  earlier chapters.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
