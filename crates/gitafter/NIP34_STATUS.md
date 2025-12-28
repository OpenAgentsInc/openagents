# NIP-34 Implementation Status

This document tracks the implementation status of NIP-34 (Git Stuff) in GitAfter.

## Core NIP-34 Event Types

### Repository Events

| Kind | Name | Status | Notes |
|------|------|--------|-------|
| 30617 | Repository Announcement | ✅ Complete | Can fetch and display repositories |
| 30618 | Repository State | ✅ Complete | Shows branches, tags, HEAD |

**Implementation**:
- View list of repositories
- Search repositories (NIP-50)
- Watch/subscribe to repositories
- Display clone URLs (git, https)
- Show repository metadata (name, description, owner)

**Missing**:
- Create new repositories
- Update repository state

### Issue Events

| Kind | Name | Status | Notes |
|------|------|--------|-------|
| 1621 | Issue | ✅ Complete | Can view, search, and create issues |

**Implementation**:
- View issues list for a repository
- View issue details with description
- Filter issues (open, closed)
- Create issue form (UI complete, publishing pending)
- Search issues (NIP-50)

**Missing**:
- Close/reopen issues
- Edit issue content
- Issue labels/milestones

### Patch and Pull Request Events

| Kind | Name | Status | Notes |
|------|------|--------|-------|
| 1617 | Patch | ✅ Complete | Can view and create patches |
| 1618 | Pull Request | ✅ Complete | Full PR support including stacked diffs |
| 1619 | PR Update | ⚠️ Partial | Status updates work, content updates TODO |

**Implementation**:
- View patches list for a repository
- View patch details with diff
- View pull requests list
- View PR details with metadata
- Create PR form with all fields (UI complete)
- Create patch form (UI complete)
- Stacked diffs support:
  - `depends_on` tag for dependencies
  - `stack` tag for grouping
  - `layer` tag for ordering
- Trajectory integration:
  - `trajectory` tag links to session
  - `trajectory_hash` for verification
  - Display trajectory timeline in PR view

**Missing**:
- Publish PRs/patches (requires identity)
- Apply patches to local repo
- Generate patches from local changes
- Inline code comments

### Status Events

| Kind | Name | Status | Notes |
|------|------|--------|-------|
| 1630 | Open | ✅ Complete | Mark PR/patch as open |
| 1631 | Applied/Merged | ✅ Complete | Mark as merged |
| 1632 | Closed | ✅ Complete | Mark as closed |
| 1633 | Draft | ✅ Complete | Mark as draft |

**Implementation**:
- StatusEventBuilder in events.rs
- UI for changing status
- Display current status
- Status history

**Missing**:
- Publish status changes (requires identity)

## GitAfter Extensions

We extend NIP-34 with agent-native workflows:

| Kind | Name | Status | Notes |
|------|------|--------|-------|
| 1634 | Issue Claim | ✅ Complete | Agent claims issue for work |
| 1635 | Work Assignment | ✅ Complete | Maintainer assigns work |
| 1636 | Bounty Offer | ✅ Complete | Attach Lightning bounty to issue |
| 1637 | Bounty Claim | ✅ Complete | Claim bounty upon PR merge |
| 39230 | Trajectory Session | ✅ Integrated | Link PR to agent work session |
| 39231 | Trajectory Event | ✅ Integrated | Individual trajectory steps |

### Issue Claims (kind:1634)

**Implementation**:
- IssueClaimBuilder in events.rs
- UI for claiming issues
- Display claims on issue detail page
- Link to trajectory session
- Show estimated completion time

**Event structure**:
```json
{
  "kind": 1634,
  "tags": [
    ["e", "<issue-id>", "", "root"],
    ["a", "30617:<owner>:<repo>"],
    ["p", "<issue-author>"],
    ["trajectory", "<session-id>"],
    ["estimate", "7200"]
  ],
  "content": "I'll work on this..."
}
```

### Bounty Offers (kind:1636)

**Implementation**:
- BountyOfferBuilder in events.rs
- UI for creating bounties
- Display bounties on issue detail page
- Show amount, expiry, conditions

**Event structure**:
```json
{
  "kind": 1636,
  "tags": [
    ["e", "<issue-id>", "", "root"],
    ["a", "30617:<owner>:<repo>"],
    ["amount", "50000"],
    ["expiry", "<timestamp>"],
    ["conditions", "must include tests"]
  ]
}
```

### Stacked Diffs

**Tags on PR events (kind:1618)**:
```json
{
  "kind": 1618,
  "tags": [
    ["depends_on", "<pr-event-id>"],  // Dependency
    ["stack", "<uuid>"],               // Group ID
    ["layer", "2", "4"]                // Layer 2 of 4
  ]
}
```

