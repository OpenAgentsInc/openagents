# EMERGENCY MANUAL ISSUES

**Created:** 2025-12-25 04:40 UTC
**Reason:** Autopilot system blocked by corrupted database, cannot use normal issue tracking

These issues need to be manually entered into the system once the database is restored.

---

## ISSUE: Fix Build Environment - Missing C Compiler

**Priority:** URGENT
**Type:** bug
**Directive:** N/A (infrastructure)

### Description

The execution environment is missing essential build tools, preventing compilation of Rust code.

### Missing Tools

- C compiler (cc, gcc, clang)
- sqlite3 CLI tool
- Python interpreter
- Docker/Podman

### Impact

- Cannot run `cargo build` or `cargo run`
- Cannot compile fixes or new code
- Cannot create new tool binaries
- Autopilot system completely blocked

### Resolution

Install build-essential package:

```bash
sudo apt-get update
sudo apt-get install -y build-essential sqlite3 python3
```

### Acceptance Criteria

- [ ] `gcc --version` succeeds
- [ ] `cargo build --bin issues-mcp` succeeds
- [ ] `sqlite3 --version` succeeds

---

## ISSUE: Recover Corrupted Autopilot Database

**Priority:** URGENT
**Type:** bug
**Directive:** d-004 (autopilot improvement)

### Description

The `autopilot.db` file is corrupted (SQLite error 11: "database disk image is malformed"). All backups are also corrupted, suggesting corruption occurred before backups were made.

### Files Affected

- `autopilot.db` (396KB - corrupted)
- `autopilot.db.backup.1766637343` (405KB - corrupted)
- `autopilot.db.corrupted` (405KB - corrupted)

### Investigation Needed

1. Determine root cause of corruption
2. Identify when corruption occurred (check git history for last known-good database)
3. Determine if any data can be recovered from corrupted file

### Recovery Options

**Option 1:** Create fresh database (data loss)
```bash
rm autopilot.db autopilot.db.*
cargo run --bin issues-mcp  # Will initialize fresh DB
```

**Option 2:** Restore from external backup (if available)
```bash
# Check if backups exist elsewhere
find /home -name "autopilot.db" -type f 2>/dev/null
# Restore from known-good backup
```

**Option 3:** Attempt data recovery
```bash
# Use sqlite3 .recover command
sqlite3 autopilot.db.corrupted ".recover" | sqlite3 autopilot-recovered.db
# Inspect recovered data
sqlite3 autopilot-recovered.db ".tables"
```

### Acceptance Criteria

- [ ] `autopilot.db` exists and is valid SQLite database
- [ ] Can run: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"issue_list","arguments":{}}}' | ./target/release/issues-mcp`
- [ ] `cargo autopilot issue ready` succeeds
- [ ] If data was recovered: verify issue history is intact

---

## ISSUE: Add Database Corruption Detection and Recovery

**Priority:** HIGH
**Type:** feature
**Directive:** d-004 (autopilot improvement)

### Description

Add automatic detection of database corruption and recovery mechanisms to prevent autopilot from getting stuck in unrecoverable states.

### Requirements

1. **Health Check on Startup**
   - Run `PRAGMA integrity_check` before initializing MCP server
   - If corruption detected, attempt automatic recovery
   - Log corruption events for analysis

2. **Automatic Backup System**
   - Create daily backups of autopilot.db before first write
   - Keep last 7 daily backups
   - Store backups in `~/.autopilot/backups/YYYY-MM-DD/`

3. **Recovery Procedures**
   - Attempt `.recover` command if corruption detected
   - Fall back to most recent valid backup
   - If all recovery fails, initialize fresh DB and log data loss

4. **Monitoring**
   - Track database health metrics (file size, integrity check duration)
   - Alert if database size changes unexpectedly
   - Log all recovery attempts to dedicated recovery log

### Implementation Files

- `crates/issues/src/db.rs` - Add integrity checks
- `crates/issues-mcp/src/main.rs` - Add health check on init
- `crates/autopilot/src/lib.rs` - Add backup system
- `docs/autopilot/DATABASE_RECOVERY.md` - Document recovery procedures

### Acceptance Criteria

- [ ] Database integrity checked on every autopilot start
- [ ] Daily backups created automatically
- [ ] Corruption auto-recovery succeeds in test scenarios
- [ ] Recovery attempts logged with full context
- [ ] Documentation includes manual recovery procedures

---

## ISSUE: Improve Autopilot Environment Requirements Documentation

**Priority:** MEDIUM
**Type:** docs
**Directive:** d-004 (autopilot improvement)

### Description

Document required tools and environment setup for autopilot execution to prevent future environment-related failures.

### Content Needed

Create `docs/autopilot/ENVIRONMENT.md`:

1. **Required System Tools**
   - Build tools (gcc, make, etc.)
   - Runtime dependencies
   - Optional but recommended tools

2. **Environment Validation Script**
   - Shell script to check all requirements
   - Report missing tools with install commands
   - Run automatically on first autopilot invocation

3. **Minimal vs Recommended Setup**
   - Minimal: what's absolutely required to run
   - Recommended: what's needed for full functionality
   - Development: what's needed to modify autopilot itself

4. **Docker/Container Setup**
   - Dockerfile with all dependencies pre-installed
   - Instructions for running autopilot in container
   - Volume mounts for workspace and database

### Acceptance Criteria

- [ ] Documentation file created
- [ ] Environment validation script implemented
- [ ] Dockerfile provided and tested
- [ ] Document linked from main README.md

---

**Note:** These issues were created manually because the normal issue tracking system was unavailable due to database corruption. They should be entered into the database once it's restored.

**Session Reference:** `docs/logs/20251224/223451-process-issues-from-database.rlog`
