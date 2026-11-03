# 72. Headers sync and block download

A new node knows one block: genesis. Before it can check anything else
it has to learn which chain has the most work, and then fetch the blocks
of that chain. Bitcoin Core does this in two passes. First it downloads
only headers, 80 bytes per block, from one peer. Then it downloads
blocks from all its peers in parallel, inside a window that moves as
blocks arrive.

Both passes have to survive peers that lie. A peer can send an endless
chain of cheap headers to fill your memory, so Core only stores headers
whose chain has enough work, and checks that without storing them
first. A peer can also stop sending a block that everyone else is
waiting for, so Core notices when the window can't move, disconnects
that peer, and asks someone else.

This chapter builds both passes from Core's `headerssync.cpp` and
`net_processing.cpp`, replays Core's own tests of them, and syncs from
real Bitcoin Core peers on regtest and signet.

## What this chapter teaches

- Block locators: how a node names the blocks it has in about
  log2(height) hashes, and how a peer finds where to start its answer.
- `HeadersSyncState`: PRESYNC follows a low-work chain while keeping one
  salted bit every few hundred headers; REDOWNLOAD fetches the same
  headers again, checks the bits, and releases headers for storage only
  after the chain has proved its work.
- `ProcessHeadersMessage`: which headers are stored at once, which go
  through presync, and which are ignored.
- `FindNextBlocksToDownload`: the 1,024-block window, the 16 blocks in
  flight per peer, and how the node decides which peer is stalling.
- The stalling timeout, which doubles for each staller up to 64 seconds
  and decays as blocks connect, and the per-block download timeout.
- What a downloaded block is checked against here: its header and its
  Merkle root.

## Where Bitcoin uses these

| Where | What it uses | Chapter |
| --- | --- | --- |
| The block index | headers stored by `AcceptBlockHeader`, the best header | 59 |
| P2P messages | `getheaders`, `headers`, `getdata`, `block` | 65 |
| The handshake | the version message and verack before any of this | 68 |
| Denial of service | the anti-DoS work threshold, `MaybePunishNodeForBlock` | 71 |
| Compact blocks | blocks near the tip, after initial download | 73 |
| AssumeUTXO | a chainstate that starts at a snapshot's height | 79 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [sync.coax](../../coax/lib/bitcoin/sync.coax) | `sy_headers_params` | `HeadersSyncParams` per chain |
| | `sy_getheaders_write_vartime` | a `getheaders` payload from a locator |
| | `sy_find_fork_vartime` | `FindForkInGlobalIndex`, the answering side |
| | `sy_hss_init`, `sy_hss_process`, `sy_hss_locator_vartime`, `sy_hss_finalize` | `HeadersSyncState` |
| | `sy_presync`, `sy_presync_one`, `sy_redownload_one`, `sy_pop_ready`, `sy_commitment` | its PRESYNC and REDOWNLOAD steps |
| | `sy_on_headers_vartime`, `sy_threshold`, `sy_maybe_getheaders` | `ProcessHeadersMessage` |
| | `sy_dl_init`, `sy_find_next_vartime` | `FindNextBlocksToDownload` |
| | `sy_dl_step`, `sy_request` | the block part of `SendMessages` |
| | `sy_unrequest`, `sy_dl_drop`, `sy_link`, `sy_dl_received`, `sy_dl_assume` | `RemoveBlockRequest`, disconnect, `BlockConnected` |
| | `sy_check_block_vartime` | a block against the header asked for |
| [blockindex.coax](../../coax/lib/bitcoin/blockindex.coax) | `bi_accept_header_vartime`, `bi_locator`, `bi_ancestor` | the block index (chapter 59) |
| [dos.coax](../../coax/lib/bitcoin/dos.coax) | `ds_threshold`, `ds_getheaders_due` | the work threshold and getheaders interval (chapter 71) |
| [p2p.coax](../../coax/lib/bitcoin/p2p.coax) | `p2p_locator_write_vartime`, `p2p_headers_read_vartime` | the messages (chapter 65) |
| [merkle.coax](../../coax/lib/bitcoin/merkle.coax) | `merkle_block_ids_vartime`, `merkle_check_vartime` | the Merkle root and its mutation check (chapter 46) |

Everything in `sync.coax` is pure. The caller says which message
arrived and when; the procedures say what Core would store, request or
disconnect. Headers and blocks are public, so all of it runs in variable
time (chapter 3).

## What is left to later chapters

This chapter downloads and stores blocks, but it doesn't validate or
connect them. Chapter 57 (full block validation), chapter 58 (connecting
a block to the UTXO set), chapter 60 (reorganizations) and chapter 61
(block files) are not written yet. So:

- A block is checked only against the header it was requested by: its
  hash, and a Merkle root without a duplicated subtree. Its scripts,
  amounts and other rules are not checked.
- Nothing is connected, so the active chain stays at genesis. The "tip"
  that moves the download window is the last block that has its data and
  all its ancestors' data, which is Core's `m_chain_tx_count != 0`. Core
  would connect those blocks next.
- There is no reorganization. Headers on several branches go into the
  block index, which tracks the most-work header (chapter 59), and the
  window follows each peer's best header. Switching the active chain
  between branches is chapter 60's work.
- Blocks are kept in memory and appended to a simple file: a 4-byte
  height, a 4-byte size, then the block. Chapter 61 replaces it with
  `blk*.dat` and undo files.

## Locators

A `getheaders` message carries a locator: hashes of blocks you have,
newest first. The first ten are one block apart; after that the step
doubles, and the list ends at genesis. A chain of height 800,000 needs
about 30 hashes. The peer answers with up to 2,000 headers that follow
the first locator hash on its own active chain, or genesis if it knows
none of them.

<!-- source: coax/lib/bitcoin/sync.coax constants -->
```jai
// net_processing.cpp and chainparams.cpp.
SY_MAX_HEADERS :: 2000;          // MAX_HEADERS_RESULTS: a full headers message
SY_WINDOW :: 1024;               // BLOCK_DOWNLOAD_WINDOW
SY_IN_FLIGHT :: 16;              // MAX_BLOCKS_IN_TRANSIT_PER_PEER
SY_STALL_DEFAULT :: 2;           // BLOCK_STALLING_TIMEOUT_DEFAULT, seconds
SY_STALL_MAX :: 64;              // BLOCK_STALLING_TIMEOUT_MAX, seconds
SY_RECENT :: 24 * 60 * 60;       // headers from every peer once the best header is this recent
// HeadersSyncParams: a commitment every period headers, and how many
// redownloaded headers wait before release.
SyHeadersParams :: struct { period: int; buffer: int; }
sy_headers_params :: (chain: int) -> SyHeadersParams {
  if chain == CHAIN_MAIN { return SyHeadersParams.{period=649, buffer=15400}; }
  if chain == CHAIN_TESTNET3 { return SyHeadersParams.{period=682, buffer=14613}; }
  if chain == CHAIN_TESTNET4 { return SyHeadersParams.{period=616, buffer=16199}; }
  if chain == CHAIN_SIGNET { return SyHeadersParams.{period=629, buffer=15885}; }
  return SyHeadersParams.{period=275, buffer=7017};
}
```

<!-- source: coax/lib/bitcoin/sync.coax locator -->
```jai
// A getheaders payload for the locator of entry i (chapter 59's
// LocatorEntries), optionally led by one more hash, and a zero stop hash:
// "send what follows the first of these you have". Returns its size.
sy_getheaders_write_vartime :: (t: [] BlockTree, first: [] u8, i: int, out: [] u8) -> int {
  idx: [64] int;
  n := bi_locator(t[0].l, i, idx[..]);
  hashes: [32 * 65] u8; k := 0;
  if first.count == 32 { for j: 0..<32 { hashes[j] = first[j]; } k = 1; }
  for m: 0..<n { for j: 0..<32 { hashes[32 * (k + m) + j] = t[0].e[idx[m]].hash[j]; } }
  stop: [32] u8;
  return p2p_locator_write_vartime(HS_PROTOCOL_VERSION, hashes[0..<32 * (k + n)], stop[..], out);
}
// The other side: FindForkInGlobalIndex. The first locator hash on the
// peer's active chain (heights[] of its chain, by position in hashes),
// else genesis; the answer starts one block after it.
sy_find_fork_vartime :: (chain: [] u8, height: int, locator: [] u8, count: int) -> int {
  for m: 0..<count {
    for h: 0..<height + 1 {
      same := true;
      for j: 0..<32 { if chain[32 * h + j] != locator[32 * m + j] { same = false; break; } }
      if same { return h; }
    }
  }
  return 0;
}
```

For a tip at height 40 the locator is 40 down to 29, then 27, 23, 15
and genesis: 16 hashes. A peer whose chain ends at 20 finds 15 first
and answers from 16.

## Presync: proving work before storing headers

Chapter 71 stopped headers whose chain has less work than the anti-DoS
threshold: the larger of the chain's minimum chain work and the work of
the active tip minus 144 blocks. A new node is far below that threshold
for most of the chain, so every early headers message has too little
work. Core can't store those headers, or a peer could fill its memory
with a cheap fork. It can't throw them away either, or it would never
sync.

`HeadersSyncState` solves this with two passes over the same headers:

1. **PRESYNC.** Follow the peer's chain from the fork point, checking
   that each header continues the previous one and that its difficulty
   changes only as the rules allow. Add up its work. Store nothing but
   one bit per `commitment_period` headers: the low bit of a SipHash of
   the header's hash, under a salt the peer never learns, at heights
   equal to a random offset modulo the period. When the work reaches
   the threshold, switch to REDOWNLOAD.
2. **REDOWNLOAD.** Ask for the same headers again from the fork point.
   Check each one against its stored bit. Keep the last
   `redownload_buffer_size` headers in a buffer, and release older
   ones for storage. Once the redownloaded work itself reaches the
   threshold, stop checking bits and release everything.

