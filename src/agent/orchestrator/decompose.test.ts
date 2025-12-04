import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import type { Task } from "../../tasks/index.js";
import {
  analyzeTask,
  decomposeTask,
  generateSubtaskId,
  readSubtasks,
  writeSubtasks,
  updateSubtaskStatus,
  createSubtaskList,
  getPendingSubtasks,
  getNextSubtask,
  isAllSubtasksComplete,
  hasFailedSubtasks,
} from "./decompose.js";

const createMockTask = (overrides: Partial<Task> = {}): Task => ({
  id: "oa-test01",
  title: "Test task",
  description: "A simple test task",
  status: "open",
  priority: 1,
  type: "task",
  labels: [],
  deps: [],
  commits: [],
  comments: [],
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  closedAt: null,
  ...overrides,
});

describe("generateSubtaskId", () => {
  test("generates correct format", () => {
    expect(generateSubtaskId("oa-abc123", 0)).toBe("oa-abc123-sub-001");
    expect(generateSubtaskId("oa-abc123", 9)).toBe("oa-abc123-sub-010");
    expect(generateSubtaskId("oa-abc123", 99)).toBe("oa-abc123-sub-100");
  });
});

describe("analyzeTask", () => {
  test("detects simple task", () => {
    const task = createMockTask({ title: "Fix typo", description: "Fix typo in readme" });
    const heuristics = analyzeTask(task);
    
    expect(heuristics.hasMultipleTargets).toBe(false);
    expect(heuristics.hasMultipleActions).toBe(false);
    expect(heuristics.isComplex).toBe(false);
  });

  test("detects multiple actions", () => {
    const task = createMockTask({
      title: "Refactor auth",
      description: "Add new auth flow, update existing handlers, remove deprecated code, test changes",
    });
    const heuristics = analyzeTask(task);
    
    expect(heuristics.hasMultipleActions).toBe(true);
  });

  test("detects testing requirement", () => {
    const task = createMockTask({
      title: "Add feature with tests",
      description: "Implement feature and add unit tests",
    });
    const heuristics = analyzeTask(task);
    
    expect(heuristics.requiresTesting).toBe(true);
  });

  test("detects documentation requirement", () => {
    const task = createMockTask({
      title: "Add new API",
      description: "Implement API and update documentation",
    });
    const heuristics = analyzeTask(task);
    
    expect(heuristics.requiresDocs).toBe(true);
  });

  test("detects complex task by length", () => {
    const longDescription = "A".repeat(600);
    const task = createMockTask({ description: longDescription });
    const heuristics = analyzeTask(task);
    
    expect(heuristics.isComplex).toBe(true);
  });
});

describe("decomposeTask", () => {
  test("creates single subtask for simple task", () => {
    const task = createMockTask({ title: "Fix typo", description: "Fix typo in readme" });
    const subtasks = decomposeTask(task);
    
    expect(subtasks).toHaveLength(1);
    expect(subtasks[0].id).toBe("oa-test01-sub-001");
    expect(subtasks[0].status).toBe("pending");
  });

  test("creates multiple subtasks for complex task", () => {
    const task = createMockTask({
      title: "Add new feature",
      description: "Add new component, update existing files, create tests, and document the changes",
    });
    const subtasks = decomposeTask(task);
    
    expect(subtasks.length).toBeGreaterThan(1);
  });

  test("respects forceSingle option", () => {
    const task = createMockTask({
      title: "Complex task",
      description: "Add, update, remove, test, document everything",
    });
    const subtasks = decomposeTask(task, { forceSingle: true });
    
    expect(subtasks).toHaveLength(1);
  });

  test("respects maxSubtasks option", () => {
    const task = createMockTask({
      title: "Very complex task",
      description: "Add component, update service, create module, test function, document class",
    });
    const subtasks = decomposeTask(task, { maxSubtasks: 2 });
    
    expect(subtasks.length).toBeLessThanOrEqual(2);
  });
});

describe("subtask file operations", () => {
  let tempDir: string;
  let openagentsDir: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "decompose-test-"));
    openagentsDir = path.join(tempDir, ".openagents");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  test("writeSubtasks creates directory and file", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    
    writeSubtasks(openagentsDir, subtaskList);
    
    const subtasksDir = path.join(openagentsDir, "subtasks");
    expect(fs.existsSync(subtasksDir)).toBe(true);
    expect(fs.existsSync(path.join(subtasksDir, "oa-test01.json"))).toBe(true);
  });

  test("readSubtasks returns null for missing file", () => {
    const result = readSubtasks(openagentsDir, "nonexistent");
    expect(result).toBeNull();
  });

  test("readSubtasks returns subtask list", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    writeSubtasks(openagentsDir, subtaskList);
    
    const result = readSubtasks(openagentsDir, task.id);
    expect(result).not.toBeNull();
    expect(result?.taskId).toBe(task.id);
    expect(result?.subtasks.length).toBeGreaterThan(0);
  });

  test("updateSubtaskStatus updates status correctly", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    writeSubtasks(openagentsDir, subtaskList);
    
    const subtaskId = subtaskList.subtasks[0].id;
    const result = updateSubtaskStatus(openagentsDir, task.id, subtaskId, "in_progress");
    
    expect(result).not.toBeNull();
    expect(result?.subtasks[0].status).toBe("in_progress");
    expect(result?.subtasks[0].startedAt).toBeDefined();
  });

  test("updateSubtaskStatus sets completedAt for done status", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    writeSubtasks(openagentsDir, subtaskList);
    
    const subtaskId = subtaskList.subtasks[0].id;
    const result = updateSubtaskStatus(openagentsDir, task.id, subtaskId, "done");
    
    expect(result?.subtasks[0].completedAt).toBeDefined();
  });
});

describe("subtask list helpers", () => {
  test("getPendingSubtasks returns only pending", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    subtaskList.subtasks.push({
      id: "oa-test01-sub-002",
      description: "Second subtask",
      status: "done",
    });
    
    const pending = getPendingSubtasks(subtaskList);
    expect(pending.every(s => s.status === "pending")).toBe(true);
  });

  test("getNextSubtask returns in_progress first", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    subtaskList.subtasks[0].status = "in_progress";
    subtaskList.subtasks.push({
      id: "oa-test01-sub-002",
      description: "Second subtask",
      status: "pending",
    });
    
    const next = getNextSubtask(subtaskList);
    expect(next?.status).toBe("in_progress");
  });

  test("getNextSubtask returns pending when no in_progress", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    
    const next = getNextSubtask(subtaskList);
    expect(next?.status).toBe("pending");
  });

  test("isAllSubtasksComplete returns true when all done/verified", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    subtaskList.subtasks[0].status = "done";
    
    expect(isAllSubtasksComplete(subtaskList)).toBe(true);
  });

  test("isAllSubtasksComplete returns false with pending", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    
    expect(isAllSubtasksComplete(subtaskList)).toBe(false);
  });

  test("hasFailedSubtasks detects failures", () => {
    const task = createMockTask();
    const subtaskList = createSubtaskList(task);
    subtaskList.subtasks[0].status = "failed";
    
    expect(hasFailedSubtasks(subtaskList)).toBe(true);
  });
});
