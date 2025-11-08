import Foundation

// MARK: - Task ID

/// Unique identifier for an overnight task
public typealias TaskID = String

// MARK: - Task Status

/// Lifecycle status of an overnight task
public enum TaskStatus: String, Codable, Sendable {
    case pending        // Queued, not yet started
    case in_progress    // Currently executing
    case completed      // Successfully finished
    case failed         // Execution failed with error
    case cancelled      // User/system cancelled
    case skipped        // Skipped due to constraints (time budget, etc.)
}

// MARK: - Overnight Task

/// A task to be executed by the overnight orchestration system
public struct OvernightTask: Codable, Sendable, Identifiable {
    /// Unique task identifier
    public let id: TaskID

    /// Operation hash for deduplication (same task = same hash)
    public let opHash: String

    /// Current status
    public var status: TaskStatus

    /// The decision that created this task
    public let decision: DecisionOutput

    /// Associated ACP session ID (if task has been started)
    public var sessionId: String?

    /// When the task was created
    public let createdAt: Date

    /// When the task execution started
    public var startedAt: Date?

    /// When the task execution completed (success or failure)
    public var completedAt: Date?

    /// Error message if status is .failed
    public var error: String?

    /// Arbitrary metadata (PR number, branch name, etc.)
    public var metadata: [String: String]

    public init(
        id: TaskID = UUID().uuidString,
        opHash: String,
        status: TaskStatus = .pending,
        decision: DecisionOutput,
        sessionId: String? = nil,
        createdAt: Date = Date(),
        startedAt: Date? = nil,
        completedAt: Date? = nil,
        error: String? = nil,
        metadata: [String: String] = [:]
    ) {
        self.id = id
        self.opHash = opHash
        self.status = status
        self.decision = decision
        self.sessionId = sessionId
        self.createdAt = createdAt
        self.startedAt = startedAt
        self.completedAt = completedAt
        self.error = error
        self.metadata = metadata
    }
}

// MARK: - Task Queue Error

public enum TaskQueueError: Error, LocalizedError {
    case taskNotFound(TaskID)
    case invalidStatusTransition(from: TaskStatus, to: TaskStatus)
    case databaseError(String)
    case duplicateTask(opHash: String)

    public var errorDescription: String? {
        switch self {
        case .taskNotFound(let id):
            return "Task not found: \(id)"
        case .invalidStatusTransition(let from, let to):
            return "Invalid status transition from \(from) to \(to)"
        case .databaseError(let msg):
            return "Database error: \(msg)"
        case .duplicateTask(let hash):
            return "Duplicate task with opHash: \(hash)"
        }
    }
}

// MARK: - Task Queue

