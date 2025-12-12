# OANIX Sprint 3: Expanded Primitives

**Date:** 2025-12-11
**Commit:** (pending)

---

## Summary

Implemented Sprint 3 of the OANIX roadmap: three new FileService implementations that serve as building blocks for higher-level services.

---

## New Filesystems

### 1. MapFs - Static/Immutable Filesystem

**File:** `src/services/map_fs.rs` (~270 lines)

Read-only filesystem built from static data at construction time. Perfect for:
- Bundled assets (`include_bytes!()`)
- Read-only task specifications
- Snapshot baselines for CowFs

```rust
let fs = MapFs::builder()
    .file("/readme.txt", b"Hello!")
    .dir("/src")
    .file("/src/main.rs", b"fn main() {}")
    .build();
```

**Key features:**
- Builder pattern for construction
- Auto-creates parent directories
- All write operations return `FsError::ReadOnly`
- Zero runtime overhead (no locks needed)

### 2. FuncFs - Computed/Dynamic Files

**File:** `src/services/func_fs.rs` (~330 lines)

Files backed by closures that compute content on-the-fly. Perfect for:
- Status files (`/task/status`)
- Live system info (`/sys/time`)
- Control files that trigger actions on write

```rust
let counter = Arc::new(AtomicU64::new(0));
let counter_read = counter.clone();
let counter_write = counter.clone();

let fs = FuncFs::builder()
    .read_only("/time", || timestamp().into_bytes())
    .read_write(
        "/counter",
        move || counter_read.load(Ordering::SeqCst).to_string().into_bytes(),
        move |data| {
            if let Ok(n) = String::from_utf8(data).unwrap().parse::<u64>() {
                counter_write.store(n, Ordering::SeqCst);
            }
        }
    )
    .write_only("/control", |cmd| handle_command(cmd))
    .build();
```

**Key features:**
- Three modes: `read_only`, `read_write`, `write_only`
- Content computed fresh on each open
- Directories derived implicitly from file paths
- Write buffers flushed on close

### 3. CowFs - Copy-on-Write Overlay

**File:** `src/services/cow_fs.rs` (~340 lines)

Layers a writable MemFs overlay on top of any read-only base. Perfect for:
- Workspace snapshots for benchmarking
- Undo/redo support
- Branching experiments

```rust
let base = MapFs::builder()
    .file("/readme.txt", b"Original")
    .build();

let cow = CowFs::new(base);

// Modify - creates copy in overlay, base unchanged
{
    let mut handle = cow.open("/readme.txt", OpenFlags::read_write()).unwrap();
    handle.write(b"Modified!")?;
}
```

**Key features:**
- Reads: overlay first, then base
- Writes: copy-on-write (copy from base to overlay on first write)
- Deletes: tracked as "tombstones"
- Directory listings merge base + overlay - tombstones
- Parent directories auto-created in overlay

---

## Architecture: Composition Pattern

These primitives are designed to compose into higher-level services:

```
TaskFs
├── /spec.json     → MapFs (immutable task definition)
├── /meta.json     → MapFs (immutable metadata)
├── /status        → FuncFs (live status computed on read)
└── /result.json   → MemFs (writable result)

WorkspaceFs (for benchmarking)
└── CowFs
    ├── base: MapFs (original project snapshot)
    └── overlay: MemFs (agent's modifications)
```

---

## Test Results

```
running 29 tests
test services::cow_fs::tests::test_read_through ... ok
test services::cow_fs::tests::test_write_creates_copy ... ok
test services::cow_fs::tests::test_create_new_file ... ok
test services::cow_fs::tests::test_delete_tombstones ... ok
test services::cow_fs::tests::test_recreate_after_delete ... ok
test services::cow_fs::tests::test_readdir_merged ... ok
test services::cow_fs::tests::test_readdir_excludes_tombstoned ... ok
test services::cow_fs::tests::test_mkdir_in_overlay ... ok
test services::cow_fs::tests::test_rename_file ... ok
test services::cow_fs::tests::test_nested_directory_in_base ... ok
test services::func_fs::tests::test_read_only_file ... ok
test services::func_fs::tests::test_read_write_file ... ok
test services::func_fs::tests::test_nested_structure ... ok
test services::func_fs::tests::test_write_to_readonly_fails ... ok
test services::func_fs::tests::test_write_only_file ... ok
test services::func_fs::tests::test_dynamic_content ... ok
test services::map_fs::tests::test_build_and_read ... ok
test services::map_fs::tests::test_nested_structure ... ok
test services::map_fs::tests::test_write_fails ... ok
test services::map_fs::tests::test_stat ... ok
test services::map_fs::tests::test_seek ... ok
... (plus existing MemFs and namespace tests)

test result: ok. 29 passed; 0 failed
```

---

## Files Modified

- `src/services/map_fs.rs` - NEW (~270 lines)
- `src/services/func_fs.rs` - NEW (~330 lines)
- `src/services/cow_fs.rs` - NEW (~340 lines)
- `src/services/mod.rs` - Updated exports
- `src/lib.rs` - Updated re-exports
- `src/error.rs` - Added `FsError::ReadOnly`
- `docs/ROADMAP.md` - Updated status

---

## Next: Sprint 4 - Standard Services

With these primitives, we can now build:

1. **TaskFs** - Compose MapFs + FuncFs + MemFs for task specs
2. **LogsFs** - Append-only logging with ATIF support
3. **WorkspaceFs** - Real filesystem wrapper with CowFs for snapshots