A peer that sends a different chain the second time fails each
commitment with probability 1/2. A header leaves the buffer only after
the commitments of the next `redownload_buffer_size` headers, about 25
of them, have matched, so a different chain gets a header stored with
probability about 2^-25. The memory cost is one bit per period
plus the buffer: about 1.4 MB for mainnet from genesis (chapter 71).
The number of commitments is also bounded by how many headers could
exist at 6 per second since the chain start, so a peer can't send an
endless chain either.

<!-- source: coax/lib/bitcoin/sync.coax hss -->
```jai
// HeadersSyncState (headerssync.cpp). The salt (k0, k1) and the
// commitment offset are random in Core; the caller draws them here.
// Commitments are kept one to a byte in the caller's memory, a FIFO;
// Core packs them into a bitdeque. The redownload buffer is a ring of
// CompressedHeader: a header without its previous hash, which is the
// hash of the header before it.
SY_PRESYNC :: 0; SY_REDOWNLOAD :: 1; SY_FINAL :: 2;
SY_STATE_NAMES :: [3] string.["PRESYNC", "REDOWNLOAD", "FINAL"];
SyCompressed :: struct { version: int; merkle_root: [32] u8; time: u32; bits: u32; nonce: u32; }
SyHss :: struct {
  state: int;
  offset: int; k0: u64; k1: u64;        // m_commit_offset and m_hasher's salt
  r: RetargetParams; period: int; buffer_size: int;
  start_height: int; start_hash: [32] u8; start_bits: u32; start_work: U256;
  min_work: U256; max_commitments: int;
  bits: [] u8; bits_head: int; bits_n: int;          // m_header_commitments
  work: U256; last_hash: [32] u8; last_bits: u32; height: int;   // PRESYNC's progress
  buf: [] SyCompressed; buf_head: int; buf_n: int;               // m_redownloaded_headers
  buf_last_height: int; buf_last_hash: [32] u8; buf_first_prev: [32] u8;
  redownload_work: U256; process_all: bool;
}
SyHssResult :: struct { success: bool; request_more: bool; released: int; }
SY_HSS_OK :: 0; SY_HSS_CLOCK :: 1; SY_HSS_MEMORY :: 2;
// The constructor. The bound on commitments is how many headers could
// exist at 6 a second from the chain start's median time past to now plus
// two hours (MAX_FUTURE_BLOCK_TIME); a clock more than that behind the
// chain start is Core's SystemClockError. Core's commitments grow to the
// bound; here they live in bits, and a chain that needs more fails the
// same way as one past the bound. buf holds buffer_size plus one message
// of headers.
sy_hss_init :: (s: [] SyHss, t: [] BlockTree, start: int, hp: SyHeadersParams, min_work: U256, now: int,
                offset: int, k0: u64, k1: u64, bits: [] u8, buf: [] SyCompressed) -> int {
  e := t[0].e[start];
  since := now - bi_median_time_past(t[0].l, t[0].e, start) + DS_MAX_FUTURE;
  if since < 0 { return SY_HSS_CLOCK; }
  x := SyHss.{state=SY_PRESYNC, offset=offset, k0=k0, k1=k1, r=t[0].p.retarget, period=hp.period, buffer_size=hp.buffer,
              start_height=t[0].l[start].height, start_hash=e.hash, start_bits=e.bits, start_work=e.work,
              min_work=min_work, work=e.work, last_hash=e.hash, last_bits=e.bits, height=t[0].l[start].height,
              bits=bits, buf=buf};
  x.max_commitments = 6 * since / hp.period;
  if bits.count == 0 || buf.count < hp.buffer + SY_MAX_HEADERS { return SY_HSS_MEMORY; }
  s[0] = x;
  return SY_HSS_OK;
}
// m_hasher(hash) & 1: SipHash-2-4 of the 32 hash bytes under the salt.
sy_commitment :: (s: [] SyHss, hash: [] u8) -> u8 { return cast,trunc(u8) (siphash24(s[0].k0, s[0].k1, hash) & 1); }
// Finalize: drop everything; the state can't be used again.
sy_hss_finalize :: (s: [] SyHss) {
  s[0].bits_n = 0; s[0].buf_n = 0; s[0].process_all = false; s[0].height = 0;
  s[0].state = SY_FINAL;
}
// ValidateAndProcessSingleHeader: PRESYNC checks only that the difficulty
// could have changed this way (chapter 48's PermittedDifficultyTransition)
// and keeps a commitment at heights offset mod period.
sy_presync_one :: (s: [] SyHss, raw: [] u8) -> bool {
  h := pow_header_read_vartime(raw);
  next := s[0].height + 1;
  if retarget_permitted_vartime(s[0].r, next, s[0].last_bits, h.bits) != RETARGET_OK { return false; }
  hash: [32] u8;
  #assert pow_block_hash_vartime(raw, hash[..]) == POW_OK;
  if next % s[0].period == s[0].offset {
    if s[0].bits_n == s[0].bits.count { return false; }
    s[0].bits[(s[0].bits_head + s[0].bits_n) % s[0].bits.count] = sy_commitment(s, hash[..]);
    s[0].bits_n += 1;
    if s[0].bits_n > s[0].max_commitments { return false; }   // the peer's chain is too long
  }
  s[0].work = u256_add(s[0].work, pow_work_vartime(h.bits));
  s[0].last_hash = hash; s[0].last_bits = h.bits; s[0].height = next;
  return true;
}
// ValidateAndStoreHeadersCommitments: the batch must continue the last
// header; once the work reaches the minimum, REDOWNLOAD starts again from
// the chain start.
sy_presync :: (s: [] SyHss, headers: [] u8, n: int) -> bool {
  for j: 0..<32 { if headers[4 + j] != s[0].last_hash[j] { return false; } }
  for k: 0..<n { if !sy_presync_one(s, headers[80 * k..<80 * k + 80]) { return false; } }
  if u256_compare_vartime(s[0].work, s[0].min_work) >= 0 {
    s[0].buf_n = 0; s[0].buf_last_height = s[0].start_height;
    s[0].buf_first_prev = s[0].start_hash; s[0].buf_last_hash = s[0].start_hash;
    s[0].redownload_work = s[0].start_work;
    s[0].state = SY_REDOWNLOAD;
  }
  return true;
}
// ValidateAndStoreRedownloadedHeader: the header must continue the
// buffer, change difficulty as permitted, and match its commitment, until
// the redownloaded work reaches the minimum; from then on (the target is
// reached) commitments are no longer checked.
sy_redownload_one :: (s: [] SyHss, raw: [] u8) -> bool {
  h := pow_header_read_vartime(raw);
  next := s[0].buf_last_height + 1;
  for j: 0..<32 { if h.prev[j] != s[0].buf_last_hash[j] { return false; } }
  prev_bits := s[0].start_bits;
  if s[0].buf_n > 0 { prev_bits = s[0].buf[(s[0].buf_head + s[0].buf_n - 1) % s[0].buf.count].bits; }
  if retarget_permitted_vartime(s[0].r, next, prev_bits, h.bits) != RETARGET_OK { return false; }
  s[0].redownload_work = u256_add(s[0].redownload_work, pow_work_vartime(h.bits));
  if u256_compare_vartime(s[0].redownload_work, s[0].min_work) >= 0 { s[0].process_all = true; }
  hash: [32] u8;
  #assert pow_block_hash_vartime(raw, hash[..]) == POW_OK;
  if !s[0].process_all && next % s[0].period == s[0].offset {
    if s[0].bits_n == 0 { return false; }              // commitment overrun
    want := s[0].bits[s[0].bits_head];
    s[0].bits_head = (s[0].bits_head + 1) % s[0].bits.count; s[0].bits_n -= 1;
    if sy_commitment(s, hash[..]) != want { return false; }   // commitment mismatch
  }
  if s[0].buf_n == s[0].buf.count { return false; }
  s[0].buf[(s[0].buf_head + s[0].buf_n) % s[0].buf.count] =
    SyCompressed.{version=h.version, merkle_root=h.merkle_root, time=h.time, bits=h.bits, nonce=h.nonce};
  s[0].buf_n += 1;
  s[0].buf_last_height = next; s[0].buf_last_hash = hash;
  return true;
}
// PopHeadersReadyForAcceptance: whatever is beyond the buffer, or all of
// it once the target is reached, rebuilt as full headers into out.
sy_pop_ready :: (s: [] SyHss, out: [] u8) -> int {
  n := 0;
  while s[0].buf_n > s[0].buffer_size || (s[0].buf_n > 0 && s[0].process_all) {
    c := s[0].buf[s[0].buf_head];
    h := BlockHeader.{version=c.version, prev=s[0].buf_first_prev, merkle_root=c.merkle_root, time=c.time, bits=c.bits, nonce=c.nonce};
    #assert 80 * (n + 1) <= out.count;
    raw := out[80 * n..<80 * n + 80];
    #assert pow_header_write_vartime(h, raw) && pow_block_hash_vartime(raw, s[0].buf_first_prev[..]) == POW_OK;
    s[0].buf_head = (s[0].buf_head + 1) % s[0].buf.count; s[0].buf_n -= 1;
    n += 1;
  }
  return n;
}
// ProcessNextHeaders: n headers, 80 bytes each; full says the message held
// 2,000. Headers released for storage go to out. Anything but success
// with more to request finalizes the state.
sy_hss_process :: (s: [] SyHss, headers: [] u8, n: int, full: bool, out: [] u8) -> SyHssResult {
  r: SyHssResult;
  if n == 0 || s[0].state == SY_FINAL { return r; }
  if s[0].state == SY_PRESYNC {
    r.success = sy_presync(s, headers, n);
    // A full message may have more; a switch to REDOWNLOAD starts over.
    // A short one in PRESYNC ends a chain with too little work.
    if r.success && (full || s[0].state == SY_REDOWNLOAD) { r.request_more = true; }
  } else if s[0].state == SY_REDOWNLOAD {
    r.success = true;
    for k: 0..<n { if !sy_redownload_one(s, headers[80 * k..<80 * k + 80]) { r.success = false; break; } }
    if r.success {
      r.released = sy_pop_ready(s, out);
      if !(s[0].buf_n == 0 && s[0].process_all) && full { r.request_more = true; }
    }
  }
  if !(r.success && r.request_more) { sy_hss_finalize(s); }
  return r;
}
// NextHeadersRequestLocator: the last header PRESYNC received, or the last
// one REDOWNLOAD stored, then the chain start's locator.
sy_hss_locator_vartime :: (s: [] SyHss, t: [] BlockTree, start: int, out: [] u8) -> int {
  if s[0].state == SY_FINAL { return -1; }
  first := ifx s[0].state == SY_PRESYNC then s[0].last_hash else s[0].buf_last_hash;
  return sy_getheaders_write_vartime(t, first[..], start, out);
}
```

