# APM (Actions Per Minute) System

This document explains the comprehensive APM analysis system in OpenAgents, including the different time window measurements, their implications, and how to interpret the data.

## What is APM?

APM (Actions Per Minute) is a productivity metric borrowed from competitive gaming (specifically StarCraft) that measures the frequency of meaningful actions a user performs. In the context of Claude Code, an "action" includes:

- **Messages**: Both user queries and assistant responses
- **Tool Uses**: When Claude uses tools like Edit, Read, Bash, etc.

## Time Window Measurements

The system provides **six time-window APM calculations** showing your productivity across different time horizons:

### 1. 1 Hour APM
**Formula**: `(Actions in Last Hour) / 60 minutes`

**What it measures**: Your productivity burst in the immediate past hour

**Example Calculation**:
- 30 actions in the last hour
- **1 Hour APM**: 30 ÷ 60 = **0.5 APM**

**Use Case**: Detecting current productivity spikes or focused work sessions

### 2. 6 Hours APM
**Formula**: `(Actions in Last 6 Hours) / 360 minutes`

**What it measures**: Your productivity over a typical work session or half-day

**Example Calculation**:
- 120 actions in the last 6 hours
- **6 Hours APM**: 120 ÷ 360 = **0.33 APM**

**Use Case**: Understanding sustained productivity over extended work periods

### 3. 1 Day APM
**Formula**: `(Actions in Last 24 Hours) / 1440 minutes`

**What it measures**: Your daily coding productivity including breaks and off-hours

**Example Calculation**:
- 200 actions in the last 24 hours
- **1 Day APM**: 200 ÷ 1440 = **0.14 APM**

**Use Case**: Daily productivity tracking and routine analysis

### 4. 1 Week APM
**Formula**: `(Actions in Last 7 Days) / 10080 minutes`

**What it measures**: Your weekly productivity pattern including weekends

**Example Calculation**:
- 800 actions in the last week
- **1 Week APM**: 800 ÷ 10080 = **0.08 APM**

**Use Case**: Understanding work-life balance and weekly coding habits

### 5. 1 Month APM
**Formula**: `(Actions in Last 30 Days) / 43200 minutes`

**What it measures**: Your monthly productivity trend and project cycles

**Example Calculation**:
- 2000 actions in the last month
- **1 Month APM**: 2000 ÷ 43200 = **0.046 APM**

**Use Case**: Long-term productivity analysis and goal setting

### 6. Lifetime APM
**Formula**: `(Total Actions) / (Total Calendar Time Since First Use)`

**What it measures**: Your overall productivity from first conversation to present

**Example Calculation**:
- 5000 total actions over 100 days of usage
- **Lifetime APM**: 5000 ÷ 144000 = **0.035 APM**

**Use Case**: Overall engagement and long-term usage patterns

## Understanding the Numbers

### Real-World Context

Based on analysis of 279 real Claude Code conversations:
- **Typical Daily APM**: 0.05-0.2 APM during active development periods
- **Peak Hour APM**: Can reach 2-10 APM during intensive coding sessions
- **Tool Distribution**: Bash (34%), Read (23%), Edit (21%)

### Time Window Comparison Examples

**High Burst Activity (2+ APM in 1 hour)**:
- Rapid debugging sessions
- Live coding or pair programming
- Intensive feature development sprints

**Sustained Activity (0.1-1 APM over 6+ hours)**:
- Full-day development work
- Project implementation phases
- Comprehensive refactoring tasks

**Background Activity (0.01-0.1 APM over days/weeks)**:
- Learning and exploration phases
- Maintenance and small fixes
- Long-term project work with breaks

### Productivity Patterns

The six time windows reveal different aspects of your coding behavior:

```
1 Hour: Shows immediate focus and productivity bursts
6 Hours: Reveals sustained work session effectiveness  
1 Day: Captures daily coding habits and routines
1 Week: Shows work-life balance and weekly patterns
1 Month: Indicates project cycles and long-term trends
Lifetime: Provides overall engagement baseline
```

**Example Analysis**:
- Developer: 1h=2.0, 6h=0.8, 1d=0.2, 1w=0.1, 1m=0.05, lifetime=0.03
- **Interpretation**: Currently in a high-productivity coding session (1h), sustained over several hours (6h), but lower overall daily activity (1d), indicating focused bursts rather than continuous coding

## What Actions Are Counted?

### Messages
- User prompts and questions
- Claude's responses and explanations
- Follow-up clarifications

### Tool Uses
The system tracks usage of all Claude Code tools:

| Category | Tools | % of Usage |
|----------|-------|------------|
| **System Operations** | Bash | ~34% |
| **File Operations** | Read, LS, Glob | ~23% |
| **Code Generation** | Edit, MultiEdit, Write | ~21% |
| **Search** | Grep, WebSearch, WebFetch | ~12% |
| **Planning** | TodoWrite, TodoRead | ~5% |
| **Other** | Task, NotebookRead, etc. | ~5% |

## Interpreting Your APM Data

### Productivity Phases

**High APM Periods** indicate:
- Active problem-solving
- Rapid iteration and testing
- Flow state coding sessions
- Debugging and troubleshooting

**Low APM Periods** indicate:
- Research and learning phases
- Code reading and understanding
- Planning and architecture design
- Breaks between coding sessions

### Time Window Strategy

Use different windows for different insights:
- **1h & 6h**: Optimize current work sessions
- **1d & 1w**: Improve daily/weekly habits
- **1m & lifetime**: Track long-term progress and goals

### Performance Optimization

To improve your APM effectively:
1. **Focus Time**: Schedule uninterrupted coding blocks
2. **Tool Mastery**: Learn efficient Claude Code workflows
3. **Batch Tasks**: Group similar activities together
4. **Regular Analysis**: Review patterns weekly/monthly

## Technical Implementation

### Data Sources

The APM system now analyzes conversations from **two sources**:

#### 1. CLI Conversations
- **Location**: `~/.claude/projects/*/[session].jsonl`
- **Source**: Official Claude Code CLI sessions
- **Storage**: Local JSONL files on user's device
- **Analysis**: Real-time parsing of JSONL conversation entries

#### 2. SDK Conversations  
- **Location**: Convex backend (`claudeSessions` and `claudeMessages` tables)
- **Source**: Claude Code SDK sessions (non-interactive)
- **Storage**: Convex cloud database
- **Analysis**: Real-time queries to Convex backend

### Calculation Method
- **Time windows**: Calculated from current timestamp backwards
- **Action counting**: Messages (user/assistant) + tool uses from both sources
- **APM formula**: Total Actions ÷ Time Window Duration in Minutes
- **Combination**: CLI and SDK stats are merged using weighted averages

### Viewing Modes

The stats page provides three viewing modes:

1. **Combined** (default): Shows totals across both CLI and SDK conversations
2. **CLI Only**: Shows statistics from local Claude Code CLI sessions  
3. **SDK Only**: Shows statistics from SDK/Convex conversations

### Breakdown Analysis

When viewing in **Combined** mode, the system displays:
- Combined totals and averages across both sources
- Side-by-side breakdown comparing CLI vs SDK metrics
- Unified tool usage statistics and recent sessions

### Accuracy Notes
- Actions only counted when conversations are active
- Silent periods (no conversation files) don't contribute to APM
- Time zones handled using UTC normalization
- Partial sessions at window boundaries are included proportionally
- SDK data requires Convex backend connectivity
- CLI data works offline from local files