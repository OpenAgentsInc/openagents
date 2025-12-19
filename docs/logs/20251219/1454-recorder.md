# Plan: Rename BlackBox to Recorder

## Summary

"BlackBox" is overloaded. Rename all occurrences of "blackbox/BlackBox" to "recorder/Recorder" and ".bbox" to ".rlog" across the codebase.

## Naming Convention

| Old | New |
|-----|-----|
| `blackbox` | `recorder` |
| `BlackBox` | `Recorder` |
| `.bbox` | `.rlog` |
| `bbox/1` | `rlog/1` |

---

## Files to Rename

### Directories
```
crates/blackbox/                    → crates/recorder/
crates/ui/src/blackbox/             → crates/ui/src/recorder/
crates/storybook/src/stories/blackbox/ → crates/storybook/src/stories/recorder/
```

### Documentation
```
crates/blackbox/docs/format.md      → crates/recorder/docs/format.md
crates/blackbox/docs/README.md      → crates/recorder/docs/README.md
```

---

## Files to Edit

### 1. Workspace Config
- `Cargo.toml` (root)
  - Line 4: `"crates/blackbox"` → `"crates/recorder"`
  - Line 42: `blackbox = { path = "crates/blackbox" }` → `recorder = { path = "crates/recorder" }`

### 2. Recorder Crate (`crates/recorder/`)
- `Cargo.toml`
  - `name = "blackbox"` → `name = "recorder"`
  - `description = "BlackBox format..."` → `description = "Recorder format..."`
  - Binary name: `blackbox` → `recorder`

- `src/lib.rs`
  - Doc comments: `BlackBox` → `Recorder`
  - Format prefix: `bbox/` → `rlog/`
  - All test fixtures

- `src/main.rs`
  - Doc comments and CLI help
  - `#[command(name = "blackbox")]` → `#[command(name = "recorder")]`
  - All `.bbox` references → `.rlog`
  - Import: `use blackbox::` → `use recorder::`

- `src/convert.rs`
  - Doc comments: `BlackBox` → `Recorder`
  - Format string: `format: bbox/1` → `format: rlog/1`
  - All test fixtures

- `src/export.rs`
  - Doc comments: `.bbox` → `.rlog`
  - Format string: `bbox/1` → `rlog/1`
  - Filename pattern: `.bbox` → `.rlog`

- `docs/format.md`
  - All `BlackBox` → `Recorder`
  - All `bbox/1` → `rlog/1`
  - All `.bbox` → `.rlog`

- `docs/README.md`
  - All `blackbox` → `recorder`
  - All `BlackBox` → `Recorder`
  - All `.bbox` → `.rlog`

### 3. UI Crate (`crates/ui/`)
- `src/lib.rs`
  - `pub mod blackbox;` → `pub mod recorder;`

- `src/recorder/mod.rs` (after rename)
  - Doc comments: `BlackBox` → `Recorder`

- `src/recorder/atoms/mod.rs`
  - Doc comments

- `src/recorder/molecules/mod.rs`
  - Doc comments

- `src/recorder/organisms/mod.rs`
  - Doc comments

- `src/recorder/sections/mod.rs`
  - Doc comments

### 4. Storybook Crate (`crates/storybook/`)
- `src/main.rs`
  - All imports: `stories::blackbox::` → `stories::recorder::`
  - All route paths: `/stories/blackbox` → `/stories/recorder`
  - All HTML labels: `BlackBox` → `Recorder`

- `src/stories/mod.rs`
  - `pub mod blackbox;` → `pub mod recorder;`

- `src/stories/recorder/*.rs` (after rename)
  - All doc comments: `BlackBox` → `Recorder`

### 5. Logs/Docs (optional)
- `docs/logs/20251219/1436-cc-blackbox-plan.md` - can leave as historical record

---

## Execution Order

1. **Rename directories** (git mv)
   ```bash
   git mv crates/blackbox crates/recorder
   git mv crates/ui/src/blackbox crates/ui/src/recorder
   git mv crates/storybook/src/stories/blackbox crates/storybook/src/stories/recorder
   ```

2. **Update Cargo.toml files**
   - Root workspace
   - Recorder crate

3. **Update source files** (use replace_all)
   - `crates/recorder/src/*.rs`
   - `crates/recorder/docs/*.md`

4. **Update UI crate**
   - Module declaration in lib.rs
   - Doc comments in recorder module files

5. **Update Storybook**
   - Imports and routes in main.rs
   - Module declaration
   - Doc comments in story files

6. **Verify**
   ```bash
   cargo check -p recorder
   cargo test -p recorder
   cargo build -p storybook
   ```

---

## Estimated Scope

| Category | Files | Edits |
|----------|-------|-------|
| Directories | 3 | rename |
| Cargo.toml | 2 | ~5 |
| Recorder src | 4 | ~50 |
| Recorder docs | 2 | ~30 |
| UI module | 5 | ~10 |
| Storybook | 10+ | ~40 |
| **Total** | **26+** | **~135** |
