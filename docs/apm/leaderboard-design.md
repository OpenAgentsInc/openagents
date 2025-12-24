# APM Leaderboard Design

**Status:** Specified  
**Directive:** d-016 (Phase 5 - Gamification)  
**Created:** 2025-12-24  

## Overview

The APM Leaderboard enables comparison of agent performance across multiple agents, users, projects, and time periods. It transforms APM from a personal metric into a competitive/collaborative tool for teams and the broader OpenAgents ecosystem.

## Goals

1. **Visibility** - Surface top performers to inspire improvement
2. **Competition** - Healthy rivalry drives velocity gains
3. **Recognition** - Reward consistent high performers
4. **Diagnostics** - Identify underperforming agents for investigation
5. **Benchmarking** - Establish community-wide APM standards

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      APM LEADERBOARD SYSTEM                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  DATA SOURCES                                                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │ Local APM   │  │ Team Relay  │  │ Global Relay│                 │
│  │ (SQLite)    │  │ (Nostr)     │  │ (Nostr)     │                 │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                 │
│         │                │                │                         │
│         └────────────────┼────────────────┘                         │
│                          ▼                                          │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    LEADERBOARD AGGREGATOR                      │  │
│  │  - Collects APM snapshots from all sources                    │  │
│  │  - Normalizes scores across different contexts                │  │
│  │  - Applies time decay for recency weighting                   │  │
│  │  - Calculates rankings and percentiles                        │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                          │                                          │
│         ┌────────────────┼────────────────┐                         │
│         ▼                ▼                ▼                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │ CLI Display │  │ Dashboard   │  │ Nostr Event │                 │
│  │             │  │ Widget      │  │ Publication │                 │
│  └─────────────┘  └─────────────┘  └─────────────┘                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Leaderboard Scopes

### 1. Personal Leaderboard (Local)
Compare your own agents/sessions over time.

```
┌─────────────────────────────────────────────────────────────────┐
│  YOUR PERSONAL BESTS                                             │
├─────────────────────────────────────────────────────────────────┤
│  #1  2025-12-20 14:32  session-abc123    47.2 APM  ⭐ ELITE     │
│  #2  2025-12-19 09:15  session-def456    38.1 APM  🔥 HIGH      │
│  #3  2025-12-18 16:45  session-ghi789    31.4 APM  🔥 HIGH      │
│  ...                                                             │
│  Current Session:       session-xyz999    22.3 APM  ✓ PRODUCTIVE│
│  Rank: #7 of 42 sessions                                         │
└─────────────────────────────────────────────────────────────────┘
```

### 2. Project Leaderboard (Team)
Compare agents working on the same project.

```
┌─────────────────────────────────────────────────────────────────┐
│  PROJECT: openagents                     Last 7 Days            │
├─────────────────────────────────────────────────────────────────┤
│  RANK  AGENT              SESSIONS  AVG APM   BEST    TREND     │
├─────────────────────────────────────────────────────────────────┤
│   1    autopilot-alpha       12      23.4     47.2      ↑       │
│   2    autopilot-beta         8      21.1     35.6      →       │
│   3    claude-interactive    24       4.8      8.2      ↑       │
│   4    codex-worker           5      18.7     28.3      ↓       │
└─────────────────────────────────────────────────────────────────┘
```

### 3. Global Leaderboard (Ecosystem)
Anonymous, opt-in comparison across the OpenAgents network.

```
┌─────────────────────────────────────────────────────────────────┐
│  GLOBAL APM LEADERBOARD                  December 2025          │
├─────────────────────────────────────────────────────────────────┤
│  RANK  AGENT (anon)      PROJECT TYPE    AVG APM   PERCENTILE   │
├─────────────────────────────────────────────────────────────────┤
│   1    agent-7f3a...     rust-workspace   31.2      99th        │
│   2    agent-2b8c...     monorepo         28.7      98th        │
│   3    agent-9d1e...     rust-crate       27.4      97th        │
│  ...                                                             │
│  247   agent-4c5f...     typescript       19.2      75th        │
│  ...                                                             │
│  YOUR AGENT: #312 of 1,247 (top 25%)                            │
└─────────────────────────────────────────────────────────────────┘
```

## Data Model