Core keeps the commitments in a `bitdeque` and the buffer in a
`std::deque`. Here the caller passes their memory: one byte per
commitment, and a ring of 48-byte compressed headers. A header in the
buffer has no previous hash; it is the hash of the header before it.

## The headers message

`ProcessHeadersMessage` decides what each headers message does:

- If the peer has a presync in progress, the headers go to it. When it
  releases headers, they are stored as already validated.
- Headers whose parent is unknown get a `getheaders` from our best
  header in return, at most once every 2 minutes.
- Headers that end on our best chain, or come from a peer with the
  `noban` permission, skip the work check.
- Otherwise, if the work from the fork point to the last header is
  below the threshold: a full message of 2,000 starts a presync, and a
  shorter one is ignored, because that chain has nothing more to come.
- Everything else is stored with `AcceptBlockHeader` (chapter 59). A
  full message asks for more from its last header.

<!-- source: coax/lib/bitcoin/sync.coax headers -->
```jai
// One peer, as the headers code keeps it (Peer and CNodeState): its
// presync, if any, with the entry it started from, when we last asked it
// for headers (0: no request out), and the best block we know it has.
SyHeadersPeer :: struct {
  hss: [1] SyHss; syncing: bool; start: int;
  last_getheaders: int;
  best: int = -1;
  noban: bool;
}
// What a headers message did. accepted headers were stored; locator is a
// getheaders payload to send (locator_n bytes, 0 for none).
SY_H_STORED :: 0;        // headers stored in the block index
SY_H_PRESYNC :: 1;       // taken by a presync (none stored yet)
SY_H_LOW_WORK :: 2;      // "Ignoring low-work chain": too little work, nothing more to come
SY_H_UNCONNECTED :: 3;   // the first header's parent is unknown
SY_H_EMPTY :: 4;         // no headers: the peer has nothing more
SY_H_INVALID :: 5;       // Misbehaving: a header failed (reason)
SY_H_NAMES :: [6] string.["stored", "presync", "low work", "unconnected", "empty", "invalid"];
SyHeaders :: struct { code: int; reason: int; accepted: int; last: int; presync_failed: bool; locator_n: int; }
// GetAntiDoSWorkThreshold of chapter 71, at the active chain's tip.
sy_threshold :: (t: [] BlockTree, min_work: U256) -> U256 {
  tip := bi_tip(t);
  return ds_threshold(t[0].e[tip].work, t[0].e[tip].bits, min_work);
}
// MaybeSendGetHeaders: at most one request out per 2 minutes.
sy_maybe_getheaders :: (p: [] SyHeadersPeer, t: [] BlockTree, first: [] u8, from: int, now: int, out: [] u8) -> int {
  if p[0].last_getheaders != 0 && !ds_getheaders_due(p[0].last_getheaders, now) { return 0; }
  p[0].last_getheaders = now;
  return sy_getheaders_write_vartime(t, first, from, out);
}
// ProcessHeadersMessage for n headers (80 bytes each, in raw). hss_bits,
// hss_buf and scratch (room for buffer + 2,000 headers) back a presync.
sy_on_headers_vartime :: (p: [] SyHeadersPeer, t: [] BlockTree, raw: [] u8, n: int, now: int, min_work: U256,
                          salt: [3] u64, hss_bits: [] u8, hss_buf: [] SyCompressed, scratch: [] u8, out: [] u8) -> SyHeaders {
  r := SyHeaders.{last=-1};
  none: [1] u8;
  if n == 0 {
    if p[0].syncing { sy_hss_finalize(p[0].hss[..]); p[0].syncing = false; }
    p[0].last_getheaders = 0;
    r.code = SY_H_EMPTY;
    return r;
  }
  // CheckHeadersPoW: each header meets its own nBits, and each continues
  // the one before.
  hash: [32] u8;
  for k: 0..<n {
    h := raw[80 * k..<80 * k + 80];
    if pow_header_check_vartime(h, t[0].p.retarget.pow_limit) != POW_OK { r.code = SY_H_INVALID; r.reason = BI_HIGH_HASH; return r; }
    if k > 0 { for j: 0..<32 { if h[4 + j] != hash[j] { r.code = SY_H_INVALID; r.reason = BI_PREV_NOT_FOUND; return r; } } }
    #assert pow_block_hash_vartime(h, hash[..]) == POW_OK;
  }
  full := n == SY_MAX_HEADERS;
  headers := raw[0..<80 * n]; count := n;
  validated := false;
  // IsContinuationOfLowWorkHeadersSync.
  if p[0].syncing {
    res := sy_hss_process(p[0].hss[..], headers, count, full, scratch);
    if res.success { p[0].last_getheaders = 0; }
    if res.request_more { r.locator_n = sy_hss_locator_vartime(p[0].hss[..], t, p[0].start, out); p[0].last_getheaders = now; }
    if p[0].hss[0].state == SY_FINAL { p[0].syncing = false; }
    if res.success {
      headers = scratch[0..<80 * res.released]; count = res.released;
      validated = true;
      if count == 0 { r.code = SY_H_PRESYNC; return r; }
    } else { r.presync_failed = true; }
  }
  have_sync := p[0].syncing;
  start := bi_find(t, headers[4..<36]);
  if start < 0 {
    // HandleUnconnectingHeaders: ask from our best header, at most every 2 minutes.
    r.code = SY_H_UNCONNECTED;
    r.locator_n = sy_maybe_getheaders(p, t, none[0..<0], t[0].best_header, now, out);
    return r;
  }
  p[0].last_getheaders = 0;
  #assert pow_block_hash_vartime(headers[80 * (count - 1)..<80 * count], hash[..]) == POW_OK;
  known := bi_find(t, hash[..]);
  if known >= 0 && (bi_ancestor(t[0].l, t[0].best_header, t[0].l[known].height) == known || bi_contains(t, known)) { validated = true; }
  if p[0].noban { validated = true; }
  if !validated {
    // TryLowWorkHeadersSync: the claimed work from the chain start.
    total := t[0].e[start].work;
    for k: 0..<count { total = u256_add(total, pow_work_vartime(pow_header_read_vartime(headers[80 * k..<80 * k + 80]).bits)); }
    threshold := sy_threshold(t, min_work);
    if u256_compare_vartime(total, threshold) < 0 {
      if count != SY_MAX_HEADERS { r.code = SY_H_LOW_WORK; return r; }
      hp := sy_headers_params(t[0].p.chain);
      code := sy_hss_init(p[0].hss[..], t, start, hp, threshold, now, cast(int) (salt[2] % cast(u64) hp.period), salt[0], salt[1], hss_bits, hss_buf);
      #assert code == SY_HSS_OK;
      p[0].syncing = true; p[0].start = start;
      res := sy_hss_process(p[0].hss[..], headers, count, true, scratch);
      if res.success { p[0].last_getheaders = 0; }
      if res.request_more { r.locator_n = sy_hss_locator_vartime(p[0].hss[..], t, start, out); p[0].last_getheaders = now; }
      if p[0].hss[0].state == SY_FINAL { p[0].syncing = false; }
      r.code = SY_H_PRESYNC;
      return r;
    }
  }
  // ProcessNewBlockHeaders: AcceptBlockHeader for each, in order.
  for k: 0..<count {
    a := bi_accept_header_vartime(t, headers[80 * k..<80 * k + 80], now);
    if a.code != BI_OK { r.code = SY_H_INVALID; r.reason = a.code; return r; }
    r.last = a.index; r.accepted += 1;
  }
  // A full message, outside a presync: ask for more from the last.
  if full && !have_sync { r.locator_n = sy_maybe_getheaders(p, t, none[0..<0], r.last, now, out); }
  // UpdateBlockAvailability: the peer has the last header.
  if p[0].best < 0 || u256_compare_vartime(t[0].e[r.last].work, t[0].e[p[0].best].work) >= 0 { p[0].best = r.last; }
  r.code = SY_H_STORED;
  return r;
}
```

## Block download

Once the best header is known, the node asks every peer for blocks.
`FindNextBlocksToDownload` walks the peer's best chain from the last
block both sides have, skipping blocks already stored or in flight from
another peer. It never asks for a block more than 1,024 past that point
(`BLOCK_DOWNLOAD_WINDOW`), and a peer never has more than 16 blocks in
flight (`MAX_BLOCKS_IN_TRANSIT_PER_PEER`).

<!-- source: coax/lib/bitcoin/sync.coax download -->
```jai
// The download state over the block index: for each entry, whether its
// data is stored (BLOCK_HAVE_DATA), whether it and every ancestor have
// data (m_chain_tx_count != 0, here "linked"), and which peer has it in
// flight (mapBlocksInFlight). tip is the most-work linked entry.
SyPeer :: struct {
  live: bool; manual: bool;
  best: int = -1;              // pindexBestKnownBlock
  common: int = -1;            // pindexLastCommonBlock
  flight: [16] int; n: int;    // vBlocksInFlight, oldest first
  stalling_since: int;         // m_stalling_since, ms; 0 for none
  downloading_since: int;      // m_downloading_since, ms
  paused_until: int;           // m_block_download_paused_until (manual peers), ms
}
SyDownload :: struct {
  t: [] BlockTree;
  have: [] u8; linked: [] u8; owner: [] int;   // owner: the peer + 1, 0 for none
  tip: int;
  timeout: int;                // m_block_stalling_timeout, seconds
  min_work: U256;
  peers: [] SyPeer; downloading_from: int;     // m_peers_downloading_from
}
sy_dl_init :: (d: [] SyDownload, t: [] BlockTree, have: [] u8, linked: [] u8, owner: [] int, peers: [] SyPeer, min_work: U256) {
  for i: 0..<have.count { have[i] = 0; linked[i] = 0; owner[i] = 0; }
  d[0] = SyDownload.{t=t, have=have, linked=linked, owner=owner, tip=0, timeout=SY_STALL_DEFAULT, min_work=min_work, peers=peers};
  have[0] = 1; linked[0] = 1;   // genesis
}
```

