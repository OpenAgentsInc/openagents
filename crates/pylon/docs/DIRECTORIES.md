# Pylon Directory Structure

This document details all file paths and directories used by Pylon.

## Base Directory

All Pylon files are stored under `~/.openagents/pylon/`.

This follows the OpenAgents convention of storing all product data under `~/.openagents/`.

## Directory Layout

```
~/.openagents/pylon/
├── config.toml              # Main configuration file
├── pylon.pid                # Daemon process ID file
├── control.sock             # Unix domain socket for IPC
├── pylon.db                 # SQLite database
├── pylon.db-wal             # SQLite WAL file (write-ahead log)
├── pylon.db-shm             # SQLite shared memory file
├── bin/                     # Optional: installed binaries
│   └── foundation-bridge    # Apple FM Bridge binary
└── neobank/                 # Neobank wallet storage
    ├── btc_wallet.redb      # BTC Cashu wallet database
    └── usd_wallet.redb      # USD Cashu wallet database
```

## Path Resolution API

All paths are resolved through `PylonConfig` in `src/config.rs`:

```rust
use pylon::config::PylonConfig;

// Get the OpenAgents base directory
let base = PylonConfig::openagents_dir()?;  // ~/.openagents

// Get the Pylon directory
let pylon = PylonConfig::pylon_dir()?;      // ~/.openagents/pylon

// Get the config file path
let config = PylonConfig::config_path()?;   // ~/.openagents/pylon/config.toml
```

The daemon module (`src/daemon/mod.rs`) provides additional path helpers:

```rust
use pylon::daemon::{runtime_dir, pid_path, socket_path, db_path};

let runtime = runtime_dir()?;   // ~/.openagents/pylon (creates if missing)
let pid = pid_path()?;          // ~/.openagents/pylon/pylon.pid
let socket = socket_path()?;    // ~/.openagents/pylon/control.sock
let db = db_path()?;            // ~/.openagents/pylon/pylon.db
```

## File Descriptions

### config.toml

Main configuration file for Pylon. Created by `pylon init`.

**Location**: `~/.openagents/pylon/config.toml`

**Format**: TOML

```toml
# Provider identity
name = "My Pylon Provider"
description = "AI compute provider"

# Nostr relays
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
]

# Payment configuration
network = "regtest"           # mainnet | testnet | signet | regtest
enable_payments = true
require_payment = true
min_price_msats = 1000        # 1 sat minimum

# Spark wallet (optional)
spark_url = "https://localhost:9737"
spark_token = "your-api-token"

# Inference backends
default_model = "llama3.2"
backend_preference = ["ollama", "llamacpp", "apple_fm"]

# Data directory override (optional)
# data_dir = "/custom/path"

# Claude tunnel configuration
[claude]
enabled = true
model = "claude-sonnet-4-20250514"
autonomy = "supervised"       # full | supervised | restricted | read_only
approval_required_tools = ["Write", "Edit", "Bash"]
allowed_tools = []            # Empty = allow requested tools
blocked_tools = []
max_cost_usd = 250000         # Micro-USD ($0.25)
cwd = "/path/to/repo"         # Optional working directory
executable_path = "/usr/local/bin/claude"  # Optional explicit path
```

**Loading**: Loaded by `PylonConfig::load()` at startup.

### pylon.pid

Process ID file for the daemon.

**Location**: `~/.openagents/pylon/pylon.pid`

**Format**: Single line containing the process ID.

```
12345
```

**Lifecycle**:
1. Created by `daemonize()` when daemon starts
2. Contains PID of the daemon process
3. Used to check if daemon is running (`is_daemon_running()`)
4. Removed on graceful shutdown

**Stale Detection**:
```rust
// Check if PID file points to running process
pub fn is_running(&self) -> bool {
    match self.read() {
        Ok(pid) => process_exists(pid),  // Uses kill(pid, 0)
        Err(_) => false,
    }
}
```

### control.sock

Unix domain socket for IPC between CLI and daemon.

**Location**: `~/.openagents/pylon/control.sock`

**Protocol**: JSON-encoded request/response over Unix socket.

**Commands**:
```rust
pub enum DaemonCommand {
    Ping,                                    // Health check
    Status,                                  // Get daemon status
    Shutdown,                                // Request graceful shutdown
    NeobankBalance { currency: String },     // Get wallet balance
    NeobankPay { bolt11: String },           // Pay Lightning invoice
    NeobankSend { amount_sats: u64, currency: String },  // Send tokens
    NeobankReceive { token: String },        // Receive tokens
    NeobankStatus,                           // Get treasury status
}
```

**Responses**:
```rust
pub enum DaemonResponse {
    Pong,
    Ok,
    Error(String),
    Status {
        running: bool,
        uptime_secs: u64,
        provider_active: bool,
        host_active: bool,
        jobs_completed: u64,
        earnings_msats: u64,
    },
    NeobankBalance { sats: u64 },
    NeobankPayment { preimage: String },
    NeobankSend { token: String },
    NeobankReceive { amount_sats: u64 },
    NeobankStatus {
        btc_balance_sats: u64,
        usd_balance_cents: u64,
        treasury_active: bool,
        btc_usd_rate: Option<f64>,
    },
}
```