### LeaderboardEntry

```rust
/// A single entry in an APM leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Unique identifier (agent pubkey hash or session ID)
    pub id: String,
    
    /// Display name (can be anonymized)
    pub display_name: String,
    
    /// APM source type
    pub source: APMSource,
    
    /// Leaderboard scope
    pub scope: LeaderboardScope,
    
    /// Average APM over the time window
    pub avg_apm: f64,
    
    /// Best (peak) APM achieved
    pub best_apm: f64,
    
    /// Number of sessions in the window
    pub session_count: u32,
    
    /// Total actions performed
    pub total_actions: u32,
    
    /// Total duration in minutes
    pub total_duration_minutes: f64,
    
    /// Current rank in the leaderboard
    pub rank: u32,
    
    /// Percentile (0-100)
    pub percentile: f64,
    
    /// Trend indicator (-1, 0, +1)
    pub trend: i8,
    
    /// Time window for this entry
    pub window: LeaderboardWindow,
    
    /// When this entry was last updated
    pub updated_at: DateTime<Utc>,
}

/// Leaderboard scope
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LeaderboardScope {
    /// Personal bests (single user)
    Personal,
    /// Project-level (team)
    Project,
    /// Organization-level
    Organization,
    /// Global (ecosystem-wide)
    Global,
}

/// Time window for leaderboard calculation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LeaderboardWindow {
    /// Current day
    Today,
    /// Last 7 days
    Week,
    /// Last 30 days
    Month,
    /// Current quarter
    Quarter,
    /// All time
    AllTime,
}
```

### LeaderboardConfig

```rust
/// Configuration for leaderboard behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardConfig {
    /// Whether to participate in global leaderboard
    pub global_opt_in: bool,
    
    /// Anonymization level for global participation
    pub anonymization: AnonymizationLevel,
    
    /// Minimum sessions required to appear on leaderboard
    pub min_sessions: u32,
    
    /// Minimum total duration (minutes) to qualify
    pub min_duration_minutes: f64,
    
    /// Whether to publish to Nostr relays
    pub publish_to_nostr: bool,
    
    /// Relay URLs for publication
    pub relay_urls: Vec<String>,
    
    /// How often to update rankings (seconds)
    pub update_interval_secs: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnonymizationLevel {
    /// Full identity visible
    None,
    /// Partial anonymization (first/last chars of pubkey)
    Partial,
    /// Fully anonymous (random identifier)
    Full,
}
```

## Database Schema

```sql
-- Leaderboard entries table
CREATE TABLE leaderboard_entries (
    id TEXT NOT NULL PRIMARY KEY,
    display_name TEXT NOT NULL,
    source TEXT NOT NULL,
    scope TEXT NOT NULL,
    avg_apm REAL NOT NULL,
    best_apm REAL NOT NULL,
    session_count INTEGER NOT NULL,
    total_actions INTEGER NOT NULL,
    total_duration_minutes REAL NOT NULL,
    rank INTEGER NOT NULL,
    percentile REAL NOT NULL,
    trend INTEGER NOT NULL DEFAULT 0,
    window TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    
    CHECK(source IN ('autopilot', 'claude_code', 'combined')),
    CHECK(scope IN ('personal', 'project', 'organization', 'global')),
    CHECK(window IN ('today', 'week', 'month', 'quarter', 'all_time'))
);

CREATE INDEX idx_leaderboard_scope_window ON leaderboard_entries(scope, window);
CREATE INDEX idx_leaderboard_rank ON leaderboard_entries(scope, window, rank);
CREATE INDEX idx_leaderboard_updated ON leaderboard_entries(updated_at);

-- Leaderboard configuration
CREATE TABLE leaderboard_config (
    id TEXT NOT NULL PRIMARY KEY DEFAULT 'default',
    global_opt_in INTEGER NOT NULL DEFAULT 0,
    anonymization TEXT NOT NULL DEFAULT 'full',
    min_sessions INTEGER NOT NULL DEFAULT 5,
    min_duration_minutes REAL NOT NULL DEFAULT 30.0,
    publish_to_nostr INTEGER NOT NULL DEFAULT 0,
    relay_urls TEXT, -- JSON array
    update_interval_secs INTEGER NOT NULL DEFAULT 3600,
    updated_at TEXT NOT NULL
);

-- Personal bests tracking
CREATE TABLE personal_bests (
    id TEXT NOT NULL PRIMARY KEY,
    metric TEXT NOT NULL, -- 'apm', 'velocity_score', 'streak'
    value REAL NOT NULL,
    session_id TEXT NOT NULL,
    achieved_at TEXT NOT NULL,
    project TEXT,
    
    UNIQUE(metric, project)
);

CREATE INDEX idx_personal_bests_metric ON personal_bests(metric);
```

