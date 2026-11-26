# Engine 4: `engines.kv_wal`

Implement a crash-safe, CRC32-verified Write-Ahead-Log Key-Value Store `WalKV` in `engines/kv_wal/__init__.py`:
1. `WalKV(path: str | Path)` opens/creates a binary WAL file at `path` and replays valid records on startup.
2. Binary record frame format (deterministic, compact):
   - 4-byte magic `b"WAL1"`, 1-byte op (`1=PUT`, `2=DEL`), 4-byte big-endian key len, 4-byte big-endian val len, key bytes, val bytes, 4-byte big-endian `zlib.crc32` over `(op + klen + vlen + key + val)`.
3. Methods:
   - `put(key: bytes, value: bytes) -> None` (appends frame and flushes).
   - `delete(key: bytes) -> None` (appends tombstone frame and flushes).
   - `get(key: bytes) -> bytes | None`.
   - `keys(prefix: bytes = b"") -> list[bytes]` (sorted lexicographically).
   - `compact() -> int`: atomically rewrites the WAL via temp file + `os.replace` keeping only live keys (removing tombstones and superseded values), returning the number of live records written.
   - `close() -> None`.
4. Crash recovery: if the tail of the WAL file has a truncated frame or corrupted CRC32 bytes, recovery MUST stop cleanly at the last valid frame, truncate the corrupt tail, and preserve all prior committed records without raising!
