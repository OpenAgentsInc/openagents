# 61. Block files and undo files

Chapter 72 downloads blocks. You need to keep their bytes after the process
exits and find them again without reading every earlier block. This chapter
builds that storage boundary: Bitcoin Core's `blk*.dat` and `rev*.dat` record
formats, reads by position, and recovery after an interrupted append.

The implementation is [blockfiles.coax](../../coax/lib/bitcoin/blockfiles.coax).
It works over caller-owned bytes. The
[file tool](../examples/live/block-files.coax) performs the file effects in
Coax through the existing native file bindings. The only new C effect is
`ftruncate` followed by a storage sync. Every decision about where to cut
stays in Coax.

You need chapter 5's SHA-256d and chapter 34's serialized transactions.
Chapter 58 will produce the undo body when it connects a block. It is a
consumer of this storage layer, so it is no longer a prerequisite. Here an
undo body is an already serialized `CBlockUndo`; the storage layer does not
construct spent coins, change a UTXO set or decide whether a block is valid.

## The record and its position

Core's ordinary block files are named `blk00000.dat`, `blk00001.dat`, and so
on. Their companion undo files use `rev` with the same file number. A block
file stores blocks in arrival order; undo records may be written later.
Neither file order is a block height index.

| Bytes | Block record | Undo record |
| --- | --- | --- |
| 0–3 | the chain's four message-start bytes | the same message start |
| 4–7 | body length, little-endian `uint32_t` | undo body length, excluding the checksum |
| 8 onward | the block, with witnesses | serialized `CBlockUndo` |
| after the body | nothing | 32-byte `SHA256d(previous_block_hash || undo_body)` |

The previous hash is the previous block's 32 serialized bytes, in wire
order. An explorer's displayed hash has the reverse byte order. A checksum
for the current block hash, or for the displayed previous hash, is wrong.

A stored position is a pair: the file number and the **body** offset. The
first body starts at 8. To read a body at position `p`, check `p >= 8`, open
its file and read the record header at `p - 8`. Check its chain magic and
length before reading the body. This chapter's codec takes the header
offset; the tool's `read` command takes Core's body position.

The reader uses Core's `MAX_SIZE`, 33,554,432 bytes, as its deserialization
cap. That is a storage bound. Chapter 53's block weight limit is a separate
consensus check. File positions fit an unsigned 32-bit offset. Core normally
rolls block files at a 128 MiB target; choosing file numbers, scheduling
rollover and coupling the block index to durable files belong to the node's
storage manager. The tool operates on one explicitly named file at a time.

<!-- source: coax/lib/bitcoin/blockfiles.coax layout -->
```jai
BF_OK :: 0; BF_END :: 1; BF_TORN :: 2; BF_MAGIC :: 3;
BF_SIZE :: 4; BF_CHECKSUM :: 5; BF_ARGUMENT :: 6; BF_CAPACITY :: 7;
BF_BLOCK :: 0; BF_UNDO :: 1;
BF_MAX_BODY :: 33554432;       // Core serialize.h MAX_SIZE
BF_MAX_FILE :: 134217728;      // Core's ordinary 128 MiB rollover target
BfFrame :: struct { code: int; size: int; n: int; }
BfRecovery :: struct { code: int; keep: int; records: int; cut: bool; }
```

## Work through genesis

Regtest's genesis block has 285 serialized bytes. Its magic is `fa bf b5 da`.
The body length, 285, is `0x011d`, so the first eight bytes are:

```text
fa bf b5 da 1d 01 00 00
```

The record occupies `8 + 285 = 293` bytes. Append another copy for this
storage example: its header starts at 293 and its body position is 301.
The complete file has 586 bytes. Duplicate blocks are useful here to make
the offset calculation visible; deciding which blocks to keep is a later
index decision.

`bf_write_vartime` checks the kind, body size, previous-hash extent, file
extent and output capacity before changing any output byte. It returns a
record length or a negative refusal code. Inputs and output must be
disjoint, live views. The caller owns that precondition.

`bf_read_vartime` checks framing and capacity first. For undo data it checks
the complete checksum before copying any body byte to the output. Its
private hasher feeds decoded bytes into chapter 5's SHA-256 work state, so
it can read bodies beyond the one-shot hash's 1 MiB input limit.

The [runnable example](../examples/block-files.coax) writes genesis, reads
it back, hashes the recovered header and obtains regtest's published
genesis hash. It then stores the one-byte serialization of an empty
`CBlockUndo` with genesis as its previous hash. Changing that hash refuses
the undo read and leaves the destination sentinel intact.

```shell
coax/bin/coax run bitcoin/examples/block-files.coax
```

```text
regtest genesis: 285 bytes, record 293 bytes, data position 8
storage header: fabfb5da1d010000
read block hash: 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206
two records: 2, keep 586, second data position 301
second append at 293 lengths: 292 torn tails, all keep the first record
XOR header at file offset 3: febab3dd15000203
empty CBlockUndo checksum: 2228d16e17f5c12fb0d6e40fedd17f7ca18001331156329d084b8ee4e86c5806
wrong previous block: refusal 5, output remains 165
damaged magic: refusal 3, cut false
```

## Read Core's obfuscated files

Core can XOR every byte of both files with an eight-byte key from `xor.dat`.
It is obfuscation, not encryption or authentication. Byte `i` of a file
meets key byte `i % 8`. A read at a nonzero offset must keep that phase;
restarting the key at each record silently changes its bytes. The record
header and the undo checksum are masked too.