/// Actor managing the queue of overnight tasks with SQLite persistence
public actor TaskQueue {
    private let db: TinyvexDbLayer

    /// Initialize task queue with database connection
    /// - Parameter db: TinyvexDbLayer instance for persistence
    public init(db: TinyvexDbLayer) async throws {
        self.db = db
        try await createTableIfNeeded()
    }

    // MARK: - Create/Enqueue

    /// Add a new task to the queue
    /// - Parameter task: The task to enqueue
    /// - Returns: The task ID
    /// - Throws: TaskQueueError.duplicateTask if opHash already exists
    public func enqueue(_ task: OvernightTask) async throws -> TaskID {
        // Check for duplicate opHash
        if try await findByOpHash(task.opHash) != nil {
            throw TaskQueueError.duplicateTask(opHash: task.opHash)
        }

        let taskJSON = try JSONEncoder().encode(task)
        let taskString = String(data: taskJSON, encoding: .utf8)!

        let sql = """
        INSERT INTO overnight_tasks (id, op_hash, status, decision_json, session_id, created_at, started_at, completed_at, error, metadata_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
        """

        let metadataJSON = try JSONEncoder().encode(task.metadata)
        let metadataString = String(data: metadataJSON, encoding: .utf8)!

        try await db.execute(sql, params: [
            task.id,
            task.opHash,
            task.status.rawValue,
            taskString,
            task.sessionId as Any,
            task.createdAt.timeIntervalSince1970,
            task.startedAt?.timeIntervalSince1970 as Any,
            task.completedAt?.timeIntervalSince1970 as Any,
            task.error as Any,
            metadataString
        ])

        return task.id
    }

    // MARK: - Read

    /// Get a task by ID
    /// - Parameter id: Task ID
    /// - Returns: The task, or nil if not found
    public func get(_ id: TaskID) async throws -> OvernightTask? {
        let sql = "SELECT decision_json FROM overnight_tasks WHERE id = ? LIMIT 1;"
        guard let row = try await db.queryOne(sql, params: [id]),
              let jsonString = row["decision_json"] as? String,
              let jsonData = jsonString.data(using: String.Encoding.utf8) else {
            return nil
        }

        return try JSONDecoder().decode(OvernightTask.self, from: jsonData)
    }

    /// Find a task by opHash (for deduplication)
    /// - Parameter opHash: Operation hash
    /// - Returns: The task, or nil if not found
    public func findByOpHash(_ opHash: String) async throws -> OvernightTask? {
        let sql = "SELECT decision_json FROM overnight_tasks WHERE op_hash = ? LIMIT 1;"
        guard let row = try await db.queryOne(sql, params: [opHash]),
              let jsonString = row["decision_json"] as? String,
              let jsonData = jsonString.data(using: String.Encoding.utf8) else {
            return nil
        }

        return try JSONDecoder().decode(OvernightTask.self, from: jsonData)
    }

    /// Get the next pending task (FIFO by created_at)
    /// - Returns: The next pending task, or nil if queue is empty
    public func dequeue() async throws -> OvernightTask? {
        let sql = """
        SELECT decision_json FROM overnight_tasks
        WHERE status = 'pending'
        ORDER BY created_at ASC
        LIMIT 1;
        """

        guard let row = try await db.queryOne(sql, params: []),
              let jsonString = row["decision_json"] as? String,
              let jsonData = jsonString.data(using: String.Encoding.utf8) else {
            return nil
        }

        return try JSONDecoder().decode(OvernightTask.self, from: jsonData)
    }

    /// Get all tasks matching a status filter
    /// - Parameter status: Optional status filter. If nil, returns all tasks.
    /// - Returns: Array of tasks
    public func all(status: TaskStatus? = nil) async throws -> [OvernightTask] {
        let sql: String
        let params: [Any]

        if let status = status {
            sql = "SELECT decision_json FROM overnight_tasks WHERE status = ? ORDER BY created_at DESC;"
            params = [status.rawValue]
        } else {
            sql = "SELECT decision_json FROM overnight_tasks ORDER BY created_at DESC;"
            params = []
        }

        let rows = try await db.queryAll(sql, params: params)
        return try rows.compactMap { row in
            guard let jsonString = row["decision_json"] as? String,
                  let jsonData = jsonString.data(using: String.Encoding.utf8) else {
                return nil
            }
            return try JSONDecoder().decode(OvernightTask.self, from: jsonData)
        }
    }

    // MARK: - Update

    /// Update task status
    /// - Parameters:
    ///   - id: Task ID
    ///   - status: New status
    /// - Throws: TaskQueueError.taskNotFound or TaskQueueError.invalidStatusTransition
    public func updateStatus(_ id: TaskID, status: TaskStatus) async throws {
        guard var task = try await get(id) else {
            throw TaskQueueError.taskNotFound(id)
        }

        // Validate status transition
        try validateStatusTransition(from: task.status, to: status)

        task.status = status

        // Update timestamps based on new status
        if status == .in_progress && task.startedAt == nil {
            task.startedAt = Date()
        }

        if [.completed, .failed, .cancelled, .skipped].contains(status) && task.completedAt == nil {
            task.completedAt = Date()
        }

        try await update(task)
    }

    /// Update entire task (status, sessionId, error, metadata, etc.)
    /// - Parameter task: Updated task
    public func update(_ task: OvernightTask) async throws {
        let taskJSON = try JSONEncoder().encode(task)
        let taskString = String(data: taskJSON, encoding: .utf8)!

        let metadataJSON = try JSONEncoder().encode(task.metadata)
        let metadataString = String(data: metadataJSON, encoding: .utf8)!

        let sql = """
        UPDATE overnight_tasks
        SET status = ?, decision_json = ?, session_id = ?, started_at = ?, completed_at = ?, error = ?, metadata_json = ?
        WHERE id = ?;
        """

        try await db.execute(sql, params: [
            task.status.rawValue,
            taskString,
            task.sessionId as Any,
            task.startedAt?.timeIntervalSince1970 as Any,
            task.completedAt?.timeIntervalSince1970 as Any,
            task.error as Any,
            metadataString,
            task.id
        ])
    }

    // MARK: - Delete

    /// Delete a task by ID
    /// - Parameter id: Task ID
    public func delete(_ id: TaskID) async throws {
        let sql = "DELETE FROM overnight_tasks WHERE id = ?;"
        try await db.execute(sql, params: [id])
    }

    /// Clear all tasks (use with caution!)
    public func deleteAll() async throws {
        let sql = "DELETE FROM overnight_tasks;"
        try await db.execute(sql, params: [])
    }

    // MARK: - Private Helpers

    private func createTableIfNeeded() async throws {
        let sql = """
        CREATE TABLE IF NOT EXISTS overnight_tasks (
            id TEXT PRIMARY KEY,
            op_hash TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL,
            decision_json TEXT NOT NULL,
            session_id TEXT,
            created_at REAL NOT NULL,
            started_at REAL,
            completed_at REAL,
            error TEXT,
            metadata_json TEXT NOT NULL,
            UNIQUE(op_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_overnight_tasks_status ON overnight_tasks(status);
        CREATE INDEX IF NOT EXISTS idx_overnight_tasks_created_at ON overnight_tasks(created_at);
        CREATE INDEX IF NOT EXISTS idx_overnight_tasks_op_hash ON overnight_tasks(op_hash);
        """

        try await db.execute(sql, params: [])
    }

    private func validateStatusTransition(from: TaskStatus, to: TaskStatus) throws {
        // Valid transitions:
        // pending → in_progress, cancelled, skipped
        // in_progress → completed, failed, cancelled
        // completed/failed/cancelled/skipped → (no transitions)

        let validTransitions: [TaskStatus: [TaskStatus]] = [
            .pending: [.in_progress, .cancelled, .skipped],
            .in_progress: [.completed, .failed, .cancelled],
            .completed: [],
            .failed: [],
            .cancelled: [],
            .skipped: []
        ]

        guard let allowed = validTransitions[from], allowed.contains(to) else {
            throw TaskQueueError.invalidStatusTransition(from: from, to: to)
        }
    }
}