**Lifecycle**:
1. Created by `ControlSocket::new()` when daemon starts
2. Old socket file removed before creating new one
3. Set to non-blocking mode
4. Removed on daemon shutdown (in `Drop` impl)

### pylon.db

SQLite database for persistent storage.

**Location**: `~/.openagents/pylon/pylon.db`

**Mode**: WAL (Write-Ahead Logging) for better concurrency.

**Tables**:

| Table | Purpose |
|-------|---------|
| `migrations` | Applied database migrations |
| `jobs` | NIP-90 job records |
| `earnings` | Payment records |
| `agents` | Agent state (for host mode) |
| `tick_history` | Agent tick execution history |

See [DATABASE.md](./DATABASE.md) for full schema documentation.

**Related Files**:
- `pylon.db-wal` - Write-ahead log (temporary)
- `pylon.db-shm` - Shared memory file (temporary)

### bin/foundation-bridge

Apple Foundation Models bridge binary.

**Location**: `~/.openagents/pylon/bin/foundation-bridge`

**Purpose**: Provides HTTP API for Apple FM on macOS.

**Binary Search Order** (in `BridgeManager::find_binary()`):
1. `~/.openagents/pylon/bin/foundation-bridge` (user-installed)
2. Next to pylon executable (bundled app)
3. Development paths (`swift/foundation-bridge/.build/...`)

**Installation**:
```bash
# Build from source
cd swift/foundation-bridge
swift build -c release

# Install to pylon directory
mkdir -p ~/.openagents/pylon/bin
cp .build/arm64-apple-macosx/release/foundation-bridge ~/.openagents/pylon/bin/
```

### neobank/

Cashu wallet storage for Neobank treasury.

**Location**: `~/.openagents/pylon/neobank/`

**Files**:
- `btc_wallet.redb` - BTC-denominated Cashu wallet
- `usd_wallet.redb` - USD-denominated Cashu wallet

**Format**: redb (Rust embedded database)

**Created By**: `NeobankService::init()` when treasury is enabled.

## Directory Creation

Pylon creates directories automatically as needed:

```rust
// In runtime_dir()
pub fn runtime_dir() -> anyhow::Result<PathBuf> {
    let dir = PylonConfig::pylon_dir()?;
    std::fs::create_dir_all(&dir)?;  // Creates ~/.openagents/pylon/
    Ok(dir)
}
```

## Permissions

Recommended permissions for security:

```bash
# Pylon directory (owner only)
chmod 700 ~/.openagents/pylon

# Config file (owner read/write)
chmod 600 ~/.openagents/pylon/config.toml

# Database (owner read/write)
chmod 600 ~/.openagents/pylon/pylon.db

# Socket is protected by directory permissions
# PID file doesn't need special permissions
```

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `RUST_LOG` | Logging level | `info` |
| `FM_BRIDGE_URL` | Override FM Bridge URL | `http://localhost:11435` |

## Migration from Legacy Paths

### From `~/.pylon/`

Early versions used `~/.pylon/`:

```bash
# Stop daemon
pylon stop

# Move data
mkdir -p ~/.openagents/pylon
cp -r ~/.pylon/* ~/.openagents/pylon/

# Restart
pylon start

# Verify
pylon doctor

# Remove old directory
rm -rf ~/.pylon
```

### From `~/.config/pylon/`

Some early versions used `dirs::config_dir()`:

```bash
# Move config and identity
mv ~/.config/pylon/config.toml ~/.openagents/pylon/
mv ~/.config/pylon/identity.mnemonic ~/.openagents/pylon/

# Remove old directory
rmdir ~/.config/pylon
```

## Troubleshooting

### Cannot Create Directory

```
Error: Permission denied (os error 13)
```

Check parent directory permissions:
```bash
ls -la ~/.openagents/
# Should be drwx------ (700) owned by you
```

### Stale PID File

```
Error: Daemon is already running
```

But daemon isn't actually running:
```bash
# Check if process exists
cat ~/.openagents/pylon/pylon.pid
ps aux | grep <pid>

# If process doesn't exist, remove stale file
rm ~/.openagents/pylon/pylon.pid
```

### Socket Connection Refused

```
Error: Connection refused
```

Daemon isn't running or socket is stale:
```bash
# Check daemon status
pylon status

# If socket exists but daemon isn't running
rm ~/.openagents/pylon/control.sock
pylon start
```

### Database Locked

```
Error: database is locked
```

Another process has the database open:
```bash
# Find processes using the database
lsof ~/.openagents/pylon/pylon.db

# Usually means another pylon instance is running
pylon stop
pylon start
```

## Backup

### Essential Files

Back up these files to preserve your identity:

```bash
cp ~/.openagents/pylon/config.toml ~/backup/
# Identity mnemonic (if stored separately)
```

### Full Backup

```bash
# Stop daemon first for consistent backup
pylon stop

# Backup everything
tar -czf pylon-backup-$(date +%Y%m%d).tar.gz ~/.openagents/pylon/

# Restart
pylon start
```

### Database Backup

Using SQLite backup API:
```bash
sqlite3 ~/.openagents/pylon/pylon.db ".backup pylon-backup.db"
```

Or while daemon is running (WAL mode is safe):
```bash
cp ~/.openagents/pylon/pylon.db pylon-backup.db
cp ~/.openagents/pylon/pylon.db-wal pylon-backup.db-wal
```