The codec takes the key as a little-endian `u64`. A zero key handles plain
files. The tool reads exactly eight bytes from the supplied `xor.dat` and
rejects a shorter or longer key file. It never changes Core's source file
or key.

For example, key bytes `01 02 03 04 05 06 07 08` and header offset 3 turn
the genesis header into `fe ba b3 dd 15 00 02 03`. The key phase at body
offset 11 is the same as at header offset 3 because the header is eight
bytes long.

Scan and import one existing Core regtest file:

```shell
coax/web/tool.sh bitcoin/examples/live/block-files.coax scan block /data/regtest/blocks/blk00000.dat fabfb5da --xor /data/regtest/blocks/xor.dat
coax/web/tool.sh bitcoin/examples/live/block-files.coax import block /data/regtest/blocks/blk00000.dat fabfb5da ./imported-blk00000.dat --xor /data/regtest/blocks/xor.dat
```

Import writes a new, plain Core-format file, retaining each record's offset.
Repeat it for each `blk*.dat` you need. It refuses an existing destination,
never overwrites the source and does not claim to validate the imported
blocks. A valid complete prefix can be imported from a file with a torn
tail or zero preallocation. The report identifies that omitted tail.

Read its first block by position, or durably append a serialized block:

```shell
coax/web/tool.sh bitcoin/examples/live/block-files.coax read block ./imported-blk00000.dat fabfb5da 8 ./genesis.bin
coax/web/tool.sh bitcoin/examples/live/block-files.coax append block ./ours-blk00000.dat fabfb5da ./genesis.bin
```

An undo append or read uses `undo` instead of `block` and requires
`--prev` with 64 hex digits in wire order. An append returns the new body
position only after the full frame and its file name are synced to storage.
An I/O failure returns failure; it never publishes a successful position.

## Recover an interrupted append

`bf_next_vartime` reads a header against the physical file length. EOF at a
record boundary is clean. EOF after a valid magic prefix or inside a
declared body or undo checksum is a torn append. `bf_recover_vartime`
returns the last complete boundary as `keep`. It also recognizes an
entirely zero physical tail, including Core's unused preallocation.

A wrong magic or impossible size followed by nonzero bytes refuses the
file. Recovery does not search for another magic and skip damaged data.
Only a reported torn or zero tail may be truncated. The native tool checks
the size and modification time across its scan, and refuses a changing
snapshot. Short I/O reads are errors, not evidence of an incomplete append.

```shell
coax/web/tool.sh bitcoin/examples/live/block-files.coax recover block ./ours-blk00000.dat fabfb5da
```

The tool truncates to `keep` and syncs before reporting success. It requires
exclusive ownership of that path: no concurrent append, rename, unlink or
key change. A normal append refuses a damaged tail until you recover it.

The 293-byte genesis record gives 293 file lengths from the start of its
append through one byte before completion. At the first length no append
has happened; the other 292 lengths are torn. Every one retains exactly
the preceding record. The checks apply this argument to both block and
undo records and to zero and nonzero XOR keys.

Framing cannot detect every kind of damage. Block records have no body
checksum. A changed body, or a full-length body whose suffix was replaced
by zeros, can still have valid framing. An undo checksum needs the block
index's previous hash, so a framing scan alone cannot check it. Validate
blocks at admission and check undo checksums when reading them. This
chapter provides no chainstate transaction, reorg or power-loss guarantee
for bytes that were never durably acknowledged.

## Independent checks

The specification here is Core's `WriteBlock`, `WriteBlockUndo` and
`ReadRawBlock` in the repository's pinned
[blockstorage.cpp](../../coax/tests/vectors/upstream/bitcoin-core/blockstorage.cpp),
revision `ddea9196`. These are local disk formats, not a BIP or a versioned
OpenAgents network contract.

The [independent oracle](../../coax/tests/bitcoin/blockfiles.py) uses Python
`struct` and `hashlib`, with no Coax codec calls in its expected answers.
It authenticates the pinned source against the vector manifest. The
[Core fixture](../fixtures/block-files-core.json) captures three original
XOR-masked records from Core 32.1's chapter-72 regtest run. Its first block
has the published regtest genesis hash. No upstream scripts run.

```shell
coax/bin/coax test coax/lib/bitcoin/blockfiles.coax
coax/bin/coax mutate coax/lib/bitcoin/blockfiles.coax --root-only --jobs 4
python3 -I coax/tests/bitcoin/blockfiles.py
python3 -I coax/tests/bitcoin/blockfiles.py --core-dir /data/regtest/blocks
```

Recorded on macOS arm64, 2026-11-03, with `coax0 0.48.0`:

- 226 mutation sites killed; zero survivors.
- 1,297 independent codec/recovery cases in each of debug and release,
  including bodies beyond 1 MiB, randomized key phases, every checksum byte,
  every physical cut, and byte corruption compared with the Python reader.
- The native file tool under ASan/UBSan: Core import and position reads,
  293 physical file cuts and recoveries, a durable append after recovery,
  and undo reads with matching and wrong previous hashes.
- The existing Core 32.1 file imports 2,501 blocks and 851,420 record bytes,
  identical byte for byte after decoding. The unused tail of its 16 MiB allocation
  is omitted from the destination; the source is untouched.
- The chapter example has identical debug and release output, matching its
  [exact transcript](../fixtures/block-files.out).

The node can now keep and recover serialized records. Chapter 58 supplies
the spent outputs that make undo useful; chapter 60 uses them during a
reorganization, and chapter 91 indexes transactions within these files.