## Nostr Integration

### APM Leaderboard Events

Leaderboard data can be published to Nostr for decentralized sharing.

#### Kind 38050: APM Score Publication

```json
{
  "kind": 38050,
  "pubkey": "<agent_pubkey>",
  "created_at": 1735084800,
  "tags": [
    ["d", "apm-score-week"],
    ["scope", "global"],
    ["window", "week"],
    ["project_type", "rust-workspace"],
    ["avg_apm", "23.4"],
    ["best_apm", "47.2"],
    ["session_count", "12"],
    ["anonymized", "true"]
  ],
  "content": "",
  "sig": "..."
}
```

#### Kind 38051: Leaderboard Snapshot

Published by relay aggregators to create point-in-time rankings:

```json
{
  "kind": 38051,
  "pubkey": "<aggregator_pubkey>",
  "created_at": 1735084800,
  "tags": [
    ["d", "global-leaderboard-week-2025-W52"],
    ["scope", "global"],
    ["window", "week"],
    ["total_participants", "1247"],
    ["median_apm", "12.3"],
    ["p90_apm", "24.5"],
    ["p99_apm", "38.2"]
  ],
  "content": "{\"rankings\": [{\"rank\": 1, \"id\": \"agent-7f3a...\", \"avg_apm\": 31.2}, ...]}",
  "sig": "..."
}
```

## CLI Commands

```bash
# View personal leaderboard
cargo autopilot apm leaderboard
cargo autopilot apm leaderboard --scope personal --window week

# View project leaderboard
cargo autopilot apm leaderboard --scope project --project openagents

# View global leaderboard (requires opt-in)
cargo autopilot apm leaderboard --scope global

# Configure global participation
cargo autopilot apm leaderboard config --opt-in --anonymization full
cargo autopilot apm leaderboard config --opt-out

# Publish score to Nostr
cargo autopilot apm leaderboard publish

# Show your global rank
cargo autopilot apm leaderboard rank
```

### Example Output

```
$ cargo autopilot apm leaderboard --scope project --window week

APM Leaderboard: openagents (Last 7 Days)
══════════════════════════════════════════════════════════════════

 RANK  AGENT                SESSIONS   AVG APM   BEST APM   TREND
──────────────────────────────────────────────────────────────────
  1    autopilot-alpha          12      23.4      47.2       ↑
  2    autopilot-beta            8      21.1      35.6       →
  3    claude-interactive       24       4.8       8.2       ↑
  4    codex-worker              5      18.7      28.3       ↓

──────────────────────────────────────────────────────────────────
Your Position: #1 of 4 agents
Percentile: 100th (top performer)
Trend: ↑ (+2.3 APM vs last week)
══════════════════════════════════════════════════════════════════
```

## Dashboard Widget

The leaderboard integrates into the autopilot dashboard:

```html
<!-- Leaderboard widget for dashboard.rs -->
<div class="leaderboard-widget">
  <h3>APM Leaderboard</h3>
  <div class="leaderboard-tabs">
    <button class="active">Personal</button>
    <button>Project</button>
    <button>Global</button>
  </div>
  <table class="leaderboard-table">
    <thead>
      <tr>
        <th>Rank</th>
        <th>Agent</th>
        <th>APM</th>
        <th>Trend</th>
      </tr>
    </thead>
    <tbody id="leaderboard-body">
      <!-- Populated via HTMX -->
    </tbody>
  </table>
  <div class="your-position">
    Your rank: <span id="your-rank">#3</span> of <span id="total">42</span>
  </div>
</div>
```

## Ranking Algorithm

### Score Calculation

The leaderboard uses a weighted scoring system:

```rust
/// Calculate leaderboard score for an agent
pub fn calculate_score(entries: &[APMSnapshot]) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }
    
    // Weights for different factors
    const AVG_APM_WEIGHT: f64 = 0.50;      // Average APM matters most
    const BEST_APM_WEIGHT: f64 = 0.20;     // Peak performance
    const CONSISTENCY_WEIGHT: f64 = 0.15;   // Low variance is good
    const VOLUME_WEIGHT: f64 = 0.15;        // More sessions = more data
    
    let avg_apm = entries.iter().map(|e| e.apm).sum::<f64>() / entries.len() as f64;
    let best_apm = entries.iter().map(|e| e.apm).fold(0.0_f64, |a, b| a.max(b));
    
    // Consistency: inverse of coefficient of variation
    let variance = entries.iter()
        .map(|e| (e.apm - avg_apm).powi(2))
        .sum::<f64>() / entries.len() as f64;
    let std_dev = variance.sqrt();
    let consistency = if avg_apm > 0.0 { 1.0 / (1.0 + std_dev / avg_apm) } else { 0.0 };
    
    // Volume: logarithmic scaling (diminishing returns)
    let volume = (entries.len() as f64).ln() / 10.0_f64.ln(); // log10 normalized
    
    // Weighted score
    avg_apm * AVG_APM_WEIGHT
        + best_apm * BEST_APM_WEIGHT
        + (consistency * 50.0) * CONSISTENCY_WEIGHT  // Scale to ~0-50 range
        + (volume * 30.0) * VOLUME_WEIGHT            // Scale to ~0-30 range
}
```

### Time Decay

Recent performance weighs more heavily:

```rust
/// Apply time decay to APM scores
pub fn apply_time_decay(apm: f64, age_days: f64, half_life_days: f64) -> f64 {
    let decay_factor = 0.5_f64.powf(age_days / half_life_days);
    apm * decay_factor
}

// Default half-life: 14 days
// Session from today: 100% weight
// Session from 14 days ago: 50% weight
// Session from 28 days ago: 25% weight
```

## Privacy Considerations

### Opt-In Only
- Global leaderboard participation is strictly opt-in
- Default: local and project leaderboards only
- Clear consent flow before publishing to Nostr

### Anonymization Options
1. **None**: Full agent identity visible (pubkey, project name)
2. **Partial**: Truncated pubkey (first 4 + last 4 chars)
3. **Full**: Random identifier, regenerated periodically

### Data Minimization
- Only aggregate metrics published (no raw session data)
- No personally identifiable information
- Project names can be hashed if desired

### Withdrawal
- Users can withdraw from global leaderboard at any time
- Historical data is deleted upon withdrawal
- Nostr events remain (immutable) but marked as withdrawn

## Implementation Phases

### Phase 1: Personal Leaderboard
- [ ] `LeaderboardEntry` and `LeaderboardConfig` types
- [ ] Personal bests tracking in SQLite
- [ ] `apm leaderboard` CLI command (personal scope)
- [ ] Integration with `apm best` command

### Phase 2: Project Leaderboard
- [ ] Multi-agent tracking per project
- [ ] Project-level aggregation
- [ ] Dashboard widget
- [ ] `--scope project` CLI flag

### Phase 3: Global Leaderboard
- [ ] Nostr event types (kind 38050, 38051)
- [ ] Opt-in configuration
- [ ] Anonymization options
- [ ] Relay publication
- [ ] Global ranking queries

### Phase 4: Advanced Features
- [ ] Seasonal leaderboards (monthly/quarterly resets)
- [ ] Achievement badges
- [ ] Streak tracking
- [ ] Team aggregation
- [ ] Historical ranking trends

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Personal leaderboard usage | 80% of active users | CLI/dashboard analytics |
| Project leaderboard adoption | 50% of teams | Project count with >1 agent |
| Global opt-in rate | 20% of users | Config table queries |
| APM improvement after feature | +15% avg APM | Before/after comparison |

## Related Documents

- [APM Methodology](methodology.md) - Core APM calculation
- [d-016 Directive](../../.openagents/directives/d-016.md) - APM tracking spec
- [Weekly Reports](../autopilot/WEEKLY_REPORTS.md) - APM in reports