<!-- source: coax/lib/bitcoin/sync.coax find -->
```jai
// FindNextBlocksToDownload: up to count blocks for peer p, and the peer
// that would be stalling us, or -1. The walk starts at the last block in
// common; it never goes past the peer's best block or more than the window
// beyond the last common block. Blocks with data are skipped, and once
// linked they move the last common block forward; blocks in flight from
// another peer are skipped, the first of them naming who we wait for.
SyFind :: struct { n: int; staller: int; }
sy_find_next_vartime :: (d: [] SyDownload, p: int, count: int, out: [] int) -> SyFind {
  f := SyFind.{staller=-1};
  t := d[0].t; l := t[0].l; e := t[0].e;
  peers := d[0].peers;
  best := peers[p].best;
  if count == 0 || best < 0 { return f; }
  if u256_compare_vartime(e[best].work, e[d[0].tip].work) < 0 || u256_compare_vartime(e[best].work, d[0].min_work) < 0 { return f; }
  fork := bi_last_common_ancestor(l, best, d[0].tip);
  if peers[p].common < 0 || u256_compare_vartime(e[fork].work, e[peers[p].common].work) > 0 || bi_ancestor(l, best, l[peers[p].common].height) != peers[p].common {
    peers[p].common = fork;
  }
  if peers[p].common == best { return f; }
  window_end := l[peers[p].common].height + SY_WINDOW;
  max_height := ifx l[best].height < window_end + 1 then l[best].height else window_end + 1;
  waiting := -1;
  h := l[peers[p].common].height + 1;
  while h <= max_height {
    i := bi_ancestor(l, best, h);
    h += 1;
    if e[i].status & BLOCK_FAILED_VALID != 0 { return f; }
    if d[0].have[i] != 0 {
      if d[0].linked[i] != 0 { peers[p].common = i; }
      continue;
    }
    if d[0].owner[i] != 0 {
      if waiting < 0 { waiting = d[0].owner[i] - 1; }
      continue;
    }
    if l[i].height > window_end {
      // We can't fetch anything, but could if the window were one larger.
      if f.n == 0 && waiting != p { f.staller = waiting; }
      return f;
    }
    out[f.n] = i; f.n += 1;
    if f.n == count { return f; }
  }
  return f;
}
```

The window is what lets the node notice a staller. If a peer has
nothing in flight and nothing it could request, but would have had a
block to request if the window were one block larger, then the window
is stuck. The first block in flight from another peer, at the start of
the window, is what holds it. That peer is the staller.

## Stalls and timeouts

`SendMessages` runs for each peer in turn. Its block part does three
things:

1. If this peer has been the staller for longer than the stalling
   timeout, disconnect it (or, for a peer added by hand, pause its
   downloads for 2 minutes and take back its blocks). Then double the
   timeout for the next staller, up to 64 seconds, so a slow link
   doesn't disconnect every peer in turn.
2. If the oldest block in flight from this peer has waited longer than
   10 minutes (the target block spacing) times 1 + 0.5 for each other
   peer downloading, disconnect it.
3. Otherwise request blocks up to 16 in flight, and if it found none to
   request but found a staller, start that peer's stall clock.

<!-- source: coax/lib/bitcoin/sync.coax step -->
```jai
// SendMessages for blocks, at time now (ms): first the stall check, then
// the download timeout, then new requests. The return says what became of
// the peer; requests go to out.
SY_OK :: 0; SY_STALLED :: 1; SY_TIMED_OUT :: 2; SY_PAUSED :: 3;
SyStep :: struct { verdict: int; n: int; staller: int; stall_started: bool; }
sy_request :: (d: [] SyDownload, p: int, i: int, now: int) {
  peers := d[0].peers;
  peers[p].flight[peers[p].n] = i; peers[p].n += 1;
  d[0].owner[i] = p + 1;
  if peers[p].n == 1 { peers[p].downloading_since = now; d[0].downloading_from += 1; }
}
sy_dl_step :: (d: [] SyDownload, p: int, now: int, out: [] int) -> SyStep {
  s := SyStep.{staller=-1};
  peers := d[0].peers;
  if !peers[p].live { return s; }
  if peers[p].stalling_since != 0 && peers[p].stalling_since < now - 1000 * d[0].timeout {
    // The window can't move and this peer holds its first block. A manual
    // peer is paused for two minutes, anyone else disconnected; the next
    // staller gets twice as long.
    if peers[p].manual {
      peers[p].paused_until = now + 120000;
      while peers[p].n > 0 { sy_unrequest(d, p, peers[p].flight[0], now); }
      s.verdict = SY_PAUSED;
    } else {
      sy_dl_drop(d, p);
      s.verdict = SY_STALLED;
    }
    d[0].timeout = ifx 2 * d[0].timeout < SY_STALL_MAX then 2 * d[0].timeout else SY_STALL_MAX;
    return s;
  }
  if peers[p].n > 0 {
    // A block in flight for block_interval * (1 + 0.5 * other peers downloading).
    others := d[0].downloading_from - 1;
    if now > peers[p].downloading_since + 1000 * d[0].t[0].p.retarget.spacing * (2 + others) / 2 {
      sy_dl_drop(d, p);
      s.verdict = SY_TIMED_OUT;
      return s;
    }
  }
  if now < peers[p].paused_until || peers[p].n >= SY_IN_FLIGHT { return s; }
  f := sy_find_next_vartime(d, p, SY_IN_FLIGHT - peers[p].n, out);
  for k: 0..<f.n { sy_request(d, p, out[k], now); }
  s.n = f.n; s.staller = f.staller;
  if peers[p].n == 0 && f.staller >= 0 && d[0].peers[f.staller].stalling_since == 0 {
    d[0].peers[f.staller].stalling_since = now;
    s.stall_started = true;
  }
  return s;
}
```

When a block arrives, its request is removed: the peer's next block
starts its download clock, and the peer is no longer stalling. If the
block's parent is linked, it links, along with every stored child after
it. Each linked block lowers the stalling timeout by 15%, down to 2
seconds, as each connected block does in Core.

<!-- source: coax/lib/bitcoin/sync.coax received -->
```jai
// RemoveBlockRequest: entry i is no longer in flight from peer p. The
// next block in its queue starts its download clock, and the peer is no
// longer stalling.
sy_unrequest :: (d: [] SyDownload, p: int, i: int, now: int) {
  peers := d[0].peers;
  k := 0;
  while k < peers[p].n && peers[p].flight[k] != i { k += 1; }
  if k == peers[p].n { return; }
  if k == 0 && now > peers[p].downloading_since { peers[p].downloading_since = now; }
  for j: k..<peers[p].n - 1 { peers[p].flight[j] = peers[p].flight[j + 1]; }
  peers[p].n -= 1;
  if peers[p].n == 0 { d[0].downloading_from -= 1; }
  peers[p].stalling_since = 0;
  d[0].owner[i] = 0;
}
// A peer disconnects: its requests go back to the pool.
sy_dl_drop :: (d: [] SyDownload, p: int) {
  peers := d[0].peers;
  for k: 0..<peers[p].n { d[0].owner[peers[p].flight[k]] = 0; }
  if peers[p].n > 0 { d[0].downloading_from -= 1; }
  peers[p].n = 0; peers[p].live = false;
}
// Link entry i and, along the best header chain, each child that already
// has data. Every block linked here is one Core would connect; each
// lowers the stalling timeout by 15% (BlockConnected), to no less than
// the default. Returns how many were linked.
sy_link :: (d: [] SyDownload, i: int) -> int {
  t := d[0].t; l := t[0].l; e := t[0].e;
  n := 0; j := i;
  while true {
    d[0].linked[j] = 1; n += 1;
    if u256_compare_vartime(e[j].work, e[d[0].tip].work) > 0 { d[0].tip = j; }
    x := d[0].timeout * 85 / 100;
    d[0].timeout = ifx x > SY_STALL_DEFAULT then x else SY_STALL_DEFAULT;
    best := t[0].best_header;
    if l[j].height >= l[best].height { break; }
    child := bi_ancestor(l, best, l[j].height + 1);
    if l[child].prev != j || d[0].have[child] == 0 { break; }
    j = child;
  }
  return n;
}
// A block arrived from peer p for entry i, already checked against its
// header. An unrequested block is ignored, as Core ignores one during
// initial block download. Returns how many blocks were linked.
sy_dl_received :: (d: [] SyDownload, p: int, i: int, now: int) -> int {
  if d[0].owner[i] != p + 1 { return -1; }
  sy_unrequest(d, p, i, now);
  d[0].have[i] = 1;
  prev := d[0].t[0].l[i].prev;
  if prev >= 0 && d[0].linked[prev] == 0 { return 0; }
  return sy_link(d, i);
}
// The blocks below entry i are taken as stored: a run that starts its
// window there, as Core's AssumeUTXO chainstate starts from its snapshot
// (chapter 79). Nothing below is downloaded or checked.
sy_dl_assume :: (d: [] SyDownload, i: int) {
  l := d[0].t[0].l;
  j := i;
  while j >= 0 { d[0].have[j] = 1; d[0].linked[j] = 1; j = l[j].prev; }
  d[0].tip = i;
}
```

## Checking a block

A block is accepted from a peer only if it was requested. Its header
must hash to the hash it was requested by, and its transactions must
hash to the header's Merkle root with no duplicated subtree, the flaw
of chapter 46. A block that fails either is the peer's fault
(`MaybePunishNodeForBlock` in chapter 71): `BLOCK_INVALID_HEADER` or
`BLOCK_MUTATED`, and the peer is disconnected.

