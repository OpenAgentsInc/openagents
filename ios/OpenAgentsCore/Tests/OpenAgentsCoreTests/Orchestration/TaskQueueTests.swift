import XCTest
@testable import OpenAgentsCore

final class TaskQueueTests: XCTestCase {

    // Helper to create a temporary DB for each test
    private func createTestQueue() async throws -> (TaskQueue, URL) {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("taskqueue_\(UUID().uuidString).sqlite")
        let db = try TinyvexDbLayer(path: tmp.path)
        let queue = try await TaskQueue(db: db)
        return (queue, tmp)
    }

    private func createTestTask(opHash: String = UUID().uuidString) -> OvernightTask {
        let decision = DecisionOutput(
            task: "Test task",
            agentMode: .claude_code,
            priority: .medium,
            estimatedDuration: 1800,
            rationale: "Test rationale",
            confidence: 0.8
        )

        return OvernightTask(
            opHash: opHash,
            decision: decision
        )
    }

    // MARK: - Enqueue/Dequeue Tests

    func testEnqueueAndGet() async throws {
        // Given: Empty queue
        let (queue, _) = try await createTestQueue()
        let task = createTestTask()

        // When: Enqueue a task
        let taskId = try await queue.enqueue(task)

        // Then: Should be retrievable by ID
        let retrieved = try await queue.get(taskId)
        XCTAssertNotNil(retrieved)
        XCTAssertEqual(retrieved?.id, taskId)
        XCTAssertEqual(retrieved?.status, .pending)
        XCTAssertEqual(retrieved?.decision.task, "Test task")
    }

    func testDequeue_returnsOldestPending() async throws {
        // Given: Multiple tasks in queue
        let (queue, _) = try await createTestQueue()

        let task1 = createTestTask()
        let task2 = createTestTask()
        let task3 = createTestTask()

        _ = try await queue.enqueue(task1)
        Thread.sleep(forTimeInterval: 0.01)  // Ensure different timestamps
        _ = try await queue.enqueue(task2)
        Thread.sleep(forTimeInterval: 0.01)
        _ = try await queue.enqueue(task3)

        // When: Dequeue
        let dequeued = try await queue.dequeue()

        // Then: Should get the first task (FIFO)
        XCTAssertNotNil(dequeued)
        XCTAssertEqual(dequeued?.id, task1.id)
    }

    func testDequeue_skipsinProgressTasks() async throws {
        // Given: Queue with pending and in-progress tasks
        let (queue, _) = try await createTestQueue()

        let task1 = createTestTask()
        var task2 = createTestTask()
        task2.status = .in_progress

        _ = try await queue.enqueue(task1)
        _ = try await queue.enqueue(task2)

        // When: Dequeue
        let dequeued = try await queue.dequeue()

        // Then: Should only get pending task
        XCTAssertEqual(dequeued?.id, task1.id)
    }

    func testDequeue_emptyQueue() async throws {
        // Given: Empty queue
        let (queue, _) = try await createTestQueue()

        // When: Dequeue
        let dequeued = try await queue.dequeue()

        // Then: Should return nil
        XCTAssertNil(dequeued)
    }

    // MARK: - Duplication Tests

    func testEnqueue_duplicateOpHash_throws() async throws {
        // Given: Task with specific opHash
        let (queue, _) = try await createTestQueue()
        let task1 = createTestTask(opHash: "duplicate-hash")
        let task2 = createTestTask(opHash: "duplicate-hash")

        // When: Enqueue first task
        _ = try await queue.enqueue(task1)

        // Then: Enqueuing duplicate should throw
        await XCTAssertThrowsErrorAsync(
            try await queue.enqueue(task2),
            "Should throw on duplicate opHash"
        ) { error in
            if case TaskQueueError.duplicateTask(let hash) = error {
                XCTAssertEqual(hash, "duplicate-hash")
            } else {
                XCTFail("Expected TaskQueueError.duplicateTask")
            }
        }
    }

    func testFindByOpHash() async throws {
        // Given: Task with known opHash
        let (queue, _) = try await createTestQueue()
        let task = createTestTask(opHash: "find-me")
        _ = try await queue.enqueue(task)

        // When: Find by opHash
        let found = try await queue.findByOpHash("find-me")

        // Then: Should find the task
        XCTAssertNotNil(found)
        XCTAssertEqual(found?.opHash, "find-me")
    }

    // MARK: - Status Transition Tests

    func testUpdateStatus_pendingToInProgress() async throws {
        // Given: Pending task
        let (queue, _) = try await createTestQueue()
        let task = createTestTask()
        let taskId = try await queue.enqueue(task)

        // When: Update to in_progress
        try await queue.updateStatus(taskId, status: .in_progress)

        // Then: Status should be updated and startedAt set
        let updated = try await queue.get(taskId)
        XCTAssertEqual(updated?.status, .in_progress)
        XCTAssertNotNil(updated?.startedAt, "startedAt should be set")
        XCTAssertNil(updated?.completedAt, "completedAt should not be set yet")
    }

    func testUpdateStatus_inProgressToCompleted() async throws {
        // Given: In-progress task
        let (queue, _) = try await createTestQueue()
        var task = createTestTask()
        task.status = .in_progress
        task.startedAt = Date()
        let taskId = try await queue.enqueue(task)

        // When: Update to completed
        try await queue.updateStatus(taskId, status: .completed)

        // Then: Status should be updated and completedAt set
        let updated = try await queue.get(taskId)
        XCTAssertEqual(updated?.status, .completed)
        XCTAssertNotNil(updated?.completedAt, "completedAt should be set")
    }