**Implementation**:
- Parse and display stack relationships
- Show dependency chain in PR view
- Indicate merge order requirements
- Display trajectory per layer

**Missing**:
- Stack visualization (graph view)
- Automatic restack on base changes
- Per-layer bounties

### Trajectory Integration

PRs can link to NIP-SA trajectory sessions:

```json
{
  "kind": 1618,
  "tags": [
    ["trajectory", "<session-id>", "<relay>"],
    ["trajectory_hash", "<sha256>"]
  ]
}
```

**Implementation**:
- Fetch trajectory session (kind:39230)
- Fetch trajectory events (kind:39231)
- Display trajectory timeline in PR view
- Show tool calls, reasoning, file edits
- Expandable/collapsible trajectory view

**Missing**:
- Trajectory hash verification
- Flag suspicious trajectories
- Compare diff to trajectory edits

## Event Builders

All builders in `src/nostr/events.rs`:

| Builder | Kind | Status | Tests |
|---------|------|--------|-------|
| PullRequestBuilder | 1618 | ✅ Complete | ✅ |
| PatchBuilder | 1617 | ✅ Complete | ✅ |
| IssueClaimBuilder | 1634 | ✅ Complete | ✅ |
| BountyOfferBuilder | 1636 | ✅ Complete | ✅ |
| WorkAssignmentBuilder | 1635 | ✅ Complete | ✅ |
| BountyClaimBuilder | 1637 | ✅ Complete | ✅ |
| StatusEventBuilder | 1630-1633 | ✅ Complete | ✅ |

All builders:
- Follow builder pattern
- Return EventTemplate
- Have comprehensive tests
- Support optional fields

## Publishing Pipeline

Event creation flow:

```
User fills form
    ↓
Handler validates input
    ↓
Builder creates EventTemplate
    ↓
❌ Sign with identity (NOT YET IMPLEMENTED)
    ↓
❌ Publish to relays (NOT YET IMPLEMENTED)
    ↓
❌ Cache locally (NOT YET IMPLEMENTED)
    ↓
Return success/error
```

**Blocker**: Event signing requires identity integration (issue #342)

## What Works

- ✅ Browse repositories from Nostr relays
- ✅ View repository details
- ✅ View issues, patches, PRs
- ✅ Search (NIP-50)
- ✅ Watch repositories
- ✅ View agent profiles
- ✅ Display trajectories
- ✅ Create event forms (UI)
- ✅ Event builders (all types)
- ✅ Real-time updates via WebSocket
- ✅ Local event cache (SQLite)
- ✅ Clone repositories

## What's Missing

### High Priority

1. **Event Signing & Publishing** (issue #342)
   - Integrate wallet identity
   - Sign EventTemplates
   - Publish to relays
   - Update local cache

2. **Repository Creation**
   - Create kind:30617 event
   - Initialize git repository
   - Push to hosting provider

3. **Patch Application**
   - Apply kind:1617 patches to local repo
   - Git operations (apply, test, commit)

### Medium Priority

4. **Code Review Interface**
   - Inline comments on diffs
   - Review summary (approve/request changes)
   - Display review threads

5. **Bounty Payments**
   - NIP-57 zap integration
   - Lightning invoice generation
   - Payment confirmation

6. **Repository Management**
   - Update repository state (kind:30618)
   - Manage collaborators
   - Access control

### Low Priority

7. **Advanced Features**
   - Stack visualization
   - Automatic restack
   - Multi-agent collaboration (NIP-EE)
   - Reputation tracking (NIP-32)

## Testing

Event builder tests: `cargo test -p gitafter events`

Coverage:
- All builders have unit tests
- Event structure validation
- Tag correctness
- Optional field handling

Missing:
- Integration tests with real relays
- E2E tests for full flows
- UI tests

## Performance

Current limitations:
- SQLite cache grows unbounded
- No pagination on large result sets
- Full event resync on startup
- No incremental updates

Optimizations needed:
- Add cache expiry
- Implement pagination
- Use REQ LIMIT properly
- Store subscription state

## Compatibility

Tested with:
- relay.damus.io ✅
- nos.lol ✅
- relay.nostr.band ✅

Known issues:
- Some relays don't support NIP-50 search
- Relay rate limiting varies
- No NIP-42 auth support yet

## Next Steps

1. Complete identity integration (#342)
2. Enable event publishing
3. Add repository creation
4. Build code review UI
5. Integrate Lightning payments

See [DEVELOPMENT.md](./DEVELOPMENT.md) for implementation details.