<!-- source: coax/lib/bitcoin/sync.coax block -->
```jai
// The check a downloaded block gets here: its header is the one we asked
// for, and its transactions hash to the header's Merkle root without a
// duplicated subtree (chapter 46). Returns 0, or chapter 71's
// BlockValidationResult for the failure.
sy_check_block_vartime :: (b: [] u8, want: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, txids: [] u8, wtxids: [] u8, work: [] u8) -> int {
  if b.count < 80 { return DS_BLOCK_CONSENSUS; }
  hash: [32] u8;
  #assert pow_block_hash_vartime(b[0..<80], hash[..]) == POW_OK;
  for j: 0..<32 { if hash[j] != want[j] { return DS_BLOCK_INVALID_HEADER; } }
  k := merkle_block_ids_vartime(b, false, ins, outs, items, txids, wtxids);
  if k.code != MERKLE_OK || k.count == 0 { return DS_BLOCK_CONSENSUS; }
  m := merkle_check_vartime(txids[0..<32 * k.count], work, b[36..<68]);
  if m == MERKLE_MISMATCH || m == MERKLE_MUTATED { return DS_BLOCK_MUTATED; }
  return ifx m == MERKLE_OK then 0 else DS_BLOCK_CONSENSUS;
}
```

## The example

[bitcoin/examples/headers-sync.coax](../examples/headers-sync.coax)
syncs simulated peers, replays Core's `headers_sync_chainwork_tests` at
full size and Core's `p2p_ibd_stalling.py` scenario, runs four peers of
different speeds through the window, and checks blocks against their
headers:

<!-- source: bitcoin/examples/headers-sync.coax example -->
```jai
#load "../../coax/lib/bitcoin/sync.coax";
#import "Basic";

SALT :: [3] u64.[0x0123456789abcdef, 0xfedcba9876543210, 7];   // Core draws these at random

hex_digit :: (x: int) -> string { return "0123456789abcdef"[x..<x + 1]; }
// The first 8 hex digits of a hash as explorers show it (bytes reversed).
short :: (h: [] u8) { i := 31; while i >= 28 { print("%", hex_digit(cast(int) h[i] >> 4)); print("%", hex_digit(cast(int) h[i] & 15)); i -= 1; } }
yes :: (b: bool) -> string { return ifx b then "ok" else "failed"; }

// ---- a peer: Bitcoin Core answering getheaders from its chain
Peer :: struct { raw: [] u8; hashes: [] u8; height: int; }
peer_chain :: (p: [] Peer, raw: [] u8, hashes: [] u8, genesis: [] u8, n: int, root: u8) {
  sy_t_chain(genesis, 4, REGTEST_TIME, root, n, raw);
  for j: 0..<32 { hashes[j] = genesis[j]; }
  for k: 0..<n { ok := pow_block_hash_vartime(raw[80 * k..<80 * k + 80], hashes[32 * (k + 1)..<32 * (k + 2)]); }
  p[0] = Peer.{raw=raw, hashes=hashes, height=n};
}
// The headers after the fork the locator finds: at most 2,000.
answer :: (p: [] Peer, getheaders: [] u8) -> [2] int {
  loc := p2p_locator_read_vartime(getheaders);
  fork := sy_find_fork_vartime(p[0].hashes, p[0].height, getheaders[loc.first..<loc.stop], loc.count);
  n := p[0].height - fork;
  if n > SY_MAX_HEADERS { n = SY_MAX_HEADERS; }
  return [2] int.[fork, n];
}
REGTEST_TIME :: 1296688602;

// ---- headers sync from genesis
Node :: struct {
  t: [1] BlockTree; l: [2600] BiLink; e: [2600] BiEntry; slots: [8192] int; chain: [2600] int;
  bits: [64] u8; buf: [9017] SyCompressed; scratch: [80 * 9017] u8;
}
sync_from :: (label: string, node: [] Node, p: [] Peer, min_work: U256, swap: [] Peer) {
  print("  %\n", label);
  hp := [1] SyHeadersPeer.[SyHeadersPeer.{}];
  getheaders: [4096] u8; none: [1] u8;
  t := node[0].t[..];
  n := sy_getheaders_write_vartime(t, none[0..<0], 0, getheaders[..]);
  round := 0; swapped := false;
  while n > 0 {
    round += 1;
    // The swapping peer serves its other chain from the time REDOWNLOAD starts.
    if swap.count > 0 && hp[0].syncing && hp[0].hss[0].state == SY_REDOWNLOAD { swapped = true; }
    from := ifx swapped then swap else p;
    a := answer(from, getheaders[0..<n]);
    r := sy_on_headers_vartime(hp[..], t, from[0].raw[80 * a[0]..<80 * (a[0] + a[1])], a[1], SY_NOW, min_work, SALT,
                               node[0].bits[..], node[0].buf[..], node[0].scratch[..], getheaders[..]);
    print("    %. % headers after %: %", round, a[1], a[0], SY_H_NAMES[r.code]);
    if r.presync_failed { print(" (the presync failed; these headers start another)"); }
    if r.code == SY_H_PRESYNC {
      s := hp[0].hss[0];
      if s.state == SY_PRESYNC { print(", PRESYNC at %, % commitments", s.height, s.bits_n); }
      else if s.state == SY_REDOWNLOAD { print(", REDOWNLOAD, % waiting, % commitments left", s.buf_n, s.bits_n); }
      else { print(", FINAL"); }
    }
    if r.code == SY_H_STORED { print(" %, best header % (", r.accepted, t[0].l[t[0].best_header].height); short(t[0].e[t[0].best_header].hash[..]); print(")"); }
    print("\n");
    n = r.locator_n;
  }
  print("    block index: % entries\n", t[0].count);
}
headers_sync :: () {
  print("headers sync from genesis (regtest, minimum chain work of 2200 blocks)\n");
  node: [1] Node;
  bi_regtest(node[0].t[..], node[0].l[..], node[0].e[..], node[0].slots[..], node[0].chain[..]);
  g := node[0].e[0].hash;
  min_work := u256_small(2 * 2200);
  raw: [80 * 2500] u8; hashes: [32 * 2501] u8; p: [1] Peer;
  peer_chain(p[..], raw[..], hashes[..], g[..], 2500, 0);
  none: [1] Peer;
  sync_from("an honest peer with 2500 blocks", node[..], p[..], min_work, none[0..<0]);
  low_raw: [80 * 2100] u8; low_hashes: [32 * 2101] u8; low: [1] Peer;
  peer_chain(low[..], low_raw[..], low_hashes[..], g[..], 2100, 1);
  sync_from("a peer with 2100 blocks, too little work", node[..], low[..], min_work, none[0..<0]);
  b_raw: [80 * 2500] u8; b_hashes: [32 * 2501] u8; b: [1] Peer;
  c_raw: [80 * 2100] u8; c_hashes: [32 * 2101] u8; c: [1] Peer;
  peer_chain(b[..], b_raw[..], b_hashes[..], g[..], 2500, 2);
  peer_chain(c[..], c_raw[..], c_hashes[..], g[..], 2100, 3);
  sync_from("a peer that presyncs 2500 blocks, then serves 2100 others", node[..], b[..], min_work, c[..]);
}

// ---- Core's headers_sync_chainwork_tests, at their size
CORE_TARGET :: 15000;
B :: CORE_TARGET - (2000 + 123);
CORE_HP :: SyHeadersParams.{period=600, buffer=B};
Core :: struct { chain: [80 * (CORE_TARGET - 1)] u8; bits: [64] u8; buf: [CORE_TARGET + SY_MAX_HEADERS] SyCompressed; out: [80 * CORE_TARGET] u8; }
init :: (s: [] SyHss, t: [] BlockTree, c: [] Core, now: int) -> int {
  return sy_hss_init(s, t, 0, CORE_HP, u256_small(CORE_TARGET * 2), now, cast(int) SALT[2], SALT[0], SALT[1], c[0].bits[..], c[0].buf[..]);
}
step :: (s: [] SyHss, r: SyHssResult) {
  print(" %, %", SY_STATE_NAMES[s[0].state], yes(r.success));
  if r.request_more { print(", more"); }
  if r.released > 0 { print(", % released", r.released); }
  print(";");
}
core_tests :: () {
  print("Core's headers_sync_chainwork_tests (15000 blocks, a commitment every 600, a buffer of 12877)\n");
  l: [4] BiLink; e: [4] BiEntry; slots: [8] int; chain: [4] int; t: [1] BlockTree;
  bi_regtest(t[..], l[..], e[..], slots[..], chain[..]);
  c: [1] Core; s: [1] SyHss;
  work := u256_small(CORE_TARGET * 2);
  first := c[0].chain[..];
  sy_t_chain(e[0].hash[..], 1, e[0].time, 0, CORE_TARGET - 1, first);
  print("  sneaky_redownload:");
  ok := init(s[..], t[..], c[..], SY_NOW);
  step(s[..], sy_hss_process(s[..], first[0..<80], 1, true, c[0].out[..]));
  step(s[..], sy_hss_process(s[..], first[80..<first.count], CORE_TARGET - 2, true, c[0].out[..]));
  sy_t_chain(e[0].hash[..], 1, e[0].time, 1, CORE_TARGET - 2, first);    // the second chain
  step(s[..], sy_hss_process(s[..], first[0..<80 * (CORE_TARGET - 2)], CORE_TARGET - 2, true, c[0].out[..]));
  print("\n");
  sy_t_chain(e[0].hash[..], 1, e[0].time, 0, CORE_TARGET - 1, first);
  for full: 0..<2 {
    print("  happy_path, % messages:", ifx full == 1 then "full" else "short");
    ok = init(s[..], t[..], c[..], SY_NOW);
    step(s[..], sy_hss_process(s[..], first, CORE_TARGET - 1, full == 1, c[0].out[..]));
    step(s[..], sy_hss_process(s[..], first[0..<80 * B], B, true, c[0].out[..]));
    step(s[..], sy_hss_process(s[..], first[80 * B..<80 * (B + 1)], 1, true, c[0].out[..]));
    r := sy_hss_process(s[..], first[80 * (B + 1)..<first.count], CORE_TARGET - 2 - B, full == 1, c[0].out[..]);
    step(s[..], r);
    h: [32] u8;
    print(" the last released is "); short(sy_t_hash(c[0].out[80 * (r.released - 1)..<80 * r.released], h[..]));
    print("\n");
  }
  print("  too_little_work:");
  sy_t_chain(e[0].hash[..], 1, e[0].time, 1, CORE_TARGET - 2, first);
  ok = init(s[..], t[..], c[..], SY_NOW);
  step(s[..], sy_hss_process(s[..], first[0..<80], 1, true, c[0].out[..]));
  step(s[..], sy_hss_process(s[..], first[80..<80 * (CORE_TARGET - 2)], CORE_TARGET - 3, false, c[0].out[..]));
  print("\n");
  at := cast(int) e[0].time - DS_MAX_FUTURE;
  print("  system_clock_lagging_behind_chain_start: 2 hours before genesis %, a second more %\n",
    ifx init(s[..], t[..], c[..], at) == SY_HSS_OK then "starts" else "refuses",
    ifx init(s[..], t[..], c[..], at - 1) == SY_HSS_CLOCK then "is SystemClockError" else "starts");
}

// ---- the download window
Sim :: struct {
  t: [1] BlockTree; l: [3100] BiLink; e: [3100] BiEntry; slots: [8192] int; chain: [3100] int; raw: [80 * 3000] u8;
  have: [3100] u8; linked: [3100] u8; owner: [3100] int; d: [1] SyDownload;
}
sim_init :: (m: [] Sim, n: int, peers: [] SyPeer) -> int {
  tip := sy_t_tree(m[0].t[..], m[0].l[..], m[0].e[..], m[0].slots[..], m[0].chain[..], n, m[0].raw[..]);
  sy_dl_init(m[0].d[..], m[0].t[..], m[0].have[..], m[0].linked[..], m[0].owner[..], peers, U256.{});
  for p: 0..<peers.count { peers[p].live = true; }
  return tip;
}
settle :: (m: [] Sim, now: int, withheld: [] int, label: string) {
  s := sy_t_settle(m[0].d[..], now, withheld);
  d := m[0].d[..];
  print("    %: % requested, tip %, % in flight, timeout % s", label, s.requested, m[0].l[d[0].tip].height, sy_t_in_flight(d), d[0].timeout);
  if s.stalls > 0 { print(", stall started"); }
  if s.dropped > 0 { print(", peer % disconnected for stalling", s.last_dropped); }
  print("\n");
}
stalling :: () {
  print("Core's p2p_ibd_stalling: 1025 blocks, 5 peers, blocks 1 and 501 withheld\n");
  m: [1] Sim; peers: [5] SyPeer;
  tip := sim_init(m[..], 1025, peers[..]);
  l := m[0].l[..];
  stall := [2] int.[bi_ancestor(l, tip, 1), bi_ancestor(l, tip, 501)];
  now := 1000000;
  asked := 0;
  for p: 0..<5 { peers[p].best = bi_ancestor(l, tip, 1024); s := sy_t_settle(m[0].d[..], now, stall[..]); asked += s.requested; }
  print("    headers to 1024 from each peer in turn: % requested, tip %, % in flight (blocks 1 and 501, both from peer %)\n",
    asked, l[m[0].d[0].tip].height, sy_t_in_flight(m[0].d[..]), m[0].owner[stall[1]] - 1);
  now += 3000; settle(m[..], now, stall[..], "3 s later");
  for p: 0..<5 { peers[p].best = tip; }
  settle(m[..], now, stall[..], "headers to 1025");
  waits := [6] int.[3000, 3000, 2000, 7000, 2000, 0];
  for k: 0..<5 { now += waits[k]; settle(m[..], now, stall[..], ifx waits[k] == 2000 then "2 s later" else ifx waits[k] == 3000 then "3 s later" else "7 s later"); }
  holder := m[0].owner[stall[0]] - 1;
  linked := sy_dl_received(m[0].d[..], holder, stall[0], now);
  print("    block 1 arrives from peer %: % blocks link, tip %, timeout % s\n", holder, linked, l[m[0].d[0].tip].height, m[0].d[0].timeout);
}
// Four peers that serve one block every 4, 6, 9 and 3000 ms.
parallel :: () {
  print("3000 blocks from 4 peers serving a block every 4, 6, 9 and 3000 ms\n");
  m: [1] Sim; peers: [4] SyPeer;
  tip := sim_init(m[..], 3000, peers[..]);
  d := m[0].d[..]; l := m[0].l[..];
  for p: 0..<4 { peers[p].best = tip; }
  pace := [4] int.[4, 6, 9, 3000]; next: [4] int; served: [4] int;
  out: [16] int; lead := 0; t := 0; starts := 0; shown := -1;
  while d[0].tip != tip && t < 600000 {
    for p: 0..<4 {
      if peers[p].live && peers[p].n > 0 && next[p] <= t {
        got := sy_dl_received(d, p, peers[p].flight[0], t);
        served[p] += 1; next[p] = t + pace[p];
      }
    }
    for p: 0..<4 {
      was := peers[p].n;
      s := sy_dl_step(d, p, t, out[..]);
      if s.verdict == SY_STALLED { print("    at % ms: peer % disconnected for stalling at tip %; timeout now % s\n", t, p, l[d[0].tip].height, d[0].timeout); }
      if s.stall_started {
        starts += 1;
        if s.staller != shown { shown = s.staller; print("    at % ms: the window is full; peer % holds block %, stall started\n", t, s.staller, l[peers[s.staller].flight[0]].height); }
      }
      if was == 0 && peers[p].n > 0 { next[p] = t + pace[p]; }
      for k: 0..<s.n { ahead := l[out[k]].height - l[d[0].tip].height; if ahead > lead { lead = ahead; } }
    }
    t += 1;
  }
  print("    done at % ms, tip %; blocks from each peer: % % % %; % stalls started; the furthest request was % blocks past the tip\n",
    t, l[d[0].tip].height, served[0], served[1], served[2], served[3], starts, lead);
}
// A block checked against the header it was asked for.
checks :: () {
  print("blocks against their headers\n");
  g: [CHAIN_GENESIS_MAX] u8; b: [1024] u8; hash: [32] u8;
  n := chain_genesis_block_vartime(CHAIN_REGTEST_PARAMS.genesis, g[..]);
  ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem; ids: [32 * 4] u8; wids: [32 * 4] u8; work: [32 * 4] u8;
  names := [9] string.["ok", "BLOCK_CONSENSUS", "BLOCK_CACHED_INVALID", "BLOCK_INVALID_HEADER", "BLOCK_MUTATED", "", "", "", ""];
  r := sy_check_block_vartime(g[0..<n], sy_t_hash(g[0..<80], hash[..]), ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  print("  regtest genesis, % bytes: %\n", n, names[r]);
  for k: 0..<n { b[k] = g[k]; }
  b[36] ^= 1;
  r = sy_check_block_vartime(b[0..<n], hash[..], ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  print("  its Merkle root changed, against the header asked for: %\n", names[r]);
  r = sy_check_block_vartime(b[0..<n], sy_t_hash(b[0..<80], hash[..]), ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  print("  the same, asked for by its own hash: %\n", names[r]);
  tx := g[81..<n];
  for k: 0..<80 { b[k] = g[k]; }
  b[80] = 2;
  for k: 0..<tx.count { b[81 + k] = tx[k]; b[81 + tx.count + k] = tx[k]; }
  ok := merkle_parent_vartime(g[36..<68], g[36..<68], b[36..<68]);
  r = sy_check_block_vartime(b[0..<81 + 2 * tx.count], sy_t_hash(b[0..<80], hash[..]), ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  print("  its coinbase twice, under the pair's root: %\n", names[r]);
  print("  punished: % for a mutated block, % for a wrong header\n",
    ifx ds_punish_block(DS_BLOCK_MUTATED, false, false) then "misbehaving" else "no", ifx ds_punish_block(DS_BLOCK_INVALID_HEADER, false, false) then "misbehaving" else "no");
}

main :: () {
  headers_sync();
  core_tests();
  print("block download\n");
  stalling();
  parallel();
  checks();
}
```