    func testUpdateStatus_invalidTransition_throws() async throws {
        // Given: Completed task
        let (queue, _) = try await createTestQueue()
        var task = createTestTask()
        task.status = .completed
        task.completedAt = Date()
        let taskId = try await queue.enqueue(task)

        // When/Then: Updating completed task should throw
        await XCTAssertThrowsErrorAsync(
            try await queue.updateStatus(taskId, status: .in_progress),
            "Should throw on invalid transition"
        ) { error in
            if case TaskQueueError.invalidStatusTransition(let from, let to) = error {
                XCTAssertEqual(from, .completed)
                XCTAssertEqual(to, .in_progress)
            } else {
                XCTFail("Expected TaskQueueError.invalidStatusTransition")
            }
        }
    }

    func testUpdate_fullTask() async throws {
        // Given: Task in queue
        let (queue, _) = try await createTestQueue()
        let task = createTestTask()
        let taskId = try await queue.enqueue(task)

        // When: Update multiple fields
        var updated = task
        updated.status = .in_progress
        updated.sessionId = "session123"
        updated.startedAt = Date()
        updated.metadata = ["pr_number": "42"]

        try await queue.update(updated)

        // Then: All fields should be updated
        let retrieved = try await queue.get(taskId)
        XCTAssertEqual(retrieved?.status, .in_progress)
        XCTAssertEqual(retrieved?.sessionId, "session123")
        XCTAssertNotNil(retrieved?.startedAt)
        XCTAssertEqual(retrieved?.metadata["pr_number"], "42")
    }

    // MARK: - Query Tests

    func testAll_noFilter() async throws {
        // Given: Tasks with various statuses
        let (queue, _) = try await createTestQueue()

        var task1 = createTestTask()
        var task2 = createTestTask()
        task2.status = .completed

        _ = try await queue.enqueue(task1)
        _ = try await queue.enqueue(task2)

        // When: Get all tasks
        let all = try await queue.all()

        // Then: Should get all tasks
        XCTAssertEqual(all.count, 2)
    }

    func testAll_statusFilter() async throws {
        // Given: Tasks with mixed statuses
        let (queue, _) = try await createTestQueue()

        var task1 = createTestTask()
        var task2 = createTestTask()
        task2.status = .completed
        var task3 = createTestTask()
        task3.status = .pending

        _ = try await queue.enqueue(task1)
        _ = try await queue.enqueue(task2)
        _ = try await queue.enqueue(task3)

        // When: Get only pending tasks
        let pending = try await queue.all(status: .pending)

        // Then: Should get 2 pending tasks
        XCTAssertEqual(pending.count, 2)
        XCTAssertTrue(pending.allSatisfy { $0.status == .pending })
    }

    // MARK: - Persistence Tests

    func testPersistence_survivesDatabaseReopen() async throws {
        // Given: Task in database
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("persist_\(UUID().uuidString).sqlite")
        let db1 = try TinyvexDbLayer(path: tmp.path)
        let queue1 = try await TaskQueue(db: db1)

        let task = createTestTask()
        let taskId = try await queue1.enqueue(task)

        // When: Close and reopen database
        let db2 = try TinyvexDbLayer(path: tmp.path)
        let queue2 = try await TaskQueue(db: db2)

        // Then: Task should still be there
        let retrieved = try await queue2.get(taskId)
        XCTAssertNotNil(retrieved)
        XCTAssertEqual(retrieved?.id, taskId)
    }

    // MARK: - Delete Tests

    func testDelete() async throws {
        // Given: Task in queue
        let (queue, _) = try await createTestQueue()
        let task = createTestTask()
        let taskId = try await queue.enqueue(task)

        // When: Delete the task
        try await queue.delete(taskId)

        // Then: Task should be gone
        let retrieved = try await queue.get(taskId)
        XCTAssertNil(retrieved)
    }

    func testDeleteAll() async throws {
        // Given: Multiple tasks
        let (queue, _) = try await createTestQueue()
        _ = try await queue.enqueue(createTestTask())
        _ = try await queue.enqueue(createTestTask())
        _ = try await queue.enqueue(createTestTask())

        // When: Delete all
        try await queue.deleteAll()

        // Then: Queue should be empty
        let all = try await queue.all()
        XCTAssertEqual(all.count, 0)
    }

    // MARK: - Error Handling Tests

    func testGet_nonexistentTask() async throws {
        // Given: Empty queue
        let (queue, _) = try await createTestQueue()

        // When: Get non-existent task
        let task = try await queue.get("nonexistent-id")

        // Then: Should return nil
        XCTAssertNil(task)
    }

    func testUpdateStatus_nonexistentTask_throws() async throws {
        // Given: Empty queue
        let (queue, _) = try await createTestQueue()

        // When/Then: Updating non-existent task should throw
        await XCTAssertThrowsErrorAsync(
            try await queue.updateStatus("nonexistent", status: .completed),
            "Should throw for non-existent task"
        ) { error in
            if case TaskQueueError.taskNotFound(let id) = error {
                XCTAssertEqual(id, "nonexistent")
            } else {
                XCTFail("Expected TaskQueueError.taskNotFound")
            }
        }
    }
}

// Helper for async error assertions
extension XCTestCase {
    func XCTAssertThrowsErrorAsync<T>(
        _ expression: @autoclosure () async throws -> T,
        _ message: String = "",
        file: StaticString = #filePath,
        line: UInt = #line,
        _ errorHandler: (Error) -> Void = { _ in }
    ) async {
        do {
            _ = try await expression()
            XCTFail(message, file: file, line: line)
        } catch {
            errorHandler(error)
        }
    }
}
