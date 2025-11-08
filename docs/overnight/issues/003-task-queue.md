# Issue #003: Implement TaskQueue

**Component**: Orchestration Layer - Work Queue
**Priority**: P0 (Critical Path)
**Estimated Effort**: 2-3 days
**Dependencies**: None (uses existing Tinyvex)
**Assignee**: TBD

---

## Overview

Implement `TaskQueue`, a persistent work queue with SQLite backing (via TinyvexDbLayer) that tracks overnight tasks through their lifecycle: pending → in_progress → completed/failed.

**Key Change from Audit**: Use existing `TinyvexDbLayer` with new `overnight_tasks` table in same DB (not separate store).

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/TaskQueue.swift`

**References**:
- `TinyvexDbLayer`: ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1
- DB path example: ios/OpenAgents/TinyvexManager.swift:28

---

## Requirements

### Functional

1. **CRUD Operations**: enqueue, dequeue, updateStatus, all (with filters), cleanup
2. **Persistence**: SQLite via Tinyvex append-only log + materialized view
3. **Deduplication**: opHash prevents duplicate tasks
4. **Status Lifecycle**: pending → in_progress → completed/failed/cancelled
5. **Observable**: AsyncStream of queue updates for UI reactivity
6. **Priority Scheduling**: High-priority tasks dequeued first

### Schema

```sql
CREATE TABLE overnight_tasks (
    id TEXT PRIMARY KEY,
    op_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    decision_json TEXT NOT NULL,
    session_id TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,
    error TEXT,
    metadata_json TEXT
);

CREATE INDEX idx_status ON overnight_tasks(status);
CREATE INDEX idx_created_at ON overnight_tasks(created_at);
CREATE INDEX idx_op_hash ON overnight_tasks(op_hash);
```

---

## Implementation

```swift
actor TaskQueue {
    private let db: TinyvexDatabase
    private let updatesContinuation: AsyncStream<TaskQueueUpdate>.Continuation

    init(databaseURL: URL) async throws {
        self.db = try await TinyvexDatabase(url: databaseURL)
        // Create tables, indices
        try await createSchema()

        // Setup observable stream
        let (stream, continuation) = AsyncStream<TaskQueueUpdate>.makeStream()
        self.updatesContinuation = continuation
        self.updates = stream
    }

    func enqueue(_ task: OvernightTask) async throws -> TaskID {
        // Check for duplicate opHash
        if let existing = try await findByOpHash(task.opHash), existing.status != .failed {
            return existing.id
        }

        // Insert task
        try await db.execute(
            """
            INSERT INTO overnight_tasks (id, op_hash, status, decision_json, created_at, metadata_json)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            [task.id, task.opHash, task.status.rawValue, task.decision.json, task.createdAt.timeIntervalSince1970, task.metadata.json]
        )

        updatesContinuation.yield(.enqueued(task.id))
        return task.id
    }

    func dequeue() async throws -> OvernightTask? {
        // Get highest priority pending task
        let rows = try await db.query(
            """
            SELECT * FROM overnight_tasks
            WHERE status = 'pending'
            ORDER BY
              CASE json_extract(decision_json, '$.priority')
                WHEN 'high' THEN 1
                WHEN 'medium' THEN 2
                WHEN 'low' THEN 3
              END,
              created_at ASC
            LIMIT 1
            """
        )

        guard let row = rows.first else { return nil }

        let task = try parseTask(from: row)
        updatesContinuation.yield(.dequeued(task.id))
        return task
    }

    func updateStatus(_ taskId: TaskID, status: TaskStatus) async throws {
        try await db.execute(
            "UPDATE overnight_tasks SET status = ?, completed_at = ? WHERE id = ?",
            [status.rawValue, status == .completed ? Date().timeIntervalSince1970 : nil, taskId]
        )
        updatesContinuation.yield(.statusChanged(taskId, status))
    }

    func all(filter: TaskFilter? = nil) async throws -> [OvernightTask] {
        // Build query with filters
        var sql = "SELECT * FROM overnight_tasks WHERE 1=1"
        var params: [Any] = []

        if let status = filter?.status {
            sql += " AND status = ?"
            params.append(status.rawValue)
        }
        // ... more filters

        let rows = try await db.query(sql, params)
        return try rows.map { try parseTask(from: $0) }
    }

    var updates: AsyncStream<TaskQueueUpdate> { get }
}

struct OvernightTask: Codable, Identifiable {
    let id: TaskID
    let opHash: String
    var status: TaskStatus
    let decision: TaskDecision
    var sessionId: String?
    let createdAt: Date
    var startedAt: Date?
    var completedAt: Date?
    var error: String?
    let metadata: [String: String]
}

enum TaskStatus: String, Codable {
    case pending, in_progress, completed, failed, cancelled
}

enum TaskQueueUpdate {
    case enqueued(TaskID)
    case statusChanged(TaskID, TaskStatus)
    case dequeued(TaskID)
}
```

---

## Testing

1. `testEnqueueDequeue()` - Basic CRUD
2. `testDeduplication()` - Same opHash returns existing ID
3. `testPriorityScheduling()` - High priority dequeued first
4. `testStatusLifecycle()` - All state transitions
5. `testObservableUpdates()` - AsyncStream yields events
6. `testCleanup()` - Remove old completed tasks

---

## Acceptance Criteria

- [ ] SQLite schema created via Tinyvex
- [ ] Deduplication via opHash works
- [ ] Priority scheduling (high > medium > low)
- [ ] Observable updates via AsyncStream
- [ ] All unit tests pass (≥95% coverage)
- [ ] No memory leaks

---

## References

- Architecture: `architecture.md` - TaskQueue section
- Existing: Tinyvex database wrapper