```shell
coax/bin/coax run bitcoin/examples/headers-sync.coax
```

```text
headers sync from genesis (regtest, minimum chain work of 2200 blocks)
  an honest peer with 2500 blocks
    1. 2000 headers after 0: presync, PRESYNC at 2000, 8 commitments
    2. 500 headers after 2000: presync, REDOWNLOAD, 0 waiting, 10 commitments left
    3. 2000 headers after 0: presync, REDOWNLOAD, 2000 waiting, 2 commitments left
    4. 500 headers after 2000: stored 2500, best header 2500 (0417a8f5)
    block index: 2501 entries
  a peer with 2100 blocks, too little work
    1. 2000 headers after 0: presync, PRESYNC at 2000, 8 commitments
    2. 100 headers after 2000: presync, FINAL
    block index: 2501 entries
  a peer that presyncs 2500 blocks, then serves 2100 others
    1. 2000 headers after 0: presync, PRESYNC at 2000, 8 commitments
    2. 500 headers after 2000: presync, REDOWNLOAD, 0 waiting, 10 commitments left
    3. 2000 headers after 0: presync (the presync failed; these headers start another), PRESYNC at 2000, 8 commitments
    4. 100 headers after 2000: presync, FINAL
    block index: 2501 entries
Core's headers_sync_chainwork_tests (15000 blocks, a commitment every 600, a buffer of 12877)
  sneaky_redownload: PRESYNC, ok, more; REDOWNLOAD, ok, more; FINAL, failed;
  happy_path, short messages: REDOWNLOAD, ok, more; REDOWNLOAD, ok, more; REDOWNLOAD, ok, more, 1 released; FINAL, ok, 14998 released; the last released is 2db18f12
  happy_path, full messages: REDOWNLOAD, ok, more; REDOWNLOAD, ok, more; REDOWNLOAD, ok, more, 1 released; FINAL, ok, 14998 released; the last released is 2db18f12
  too_little_work: PRESYNC, ok, more; FINAL, ok;
  system_clock_lagging_behind_chain_start: 2 hours before genesis starts, a second more is SystemClockError
block download
Core's p2p_ibd_stalling: 1025 blocks, 5 peers, blocks 1 and 501 withheld
    headers to 1024 from each peer in turn: 1024 requested, tip 0, 2 in flight (blocks 1 and 501, both from peer 0)
    3 s later: 0 requested, tip 0, 2 in flight, timeout 2 s
    headers to 1025: 0 requested, tip 0, 2 in flight, timeout 2 s, stall started
    3 s later: 2 requested, tip 0, 2 in flight, timeout 4 s, stall started, peer 0 disconnected for stalling
    3 s later: 0 requested, tip 0, 2 in flight, timeout 4 s
    2 s later: 2 requested, tip 0, 2 in flight, timeout 8 s, stall started, peer 1 disconnected for stalling
    7 s later: 0 requested, tip 0, 2 in flight, timeout 8 s
    2 s later: 2 requested, tip 0, 2 in flight, timeout 16 s, stall started, peer 2 disconnected for stalling
    block 1 arrives from peer 3: 500 blocks link, tip 500, timeout 2 s
3000 blocks from 4 peers serving a block every 4, 6, 9 and 3000 ms
    at 1976 ms: the window is full; peer 3 holds block 49, stall started
    at 5001 ms: peer 3 disconnected for stalling at tip 49; timeout now 4 s
    at 5002 ms: the window is full; peer 0 holds block 50, stall started
    done at 8762 ms, tip 3000; blocks from each peer: 1417 943 639 1; 17 stalls started; the furthest request was 1024 blocks past the tip
blocks against their headers
  regtest genesis, 285 bytes: ok
  its Merkle root changed, against the header asked for: BLOCK_INVALID_HEADER
  the same, asked for by its own hash: BLOCK_MUTATED
  its coinbase twice, under the pair's root: BLOCK_MUTATED
  punished: misbehaving for a mutated block, misbehaving for a wrong header
```

Its output, 45 lines, is
[bitcoin/fixtures/headers-sync.out](../fixtures/headers-sync.out). Some
things to read in it:

- The honest peer's 2,500 headers arrive twice: 4,500 headers in four
  messages, the first two presynced and the last two redownloaded. The
  minimum chain work of 2,200 blocks is reached at height 2,200, so the
  commitments at 2,207 and later are never checked, and all 2,500
  headers are released by the last message.
- The peer with 2,100 blocks never reaches the threshold. Its short
  second message ends the presync, and nothing is stored.
- The third peer presyncs one chain and then serves another during
  REDOWNLOAD. A commitment fails, the presync ends, and its headers
  start a new presync, which ends like the second peer's.
- In the stalling scenario, peer 0 holds blocks 1 and 501. The window
  starts at 1 and ends at 1,024; once the peers announce 1,025, the
  node can't request block 1,025, so peer 0 is the staller. Peers 0, 1
  and 2 are disconnected in turn with timeouts of 2, 4 and 8 seconds.
  When block 1 arrives from peer 3, blocks 1 to 500 link at once, and
  the 500 linked blocks bring the timeout back to 2 seconds. This is
  what Core's `p2p_ibd_stalling.py` checks.
- With four peers serving a block every 4, 6, 9 and 3,000 ms, the slow
  peer holds the window after about 2 seconds and is disconnected 3
  seconds later. After that the fastest peer is sometimes the staller
  for a moment, but each block it delivers resets its stall clock, as
  in Core, so it is never disconnected.

## Syncing from Bitcoin Core

[bitcoin/examples/live/sync-node.coax](../examples/live/sync-node.coax)
is the same code with sockets. It opens v1 connections to Bitcoin Core
peers, does the version handshake, syncs headers from the first peer
that answers, and then downloads blocks from every peer with
`getdata` for witness blocks. Each block is checked against its header
and appended to a file. It is a tool, built with
[coax/web/tool.sh](../../coax/web/tool.sh), because it needs sockets and
a large stack for a block index of 600,000 entries.

[coax/tests/bitcoin/sync_live.py](../../coax/tests/bitcoin/sync_live.py)
runs it:

```shell
python3 -I coax/tests/bitcoin/sync_live.py --bitcoind PATH/bitcoind regtest
python3 -I coax/tests/bitcoin/sync_live.py signet --blocks 200
```

### Regtest

The script starts Bitcoin Core 32.1 in a temporary data directory,
mines 2,500 blocks, sends 500 wallet transactions in the last 100, and
runs the node three times against four connections to it. Regtest's
minimum chain work is zero, so the first run passes `--min-work 4400`
(2,200 blocks of regtest work) to make the headers go through presync,
as `-minimumchainwork` does for Core. After each run the script reads
every stored block back and compares it byte for byte with bitcoind's
`getblock`. On 2026-11-03:

```text
bitcoind 8674: regtest in $TMPDIR/ch72-regtest-mrs00wea, P2P port 51971
/Satoshi:32.1.0/: 2500 blocks mined, 500 wallet transactions in the last 100, tip 0b615e14e80c5412

--- regtest, presync: minimum chain work 4400
sync-node: regtest, genesis 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206, minimum chain work 4400
  peer 0: 127.0.0.1:51971 connected
  peer 1: 127.0.0.1:51971 connected
  peer 2: 127.0.0.1:51971 connected
  peer 3: 127.0.0.1:51971 connected
  4 peers ready, 0 ms
    headers message 1: 2000 headers, presync, PRESYNC at height 2000, 7 commitments
    headers message 2: 500 headers, presync, REDOWNLOAD at height 0, 9 commitments left, 0 waiting
    headers message 4: 500 headers, stored, 2500 stored so far, best header 2500
  headers: 4 messages, 4500 presynced, 2500 stored in 76 ms; best header 2500 (0b615e14e80c5412ee9f20f4d8e2000489ea16a38e85972d468383be48812f5e)
  blocks: 2500 to fetch, heights 1 to 2500, from peer 0 (best 2500) peer 1 (best 2500) peer 2 (best 2500) peer 3 (best 2500)
    500 blocks stored, tip 500, 42 ms
    1000 blocks stored, tip 1000, 66 ms
    1500 blocks stored, tip 1500, 90 ms
    2000 blocks stored, tip 2000, 113 ms
    2500 blocks stored, tip 2500, 139 ms
  blocks: 2500 stored (831127 bytes) in 139 ms, tip 2500 (0b615e14e80c5412ee9f20f4d8e2000489ea16a38e85972d468383be48812f5e); from each peer: 625 625 625 625; 0 stalls started, 0 invalid
check: 2500 blocks stored, 2500 byte for byte equal to getblock, tip 0b615e14e80c5412: True

--- regtest, refused: minimum chain work 6000
sync-node: regtest, genesis 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206, minimum chain work 6000
  peer 0: 127.0.0.1:51971 connected
  peer 1: 127.0.0.1:51971 connected
  peer 2: 127.0.0.1:51971 connected
  peer 3: 127.0.0.1:51971 connected
  4 peers ready, 0 ms
    headers message 1: 2000 headers, presync, PRESYNC at height 2000, 7 commitments
    headers message 2: 500 headers, presync, the presync ended
  headers: 2 messages, 2500 presynced, 0 stored in 33 ms; best header 0 (0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206)
  the peer's chain has too little work: nothing stored, no blocks to fetch
check: refused with nothing stored: True

--- regtest, Core default: minimum chain work of chainparams (0)
sync-node: regtest, genesis 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206, minimum chain work 0
  peer 0: 127.0.0.1:51971 connected
  peer 1: 127.0.0.1:51971 connected
  peer 2: 127.0.0.1:51971 connected
  peer 3: 127.0.0.1:51971 connected
  4 peers ready, 0 ms
    headers message 1: 2000 headers, stored, 2000 stored so far, best header 2000
  headers: 2 messages, 0 presynced, 2500 stored in 34 ms; best header 2500 (0b615e14e80c5412ee9f20f4d8e2000489ea16a38e85972d468383be48812f5e)
  blocks: 2500 to fetch, heights 1 to 2500, from peer 0 (best 2500) peer 1 (best 2500) peer 2 (best 2500) peer 3 (best 2500)
    500 blocks stored, tip 500, 25 ms
    1000 blocks stored, tip 1000, 48 ms
    1500 blocks stored, tip 1500, 72 ms
    2000 blocks stored, tip 2000, 94 ms
    2500 blocks stored, tip 2500, 120 ms
  blocks: 2500 stored (831127 bytes) in 120 ms, tip 2500 (0b615e14e80c5412ee9f20f4d8e2000489ea16a38e85972d468383be48812f5e); from each peer: 625 625 625 625; 0 stalls started, 0 invalid
check: 2500 blocks stored, 2500 byte for byte equal to getblock, tip 0b615e14e80c5412: True
bitcoind 8674 stopped
live acceptance: passed
```

- With a minimum of 2,200 blocks, the headers go through PRESYNC and
  REDOWNLOAD and all 2,500 are stored. All 2,500 blocks come from the
  four connections and equal bitcoind's.
- With a minimum of 3,000 blocks, more than the chain has, the presync
  ends with nothing stored and no blocks requested.
- With Core's regtest minimum of zero, the headers are stored at once.

### Signet

Signet's minimum chain work is above the work of its first headers, so
every header from genesis goes through presync, against peers on the
public network. The script resolves Core's signet DNS seeds, runs the
node against up to 12 of them, downloads the last 200 blocks, and
compares the best header with the block mempool.space shows at that
height. The blocks below those 200 are taken as stored, the way an
AssumeUTXO chainstate starts above its snapshot (chapter 79): the full
signet chain is many gigabytes, and nothing here validates it. On
2026-11-03:

```text
signet seeds: 25 IPv4 addresses
sync-node: signet, genesis 00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6, minimum chain work 00000000000000000000000000000000000000000000000000001090e9dc1520
  peer 0: 78.40.194.29:38333 connected
  peer 1: 51.195.190.15:38333 connected
  peer 2: 195.192.213.241:38333 connected
  peer 3: 168.119.11.220:38333 connected
  peer 4: 95.141.35.117:38333 connected
  peer 5: 153.126.143.201:38333 connected
  peer 6: 152.53.151.148:38333 connected
  peer 7: 173.201.39.135:38333 unreachable
  peer 8: 71.71.231.69:38333 connected
  peer 9: 174.138.77.43:38333 connected
  peer 10: 198.206.204.71:38333 connected
  peer 11: 77.164.109.49:38333 connected
  11 peers ready, 371 ms
    headers message 1: 2000 headers, presync, PRESYNC at height 2000, 3 commitments
    headers message 100: 2000 headers, presync, PRESYNC at height 200000, 318 commitments
    headers message 161: 2000 headers, presync, REDOWNLOAD at height 0, 512 commitments left, 0 waiting
    headers message 200: 2000 headers, stored, REDOWNLOAD at height 78000, 388 commitments left, 15885 waiting, 62115 stored so far, best header 62115
    headers message 300: 2000 headers, stored, REDOWNLOAD at height 278000, 70 commitments left, 15885 waiting, 262115 stored so far, best header 262115
    headers message 322: 2000 headers, stored, 322000 stored so far, best header 322000
  headers: 326 messages, 336000 presynced, 329283 stored in 105539 ms; best header 329283 (0000000aa9bbf4a9966fc430f45206b421d6cecdbe7de118fc471b0a98e1857c)
  blocks: 200 to fetch, heights 329084 to 329283, from peer 0 (best 329283) peer 1 (best 329283) peer 2 (best 329283) peer 3 (best 329283) peer 4 (best 329283) peer 5 (best 329283) peer 6 (best 329283) peer 8 (best 329283) peer 9 (best 329283) peer 10 (best 329283) peer 11 (best 329283)
    40 blocks stored, tip 329084, 689 ms
    80 blocks stored, tip 329086, 901 ms
    120 blocks stored, tip 329087, 1243 ms
    160 blocks stored, tip 329087, 1595 ms
    200 blocks stored, tip 329283, 8962 ms
  blocks: 200 stored (26394087 bytes) in 8962 ms, tip 329283 (0000000aa9bbf4a9966fc430f45206b421d6cecdbe7de118fc471b0a98e1857c); from each peer: 16 16 18 18 17 17 16 0 24 25 17 16; 0 stalls started, 0 invalid
check: best header 329283 is 0000000aa9bbf4a9966fc430f45206b421d6cecdbe7de118fc471b0a98e1857c; mempool.space says 0000000aa9bbf4a9966fc430f45206b421d6cecdbe7de118fc471b0a98e1857c; 200 blocks stored: True
live acceptance: passed
```

- Every one of the 329,283 signet headers went through presync: 168
  messages of PRESYNC with 512 commitments, then REDOWNLOAD, which
  checked the commitments and released headers as its buffer of 15,885
  filled. The node asked for each header twice, about 53 MB in all, and
  stored none until its chain had proved the minimum chain work.
- The best header equals the block mempool.space shows at height
  329,283.
- The last 200 blocks, 26 MB, came from 10 peers. Each was checked
  against its header and Merkle root. One of them is 1,066,136 bytes,
  which found a limit of chapter 65's frame reader: its checksum goes through
  the byte helpers, which take at most 1 MiB (`CB_MAX_BYTES`), so it
  reports a larger message as a wrong checksum. The tool hashes such a
  payload in 1 MiB pieces and reads the frame again; the limit itself is
  left to chapter 65.
- Signet's blocks below those 200 were not downloaded or checked.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/headers-sync.coax](../examples/checks/headers-sync.coax),
which loads the example. Whatever the salt and the commitment offset, an
honest chain passes PRESYNC and REDOWNLOAD and comes out whole and in
order. Whichever block a peer withholds, no request goes more than the
window past the last block in common, and the rest of the window
arrives:

<!-- source: bitcoin/examples/checks/headers-sync.coax check -->
```jai
// Whatever the salt and the commitment offset, an honest chain passes
// PRESYNC and REDOWNLOAD and comes out whole, in order.
#forall "an honest chain always comes through" k: 0..<6 #steps 8000000000 {
  l: [4] BiLink; e: [4] BiEntry; slots: [8] int; chain: [4] int; t: [1] BlockTree;
  bi_regtest(t[..], l[..], e[..], slots[..], chain[..]);
  raw: [80 * 300] u8; bits: [32] u8; buf: [100 + SY_MAX_HEADERS] SyCompressed; out: [80 * 400] u8; s: [1] SyHss;
  sy_t_chain(e[0].hash[..], 1, e[0].time, cast,trunc(u8) k, 300, raw[..]);
  hp := SyHeadersParams.{period=25, buffer=100};
  #assert sy_hss_init(s[..], t[..], 0, hp, u256_small(2 * 290), SY_NOW, k * 4, 0x9e3779b97f4a7c15 ^ cast(u64) (k + 1), cast(u64) k, bits[..], buf[..]) == SY_HSS_OK;
  r := sy_hss_process(s[..], raw[..], 300, true, out[..]);
  #assert r.success && s[0].state == SY_REDOWNLOAD;
  r = sy_hss_process(s[..], raw[..], 300, false, out[..]);
  #assert r.success && r.released == 300 && s[0].state == SY_FINAL;
  for j: 0..<80 * 300 { #assert out[j] == raw[j]; }
}
// Whichever block a peer withholds, no request goes more than the window
// past the last block in common, and the rest of the window arrives.
#forall "requests stay inside the window" k: 0..<4 #steps 8000000000 {
  m: [1] Sim; peers: [3] SyPeer;
  tip := sim_init(m[..], 1100, peers[..]);
  l := m[0].l[..];
  held := [1] int.[bi_ancestor(l, tip, 1 + 37 * k)];
  for p: 0..<3 { peers[p].best = tip; }
  s := sy_t_settle(m[0].d[..], 1000, held[..]);
  for i: 0..<m[0].t[0].count { if m[0].have[i] != 0 || m[0].owner[i] != 0 { #assert l[i].height <= l[held[0]].height - 1 + SY_WINDOW; } }
  end := l[held[0]].height - 1 + SY_WINDOW;
  #assert l[m[0].d[0].tip].height == l[held[0]].height - 1 && s.requested == ifx end < 1100 then end else 1100;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/headers-sync.coax
```

`coax test` runs 652 checks, including the loaded files' own checks, in about 4 minutes on 2026-11-03.

### The library's own checks

`sync.coax` is an `#authority` file. Its checks port Bitcoin Core's
`src/test/headers_sync_chainwork_tests.cpp` (`sneaky_redownload`,
`happy_path` with short and full messages, `too_little_work` and
`system_clock_lagging_behind_chain_start`) at a tenth of the size: 1,500
blocks with a commitment every 60, so the test keeps 25 commitments.
They also check SipHash's reference vector under Core's test key
(`hash_tests.cpp`), the commitment bound, locators, each branch of
`ProcessHeadersMessage`, Core's `p2p_ibd_stalling.py` (the staller, the
doubling timeout, and the manual peer that is paused instead of
disconnected), the download timeout, the minimum work, a run that
starts high, and the block checks.

```shell
coax/bin/coax test --run-steps 250000000 coax/lib/bitcoin/sync.coax
```

`coax test` runs 650 checks, including those of the files it loads, in about 6 minutes on 2026-11-03. Mutation testing: not run (owner speed order).

### Bitcoin Core's code

[sync.py](../../coax/tests/bitcoin/sync.py) computes every line the
example prints. It transliterates `HeadersSyncState` from the pinned
`headerssync.cpp`; `ProcessHeadersMessage`,
`IsContinuationOfLowWorkHeadersSync`, `TryLowWorkHeadersSync`,
`GetAntiDoSWorkThreshold`, `FindNextBlocksToDownload`,
`FindNextBlocks`, `BlockRequested`, `RemoveBlockRequest` and the block
part of `SendMessages` from `net_processing.cpp`; and the sizes of
Core's test from `headers_sync_chainwork_tests.cpp`. It reads the
constants from the pinned sources, mines headers the way Core's tests
do, and compares its transcript with the fixture:

```shell
python3 -I coax/tests/bitcoin/sync.py --coax coax/bin/coax
```

On 2026-11-03 all 45 lines agree, and the example's output
equals them in debug and release.

## Summary

- A locator names the blocks you have in about log2(height) hashes; the
  peer answers from the first one it has.
- Headers whose chain has too little work are never stored. PRESYNC
  follows them keeping one salted bit per period; REDOWNLOAD checks the
  bits and releases headers only once their work is proven.
- Blocks come from every peer at once, at most 16 in flight per peer and
  never more than 1,024 past the last block in common.
- A peer that holds the window still is disconnected after a stalling
  timeout that starts at 2 seconds and doubles for each staller.
- A block is stored only if it is the one requested and its Merkle root
  is right. Full validation, connecting and reorganizations are later
  chapters.

## Dependencies

- [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md)
- [59. The block index and the most-work chain](ch059-the-block-index-and-the-most-work-chain.md)
- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md)
- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md)
- [71. Misbehavior, limits and denial of service](ch071-misbehavior-limits-and-denial-of-service.md)

The plan listed chapter 60, reorganizations, as a dependency. Nothing
here needs it: headers on any branch go into chapter 59's block index,
and blocks are stored, not connected, so the active chain never has to
switch.

## Sources

- Bitcoin Core at revision `ddea9196`: `test/headers_sync_chainwork_tests.cpp`
  and `test/functional/p2p_ibd_stalling.py`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/headers_sync_chainwork_tests.cpp)
  by this chapter; `headerssync.cpp`, `headerssync.h`,
  `net_processing.cpp`, `net_processing.h`, `chainparams.cpp`,
  `chain.h` and `test/hash_tests.cpp`, pinned there by earlier chapters.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
